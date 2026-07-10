# Task Plan

## Linked Issue

GH-957 / #957

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP957-T1` Owner: coordinator. Dependencies: none. Done when: `specs/GH957/` 三件套已提交，且 spec PR 相对刷新后的 `origin/main...HEAD` 精确只包含这三个文件；linked issue、产品不变量、技术映射和 task IDs 经独立 spec review 判定一致. Verify: `git fetch --no-tags origin main`; `git ls-files --error-unmatch specs/GH957/product.md specs/GH957/tech.md specs/GH957/tasks.md`; `diff -u <(printf '%s\n' specs/GH957/product.md specs/GH957/tasks.md specs/GH957/tech.md | LC_ALL=C sort) <(git diff --name-only origin/main...HEAD | LC_ALL=C sort)`; `git diff --exit-code HEAD -- specs/GH957`; `git diff --check origin/main...HEAD`; `rg -n "GH-957 / #957" specs/GH957`; `rg -n "SP957-T[1-8]" specs/GH957/tasks.md`; reviewer evidence 的 `head_sha` 必须精确等于 `git rev-parse HEAD` 且 verdict 无 unresolved finding。
- [ ] `SP957-T2` Owner: human maintainer/security reviewer. Dependencies: `SP957-T1`. Done when: 与 spec PR author、agent、bot 不同且属于 committed trusted-human login allowlist 的 human 对当前 `headRefOid` 提交 `APPROVED` review，review body 包含精确 marker `GH957-SPEC-SECURITY-SCOPE-APPROVED`，明确批准 `product.md` / `tech.md` 的 auth/security 边界、#969 scope split 与进入实现. Verify: 原样执行 `GH957-GATE-HUMAN-APPROVAL`；canonical committed allowlist 当前精确为 `[]`，所以本 gate 必须阻断，直到维护者提交真实 login、更新 spec head 并重新取得独立人类审查。runtime 参数不能增加 reviewer；GraphQL/JQ 还必须同时断言 membership、current-head commit、trusted association、human actor、非 PR author/bot/agent、marker 和 approval count `> 0`。
- [ ] `SP957-T3` Owner: coordinator. Dependencies: `SP957-T2`. Done when: Draft PR #954 已按 `GH957-GATE-SUPERSESSION` 关闭；closure-time comment 使用 `GH957-SUPERSESSION`，明确写出 `PR #954 is superseded`，链接 #957、active successor PR 及其当时 40 位 head 的完整 commit URL；关闭后 open PR 的 `src/auth/types.rs` overlap set 精确为空，随后 T4 才可创建 implementation diff. Verify: 关闭前断言 successor 为 `OPEN` 并记录 current head；在 exact comment/close 的紧邻前一步记录 `PRE_CLOSE_AT`，不得插入其他操作；关闭后 canonical GraphQL/JQ 断言 successor 仍为 `OPEN` 且 head 未变、#954 为 `CLOSED`、exact URL comment 存在、`PRE_CLOSE_AT <= comment.createdAt <= closedAt`、全部 pagination closed，且 `src/auth/types.rs` overlap count `== 0`；保存 pre-close 与 post-close UTC evidence。
- [ ] `SP957-T4` Owner: implementation owner. Dependencies: `SP957-T3`. Done when: T3 的 empty-overlap gate 已通过后才创建 implementation diff；`AuthMethod` 不再派生敏感字段 `Debug`，所有携带凭证的变体只输出 method kind 与固定 `[REDACTED]`；提交 diff 包含唯一命名的 exact-output 基础测试. Verify: `git diff --unified=80 origin/main...HEAD -- src/auth/types.rs`; `rg -n "fn auth_method_debug_exact_output_for_all_variants" src/auth/types.rs`; `bash -o pipefail -c 'cargo test --all-features --lib -- --list | grep -Fqx "auth::types::tests::auth_method_debug_exact_output_for_all_variants: test"'`; `cargo test --all-features --lib auth::types::tests::auth_method_debug_exact_output_for_all_variants -- --exact`；listing assertion 必须紧邻并先于 exact run；人工检查该测试对 `Jwt("[REDACTED]")`、`ApiKey("[REDACTED]")`、`Session("[REDACTED]")`、`None` 四个完整字符串逐一使用 `assert_eq!`，不接受 `contains`/prefix/suffix/snapshot oracle。
- [ ] `SP957-T5` Owner: implementation owner. Dependencies: `SP957-T4`. Done when: 唯一命名的 boundary test 对 JWT、API key、session 各使用至少两个不同输入，并覆盖空串、Unicode/换行和 `[REDACTED]`；所有断言比较完整输出，`None` 由 T4 的 all-variants test 精确覆盖. Verify: `git diff --unified=80 origin/main...HEAD -- src/auth/types.rs`; `rg -n "fn auth_method_debug_exact_output_for_boundary_secrets" src/auth/types.rs`; `bash -o pipefail -c 'cargo test --all-features --lib -- --list | grep -Fqx "auth::types::tests::auth_method_debug_exact_output_for_boundary_secrets: test"'`; `cargo test --all-features --lib auth::types::tests::auth_method_debug_exact_output_for_boundary_secrets -- --exact`; `bash -o pipefail -c 'cargo test --all-features --lib -- --list | grep -Fqx "auth::types::tests::auth_method_debug_exact_output_for_all_variants: test"'`; `cargo test --all-features --lib auth::types::tests::auth_method_debug_exact_output_for_all_variants -- --exact`；每个 listing assertion 必须紧邻并先于对应 exact run；人工逐项确认三个 secret 变体均有 `>= 2` 个不同/边界输入且每次均以 `assert_eq!(format!("{:?}", ...), <exact-full-string>)` 验证。
- [ ] `SP957-T6` Owner: review owner. Dependencies: `SP957-T4`, `SP957-T5`. Done when: 全部 production Rust sources 的 bypass scan 未发现日志/格式化调用绕过安全 formatter 直接输出 `AuthMethod` 内部凭证；每个相关命中都有独立 disposition，#969 只能用于明确属于独立 session identifier 日志的命中. Verify: `rg -n --glob '*.rs' "AuthMethod|auth_method|Authenticating request" src`; `rg -n --glob '*.rs' "(trace|debug|info|warn|error)!|format(_args)?!" src`；review evidence 记录两个命令的完整 output hash，并为每个与 auth credential、`AuthMethod`、其字段或输出 sink 相关的命中逐行填写 `path:line | symbol/use | credential-bearing | output-sink | disposition | issue`；review owner 将 disposition 行与原始相关命中一一对账，任何缺行、未知命中或无 issue 的 deferred hit 都失败。
- [ ] `SP957-T7` Owner: verification owner. Dependencies: `SP957-T4`, `SP957-T5`, `SP957-T6`. Done when: 固定 Rust toolchain 下格式、编译、lint、全量测试通过，implementation diff 精确符合单文件 allowlist 且没有 competing overlap. Verify: `git fetch --no-tags origin main`; `diff -u <(printf '%s\n' src/auth/types.rs) <(git diff --name-only origin/main...HEAD | LC_ALL=C sort)`; `git diff --check origin/main...HEAD`; `cargo fmt --all -- --check`; `cargo check --all-features`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; `bash scripts/guards/check_pr_overlap.sh`。`bash scripts/guards/check_pr_scope.sh origin/main` 仅作为 informational output 保存，因为脚本无条件 `exit 0`，不能满足 exact scope gate。

