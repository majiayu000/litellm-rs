# Task Plan

## Linked Issue

GH-1130 / #1130

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1130-T0` Covers: B-001 ～ B-010. Owner: maintainer/spec owner. Dependencies: none. Done when: 单一 effective scope、legacy admin-only 与 foreign-as-not-found 绑定最终 spec SHA 获批，Issue 添加 `ready_to_implement`. Verify: SpecRail workflow/spec checks与 implement route gate。

## 实现任务

- [ ] `SP1130-T1` Covers: B-001, B-004, B-005, B-009. Owner: auth implementation owner. Dependencies: SP1130-T0. Done when: `FileOwnerScope` 为单值 enum，caller 只从可信 context/extensions 解析 team -> user -> API key，admin 与 auth-disabled 分支明确，缺身份 fail-closed. Verify: scope priority/RBAC/auth-disabled unit matrix.
- [ ] `SP1130-T2` Covers: B-006, B-007, B-008. Owner: storage implementation owner. Dependencies: SP1130-T1. Done when: dispatch、Local、S3 持久化同一 owner enum；legacy `None` 兼容；公开 `FileObject` 不含 owner. Verify: Local/S3 round-trip、legacy、bad metadata tests.
- [ ] `SP1130-T3` Covers: B-002, B-003, B-004, B-005, B-010. Owner: route implementation owner. Dependencies: SP1130-T2. Done when: list/get/content/delete 全部复用唯一授权 helper；foreign 与 missing 公开响应相同；unauthorized 不调用 backend content/delete. Verify: route mock/backend call-count tenant matrix.
- [ ] `SP1130-T4` Covers: B-001 ～ B-010. Owner: security test owner. Dependencies: SP1130-T3. Done when: same-user cross-team、multiple keys/users/teams、admin、legacy、auth-disabled、日志/JSON 泄露矩阵完整. Verify: focused files route/storage security tests.
- [ ] `SP1130-T5` Covers: B-001 ～ B-010. Owner: verification owner. Dependencies: SP1130-T4. Done when: Claude 原 OR-matching/403 方案已移除，完整 Rust/SpecRail gate 与 auth 安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。auth scope、storage schema 和 route enforcement 是串行依赖，且共同触碰
Files API 契约。单一 owner 在 `.claude/worktrees/agent-a5c5acb27baefcb98`
完成，避免共享文件并发编辑。

## 验证

- foreign 与 nonexistent 的 status、error type/code 和正文必须逐字等价或经稳定字段比较等价。
- S3 测试不得访问真实 AWS；使用 feature-aware mock/fixture。
- list 对损坏 metadata 显式失败，不允许 skip 后静默返回。
- 最终 PR 使用 `Fixes #1130`，必须有人审查 auth/security 代码。

## Handoff Notes

- Claude 原 `FileOwner { user_id, team_id, api_key_id }` + 任一匹配会产生跨 team bypass。
- Claude 原 foreign `403` 可被用作文件存在性 oracle；按规格改为 uniform not-found。
- 旧文件不自动归属，只有 admin 可见。
- 不自动合并、不 force-push。
