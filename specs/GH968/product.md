# Product Spec

## Linked Issue

GH-968 / #968

complexity: large

## 用户问题

Gateway 允许通过 provider 配置、环境变量和部分请求级 override 改写上游 endpoint。当前配置期 URL/DNS
校验与实际出站连接使用的 HTTP client 并不一致：多数 provider 仍使用普通 resolver，部分原生 provider
和 SDK-compatible route 还直接创建 `reqwest::Client`。攻击者可让域名在配置期解析到公网地址、在请求期
重绑定到内网或 metadata 地址，或者通过 redirect/proxy 绕过原始校验。

## 目标

- 所有 Gateway 运行时可配置的 provider endpoint 默认采用 public-only 出站策略。
- 初始 URL、每次实际 DNS resolution 和每个 redirect hop 都执行同一安全策略。
- 为可信部署提供按 provider、显式、受限的 private-network opt-in，同时永久保护 metadata 等特殊地址。
- 普通请求、流式请求、health check 和直接消费 provider 配置的旁路 API 使用一致策略。
- 用确定性测试证明 DNS rebinding 被实际连接层阻断，而不依赖公网 DNS 或时间窗口。

## 非目标

- 不处理 webhook、MCP/A2A、数据库、Redis、S3、Qdrant、observability 或 pricing 的 URL。
- 不处理请求 payload 中的媒体 URL；这些 URL 由各 provider 远端获取，不由 Gateway 建立 socket。
- 不重构 `DefaultRouter`/`UnifiedRouter`；该工作由 #965 跟踪。
- 不改变 Rust SDK 作为本地调用方时的 `SdkProviderConfig.base_url`；本 issue 保护 Gateway 服务端出站边界。
- 不为请求 body、tenant header 或 `CompletionOptions` 提供 private-network opt-in。
- 不宣称 HTTP proxy 可以保留目标域名的本地 DNS 安全保证。

## Behavior Invariants

1. B-001 当 provider endpoint 可由 Gateway 配置、provider 环境变量或请求级 API base 覆盖时，默认访问
   策略必须是 `public_only`；字段缺失、空值或未知值不得隐式启用私网访问。
2. B-002 `public_only` 必须在建立 socket 前拒绝 localhost、loopback、RFC1918、CGNAT、IPv6 ULA、
   link-local、metadata、unspecified、multicast、benchmark、documentation 和其他 reserved literal 地址；
   配置失败不得表现为 provider 可用。
3. B-003 当 hostname 在配置期解析为公网地址、请求期重新解析为 B-002 地址时，每次新连接都必须失败，
   且不得回退到普通 resolver/client、缓存的旧地址或 warning-only 降级。
4. B-004 `public_only` 可跟随的每个 redirect hop 都必须重新校验目标 URL 并在连接时重新校验解析结果；
   任一 hop 指向私网、metadata 或 reserved 地址时，目标 socket 不得收到请求。
5. B-005 `private_network` 只能由对应 provider 的启动配置、provider 环境配置或受信任程序化 builder
   显式启用；授权不得由请求/tenant 输入获得，不得泄漏给其他 provider，也不得因 client cache 复用而扩大。
6. B-006 `private_network` 只额外允许该 provider 已配置 authority 的 loopback、RFC1918 和 IPv6 ULA
   地址；metadata、link-local、unspecified、multicast、CGNAT、benchmark、documentation 和其他 reserved
   地址在该模式下仍永久拒绝，且该模式不自动跟随 redirect。
7. B-007 同一 provider 的普通、流式、重试、health-check 及 batches/images/moderations/fine-tuning/rerank/
   Gemini SDK-compatible 等直接 route 必须使用同一 endpoint policy；任何单一路径不得保留普通 client 旁路。
8. B-008 `public_only` 出站不得使用系统代理；若 provider 的显式 proxy 会令目标 DNS 在代理端解析，配置
   必须 fail closed，而不是声称仍受本地 SSRF guard 保护。
9. B-009 URL 解析、DNS resolution、安全 client 构建或 policy 传播失败必须返回可定位的配置/网络错误；
   禁止静默改用普通 client、旧全局 client 或无策略 fallback。
10. B-010 源码架构检查必须阻断受保护 provider/runtime route 在统一 client 模块之外重新引入
    `Client::new`、`ClientBuilder::new` 或普通 client factory；测试代码与明确不在本 issue 范围的模块需使用
    精确 allowlist，不能用数量 baseline 掩盖新增旁路。
11. B-011 固定官方公网 endpoint 的现有配置继续工作；既有 localhost/self-hosted provider 在升级后必须
    显式选择 `private_network`，缺少该选择时应在启动阶段给出明确错误而不是运行时偶发失败。
12. B-012 DNS rebinding、literal metadata 和 redirect 负例必须证明目标 listener 未建立连接；仅断言 helper
    返回错误、依赖真实 DNS 或复用配置期解析结果均不足以作为完成证据。

## 验收标准

- [ ] `public_only` 和 `private_network` 的配置、默认值、环境变量与非法组合有回归测试。
- [ ] 可注入 resolver 测试覆盖 public-at-validation 到 private/metadata-at-connect，并证明未建立 socket。
- [ ] 初始 literal、普通请求、流式请求、redirect 和 health-check 均受统一策略保护。
- [ ] Gateway 当前所有可运行 provider 及直接 route 无普通 client 旁路，源码架构检查为零命中。
- [ ] private opt-in 按 provider/authority 隔离，允许 loopback/RFC1918/ULA 但仍拒绝 metadata/link-local/reserved。
- [ ] 全量格式、编译、strict Clippy、feature matrix、测试、scope/overlap 和 PR gate 通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-001, B-009；缺失策略为 public-only，空/未知值失败。 |
| 错误与失败路径 | covered: B-002, B-003, B-004, B-008, B-009；所有安全前提失败均 fail closed。 |
| 授权/权限 | covered: B-005, B-006；私网授权按 provider/authority 隔离且来源为闭集。 |
| 并发/竞态 | covered: B-003, B-005, B-012；连接时重新解析，cache 不得跨授权，listener 证明无竞态漏连。 |
| 重试/幂等 | covered: B-003, B-007；每次新连接/重试重新应用策略，重复失败不得降级。 |
| 非法状态转换 | covered: B-001, B-005, B-006；请求输入不能把 public-only 转为 private-network。 |
| 兼容/迁移 | covered: B-011；公网兼容，自托管配置需要显式迁移。 |
| 降级/回退 | covered: B-003, B-009；普通 client fallback 被禁止。 |
| 证据与审计完整性 | covered: B-010, B-012；架构 guard 和无 socket 红绿测试缺一即未完成。 |
| 取消/中断 | covered: B-003, B-009；取消后的重试仍重新走 policy，不复用部分安全结果。 |

## 发布说明

可配置 provider endpoint 现在默认只允许公网目标并在连接/redirect 时持续校验。localhost 或私网自托管
endpoint 需要在对应 provider 上显式配置 `endpoint_access: private_network`；metadata、link-local 和其他
reserved 地址始终不可访问。
