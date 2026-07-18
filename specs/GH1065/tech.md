# Tech Spec

## Linked Issue
#1065（parent #1064）

## Product Spec
见 `specs/GH1065/product.md`（B-001 ~ B-006）。

## Codebase Context（事实，`docs/gh1065-webhook-ssrf-spec@50999dd4` 现证）
### 可构造发送路径
| Sender | 当前 client | URL admission | 发送/重试 |
|----|----|----|----|
| `core/webhooks` | `create_custom_client`，构造失败 fallback 到 `default_outbound_client` | 仅空值 + 字符串前缀 `http(s)` | `delivery.rs` 的 `.post()`；队列按配置重试 |
| `core/budget/alerts.rs` | `create_custom_client`，构造失败 fallback 到 `default_outbound_client` | `add_webhook` 无校验 | `send_single_webhook` 的 `.post()`；显式循环重试 |
| `monitoring/alerts/channels.rs` | 每次发送 clone `default_outbound_client` | `SlackChannel::new` 无校验 | `SlackChannel::send` 单次 `.post()` |
| `core/observability/logging.rs` | 每次发送 clone `default_outbound_client` | `add_destination` 无校验 | `LogDestination::Webhook` 分支单次 `.post()` |

运行时分类以 product 的 Sender Inventory 为准。特别是，`WebhookManager` 与
core observability 有明确 temporary exemption，monitoring alert manager 当前固定不构造；
budget subsystem 虽已 wired，也没有证据证明 `BudgetAlertManager` webhook 进入 AppState。
本 spec 不把“public 可构造”写成“当前生产可达”。

### 可复用的连接期安全基础设施
- `src/utils/net/http.rs::ProviderHttpClient` 把 `ProviderEndpointPolicy` 绑定在
  request builder 与底层 reqwest client：
  - `ProviderHttpClient::new(policy, timeout)` / `no_redirect(policy, timeout)`；
  - request boundary 调 `policy.validate_url_without_resolution`；
  - `PolicyDnsResolver` 在实际 DNS/连接阶段按 access 校验每个地址；
  - builder `.no_proxy()`；
  - public policy 的 redirect policy 会校验 redirect URL；本 issue 使用
    `no_redirect` 进一步固定 webhook 兼容边界。
- `src/core/net/ssrf_guard.rs` 提供 `ProviderEndpointPolicy::public_only()`、
  URL 静态校验与唯一 IP 分类 SSOT。
- `src/utils/net/http/provider_tests.rs` 已有 sequence resolver、tripwire connector
  与 `assert_listener_did_not_accept` 的确定性测试模式，可复用/提取，禁止另造弱化版。

## 已解决决策
| ID | 决策 | 结果 |
|----|------|------|
| HD-1 | 内网 webhook opt-in | **Resolved：**T001-T004 全部 `public_only()`；本 issue 不引入 opt-in |
| HD-2 | pre-flight 还是连接期 | **Resolved：**`ProviderHttpClient` connection-time enforcement 是安全边界；pre-flight-only 明确禁止 |
| HD-SCOPE | 只修通用 manager 还是全部 sender | **Resolved：**覆盖四条可构造发送路径，按 T001→T004 串行交付 |

## 共同设计约束
1. admission 使用 `ssrf_guard` 解析规范 URL，仅允许 `http` / `https`；不得用
   `starts_with` 或 sender 私有的 IP/range 判断。
2. 每个 sender 持有或在受控构造阶段获得
   `ProviderHttpClient::no_redirect(ProviderEndpointPolicy::public_only(), timeout)`。
   若实现时存在名称不同但能力完全相同的共享 client，必须以代码证据证明：
   request-bound URL 校验、connection-time DNS/IP 校验、`.no_proxy()`、redirect none。
3. 禁止 `default_outbound_client`、`create_custom_client` 或裸
   `reqwest::Client` 作为 webhook fallback。安全 client 构造失败必须让构造/注册/
   发送显式失败。
4. 重试只可复用 policy-bound client。每次应用层 attempt 都重新经过 request
   boundary；每个实际新连接都重新经过 policy DNS resolver。连接池复用不允许通过
   替换 client 或 generic-client fallback 绕过策略。
5. redirect 全部禁用。3xx 按 sender 的非成功响应处理，绝不自动请求 `Location`。
6. 策略错误必须保留可分类原因并映射到既有 sender error/失败统计；日志不得泄露
   webhook secret、带 userinfo URL 或敏感 query。

