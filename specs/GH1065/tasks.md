# Task Plan

## Linked Issue
#1065（parent #1064）

## Spec Packet
- `specs/GH1065/product.md`（B-001 ~ B-004）
- `specs/GH1065/tech.md`（设计方案、HD-1/HD-2、tranche、测试、回滚）

## 前置门（implementation 前必须完成）
- [ ] `SP1065-T000` Covers: HD-1, HD-2. Owner: maintainer 决策者. Dependencies: 本 spec packet 合并. Done when: HD-1（首 tranche public-only、不放行内网 opt-in）与 HD-2（pre-flight 校验 vs 连接期 pin IP，与 #968 provider 路径保持一致）均记录为 resolved；在此之前 T001 blocked. Verify: 人工核对 spec 无 unresolved 决策；`python3 <specrail>/checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH1065"`.

## 实现任务
- [ ] `SP1065-T001` Covers: B-001, B-002, B-003, B-004. Owner: webhook-ssrf owner. Dependencies: SP1065-T000 resolved. Done when: `register_webhook` 在现有 scheme 检查处改调 `crate::core::net::ssrf_guard::validate_outbound_url_str`，私网/保留/非 http(s)/空 URL fail-closed 拒绝注册并返回既有 webhook error（不入注册表）；webhook 投递发送点（`delivery.rs`）在 `client.post` 前对目标 URL 调 `validate_provider_endpoint_url`（`ProviderEndpointPolicy::public_only()`），失败则该次投递标记 failed + error 级日志 + 进入既有失败统计，不静默降级（U-29）；若 HD-2 选 pin-IP 则把已校验 IP 经 reqwest `resolve()` override 注入连接；`WebhookManager.client` 改用 `build_outbound_client`/`default_outbound_client`，删除裸 `Client::new()`；webhooks 目录不新增任何 SSRF/IP 判定逻辑（只调用 `ssrf_guard`），无新依赖；PR 限本 scope、≤6 非文档文件 / ≤500 changed lines，使用 `Refs #1065`. Verify: 注册负例 fixture 覆盖 `169.254.169.254`/`127.0.0.1`/`10.0.0.1`/`http://[::1]`/`file://`/空 全部拒绝；投递负例用 `validate_provider_endpoint_url_with_resolver` 注入 rebinding（公网域名→私网 IP）证明投递被拒且标记 failed；正例证明合法公网注册+投递不回归、统计正确；`rg -n "Client::new" src/core/webhooks` 零命中；`rg -n "is_private|169\.254|RFC1918|loopback" src/core/webhooks` 仅注释/无自写判定；`cargo test --all-features --locked core::webhooks`；`cargo fmt --all -- --check`；`cargo clippy --all-targets --all-features -- -D warnings`；`bash scripts/guards/check_pr_scope.sh`；`bash scripts/guards/check_pr_overlap.sh`；SEC-11 触及安全出站，需人工安全 review；exact-head independent review 与 required PR gate.

## Product-to-Test Mapping
| 需求 | 验证 |
|------|------|
| B-001 注册期校验 | 注册负例 fixture（6 类非法 URL 全拒） |
| B-002 投递期防 rebinding | 注入 resolver 的 rebinding 负例 |
| B-003 fail-closed + 可观测 | 断言拒绝路径 error 级日志 + failed 统计 |
| B-004 复用不重复 | `rg` guard：无裸 Client、无自写 IP 判定、无新依赖 |

## Merge Gate（每个 PR）
format / check / strict clippy / `cargo test` / scope guard / overlap guard / SEC-11 人工安全 review / exact-head independent review。
