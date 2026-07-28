# Task Plan

## Linked Issue

GH-1130 / #1130

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 决策门

- [ ] `SP1130-T0` Owner: maintainer/spec owner. Done when: the detailed exact-head spec gate below is satisfied. Verify: the SpecRail and exact-head evidence commands below.
  Owner: maintainer/spec owner.
  Dependencies: none.
  Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017,
  B-018, B-019.
  Files: spec packet only；implementation manifest 保持只读。
  Done when: Issue 在 amendment 期间保持 `ready_to_spec`；本 amendment exact head 的
  provenance 三态、400/401/5xx 分流、subject-bound `VerifiedActiveTeam`、public
  struct/handler/method compatibility、互斥 caller scope、UUID owner exact wire、
  proof-aware route adapters、legacy purpose、API-key policy fail-closed、S3 exact 404
  与完整 pagination 通过 spec CI、独立内容复核和 content-bound human approval，
  spec PR 合并后才设置 `ready_to_implement`。
  Verify: `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1130`;
  `python3 checks/route_gate.py --repo . --route write_spec --issue 1130 --state ready_to_spec --json`;
  exact-head review evidence 与合并后 implement route gate。

## 实现任务

- [ ] `SP1130-T1` Owner: auth/JWT implementation owner. Done when: every detailed auth/provenance condition below is satisfied. Verify: the focused auth commands below.
  Owner: auth/JWT implementation owner.
  Dependencies: SP1130-T0.
  Covers: B-001, B-005, B-011, B-012, B-013, B-014, B-015, B-016.
  Files: `src/auth/jwt/{types,handler,tests}.rs`,
  `src/auth/{user_management,system,tests}.rs`,
  `src/server/routes/auth/{models,login,token,mod}.rs`（10 paths）。
  Done when: public `Claims` 与 auth request DTO/handler signatures 不变，internal
  access envelope 实现 exact marker 三态，private wire adapters 接收 selection 且是
  auth route 唯一 registration；legacy 忽略 guessed team；unknown version
  team/no-team 与 stale membership 均为 401 且不 fallback；login/refresh optional
  UUID selection 的 malformed/foreign/missing/inactive/suspended 为 400 且 encode count
  为零；repository/RBAC `Err` 为 5xx；只有字段私有、共享 validator 构造的
  `VerifiedActiveTeam { user_id, team_id }` 可走 typed Team issuer，issuer 在 encode
  前验证 proof user 等于 token subject；existing public JWT/AuthSystem 方法
  参数/返回类型不变，raw `team_id: Some` 在 encode 前拒绝，team 与 membership 都须
  active，所有 `team_ids.first()` 路径消失。
  Verify: `cargo test --lib auth::jwt::tests -- --nocapture`;
  `cargo test --lib auth::tests -- --nocapture`;
  login/token inline tests；public signature compile fixture。

- [ ] `SP1130-T2` Owner: file-storage implementation owner. Done when: every detailed storage/wire/pagination condition below is satisfied. Verify: the focused storage and S3 commands below.
  Owner: file-storage implementation owner.
  Dependencies: SP1130-T1.
  Covers: B-002, B-006, B-007, B-009, B-010, B-014, B-016, B-018.
  Files: `src/storage/files/{mod,types,storage,local,s3,tests}.rs`（6 paths）。
  Done when: `FileOwnerScope` 仅有 Team/User/ApiKey UUID variants；public
  `FileMetadata` 字段集合与 struct literal 保持不变；crate-private flat
  `StoredFileMetadata` envelope 的 owner serde-defaulted，兼容 legacy bare metadata；
  existing public Local/S3/FileStorage/StorageLayer store/metadata 方法签名不变并写
  legacy `None`；owner adjacent-tag wire 精确为 `scope + id`；新增 owner 非 optional
  的 crate-internal owned-store 与 metadata-with-owner；Local/S3 round-trip 一致；
  S3 key 固定 `litellm-owner`，value 为 version 1/scope/id，只有 missing key 是
  legacy；Local/S3 list 对 `Some(0)` 都是零 I/O/空结果；S3 稳定 prefix，以
  `min(1000, remaining)` 遍历所有 continuation pages，检测 missing/empty/repeated
  token，later-page error 整体失败；shared HeadObject mapper 只按 modeled
  `is_not_found()`/raw service 404 映射 NotFound，其余保持 error。
  Verify: `cargo test --lib storage::files::tests -- --nocapture`;
  `cargo test --features s3 --lib storage::files::s3::tests -- --nocapture`;
  `cargo check --all-features`。

