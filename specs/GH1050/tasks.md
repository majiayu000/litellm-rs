# Task Plan

## Linked Issue

GH-1050 / #1050

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1050-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH1050 product/tech/tasks packet 齐全，B-001 至 B-007 连续且 product-to-test/task coverage 完整. Verify: `test -f specs/GH1050/product.md && test -f specs/GH1050/tech.md && test -f specs/GH1050/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1050/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1050/tasks.md | sort -u`; `git diff --check origin/main...HEAD`.
- [ ] `SP1050-T2` Covers: B-001, B-002, B-004. Owner: implementation owner. Dependencies: SP1050-T1. Done when: BatchStatus exposes canonical snake_case encoding, serde uses it, and parser accepts exact canonical plus historical Debug closed sets. Verify: table-driven domain tests assert eight exact strings, JSON round-trip and eight historical aliases.
- [ ] `SP1050-T3` Covers: B-003, B-006. Owner: implementation owner. Dependencies: SP1050-T2. Done when: create/update DB paths and both processor callsites pass typed status, persist only canonical text, preserve transaction/not-found and status timestamp mapping, and contain no Debug formatting. Verify: focused SQLite update/timestamp tests; `rg -n 'format!\("\{:\?\}".*status|update_batch_status\([^,]+, *&?format!' src/core/batch src/storage/database/seaorm_db/batch_ops.rs` has no production hit.
- [ ] `SP1050-T4` Covers: B-004, B-005, B-007. Owner: implementation owner. Dependencies: SP1050-T2. Done when: list accepts canonical/historical rows, unknown row returns `Err`, and mixed valid+invalid query does not return partial/default records. Verify: in-memory SQLite tests insert/update raw statuses and assert exact variants/error.
- [ ] `SP1050-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: verification owner. Dependencies: SP1050-T3, SP1050-T4. Done when: diff is limited to GH1050 spec, batch status/domain/processor/storage code and focused tests; format, all-target/all-feature check, strict Clippy and full serial tests pass on final head. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `git diff --check origin/main...HEAD`; `git diff --stat origin/main...HEAD`.

## 执行顺序

Spec packet → failing processor/DB reproduction → canonical domain contract → typed write path → strict compatible read path → focused verification → repository-wide verification。相关生产文件存在直接依赖，由单一 implementation owner 串行修改。

## 验证

- Product invariant set 与 tasks `Covers:` union 精确为 B-001 至 B-007。
- Impl PR 使用 `Fixes #1050` 并以 Spec branch 为 base，代码审查与规范审查分离。
- 远端 PR 保持未自动合并，只报告 current-head checks 事实。

## Handoff Notes

- historical Debug values 是 read-only compatibility aliases；任何新写入仍必须 canonical snake_case。
- unknown status 不能映射为真实 `BatchStatus::Failed`，也不能只跳过该 row。
- 不夹带 metadata、request/result persistence、schema migration 或 transition redesign。
