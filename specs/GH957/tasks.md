# Task Plan

## Linked Issue

GH-957 / #957

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP957-T1` Owner: coordinator. Dependencies: none. Done when: `specs/GH957/` 三件套非空，linked issue、产品不变量、技术映射和 task IDs 一致. Verify: `test -s specs/GH957/product.md`; `test -s specs/GH957/tech.md`; `test -s specs/GH957/tasks.md`; `git diff --check`。
- [ ] `SP957-T2` Owner: human maintainer/security reviewer. Dependencies: `SP957-T1`. Done when: 维护者明确批准 `product.md` 与 `tech.md` 的 auth/security 边界，确认 #969 已正确拆出，且批准进入实现. Verify: #957 或 spec PR 上存在人类确认记录；agent review 不能满足此 gate。
- [ ] `SP957-T3` Owner: coordinator. Dependencies: `SP957-T2`. Done when: Draft PR #954 已安全拆分或标记 superseded，且没有 open PR 继续占有 `src/auth/types.rs` 的重叠实现. Verify: `gh pr view 954 --json state,isDraft,files`; `gh pr list --state open --json number,headRefName,url`; `bash scripts/guards/check_pr_overlap.sh`。
- [ ] `SP957-T4` Owner: implementation owner. Dependencies: `SP957-T3`. Done when: `AuthMethod` 不再派生敏感字段 `Debug`，所有携带凭证的变体只输出 method kind 与固定 `[REDACTED]`. Verify: `git diff -- src/auth/types.rs`; `cargo test auth::types --all-features`。
- [ ] `SP957-T5` Owner: implementation owner. Dependencies: `SP957-T4`. Done when: JWT、API key、session、`None` 均有精确输出测试；每个 secret 变体用至少两个不同输入证明输出相同，并覆盖空串、Unicode/换行和 `[REDACTED]`. Verify: `cargo test auth::types --all-features`。
- [ ] `SP957-T6` Owner: review owner. Dependencies: `SP957-T4`, `SP957-T5`. Done when: 定向搜索未发现日志绕过安全 formatter 直接读取 `AuthMethod` 内部字段；独立 session 日志保持由 #969 跟踪. Verify: `rg -n "AuthMethod|Authenticating request" src/auth src/server` 并人工审查命中。
- [ ] `SP957-T7` Owner: verification owner. Dependencies: `SP957-T4`, `SP957-T5`, `SP957-T6`. Done when: 固定 Rust toolchain下格式、编译、lint、全量测试与 scope/overlap guards 通过. Verify: `cargo fmt --all -- --check`; `cargo check --all-features`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`; `bash scripts/guards/check_pr_scope.sh`; `bash scripts/guards/check_pr_overlap.sh`。

## 并行拆分

- 单文件实现与同文件测试必须由同一 implementation owner 串行完成，避免共享文件写冲突。
- review owner 只读，可在实现提交后独立检查 diff 与日志搜索结果。
- `SP957-T2` 是强制 human gate，不与 agent lane 并行替代。

## 验证

- [ ] `SP957-T8` Owner: human maintainer/security reviewer + merge-review owner. Dependencies: `SP957-T7`. Done when: 人类 auth/security review 覆盖当前 PR head；CI 全绿；无 unresolved review threads；merge state clean；最终 PR 使用 `Fixes #957`. Verify: `gh pr view <pr> --json headRefOid,isDraft,mergeStateStatus,mergeable,reviewDecision,statusCheckRollup,closingIssuesReferences`; `gh api graphql -f query='query { repository(owner:"majiayu000", name:"litellm-rs") { pullRequest(number:<pr>) { reviewThreads(first:100) { nodes { isResolved } } } } }'`; gate 查询时间与 head SHA 写入 evidence。

## Handoff Notes

- Spec PR 使用 `Refs #957`，保持 Draft 直到 `SP957-T2` 人类 spec/security approval，不能提前关闭 issue。
- 最终 implementation PR 满足全部 acceptance criteria 后才使用 `Fixes #957`。
- #958、#959、#960 与 #961 不属于本 issue，不得重新打包进同一实现 PR。
