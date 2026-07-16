# Task Plan

## Linked Issue

GH-1035 / #1035

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP1035-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006. Owner: coordinator. Dependencies: none. Done when: GH1035 product/tech/tasks 三件套完整，范围明确排除其他依赖更新，所有 behavior invariant 均有 task 和 test mapping. Verify: `test -f specs/GH1035/product.md -a -f specs/GH1035/tech.md -a -f specs/GH1035/tasks.md`; `rg -o "B-[0-9]{3}" specs/GH1035/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH1035/tasks.md | sort -u`.
- [ ] `SP1035-T2` Covers: B-001, B-002, B-003. Owner: implementation owner. Dependencies: SP1035-T1. Done when: 精确 Cargo update 将唯一 `spin 0.9.8` package 更新到 `0.9.9`，工作树只修改 `Cargo.lock`，diff 只有 version/checksum. Verify: `cargo update -p spin@0.9.8 --precise 0.9.9`; `git diff --stat`; `git diff -- Cargo.lock`; `cargo update -p spin@0.9.8 --precise 0.9.9 --dry-run` 预期无变化.
- [ ] `SP1035-T3` Covers: B-001, B-003, B-004. Owner: verification owner. Dependencies: SP1035-T2. Done when: 旧版本从完整依赖图消失，新版本仍只沿既有 `flume` 路径解析，audit 不再报告目标 warning 且没有新增 ignore. Verify: `cargo tree -i spin@0.9.8 --locked --all-features` 预期无匹配；`cargo tree -i spin@0.9.9 --locked --all-features`; `cargo audit`; `git diff origin/main...HEAD -- .cargo Cargo.toml deny.toml` 预期为空.
- [ ] `SP1035-T4` Covers: B-002, B-003, B-005. Owner: verification owner. Dependencies: SP1035-T3. Done when: 最小 lockfile diff 通过完整 Rust 格式、编译、strict Clippy 与全 feature serial tests. Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`.
- [ ] `SP1035-T5` Covers: B-006. Owner: coordinator. Dependencies: SP1035-T4. Done when: Impl PR 以 Spec 分支为 base、只显示 `Cargo.lock` 实现 diff、关联 #1035、记录 fresh verification 且保持未合并. Verify: `gh pr view <impl-pr> --json baseRefName,headRefName,state,mergeable,body,statusCheckRollup`; `gh issue view 1035 --json state`.

## 执行顺序

1. 合并前保持 Spec PR 独立可审查。
2. Impl 分支从 Spec branch HEAD 派生，避免 Impl PR 重复展示 Spec 文件。
3. 精确更新并审核 lockfile diff 后再运行完整验证。
4. 验证通过后创建 Impl PR，不自动合并。

## 验证

- Product invariant set 与 tasks `Covers:` union 均为 B-001 至 B-006，无 orphan。
- 实现只包含 `Cargo.lock` 的 `spin 0.9.8 -> 0.9.9` version/checksum 变化。
- `cargo audit` 保留其他独立 warning 的可见性。
- Impl PR 使用 `Fixes #1035`，但在人工合并前 Issue 保持 open。

## Handoff Notes

- `spin 0.10.0` 是另一条依赖路径和独立优化事项，不得混入本 PR。
- 若精确 update 产生任何额外 package 变化，先停止并重新调查 resolver，而不是接受扩大范围。
- 不修改用户工作树中的 `WORKFLOW.md`。
