# Task Plan

## Linked Issue

GH-1130 / #1130

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1130-T0` Covers: B-001 ～ B-010. Owner: maintainer/spec owner. Dependencies: none. Done when: amendment 审查期间 Issue 移出 `ready_to_implement`；本 amendment exact head 的可信 active-team provenance、API-key admin attenuation、S3 精确 NotFound 与全分页授权过滤契约获得内容绑定批准并合并后，才恢复 `ready_to_implement`. Verify: SpecRail workflow/spec checks 与 implement route gate。

## 实现任务

- [ ] `SP1130-T1` Covers: B-001, B-004, B-005, B-009. Owner: auth implementation owner. Dependencies: SP1130-T0. Done when: `FileOwnerScope` 为单值 enum；API-key team 来自 key；`/auth/login` 与 `/auth/refresh` 接受可选 snake_case `team_id` 并以当前 server-side membership 校验，只有 typed `VerifiedActiveTeam` 可签发 `team_id` + `team_scope_version = 1`；旧 token 缺少 marker 时忽略历史 team claim 并安全回退 user scope；签发/读取均禁止 `team_ids.first()`；admin 使用 shared crate-private direct/runtime permission parser，empty/restricted key 不继承 owner admin role；auth-disabled 与缺身份分支明确. Verify: login/refresh selection、legacy token、multi-team scope provenance、key attenuation/RBAC/auth-disabled unit matrix.
- [ ] `SP1130-T2` Covers: B-002, B-006, B-007, B-008. Owner: storage implementation owner. Dependencies: SP1130-T1. Done when: dispatch、Local、S3 持久化同一 owner enum；legacy `None` 兼容；S3 list 遍历 continuation 到结束；HeadObject 仅以 AWS SDK `is_not_found()` 分类或 HTTP 404 映射 NotFound，不依赖 HeadObject 不提供的错误正文；公开 `FileObject` 不含 owner. Verify: Local/S3 round-trip、legacy、bad metadata、>1000 pagination、404/5xx tests.
- [ ] `SP1130-T3` Covers: B-002, B-003, B-004, B-005, B-010. Owner: route implementation owner. Dependencies: SP1130-T2. Done when: list/get/content/delete 全部复用唯一授权 helper；全部分页候选集完成授权过滤后返回完整 authorized set，因当前 public Files API 没有 list limit/count 字段而不得虚构该契约；foreign 与真实 missing 公开响应相同；unauthorized 不调用 backend content/delete. Verify: route mock/backend call-count tenant/full-list matrix.
- [ ] `SP1130-T4` Covers: B-001 ～ B-010. Owner: security test owner. Dependencies: SP1130-T3. Done when: login/refresh active-team 选择、invalid/stale membership、legacy unversioned claim、multi-team no-selection User-scope fallback、same-user cross-team、multiple keys/users/teams、key direct/runtime admin、empty/restricted key + admin owner、admin JWT、legacy file metadata、auth-disabled、日志/JSON 泄露矩阵完整. Verify: focused auth/files route/storage security tests.
- [ ] `SP1130-T5` Covers: B-001 ～ B-010. Owner: verification owner. Dependencies: SP1130-T4. Done when: 既有 OR-matching/403/first-team/first-page 方案均已移除，完整 Rust/SpecRail gate 与 auth 安全人工 review 通过. Verify: `cargo fmt --check`; `cargo check`; `cargo check --all-features`; `cargo clippy --all-targets --all-features -- -D warnings`; focused S3 tests; `cargo test`; workflow/spec checks; `git diff --check`.

## 并行拆分

不并行。auth scope、storage schema 和 route enforcement 是串行依赖，且共同触碰
Files API 契约。实现与验证只在本 session 的 issue worktree 串行完成，避免共享文件
并发编辑。

## 验证

- foreign 与 nonexistent 的 status、error type/code 和正文必须逐字等价或经稳定字段比较等价。
- S3 测试不得访问真实 AWS；使用 feature-aware mock/fixture。
- S3 pagination fixture 必须超过单页容量，验证 continuation token 到结束、完整 authorized
  list 返回且无虚构 limit/count；S3 404 与非 404 错误分别断言。
- list 对损坏 metadata 显式失败，不允许 skip 后静默返回。
- 最终 PR 使用 `Fixes #1130`，必须有人审查 auth/security 代码。

## Handoff Notes

- Claude 原 `FileOwner { user_id, team_id, api_key_id }` + 任一匹配会产生跨 team bypass。
- Claude 原 foreign `403` 可被用作文件存在性 oracle；按规格改为 uniform not-found。
- 旧文件不自动归属，只有 admin 可见。
- 不自动合并、不 force-push。
