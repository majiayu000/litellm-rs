# Product Spec

## Linked Issue
#1065 — security(webhooks): 出站投递复用 SSRF-safe client + 连接期 IP 校验
Parent roadmap: #1064

## 问题陈述（事实，`main@375bcd85` 现证）
Webhook 出站投递使用裸 `reqwest::Client`（`src/core/webhooks/manager.rs:13,24`）。
注册时只校验 URL 非空与 `http(s)` scheme（`manager.rs:62-68`），投递时无任何 SSRF 防护。
因此运营方注册的 webhook URL 可指向：

- 云元数据端点（`169.254.169.254`）
- 回环 / RFC1918 内网（`127.0.0.1`、`10.x`、`192.168.x`）
- IPv6 ULA / 保留段
- 通过 **DNS rebinding**（注册时解析为公网、投递时解析为内网）绕过纯静态校验

这些地址在投递时被网关代为请求，构成 SSRF（CWE-918）。

审计（`docs/audit/2026-07-09-design-issues-with-context.md` CR-8）明确记录：该 finding **未被任何 open issue 追踪**；
#968 仅覆盖 provider `base_url` 的运行时 SSRF-safe client，**不含 webhook 投递路径**。

## 产品需求
| ID | 需求 |
|----|------|
| B-001 | webhook 注册时对 URL 做静态 SSRF 校验（scheme + 私网/保留段字面量），拒绝非法目标 |
| B-002 | webhook **投递时**按 DNS 解析后的实际 IP 做 SSRF 校验，防 DNS rebinding / TOCTOU |
| B-003 | 校验默认 **public-only、fail-closed**：解析失败或命中私网/保留段一律拒绝并 error 级可观测（U-29），不静默降级 |
| B-004 | **复用**仓库既有 SSRF guard 与 outbound client，不新增第二套 SSRF 逻辑、不新增依赖 |

## 验收（product-to-behavior）
- 注册指向 `169.254.169.254` / `127.0.0.1` / `10.0.0.1` / `file://` / 空 → 注册被拒，返回明确错误
- 注册指向公网域名、投递时该域名解析为私网 IP（rebinding）→ 投递被拒、error 日志、投递标记 failed
- 合法公网 webhook → 投递正常，行为不回归
- `rg` 证明 webhook 路径不再直接构造裸 `Client` 绕过 guard

## Non-Goals
- 不引入 webhook 目标 allowlist / 自托管内网 opt-in（见 tech §Human Decision Gate，留待后续 tranche）
- 不改 webhook 事件模型、重试策略、签名机制
- 不处理 provider `base_url`（已由 #968 覆盖）
