# Tech Spec

## Linked Issue
#1065（parent #1064）

## Product Spec
见 `specs/GH1065/product.md`（B-001 ~ B-004）。

## Codebase Context（事实，`main@375bcd85` 现证）
- `src/core/webhooks/manager.rs`
  - `WebhookManager { client: reqwest::Client }`（`:22-24`）— 裸 client
  - `register_webhook`（`:58-79`）只校验非空 + `http(s)` scheme（`:62-68`）
  - `send_event` 入队（`:91-146`）；`start_delivery_processor` → `process_delivery_queue`（`:168-183`）
  - 已 import `crate::core::http::outbound::default_outbound_client`（`:9`）与 `crate::utils::net::http::create_custom_client`（`:12`）
- 实际发包点：`src/core/webhooks/delivery.rs`（投递处理）
- **可复用**的既有 SSRF 基础设施（#968 成果）：
  - `src/core/net/ssrf_guard.rs`
    - `validate_outbound_url_str(raw) -> Result<Url, SsrfError>`（静态，无解析，`:162`）
    - `validate_provider_endpoint_url(url, access) -> Result<(), SsrfError>`（解析 + IP 校验，`:201`）
    - `validate_provider_endpoint_url_with_resolver(url, access, resolver)`（可注入 resolver，`:217`，测试友好）
    - `ProviderEndpointPolicy::public_only()`（`:92`）/ `ProviderEndpointAccess`（`:12`）
    - `is_private_or_reserved_ip(ip)`（`:405`）/ `is_private_or_reserved_host`（`:327`）
  - `src/core/http/outbound.rs`：`default_outbound_client()`（`:14`）、`build_outbound_client(profile)`（`:57`）、`OutboundProfile`

## Duplicate / Overlap Boundary
- **禁止**在 webhooks 下新写任何 SSRF/IP 判定逻辑——只调用 `ssrf_guard.rs`（避免第二 SSOT）。
- 与 #968 边界：#968 = provider `base_url`；本 spec = webhook 投递。共享同一 `ssrf_guard`，不改 #968 的 provider 路径。
- 不新增依赖（复用既有 `url`/`reqwest`/`ssrf_guard`）。

## Human Decision Gate
| ID | 决策 | 推荐默认 | 状态 |
|----|------|----------|------|
| HD-1 | 是否允许自托管场景把 webhook 指向内网（opt-in allowlist / `ProviderEndpointAccess::for_base_url` 式放行）？ | **首个 tranche 不放行，硬编码 public-only**；内网 opt-in 留后续 tranche（需配置模型 + 安全 review） | 待 maintainer 确认 |
| HD-2 | 投递期防 rebinding 采用「pre-flight 解析校验」还是「连接期 pin 已校验 IP」？ | 与 provider 路径（#968）保持一致；若 provider 已 pin IP 则同样 pin，否则至少 pre-flight 解析校验 + 记录残余 TOCTOU 风险 | 待确认（依赖 #968 实现细节） |

> 实现前必须 resolve HD-1/HD-2；未 resolve 则 implementation blocked。

## 设计方案
### 1. 注册期静态校验（B-001）
`register_webhook` 在现有 scheme 检查处改为调用 `validate_outbound_url_str(&config.url)`：
- 成功返回归一化 `Url` 并继续注册
- `SsrfError` → 返回注册错误（复用现有 webhook error 类型），不入注册表

### 2. 投递期解析后校验（B-002/B-003）
投递发包前（`delivery.rs` 发送点），对目标 URL 调用
`validate_provider_endpoint_url(&url, ProviderEndpointAccess::from(ProviderEndpointPolicy::public_only()))`：
- 校验通过才 `client.post(...)`；失败则该次投递标记 failed、error 级日志、进入既有失败统计
- 若 HD-2 选择 pin IP：把 `validate_*` 返回/解析到的 IP 通过 reqwest `resolve()` override 注入，连接到已校验 IP，彻底闭合 rebinding

### 3. Client 收敛（B-004）
`WebhookManager` 的 `client` 改用 `build_outbound_client(OutboundProfile::…)` / `default_outbound_client()`，
与其余出站路径统一（超时/连接池/代理策略一致）。不再裸 `Client::new()`。

## Tranche Plan and File Budgets
| Tranche | Scope | 文件预算 |
|---------|-------|----------|
| SP1065-T001 | 注册期 + 投递期 SSRF 校验 + client 收敛（单一 focused 改动） | ≤6 非文档文件 / ≤500 changed lines，`Refs #1065` |

> 单 tranche 即可闭合；若 HD-2 选 pin-IP 使 diff 超预算，拆 T002 承接连接期 resolver。

## 数据流
```
register_webhook(url)
  └─ validate_outbound_url_str(url)  ── Err ─▶ 拒绝注册(error)
        └─ Ok ─▶ 入注册表
process_delivery_queue → deliver(url)
  └─ validate_provider_endpoint_url(url, public_only)  ── Err ─▶ 投递 failed(error, 统计)
        └─ Ok ─▶ [可选 pin IP] client.post(url) 发送
```

## 备选方案
- **A（推荐）** 复用 `ssrf_guard` pre-flight 解析校验 + 与 provider 一致的 pin 策略：一致、低风险、无新依赖。
- **B** 自建 webhook 专用 SSRF 逻辑：违反 anti-duplication（第二 SSOT），拒绝。
- **C** 仅注册期静态校验、不做投递期：无法防 DNS rebinding，达不到 B-002，拒绝。

## 风险
- **DNS rebinding 残余 TOCTOU**（若 HD-2 选 pre-flight 而非 pin IP）：pre-flight 解析与 reqwest 实际连接可能各自解析。缓解：pin 已校验 IP 到连接（HD-2）。
- 合法内网 webhook 被拒（自托管场景）：由 HD-1 显式决策；首 tranche public-only 是安全默认。
- 行为回归：合法公网投递路径不得受影响 —— 由测试保证。

## 测试计划
- 注册负例：`169.254.169.254` / `127.0.0.1` / `10.0.0.1` / `http://[::1]` / `file://x` / 空 → 全部 fail-closed
- 投递负例：用可注入 resolver（`validate_provider_endpoint_url_with_resolver`）模拟公网域名解析到私网 IP（rebinding）→ 投递被拒
- 正例：公网目标注册 + 投递成功，统计正确
- guard：`rg -n "reqwest::Client::new|Client::new\(\)" src/core/webhooks` 零命中（不得绕过 outbound 工厂）
- `rg` 证明 webhooks 目录无自写 IP/私网判定（只调 `ssrf_guard`）

## 回滚方案
单 tranche 改动，`git revert` 即回退到裸 client 行为；无 schema / 配置 / 依赖变更，回滚无副作用。
