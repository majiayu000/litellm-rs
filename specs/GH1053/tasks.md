# Task Plan

## Linked Issue

GH-1053 / #1053

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1053-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: coordinator. Dependencies: none. Done when: GH1053 product/tech/tasks packet 齐全，B-001 至 B-006 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1053/product.md && test -f specs/GH1053/tech.md && test -f specs/GH1053/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1053/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1053/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1053-T2` Covers: B-001, B-003, B-004. Owner: implementation owner. Dependencies: SP1053-T1. Done when: `list_legacy_um_teams` uses all-or-error row deserialization, removes warning/skip, returns a typed redacted error with `um_teams.data` context, and never returns a converted prefix. Verify: focused corrupt-row repository test; `rg -n "Skipping invalid legacy um_teams row" src/storage/database/seaorm_db/team_repository/legacy_sync.rs` has no hit.
- [ ] `SP1053-T3` Covers: B-002, B-003, B-005, B-006. Owner: implementation owner. Dependencies: SP1053-T2. Done when: list/count/get_by_name/get_user_teams propagate corrupt-row failure through existing `?`, mixed valid+corrupt rows cannot return partial values, and valid bridge/member/name-conflict behavior is unchanged. Verify: focused propagation/valid-peer tests and the complete `team_repository_tests` module.
- [ ] `SP1053-T4` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: verification owner. Dependencies: SP1053-T3. Done when: diff is limited to GH1053 spec, legacy enumeration, and focused repository tests; format, all-target/all-feature check, strict Clippy and full serial tests pass on final head. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → failing corrupt-row reproduction → strict all-or-error enumeration → propagation assertions → valid regression tests → repository-wide verification。生产修改集中在单个 legacy sync helper，由单一 implementation owner 串行执行。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-006。
- Impl PR 使用 `Fixes #1053` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- red reproduction 已证明 current `origin/main` 返回 `Ok(([], 0))`；不要把 warning 文案测试当作行为验证。
- error 需要 `um_teams.data` context，但不得拼接 raw corrupt payload。
- 不夹带 member conversion、name-conflict、migration、repair 或其他 JSON entity 修改。
