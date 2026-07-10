# Task Plan

## Linked Issue

GH-957 / #957

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP957-T1` Owner: coordinator. Dependencies: none. Done when: `specs/GH957/` 三件套已提交，linked issue、产品不变量、技术映射和 task IDs 经独立 spec review 判定一致. Verify: `git ls-files --error-unmatch specs/GH957/product.md specs/GH957/tech.md specs/GH957/tasks.md`; `git diff --exit-code HEAD -- specs/GH957`; `git diff --check origin/main...HEAD`; `rg -n "GH-957 / #957" specs/GH957`; `rg -n "SP957-T[1-8]" specs/GH957/tasks.md`; reviewer evidence 的 `head_sha` 必须等于 `git rev-parse HEAD` 且 verdict 无 unresolved finding。
- [ ] `SP957-T2` Owner: human maintainer/security reviewer. Dependencies: `SP957-T1`. Done when: 维护者对 spec PR 当前 head 提交 `APPROVED` review，明确批准 `product.md` / `tech.md` 的 auth/security 边界、#969 scope split 与进入实现. Verify: `gh pr view <spec-pr> --json headRefOid,reviews --jq '. as $pr | [$pr.headRefOid, ([$pr.reviews[] | select(.state == "APPROVED" and .commit.oid == $pr.headRefOid)] | length)]'` 的 approval count 至少为 1；agent review 不满足该 gate。
- [ ] `SP957-T3` Owner: coordinator. Dependencies: `SP957-T2`. Done when: Draft PR #954 已 `CLOSED` 并评论链接 successor issues/PRs，且所有 open PR 的 files 中都没有 `src/auth/types.rs`. Verify: `gh pr view 954 --json state,comments --jq '[.state, ([.comments[].body | select(contains("#957"))] | length)]'` 返回 `CLOSED` 且 comment count 至少为 1；`gh api graphql -f query='query { repository(owner:"majiayu000", name:"litellm-rs") { pullRequests(states:OPEN, first:100) { nodes { number files(first:100) { nodes { path } } } } } }' --jq '.data.repository.pullRequests.nodes[] | select(any(.files.nodes[]; .path == "src/auth/types.rs")) | .number'` 无输出。
- [ ] `SP957-T4` Owner: implementation owner. Dependencies: `SP957-T3`. Done when: `AuthMethod` 不再派生敏感字段 `Debug`，所有携带凭证的变体只输出 method kind 与固定 `[REDACTED]`. Verify: `git diff -- src/auth/types.rs`; `cargo test auth::types --all-features`。
- [ ] `SP957-T5` Owner: implementation owner. Dependencies: `SP957-T4`. Done when: JWT、API key、session、`None` 均有精确输出测试；每个 secret 变体用至少两个不同输入证明输出相同，并覆盖空串、Unicode/换行和 `[REDACTED]`. Verify: `cargo test auth::types --all-features`。
- [ ] `SP957-T6` Owner: review owner. Dependencies: `SP957-T4`, `SP957-T5`. Done when: 定向搜索未发现日志绕过安全 formatter 直接读取 `AuthMethod` 内部字段；独立 session 日志保持由 #969 跟踪. Verify: `rg -n "AuthMethod|Authenticating request" src/auth src/server` 并人工审查命中。
- [ ] `SP957-T7` Owner: verification owner. Dependencies: `SP957-T4`, `SP957-T5`, `SP957-T6`. Done when: 固定 Rust toolchain下格式、编译、lint、全量测试与 scope/overlap guards 通过. Verify: `cargo fmt --all -- --check`; `cargo check --all-features`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; `bash scripts/guards/check_pr_scope.sh`; `bash scripts/guards/check_pr_overlap.sh`。

## 并行拆分

- 单文件实现与同文件测试必须由同一 implementation owner 串行完成，避免共享文件写冲突。
- review owner 只读，可在实现提交后独立检查 diff 与日志搜索结果。
- `SP957-T2` 是强制 human gate，不与 agent lane 并行替代。

## 验证

- [ ] `SP957-T8` Owner: human maintainer/security reviewer + merge-review owner. Dependencies: `SP957-T7`. Done when: 人类 auth/security `APPROVED` review 绑定当前 PR head；全部 check conclusion 为 `SUCCESS`；无 unresolved review threads；`mergeStateStatus` 为 `CLEAN`；最终 PR 使用 `Fixes #957`. Verify: `gh pr view <pr> --json headRefOid,isDraft,mergeStateStatus,mergeable,statusCheckRollup,closingIssuesReferences,reviews`; `gh pr view <pr> --json headRefOid,reviews --jq '. as $pr | [$pr.headRefOid, ([$pr.reviews[] | select(.state == "APPROVED" and .commit.oid == $pr.headRefOid)] | length)]'` 的 approval count 至少为 1；`gh api graphql -f query='query { repository(owner:"majiayu000", name:"litellm-rs") { pullRequest(number:<pr>) { reviewThreads(first:100) { nodes { isResolved } } } } }' --jq '[.data.repository.pullRequest.reviewThreads.nodes[] | select(.isResolved == false)] | length'` 返回 0；gate 查询 UTC 时间与 head SHA 写入 evidence。

## Handoff Notes

- Spec PR 使用 `Refs #957`，保持 Draft 直到 `SP957-T2` 人类 spec/security approval，不能提前关闭 issue。
- 最终 implementation PR 满足全部 acceptance criteria 后才使用 `Fixes #957`。
- #958、#959、#960 与 #961 不属于本 issue，不得重新打包进同一实现 PR。
