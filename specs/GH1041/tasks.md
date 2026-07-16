# Task Plan

## Linked Issue

GH-1041 / #1041

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1041-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH1041 product/tech/tasks packet 存在，behavior invariants 连续且 product-to-test/task coverage 完整，spec 文档检查通过. Verify: `test -f specs/GH1041/product.md && test -f specs/GH1041/tech.md && test -f specs/GH1041/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1041/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1041/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1041-T2` Covers: B-001, B-002, B-003. Owner: implementation owner. Dependencies: SP1041-T1. Done when: protocol boundary DTOs emit/accept canonical camelCase, typed initialize result requires protocol version/capabilities/server info, and only `2024-11-05` is accepted. Verify: focused `core::mcp::protocol` and initialize parser tests, including missing/wrong-type/version-negative cases.
- [ ] `SP1041-T3` Covers: B-004, B-005. Owner: implementation owner. Dependencies: SP1041-T2. Done when: HTTP/SSE request and notification paths share status/error behavior while notification has no ID, accepts empty 2xx body, and rejects non-2xx/network/auth/rate-limit failures explicitly. Verify: loopback HTTP tests inspect serialized messages and response handling.
- [ ] `SP1041-T4` Covers: B-004, B-006, B-007. Owner: implementation owner. Dependencies: SP1041-T2, SP1041-T3. Done when: connect commits capabilities and Connected only after valid initialize result plus successful initialized notification; every failure leaves Failed/None. Verify: focused lifecycle success and notification-rejection tests assert message order, state, and capabilities.
- [ ] `SP1041-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: verification owner. Dependencies: SP1041-T4. Done when: diff is limited to GH1041 spec and MCP protocol/server implementation/tests, formatting/build/lint/full tests pass with fresh output, and no production SSRF rule is weakened. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → protocol DTO/parser → notification transport → lifecycle state commit → focused tests → repository-wide verification。步骤共享 `protocol.rs` 与 `server.rs`，由单一 implementation owner 串行完成，避免并行写入冲突。

## 验证

- Product invariant set 与 tasks `Covers:` union 均为 B-001 至 B-007，无 orphan。
- Impl PR 使用 `Fixes #1041`，并以 Spec branch 为 base，使实现与 Spec 审查历史分离。
- 远端 PR 只报告 current-head CI 事实；不自动合并。

## Handoff Notes

- 仓库当前只声明 `2024-11-05`，不得顺手升级到最新 MCP 版本。
- loopback client 只能存在于 test code；production `McpServer::new` 继续委托 SSRF-safe client。
- JSON-RPC response ID validation 与 concurrent connect serialization 留给独立证据和独立 issue。