- [ ] `SP1130-T3` Owner: Files authorization/policy implementation owner. Done when: every detailed caller/route/concealment condition below is satisfied. Verify: the focused policy/route commands below.
  Owner: Files authorization/policy implementation owner.
  Dependencies: SP1130-T2.
  Covers: B-001, B-002, B-003, B-004, B-005, B-008, B-009, B-010,
  B-013, B-014, B-016, B-017, B-018, B-019.
  Files: `src/server/routes/ai/context.rs`,
  `src/server/middleware/auth.rs`, `src/server/routes/ai/{files,mod}.rs`（4 paths）。
  Done when: checked API-key helper 对 direct/runtime admin、empty/restricted key 与
  malformed policy 返回精确 Result；任何 API key 都阻止 admin-owner role fallback；
  middleware policy/check `Err` 为 generic 5xx；FileCaller 的 ApiKey 与 JWT branches
  互斥，key branch 只按 key team/user/id，JWT branch 只按 verified team/user，
  principal/context mismatch 或 invalid UUID 为 5xx 且不 fallback；auth-enabled
  upload 只调用 owned-store；list/get/content/delete 共用唯一 authorization helper；
  legacy missing/invalid purpose 整体显式失败；完整 candidates 过滤后才返回；
  metadata error 不 skip；foreign 与 missing 公开 404 完全等价；unauthorized 不读取
  content、不删除。五个 public Files handlers 保持签名，proofless call 在 auth-enabled
  时零 storage calls；HTTP routes 只注册 private proof-aware adapters。
  Verify: `cargo test --lib server::routes::ai::context::tests -- --nocapture`;
  focused route mock call-count/error-shape tests；source review 证明没有 legacy-store
  fallback。

- [ ] `SP1130-T4` Owner: security integration-test owner. Done when: the complete detailed security matrix below is implemented. Verify: the integration targets below.
  Owner: security integration-test owner.
  Dependencies: SP1130-T3.
  Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017,
  B-018, B-019.
  Files: `tests/files_routes.rs`,
  `tests/integration/auth_middleware_tests_parts/mod.rs`,
  `tests/integration/auth_middleware_tests_parts/rejection_rate_limit.rs`,
  `tests/public_api_compat.rs`（4 paths）。
  Done when: integration matrix 覆盖 two users/teams/keys、same-user cross-team、
  multi-team no-selection、legacy/1/unknown markers（team Some/None）、stale/inactive
  membership、selection 400、token 401、repository/RBAC/policy/metadata 5xx、direct/
  runtime/empty/restricted/corrupt key + admin owner、admin JWT、legacy files、
  auth-disabled、key-vs-JWT exclusive scope/mismatch、legacy purpose、完整 list、
  foreign/missing、owner/policy redaction、public struct/handler/method compile、
  private adapter registration 与 proofless-wrapper backend call counts；S3 fixture
  不访问真实 AWS。
  Verify: `cargo test --all-features --test files_routes -- --nocapture`;
  `cargo test --all-features --test lib integration::auth_middleware_tests -- --nocapture`;
  `cargo test --all-features --test public_api_compat -- --nocapture`;
  T1/T2 的完整 module commands。

