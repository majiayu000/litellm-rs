# Task Plan

## Linked Issue

GH-1038 / #1038

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1038-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: coordinator. Dependencies: none. Done when: GH1038 product/tech/tasks 三件套完整，范围排除其他依赖更新，全部 invariant 有 task/test mapping. Verify: `test -f specs/GH1038/product.md -a -f specs/GH1038/tech.md -a -f specs/GH1038/tasks.md`; product/tasks 的 `B-[0-9]{3}` 集合一致.
- [ ] `SP1038-T2` Covers: B-001, B-002, B-003. Owner: implementation owner. Dependencies: SP1038-T1. Done when: precise update 将唯一 `spin 0.10.0` package 更新到 `0.10.1`，只修改 `Cargo.lock`；diff 只含 package version/checksum 与 `crc-fast` 既有版本消歧引用. Verify: `cargo update -p spin@0.10.0 --precise 0.10.1`; `git diff --stat`; `git diff -- Cargo.lock`; `cargo update -p spin@0.10.1 --precise 0.10.1 --dry-run`.
- [ ] `SP1038-T3` Covers: B-001, B-003, B-004. Owner: verification owner. Dependencies: SP1038-T2. Done when: 旧 package 从完整依赖图消失，新 package 沿既有 AWS S3 checksum 路径解析，audit 不再报告目标 warning且无新增 ignore. Verify: `cargo tree -i spin@0.10.0 --locked --all-features` 预期失败；`cargo tree -i spin@0.10.1 --locked --all-features`; `cargo audit`; `git diff origin/main...HEAD -- .cargo Cargo.toml deny.toml` 预期为空.
- [ ] `SP1038-T4` Covers: B-002, B-003, B-005. Owner: verification owner. Dependencies: SP1038-T3. Done when: 最小 lockfile diff 通过格式、编译、strict Clippy 与全 feature serial tests. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`.
- [ ] `SP1038-T5` Covers: B-006. Owner: coordinator. Dependencies: SP1038-T4. Done when: Impl PR 以 Spec 分支为 base、只显示 `Cargo.lock`、关联 #1038、记录 fresh verification 且保持未合并. Verify: `gh pr view <impl-pr> --json baseRefName,headRefName,state,mergeable,body,statusCheckRollup`; `gh issue view 1038 --json state`.

## 执行顺序

1. Spec PR 独立面向 `main`。
2. Impl 分支从 Spec branch HEAD 派生，Impl PR 只展示实现 diff。
3. 精确更新并审核 lockfile diff 后运行完整验证。
4. 验证通过后创建 Impl PR，不自动合并。

## 验证

- Product invariant set 与 tasks `Covers:` union 均为 B-001 至 B-006，无 orphan。
- 实现只有 `spin 0.10.0 -> 0.10.1` package metadata 与 `crc-fast` 既有版本消歧引用变化。
- `cargo audit` 保留其他独立 warning 的可见性。
- Impl PR 使用 `Fixes #1038`，人工合并前 Issue 保持 open。

## Handoff Notes

- 不把 #1035 的 `spin 0.9.x` lockfile diff 混入本 PR。
- 若 precise update 出现 probe 之外的 package 变化，停止并调查 resolver。
- 不修改用户工作树中的 `WORKFLOW.md`。
