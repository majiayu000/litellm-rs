# Task Plan

## Linked Issue

GH-1044 / #1044

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1044-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH1044 product/tech/tasks packet 齐全，B-001 至 B-007 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1044/product.md && test -f specs/GH1044/tech.md && test -f specs/GH1044/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1044/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1044/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1044-T2` Covers: B-001, B-002, B-006, B-007. Owner: implementation owner. Dependencies: SP1044-T1. Done when: SeaORM API-key entity/domain conversions are fallible, strict for every non-null JSON field, preserve genuine NULL optionals, and emit redacted field-context errors without fallback JSON. Verify: focused entity conversion tests plus `rg -n 'unwrap_or_default|unwrap_or_else|\.ok\(\)' src/storage/database/entities/api_key.rs` yields no JSON conversion fallback.
- [ ] `SP1044-T3` Covers: B-003, B-004. Owner: implementation owner. Dependencies: SP1044-T2. Done when: create/hash/id lookup and user/team/global list propagate conversion errors; malformed rate limits cannot produce an authenticated domain key or partial list. Verify: in-memory DB tests inject corrupt rate-limit row and assert all lookup/list results are `Err`.
- [ ] `SP1044-T4` Covers: B-005, B-007. Owner: implementation owner. Dependencies: SP1044-T2. Done when: usage update validates before mutation/UPDATE, corrupt row returns error, transaction does not overwrite the raw value, and error omits sentinel/key identity. Verify: DB test reads raw usage column before/after failed update and asserts exact equality.
- [ ] `SP1044-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: verification owner. Dependencies: SP1044-T3, SP1044-T4. Done when: diff is limited to GH1044 spec, API-key conversion/operations and focused tests; format, all-feature check, strict Clippy and full serial tests pass with fresh output. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → fallible conversion → DB propagation → corruption/rollback tests → focused verification → repository-wide verification。转换与 consumer 存在直接依赖，由单一 implementation owner 串行修改。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-007。
- Impl PR 使用 `Fixes #1044` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- `rate_limits=NULL` 是合法缺失；`rate_limits='broken'` 必须是错误，不能合并语义。
- error 中不得包含 raw field value、key hash、prefix 或 secret。
- 不顺手增加 migration、repair 或改变 valid policy precedence。
