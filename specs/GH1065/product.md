# Product Spec

## Linked Issue
#1065 — security(webhooks): 出站投递复用 SSRF-safe client + 连接期 IP 校验
Parent roadmap: #1064

## 问题陈述（事实，`docs/gh1065-webhook-ssrf-spec@50999dd4` 现证）
仓库存在多条可由公开类型构造的 webhook 出站路径。它们使用裸
`reqwest::Client`、`create_custom_client` 或 `default_outbound_client`，没有把
public-only 目标策略绑定到每次实际连接：

- `core::webhooks::WebhookManager`：通用 webhook 队列投递；
- `core::budget::BudgetAlertManager`：预算告警 webhook；
- `monitoring::alerts::SlackChannel`：Slack webhook；
- `core::observability::LogAggregator` 的 `LogDestination::Webhook`。

仅在注册或发送前解析域名无法关闭 DNS rebinding / TOCTOU：校验后，实际连接、
重试或重定向仍可能解析到回环、RFC1918、链路本地、云元数据或其他保留地址。
这些地址一旦被进程代为请求即构成 SSRF（CWE-918）。

## Sender Inventory 与运行时分类
| Sender | 构造入口 / 发包点 | 当前网关运行时分类 |
|----|----|----|
| 通用 webhook | `WebhookManager::{new,register_webhook}` / `delivery.rs` | public 可构造；`subsystem_registry` 明确为 `TemporaryExemption`，未由 gateway runtime 构造 |
| 预算告警 webhook | `BudgetAlertManager::{new,with_config,add_webhook}` / `alerts.rs` | public 可构造且有 budget 初始化 helper；不得据此声称 webhook 已接入 AppState 生产路径 |
| Slack webhook | `AlertManager::new` → `SlackChannel::new` / `channels.rs` | public 可构造；`MonitoringSystem::new` 当前固定 `alerts = None` |
| 日志 webhook | `LogAggregator::add_destination(LogDestination::Webhook)` / `logging.rs` | public 可构造；core observability 在 `subsystem_registry` 为 `TemporaryExemption` |

`AlertChannel::{Slack,Discord,Teams,Webhook}` 当前只是声明，没有对应网络发送实现，
不算第五条 sender；后续一旦接线，必须在接线 PR 中纳入同一策略。安全范围按
“可构造发送路径”确定，而不是只按当前生产可达性确定。

## 产品需求
| ID | 需求 |
|----|------|
| B-001 | 四条可构造 sender 的 URL admission 仅接受规范的 `http` / `https` URL；空值、非法 URL、非 HTTP(S)、私网/保留地址字面量 fail-closed |
| B-002 | 四条 sender 的每次实际新连接都由既有 connection-time DNS/IP policy 执行 **public-only** 校验，关闭 DNS rebinding / TOCTOU；禁止用 pre-flight-only 校验替代 |
| B-003 | webhook client 必须禁用环境/系统代理；重定向必须禁用；应用层重试的每次发送与底层新连接都不得脱离同一策略绑定 client |
| B-004 | 解析失败、策略拒绝、任意非 2xx（包括 redirect 3xx）或 client 构造失败均显式返回/记录 error，并进入该 sender 既有失败语义；错误与日志不得泄露 webhook URL 的 userinfo 或 query；禁止 generic outbound client fallback 或静默降级 |
| B-005 | 复用 `ProviderHttpClient`（或实现时已有、能力完全相同的 connection-time policy-bound client）与 `ssrf_guard`；不复制 IP 分类、不新增依赖 |
| B-006 | 通过显式串行 tranche 覆盖全部四条 sender；每个 tranche 保留原 sender 的 payload、header、签名、超时和成功状态语义，并记录必要的兼容性变化；Slack 与 LogAggregator 固定保留当前 120 秒 request timeout |

## 已决策安全默认
- **HD-1 resolved：**首轮全部 public-only，不提供内网 opt-in / allowlist。
- **HD-2 resolved：**必须连接期执行策略；pre-flight 只能作为更早的错误提示，不能作为安全边界。
- **HD-SCOPE resolved：**四条可构造 sender 全部纳入，按 T001→T004 串行交付；不得因当前未由 gateway runtime 构造而遗漏。

## 验收（product-to-behavior）
- 完整非法 URL fixture（空、malformed、非 HTTP(S)、回环、RFC1918、链路本地/
  metadata、IPv6 loopback/ULA/link-local、IPv4-mapped IPv6、NAT64 metadata/
  reserved encoding）在 admission 或发送前 fail-closed。
- 可注入 sequence resolver 证明“预校验公网、连接期改绑私网”被拒；tripwire
  listener 明确证明目标 socket **未 accept**。
- 公网源返回私网/保留地址重定向时不跟随，目标 listener 未 accept；普通
  webhook 也不依赖 redirect 成功；LogAggregator 等 sender 必须把 source 的
  3xx 本身计为非成功并进入 error path。
- sender 自带重试时，每次 attempt 仍经同一 policy-bound client；强制新连接后
  的 rebind 仍被拒且 listener 未 accept。
- 合法公网直连保持 payload、headers、签名、超时与成功/失败统计语义。
- Slack 与 LogAggregator 的合法直连回归明确断言 120 秒 request timeout；URL
  admission/error/log capture 断言 userinfo 与 query secret 均不可见。
- `rg` 证明四条路径不再使用裸/generic client 发 webhook，也没有自写 IP 分类。

## Non-Goals
- 不提供内网 webhook、目标 allowlist 或配置开关；这些只能由后续安全评审 tranche 引入。
- 不改变 webhook 事件模型、payload schema、签名算法或预算/监控/日志业务规则。
- 不处理 provider `base_url`（#968 已覆盖）或非 webhook 的任意 integration endpoint。
- 不以本 issue 顺带把未接线 sender 接入 gateway runtime。