## 并行拆分

- 单文件实现与同文件测试必须由同一 implementation owner 串行完成，避免共享文件写冲突。
- review owner 只读，可在实现提交后独立检查 diff 与日志搜索结果。
- `SP957-T2` 与 `SP957-T8` 是独立、强制、绑定各自 current head 的 human gates，不与 agent lane 并行替代。

## 验证

- [ ] `SP957-T8` Owner: human maintainer/security reviewer + merge-review owner. Dependencies: `SP957-T7`. Done when: implementation PR 对同一 current head 完整通过 `GH957-GATE-PR-READY`，并由 committed trusted-human login allowlist 中、与 PR author/agent/bot 不同的 human 提交含 `GH957-IMPLEMENTATION-SECURITY-SCOPE-APPROVED` 的 current-head `APPROVED` review. Verify: 设置 numeric `PR` 后原样执行 `tech.md` 的 `GH957-GATE-PR-READY` canonical command；随后不改变 `PR`/`HEAD_SHA`，原样执行 `GH957-GATE-HUMAN-APPROVAL`。committed allowlist 当前为 `[]`，因此 T8 必须阻断，直到维护者提交真实 login 并让更新后的 spec head 重新审查；runtime input 不能增加 reviewer。记录查询 UTC、完整 JSON、head 与 reviewer evidence；API error、null、空 checks、未知 check type 或 pagination 均 fail closed。

## Handoff Notes

- Spec PR 使用 `Refs #957`，保持 Draft 直到 `SP957-T2` 人类 spec/security approval，不能提前关闭 issue。
- 最终 implementation PR 满足全部 acceptance criteria 后才使用 `Fixes #957`。
- implementation PR 的 exact changed-file allowlist 是 `src/auth/types.rs`；任何额外文件先更新并重新批准 spec，不能靠 informational scope guard 放行。
- #958、#959、#960 与 #961 不属于本 issue，不得重新打包进同一实现 PR。
