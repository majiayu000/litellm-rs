# Task Plan

## Linked Issue

GH-1059 / #1059

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1059-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH1059 product/tech/tasks packet 齐全，B-001 至 B-007 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1059/product.md && test -f specs/GH1059/tech.md && test -f specs/GH1059/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1059/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1059/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1059-T2` Covers: B-001, B-002, B-003, B-006, B-007. Owner: implementation owner. Dependencies: SP1059-T1. Done when: SeaORM delete captures canonical/legacy ExecResults, explicitly rolls back and returns NotFound when both are zero, preserves rollback errors, and skips post-commit user cleanup. Verify: focused missing-team/orphan-member/legacy-user regression.
- [ ] `SP1059-T3` Covers: B-004, B-005. Owner: implementation owner. Dependencies: SP1059-T2. Done when: either canonical or legacy affected row commits deletion, and bridged/canonical/legacy-only membership cleanup behavior remains. Verify: complete `team_repository_tests` module.
- [ ] `SP1059-T4` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: verification owner. Dependencies: SP1059-T3. Done when: diff is limited to GH1059 spec, canonical delete implementation and focused repository tests; format, all-target/all-feature check, strict Clippy and full serial tests pass on final head. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → failing missing-delete reproduction → dual affected-row guard + rollback → orphan/user side-effect assertions → supported delete regressions → repository-wide verification。transaction 代码直接相关，由单一 implementation owner 串行修改。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-007。
- Impl PR 使用 `Fixes #1059` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- existence 是 canonical/legacy delete affected-row 的 OR；禁止只检查 canonical。
- both zero 必须 rollback member delete，且不得进入 post-commit user cleanup。
- 不夹带 foreign-key migration、orphan repair、soft delete 或其他 repository operation 修改。