## 串行 Tranche Plan
| Tranche | Sender / 文件预算 | Done when |
|----|----|----|
| SP1065-T001 | 通用 `core/webhooks`；≤6 非文档文件 / ≤500 changed lines | 注册期规范 URL 校验；manager 只持有 public-only/no-redirect policy client；队列重试、签名、统计保持；删除构造 fallback |
| SP1065-T002 | `core/budget/alerts.rs`；≤4 非文档文件 / ≤400 changed lines | `add_webhook` admission 可报告错误；预算 webhook 重试全走 policy client；payload/header/timeout 语义保持；删除 fallback |
| SP1065-T003 | monitoring `SlackChannel`；≤4 非文档文件 / ≤350 changed lines | 构造/admission 可报告错误；send 只走 policy client；Slack payload 语义保持 |
| SP1065-T004 | observability `LogAggregator` webhook；≤4 非文档文件 / ≤350 changed lines | destination admission 可报告错误；flush webhook 只走 policy client；entries/header 语义保持 |

T001 合并并同步 main 后才开始 T002，依次类推。每个 tranche 独立 `Refs #1065`；
只有 T004 在前序全部落地后可使用 closing keyword。

## 兼容性与 scope 记录
- public-only 会拒绝此前可指向内网/本机的 webhook，这是刻意的安全行为变化；
  本轮没有兼容开关。
- 禁用 redirect 会把此前可能成功的 3xx 链变为失败；调用方需直接配置最终公网 URL。
- 目前三个 admission API 中有 infallible 入口（budget add、Slack constructor、
  LogAggregator builder）。实现 tranche 必须选择并记录可审计的 fallible API 迁移；
  不得用“记录 warning 后仍保存”、丢弃配置或 generic fallback 保持表面兼容。
- timeout 必须在 policy client 构造阶段绑定；同一 manager 中每目标 timeout 不一致时，
  可按 `(public_only, timeout, no_redirect)` 使用既有 cache，但不可降级成 request-level
  generic client。
- tranche 只改该 sender 与必要共享测试设施；不得顺带接线 runtime、改事件 schema、
  新增依赖或扩展到非 webhook endpoint。

## 确定性测试设计
### 完整 URL fixture
每个 admission surface 使用共享 table（或同一 SSOT 的等价 table）覆盖：
- `""`、空白、malformed、`file://`、`ftp://`；
- `http://127.0.0.1`、`http://10.0.0.1`、`http://169.254.169.254`；
- `http://[::1]`、IPv6 ULA、IPv6 link-local；
- IPv4-mapped IPv6 metadata、NAT64 metadata/reserved 表示；
- 合法 `https://example.com/hook` 正例。

不要用外网 DNS/网络作为测试 oracle。

### 连接期 rebind
复用 sequence resolver：第一次（admission/pre-flight）返回公网地址，第二次实际连接
返回绑定到本机 tripwire port 的受限地址。断言：
1. send 返回 endpoint-policy error；
2. resolver 查询序列证明连接期答案被消费并校验；
3. `assert_listener_did_not_accept` 证明 socket 未建立；
4. 该 sender 的失败状态/统计按既有语义更新。

矩阵至少包含 loopback、RFC1918、link-local metadata、IPv6 loopback/ULA，以及仓库
guard 已支持的 mapped/NAT64 metadata 表示。

### redirect 与 retry/new connection
- 公网 source fixture 返回 302，`Location` 指向 tripwire private listener；断言 3xx
  不被跟随、target listener 未 accept。
- 对有应用层重试的通用/budget sender，sequence resolver 在后续 attempt 或强制
  `Connection: close` 后返回受限地址；断言后续新连接仍拒绝、listener 未 accept，
  且不会换用 generic client。
- 合法公网直连 fixture 断言 payload、headers、签名、timeout、状态/统计不回归。

## Product-to-Test Mapping
| 需求 | 验证 |
|----|----|
| B-001 | 四个 admission surface 的完整 URL fixture |
| B-002 | 每 tranche 的 sequence-resolver rebind + listener-not-accepted |
| B-003 | no-proxy 构造证据；redirect target listener；retry/forced-new-connection rebind |
| B-004 | client 构造/策略错误测试 + 既有失败状态/统计；无 fallback `rg` |
| B-005 | `rg` 证明只复用 policy client/guard、无 sender 私有 IP 分类、依赖不变 |
| B-006 | 四 tranche 合并证据 + 合法直连兼容回归测试 |

## Verification
每个 tranche 至少执行：

```bash
cargo fmt --all -- --check
cargo check --all-features --locked
cargo test --all-features --locked <sender-specific-filter>
cargo clippy --all-targets --all-features -- -D warnings
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

并执行针对该 tranche 的 `rg` guard，证明发包点不出现
`default_outbound_client`、`create_custom_client`、裸 `reqwest::Client` 或自写
private/reserved 分类。T004 后对四条路径执行全量 guard。

## 回滚方案
每 tranche 独立 revert，且不得回退到 unsafe generic client。若安全 client 无法构造，
保持 sender fail-closed；需要恢复内网 webhook 或 redirect 的场景必须另开带安全设计与
人工 review 的 issue，不能作为紧急 fallback。
