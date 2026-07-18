# Task Plan

## Linked Issue
#1065（parent #1064）

## Spec Packet
- `specs/GH1065/product.md`（B-001 ~ B-006）
- `specs/GH1065/tech.md`（sender inventory、已决策安全默认、T001-T004、测试与回滚）

## 决策门（已完成）
- [x] `SP1065-T000` Covers: HD-1, HD-2, HD-SCOPE. Owner: spec owner. Dependencies: code inventory at `50999dd4`. Done when: 记录 public-only、connection-time enforcement、全部四条可构造 sender 串行覆盖；明确 pre-flight-only、proxy、redirect 与 generic fallback 均不可作为安全方案；不误称未接线 sender 当前生产可达. Verify: 对照 product Sender Inventory 与 tech 已解决决策表。

## 串行实现任务
- [ ] `SP1065-T001` Covers: B-001, B-002, B-003, B-004, B-005, B-006（通用 webhook slice）. Owner: core-webhooks SSRF owner. Dependencies: SP1065-T000. Done when: `WebhookManager::register_webhook` 复用 guard 做规范 `http`/`https` admission；manager 只持有 `ProviderHttpClient::no_redirect(public_only, timeout)`（或能力完全相同的既有 policy-bound client）；删除 `create_custom_client` / `default_outbound_client` fallback；队列每次 attempt 与实际新连接保持策略绑定；策略拒绝进入既有 retry/failed/error 语义；payload、headers、签名、timeout、成功统计保持；scope ≤6 非文档文件 / ≤500 changed lines，PR `Refs #1065`. Verify: 完整 URL fixture（含 hostname-only 保留名）；rebind + forced-new-connection + redirect tripwire 均 listener-not-accepted；userinfo/query error/log capture 脱敏负例；合法直连回归；sender `rg` guard 不得替代运行时脱敏测试；`cargo test --all-features --locked core::webhooks`；tech Verification 全套；SEC-11 人工安全 review；exact-head independent review 与 PR gate。

- [ ] `SP1065-T002` Covers: B-001, B-002, B-003, B-004, B-005, B-006（预算 webhook slice）. Owner: budget webhook SSRF owner. Dependencies: SP1065-T001 merged and branch rebased on current main. Done when: `BudgetAlertManager::add_webhook` 采用明确 fallible admission migration；manager/client cache 仅使用 public-only/no-redirect policy client；显式重试每次 attempt 与新连接均不可绕过策略；删除 generic fallback；原 payload、custom headers、per-webhook timeout、成功/失败日志语义保持；scope ≤4 非文档文件 / ≤400 changed lines，PR `Refs #1065`. Verify: 完整 URL fixture（含 hostname-only 保留名）；第二次 attempt/强制新连接 rebind + redirect tripwire listener-not-accepted；userinfo/query error/log capture 脱敏负例；合法直连/headers/timeout/retry 回归；sender `rg` guard 不得替代运行时脱敏测试；`cargo test --all-features --locked core::budget`；tech Verification 全套；SEC-11 人工安全 review；exact-head independent review 与 PR gate。

- [ ] `SP1065-T003` Covers: B-001, B-002, B-003, B-004, B-005, B-006（Slack webhook slice）. Owner: monitoring webhook SSRF owner. Dependencies: SP1065-T002 merged and branch rebased on current main. Done when: `SlackChannel` construction/admission 采用明确 fallible migration；`send` 只使用 public-only/no-redirect policy client，无 `default_outbound_client`；策略错误显式返回；Slack payload/channel/username/severity 与当前 120 秒 request timeout 语义保持；不声称或顺带实现 MonitoringSystem runtime wiring；scope ≤4 非文档文件 / ≤350 changed lines，PR `Refs #1065`. Verify: 完整 URL fixture（含 hostname-only 保留名）；rebind + redirect tripwire listener-not-accepted；合法直连 payload 与 120 秒 timeout 回归；sender `rg` guard；`cargo test --all-features --locked monitoring::alerts`；tech Verification 全套；SEC-11 人工安全 review；exact-head independent review 与 PR gate。

- [ ] `SP1065-T004` Covers: B-001, B-002, B-003, B-004, B-005, B-006（日志 webhook final slice）. Owner: observability webhook SSRF owner. Dependencies: SP1065-T003 merged and branch rebased on current main. Done when: `LogAggregator::add_destination(LogDestination::Webhook)` 采用明确 fallible admission migration；flush 的 webhook branch 只使用 public-only/no-redirect policy client，无 `default_outbound_client`；策略错误与任意非 2xx（包括 3xx）走现有显式 error 路径；entries、custom headers 与当前 120 秒 request timeout 语义保持；不声称或顺带实现 observability runtime wiring；scope ≤4 非文档文件 / ≤350 changed lines，final PR closing #1065. Verify: 完整 URL fixture（含 hostname-only 保留名）；rebind + redirect tripwire listener-not-accepted，并断言 source 3xx 使 flush 返回/记录 error；userinfo/query error/log capture 脱敏负例；合法直连 entries/headers 与 120 秒 timeout 回归；四 sender 全量 `rg` guard不得替代运行时脱敏测试；`cargo test --all-features --locked core::observability`；tech Verification 全套；SEC-11 人工安全 review；exact-head independent review 与 PR gate。

## Product-to-Task/Test Mapping
| 需求 | Tasks | 必备证据 |
|----|----|----|
| B-001 URL admission | T001-T004 | 每个公开 admission surface 的完整 URL fixture，包含 IP literal 与 hostname-only 保留名 |
| B-002 连接期 public-only | T001-T004 | sequence resolver + rebind + target listener 未 accept |
| B-003 proxy/redirect/retry/new connection | T001-T004（重试重点 T001/T002） | `.no_proxy()`/no-redirect 共享 client 证据；redirect 与 forced-new-connection tripwire |
| B-004 fail-closed / no fallback | T001-T004 | client 构造/策略错误与非 2xx 进入既有失败语义；LogAggregator 3xx error；URL userinfo/query capture 脱敏；sender `rg` guard |
| B-005 单一 SSOT | T001-T004 | 只引用 `ProviderHttpClient`/`ssrf_guard`；无自写 IP 分类、无新依赖 |
| B-006 全 sender + 兼容 | T001-T004 串行 | 四个 merge/PR gate 证据；合法公网 payload/header/signature/timeout/status 回归；T003/T004 固定 120 秒 timeout |

## Merge Gate（每个 PR）
fresh format / check / strict clippy / sender tests / scope guard / overlap guard /
reviewThreads 全部 resolved / SEC-11 人工安全 review / exact-head independent review /
required PR gate。T001-T003 只 `Refs #1065`；只有完成全量 closure audit 的 T004 可关闭 issue。
