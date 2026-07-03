# Task Plan

## Linked Issue

GH-843 / #843

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP843-T1` Owner: coordinator. Done when: `specs/GH843/` 三件套通过 SpecRail packet validation. Verify: `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH843"`.
- [ ] `SP843-T2` Owner: maintainer. Done when: #843 批复 public export 删除策略、feature flag 是 gate 还是移除、是否需要 breaking CHANGELOG（SpecRail human gate `spec_approval`）. Verify: #843 issue thread 明确批复。
- [ ] `SP843-T3` Owner: coordinator. Done when: 删除 `.rlib`、一次性顶层脚本和 scratch 文档，并更新 `.gitignore` 防止编译产物再次提交；合法 deployment/guard/test scripts 保留. Verify: `git ls-files <target paths>` 无输出；`cargo check`。
- [ ] `SP843-T4` Owner: coordinator. Done when: 用当前 main 搜索确认 `ConcurrentMap` / `ConcurrentVec` / `VersionedMap` 没有生产调用点；删除无调用点实现、tests、docs/re-exports，保留 `AtomicValue`. Verify: production `rg` 输出；`cargo check --all-features`。
- [ ] `SP843-T5` Owner: coordinator. Done when: `websockets` / `analytics` / `enterprise` feature flags 完成 gate-or-remove 决策，`Cargo.toml`、health feature reporting、docs.rs metadata、README/Cargo 注释与实际行为一致. Verify: `cargo check`; `cargo check --all-features`; adjusted docs.rs `cargo doc` command。
- [ ] `SP843-T6` Owner: verification owner. Done when: 全仓 deterministic checks 通过，PR body 附 git/ripgrep/feature matrix 输出. Verify: `cargo test --all-features`。

## 并行拆分

- SP843-T3 只改仓库文件清理和 `.gitignore`，可独立 PR。
- SP843-T4 改 `src/utils/sync` 与 exports，不得与其他 lane 同写这些文件。
- SP843-T5 改 `Cargo.toml`、feature-gated modules 和 docs，不得与 feature 相关 PR 并行写同文件。

## Handoff Notes

- 不要删除 `deployment/`、`scripts/guards/`、`scripts/test/` 下仍有运行价值的脚本。
- 如果维护者不接受 public export 删除，SP843-T4 改为 deprecate-first，并重新更新验收标准。
- feature flag 清理要以“真实编译行为”为准，不接受只更新 health 文本的表面修复。
