# Task Plan

## Linked Issue

GH-1047 / #1047

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1047-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH1047 product/tech/tasks packet 齐全，B-001 至 B-007 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1047/product.md && test -f specs/GH1047/tech.md && test -f specs/GH1047/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1047/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1047/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1047-T2` Covers: B-001, B-002, B-003, B-006. Owner: implementation owner. Dependencies: SP1047-T1. Done when: canonical user entity conversion is fallible, strictly parses role/status, maps every declared valid enum including Deleted, and returns redacted field-context errors without default fallbacks. Verify: focused entity/SQLite tests; `rg -n 'unwrap_or\(UserRole::User\)|_ => UserStatus::Pending' src/storage/database/entities/user.rs` returns no fallback hit.
- [ ] `SP1047-T3` Covers: B-004, B-006, B-007. Owner: implementation owner. Dependencies: SP1047-T2. Done when: canonical ID/username/email lookups propagate conversion failures, invalid present rows cannot trigger legacy fallback, and genuine missing rows remain `Ok(None)`. Verify: in-memory SQLite tests exercise corrupt role/status through all three lookup paths and missing lookup regression.
- [ ] `SP1047-T4` Covers: B-005, B-007. Owner: implementation owner. Dependencies: SP1047-T3. Done when: JWT authentication propagates lookup/storage errors through `Result` while preserving the existing results for missing and inactive users. Verify: focused auth tests plus diff inspection of `authenticate_jwt` three-way match.
- [ ] `SP1047-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: verification owner. Dependencies: SP1047-T3, SP1047-T4. Done when: diff is limited to GH1047 spec, user conversion/query/auth code and focused tests; format, all-target/all-feature check, strict Clippy and full serial tests pass with fresh output. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → failing corruption/Deleted reproduction tests → fallible entity conversion → query propagation → JWT error propagation → focused verification → repository-wide verification。转换、query 和 auth 存在直接依赖，由单一 implementation owner 串行修改。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-007。
- Impl PR 使用 `Fixes #1047` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- `deleted` 是合法 persisted status，不是 corruption；必须精确恢复为 `UserStatus::Deleted`。
- 错误不得包含 raw invalid value、username、email、password hash 或 token。
- canonical present-but-invalid row 必须阻止 legacy fallback；只有真实 `None` 才允许既有 fallback。
- 不顺手增加 migration、repair、role 语义变化或 legacy JSON 重构。