- [ ] `SP1130-T5` Owner: verification owner + independent human security reviewer. Done when: every detailed current-head gate below is green. Verify: the complete verification set below.
  Owner: verification owner + independent human security reviewer.
  Dependencies: SP1130-T4.
  Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017,
  B-018, B-019.
  Files: read-only verification；findings 返回对应 owner，不新增 manifest path。
  Done when: implementation diff 恰好是 tech manifest 的 24 paths；OR matching、
  foreign 403、first-team、unknown-version fallback、first-page、policy-error 403/detail、
  metadata skip 与 destructive public signature changes 全部不存在；focused/full Rust
  gates、SpecRail checks、current-head CI、resolved review threads 与 independent
  auth/security human review 全部通过；listed public structs/handlers 未改变，
  proofless route bypass 不存在，legacy bare/new envelope compatibility 有 fresh
  evidence。最终实现 PR 使用
  `Fixes #1130`；未经 human merge authorization 不合并。
  Verify: 下列完整 verification set 与 current-head PR gate evidence。

## 并行拆分

不并行写。T1 → T2 → T3 → T4 是串行依赖；每步只有一个 writer，文件 ownership
如上且无重叠并发。T5 只读。任何新发现若需要 manifest 外文件，停止实现并回到
SP1130-T0 的 spec amendment/human approval gate。

## 验证

- `cargo fmt --check`
- `cargo check`
- `cargo check --all-features`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --lib auth::jwt::tests -- --nocapture`
- `cargo test --lib auth::tests -- --nocapture`
- `cargo test --lib storage::files::tests -- --nocapture`
- `cargo test --features s3 --lib storage::files::s3::tests -- --nocapture`
- `cargo test --all-features --test files_routes -- --nocapture`
- `cargo test --all-features --test lib integration::auth_middleware_tests -- --nocapture`
- `cargo test --all-features --test public_api_compat -- --nocapture`
- `cargo test`
- `python3 checks/check_workflow.py --repo .`
- `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1130`
- `python3 checks/route_gate.py --repo . --route write_spec --issue 1130 --state ready_to_spec --json`
- `git diff --check`
- manifest JSON 必须 parse、`issue == 1130`、`complete == true`、path count/set
  恰好为 24、全部存在且无重复，`spec_refs` 与 product B-ID set 完全相等。
- product B-ID set、tech mapping set、task `Covers:` union 必须都是
  B-001 ～ B-019，无 orphan/missing ID。
- focused test filter 若匹配 0 tests 不算通过；必须同时有完整 module/target output。
- foreign/nonexistent status、error type/code/body 必须等价；unknown/stale token
  为 401；selection 为 400；repository/RBAC/policy/storage `Err` 为 generic 5xx。
- S3 pagination fixture 必须超过一页并验证 continuation 到结束、missing/repeat token
  failure 与 cross-page `Some(limit)`；不得访问真实 AWS。
- compile/source-boundary fixture 必须证明 existing public
  `FileMetadata`/`Claims`/auth request struct literals、store/metadata/JWT/auth/Files
  handler call signatures 不变，且 auth/Files HTTP routes 只注册 private wire/proof
  adapters；proofless Files wrappers 在 auth-enabled 时零 storage calls。

## Handoff Notes

- 被否决的 `FileOwner { user_id, team_id, api_key_id }` + 任一匹配方案会产生
  same-user cross-team bypass。
- 被否决的 raw-team JWT issuer 无法证明 provenance；只有
  `VerifiedActiveTeam` typed path 可签 Team token。
- 被否决的 foreign 403 是存在性 oracle；按规格使用 uniform 404。
- legacy 文件不自动归属；auth-enabled 时仅 admin 可见。
- 当前 human gate 未满足：本 amendment exact head 尚无 content-bound approval、
  未合并，Issue 必须保持 `ready_to_spec`，不得启动实现。
- 本 task edit 不 commit、push、comment、resolve thread 或 merge。
