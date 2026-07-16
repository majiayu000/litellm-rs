# Task Plan

## Linked Issue

GH-1056 / #1056

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1056-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: coordinator. Dependencies: none. Done when: GH1056 product/tech/tasks packet 齐全，B-001 至 B-006 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1056/product.md && test -f specs/GH1056/tech.md && test -f specs/GH1056/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1056/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1056/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1056-T2` Covers: B-001, B-002, B-005, B-006. Owner: implementation owner. Dependencies: SP1056-T1. Done when: SeaORM canonical team update captures `ExecResult`, returns `NotFound` on zero rows before legacy sync, preserves database errors, and does not upsert. Verify: focused missing-team test plus canonical/legacy absence assertions.
- [ ] `SP1056-T3` Covers: B-003, B-004. Owner: implementation owner. Dependencies: SP1056-T2. Done when: exactly-one-row update still touches/persists team and runs existing legacy synchronization/preservation logic unchanged. Verify: complete `team_repository_tests` module.
- [ ] `SP1056-T4` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: verification owner. Dependencies: SP1056-T3. Done when: diff is limited to GH1056 spec, canonical update implementation and focused repository tests; format, all-target/all-feature check, strict Clippy and full serial tests pass on final head. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → failing missing-update reproduction → affected-row guard → no-side-effect assertions → successful update regressions → repository-wide verification。实现文件直接相关，由单一 implementation owner 串行修改。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-006。
- Impl PR 使用 `Fixes #1056` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- red reproduction 已证明 current `origin/main` 对 never-created UUID 返回 `Ok(Team)`。
- `rows_affected == 0` check 必须发生在 legacy sync 之前；禁止 SELECT-before-update 或 upsert。
- 不夹带 legacy update、optimistic locking、name conflict、delete/member 修改。
