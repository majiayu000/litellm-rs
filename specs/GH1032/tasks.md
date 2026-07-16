# Task Plan

## Linked Issue

GH-1032 / #1032

## Execution Constraints

- 实现分支从包含本 Spec 的基线创建，其 `origin/main` 基底为
  `0baa92798d15630edc0b6abd65646b25e49ca23c`。
- Spec PR 与 Impl PR 分离；不自动合并。
- 实现 diff 只能包含 `Cargo.lock`，不得修改 manifest、audit 配置或源码。

## Implementation Tasks

### SP1032-T1 — 精确更新 anyhow lock entry

- Owner: implementation agent
- Dependencies: approved `product.md` and `tech.md`
- Covers: B-001, B-002
- Work:
  - 执行 `cargo update -p anyhow --precise 1.0.103`。
  - 检查 lockfile package diff，拒绝任何无关 package 更新。
- Done when:
  - `Cargo.lock` 的 `anyhow` 为 `1.0.103`，且 `1.0.102` 不再存在。
  - 唯一变化是 anyhow version/checksum。
- Verify:
  - `cargo tree -i anyhow --locked --all-features`
  - `cargo update -p anyhow --precise 1.0.103 --dry-run`
  - `git diff -- Cargo.lock`

## Verification Tasks

### SP1032-T2 — 恢复安全门并验证兼容性

- Owner: implementation agent
- Dependencies: SP1032-T1
- Covers: B-003, B-004, B-005, B-006
- Done when:
  - `cargo audit --deny unsound` 成功且不含 `RUSTSEC-2026-0190`。
  - 普通 audit 继续显示剩余基线 warning。
  - 格式、all-target/all-feature check、strict Clippy 与全 feature tests 在 `--locked` 下通过。
- Verify:
  - `cargo fmt --all -- --check`
  - `cargo audit --deny unsound`
  - `cargo audit`
  - `cargo check --all-targets --all-features --locked`
  - `cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cargo test --all-features --locked`

## Handoff

### SP1032-T3 — 独立 Impl PR 交付

- Owner: implementation agent
- Dependencies: SP1032-T2
- Covers: none — 该任务只封装验证证据，不新增产品行为。
- Done when:
  - Impl PR 使用 `Fixes #1032`，链接 Spec PR，并记录 precise update、audit 与测试证据。
  - PR diff 只有 `Cargo.lock`，保持未合并等待最终人工决定。
- Verify:
  - `git diff --check origin/main...HEAD`
  - `git diff --name-only origin/main...HEAD`

## Invariant Coverage Audit

- Product IDs: `B-001, B-002, B-003, B-004, B-005, B-006`
- Task coverage union: `B-001, B-002, B-003, B-004, B-005, B-006`
- Missing: none
