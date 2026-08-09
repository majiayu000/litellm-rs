# Tech Spec

## Linked Issue

Private advisory: `GHSA-96qr-wjxq-r4xj`

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Compatibility entry | `src/config/validation/ssrf.rs:16` | 公开函数自行解析 URL、限制 scheme、分类 host 并解析 DNS | 必须保留入口契约，同时消除独立安全策略 |
| Duplicate classifier | `src/config/validation/ssrf.rs:66`, `src/config/validation/ssrf.rs:149` | 旧分类缺少 IPv4 multicast/benchmark 与多类 IPv6 special-purpose 范围 | 已复现策略分叉的根因 |
| DNS path | `src/config/validation/ssrf.rs:117` | 兼容入口直接调用系统 resolver，解析失败时 fail closed | 委托后应保留 fail-closed 语义 |
| Canonical entry | `src/core/net/ssrf_guard.rs:200` | `validate_provider_endpoint_url` 对 URL 和 access policy 执行统一校验 | 兼容入口应固定委托 `PublicOnly` |
| Canonical classifier | `src/core/net/ssrf_guard.rs:344`, `src/core/net/ssrf_guard.rs:404` | 统一 guard 维护 provider endpoint 的允许/拒绝范围 | 唯一安全策略来源 |
| Legacy tests | `src/config/validation/ssrf.rs:193`, `src/config/validation/ssrf.rs:615` | 多个成功用例解析真实公网域名 | 全量测试曾因临时 DNS 失败出现单例失败 |
| Cross-module compatibility tests | `src/config/validation/tests.rs:563` | 另一组公开入口测试也用真实公网域名作为成功输入 | 首次实现全量测试新鲜暴露四个同根因失败 |
| Compatibility contract | `src/config/validation/ssrf.rs` | 入口仅接受 HTTP/HTTPS、失败保持 fail closed，并委托统一 guard 的 `PublicOnly` policy | 删除历史 packet 后仍需自包含保留该契约 |

## 根因

GH968 引入并逐步接线了统一 provider endpoint policy，但 `config::validation::ssrf` 的兼容入口没有随实现
收敛，仍复制 hostname blocklist、IP 范围判断和 DNS 流程。旧 IPv4 判断只覆盖 private、link-local、
documentation、CGNAT 和 `>=240/4`，未覆盖 multicast 与 benchmark；旧 IPv6 判断也未覆盖 multicast、
documentation 和更多 IETF special-purpose 范围。因此同一个 URL 经两个公开路径会得到不同安全结论。

测试将“URL 路径/query 是否保持可接受”与“公网域名此刻能否解析”耦合，导致语法无关的成功用例在
全量并发测试中受外部 DNS 状态影响。

## 设计方案

### 1. Compatibility adapter

- `validate_url_against_ssrf` 继续先用 `Url::parse` 生成带 `context` 的格式错误。
- 显式保留仅允许 `http`/`https` 的兼容契约；统一 guard 还支持 `ws`/`wss`，因此不能直接扩大入口。
- 对通过 scheme 检查的 URL 调用
  `core::net::validate_provider_endpoint_url(&url, ProviderEndpointAccess::PublicOnly)`。
- 将 `SsrfError` 映射为包含 `context` 的 `String`；不吞掉、不降级、不重试。
- 删除本文件的 hostname/IP blocklist、编码 IP 特判、直接 DNS 逻辑和
  `is_private_or_internal_ip`，使统一 guard 成为唯一分类来源。

### 2. Deterministic regression tests

- 将 `src/config/validation/ssrf.rs` 与 `src/config/validation/tests.rs` 中所有合法公网域名成功用例替换为
  公网 literal，同时保留原用例要验证的 HTTP/HTTPS、端口、path、query 形态。
- 增加表驱动负例覆盖已复现分叉：IPv4 multicast `224.0.0.1`、benchmark `198.18.0.1`、IPv6 multicast
  `ff02::1`、documentation `2001:db8::1`。
- 删除仅测试已移除私有 helper 的测试；对应范围由兼容入口的行为测试和统一 guard 既有分类测试覆盖。
- 错误断言验证 `context` 与安全失败语义，不保留依赖旧内部文案的断言。

## 影响文件

| Path | Change |
| --- | --- |
| `src/config/validation/ssrf.rs` | 将兼容入口改为统一 guard adapter；删除重复分类；调整并新增确定性测试 |
| `src/config/validation/tests.rs` | 将 compatibility 成功用例改为确定性公网 literal |

不修改 `src/core/net/ssrf_guard.rs`：根因是兼容入口未消费现有规范，而非规范分类本身缺失。

## Product-to-Test Mapping

| Behavior invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | compatibility adapter + duplicate classifier deletion | `rg -n "is_private_or_internal_ip|ToSocketAddrs" src/config/validation/ssrf.rs` 零命中；focused tests 通过 |
| B-002 | canonical `PublicOnly` delegation + divergence table | focused test `test_canonical_special_purpose_ranges_blocked` 覆盖四个已复现地址 |
| B-003 | parse/scheme adapter + canonical DNS error propagation | 既有 invalid URL/scheme/localhost/metadata tests；`cargo test --all-features --locked config::validation::ssrf::tests` |
| B-004 | error mapping | `test_context_in_error_message` 与 `test_context_in_scheme_error` |
| B-005 | unchanged public signature + literal positive matrix | compile/check + port/path/query/public literal tests |
| B-006 | 两个测试模块的 literal-only 成功用例 + repeated focused run | Cargo 编译当前 lib test binary 一次；对两个 `config::validation` filter 各直接运行该 binary 50 次 |

## 风险与缓解

- 风险：统一 guard 支持 `ws`/`wss`，直接委托可能意外扩大兼容 API。缓解：adapter 在委托前保留
  HTTP/HTTPS 白名单并保留 scheme 负例。
- 风险：错误内部文案变化影响依赖字符串的调用方。缓解：公开返回类型与 `context` 保持，安全错误继续包含
  `SSRF protection`；不承诺旧实现细节文案。
- 风险：真实域名正例减少 DNS 集成覆盖。缓解：DNS fail-closed 和 resolver 行为由统一 guard 的确定性
  resolver tests 覆盖；本文件只验证 compatibility adapter。

## 验证计划

```sh
cargo fmt --all -- --check
cargo test --all-features --locked config::validation::ssrf::tests
cargo test --all-features --locked config::validation::tests::test_ssrf_validation
# TEST_BIN 使用上一条 cargo test 输出的当前 lib test executable 路径。
for i in {1..50}; do
  "$TEST_BIN" config::validation::ssrf::tests --quiet &&
  "$TEST_BIN" config::validation::tests::test_ssrf_validation --quiet || exit 1
done
cargo check --all-targets --all-features --locked
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
cargo audit
```

## 回滚

若统一 adapter 产生未预期兼容问题，回滚实现提交即可恢复原函数；Spec 与回归用例保留用于重新设计。
不得通过放宽统一分类、忽略 DNS 错误或恢复缺失范围的旧 classifier 来回滚。
