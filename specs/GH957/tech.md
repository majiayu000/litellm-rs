# Tech Spec

## Linked Issue

GH-957 / #957

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Auth method type | `src/auth/types.rs:38-48` | `AuthMethod` 派生 `Debug`，字符串字段被逐值输出 | 凭证泄露根因 |
| Authentication entry | `src/auth/system.rs:61-70` | `debug!("Authenticating request: {:?}", auth_method)` | live 日志触发点 |
| Type tests | `src/auth/types.rs` 的 `#[cfg(test)]` 模块 | 没有凭证格式化安全回归测试 | 需要锁定不变量 |

## 设计方案

1. 从 `AuthMethod` 移除派生 `Debug`，保留 `Clone`。
2. 为 `AuthMethod` 手写 `std::fmt::Debug`：
   - `Jwt(_)` 输出可识别的 `Jwt("[REDACTED]")`；
   - `ApiKey(_)` 输出可识别的 `ApiKey("[REDACTED]")`；
   - `Session(_)` 输出可识别的 `Session("[REDACTED]")`；
   - `None` 输出 `None`。
3. 不读取凭证内容生成 prefix、suffix、hash、长度或其他派生信息。固定占位符是唯一字段输出。
4. 保留 `AuthSystem::authenticate` 的现有日志事件；其 `{:?}` 将自动使用安全实现，避免在调用点重复
   redaction 逻辑。
5. 在 `src/auth/types.rs` 内添加两个唯一命名的聚焦测试：
   `auth_method_debug_exact_output_for_all_variants` 对 `Jwt`、`ApiKey`、`Session`、`None` 的完整字符串
   使用 `assert_eq!`；`auth_method_debug_exact_output_for_boundary_secrets` 对三个 secret 变体各使用至少
   两个不同输入，并覆盖空字符串、Unicode/换行符和 `[REDACTED]`。两个测试都只能使用完整字符串相等
   oracle，不能用 `contains`、prefix/suffix 或 snapshot 代替。不同输入同输出共同排除 hash、长度和其他
   输入派生信息。
6. 对 `src/**/*.rs` 全部 production Rust sources 做定向搜索，检查 `AuthMethod` 值、内部字段及认证入口
   是否被日志或格式化调用绕过安全 formatter。审查证据必须逐项处置每个相关命中，记录
   `path:line`、用途、是否到达输出 sink、结论与跟踪 issue；不能用“人工看过”概括。已确认的独立
   session identifier 日志只有在逐项 referral 到 #969 后才能保持 scope split；该 referral 只是分类证据，
   不代表 #969 已实现、验证或通过。

## Implementation Preconditions

1. 维护者必须明确处置本 issue 的 human gate。当前处置是维护者 `lifcc` 在 2026-07-12 当前会话中的
   原话 `你可以merge 放开 humangate`；它授权本次 implx run 对剩余 maintainer-only authorization 和
   disposition gates 使用保守默认值，并授权证据齐全后的 merge。该 waiver 不替代 CI、current-head
   独立 reviewer、review threads、PR gate、merge state 或 runtime gate。
2. `product.md`、`tech.md` 与 `tasks.md` 必须获得绑定 spec PR 最终 current head 的独立只读安全审查。
   reviewer lane 不能是 coordinator，且必须记录 native thread ID、head SHA、verdict 和 unresolved findings；
   spec PR gate 与 runtime ledger gate 也必须通过。
3. Draft PR #954 已修改 `src/auth/types.rs` 并包含同一实现候选。开始实现前必须验证 #954 仍为 `CLOSED`，
   并验证没有 open PR 修改 `src/auth/types.rs`。不得为了补造历史 marker 或 timestamp 而 reopen/close #954。
4. 最终 implementation PR 在合并前必须再次取得绑定 implementation current head 的独立 auth/security
   reviewer verdict；spec reviewer evidence 不能复用。该 verdict 与标准 PR gate 共同保留安全审查边界。

## Fail-closed Gate Contracts

### `GH957-GATE-MAINTAINER-WAIVER`

- waiver evidence 必须位于本次运行的本地 `.specrail/runtime/current.json`，不得提交进仓库，也不得声称
  它是 GitHub `APPROVED` review。
- evidence 必须精确记录 actor、source、原话、记录时间、授权 scope，以及仍保留的六类 gate。缺失、改写、
  扩大 scope 或将 waiver 当作 independent review 都失败。
- GH957 disposition 必须精确为 `GH957-MAINTAINER-WAIVER-2026-07-12` /
  `WAIVE_NON_AUTHOR_HUMAN_APPROVAL`，scope 只覆盖 `SP957-T2` 与 `SP957-T8` 的 non-author human
  `APPROVED`-review conjunct，并明确列出它不构成 human review 或 GitHub `APPROVED` review。
- waiver 是 run-scoped maintainer decision，不绑定某个 head；spec 和 implementation 各自仍必须取得新的
  current-head independent reviewer evidence。

Canonical command：

```bash
set -euo pipefail
: "${WAIVER_EVIDENCE:?set WAIVER_EVIDENCE to the current runtime checkpoint}"
date -u +%Y-%m-%dT%H:%M:%SZ
jq -e '
  .intent_contract.maintainer_human_gate_waiver as $waiver
  | ($waiver.issue_dispositions | map(select(.issue == 957)) | .[0]) as $gh957
  | ($waiver.actor == "lifcc")
    and ($waiver.source == "current conversation")
    and ($waiver.quoted_authorization == "你可以merge 放开 humangate")
    and ($waiver.recorded_at | test("^2026-07-12T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
    and ($waiver.scope | contains("remaining maintainer authorization"))
    and ($gh957.waiver_id == "GH957-MAINTAINER-WAIVER-2026-07-12")
    and ($gh957.decision == "WAIVE_NON_AUTHOR_HUMAN_APPROVAL")
    and ($gh957.task_scope == [
      "SP957-T2",
      "SP957-T8 non-author human APPROVED-review conjunct only"
    ])
    and (["human review", "GitHub APPROVED review"] - $gh957.not_evidence_of | length == 0)
    and ([
      "fresh CI",
      "independent current-head reviewer lane",
      "resolved review threads",
      "clean merge state",
      "offline PR gate",
      "runtime ledger gate"
    ] - $waiver.preserved_gates | length == 0)
' "$WAIVER_EVIDENCE"
```

### `GH957-GATE-SUPERSESSION-STATE`

- #954 必须为 `CLOSED`，其远端文件列表必须包含 `src/auth/types.rs`，证明它确实是被处置的重叠候选。
- 既有 OWNER disposition comment 的 database ID 必须为 `4937803331`，其时间不晚于 `closedAt`，正文必须
  指向 #957 replacement 并明确本 PR 不合并；comments/files pagination 必须闭合。
- 创建 implementation diff 前，所有 open PR 的 `src/auth/types.rs` overlap set 必须精确为空；API error、
  null 或 pagination 都失败。
- 该 gate 验证当前远端状态，不通过 reopen/close 修改历史来制造 closure-time evidence。

Canonical command：

```bash
set -euo pipefail
gh api graphql -f query='query {
  repository(owner:"majiayu000", name:"litellm-rs") {
    old: pullRequest(number:954) {
      state
      closedAt
      files(first:100) {
        pageInfo { hasNextPage }
        nodes { path }
      }
      comments(first:100) {
        pageInfo { hasNextPage }
        nodes { databaseId body createdAt authorAssociation }
      }
    }
    pullRequests(states:OPEN, first:100) {
      pageInfo { hasNextPage }
      nodes {
        number
        files(first:100) {
          pageInfo { hasNextPage }
          nodes { path }
        }
      }
    }
  }
}' | jq -e '
  .data.repository as $repo
  | $repo.pullRequests as $prs
  | $repo.old as $old
  | ($old != null)
    and ($old.state == "CLOSED")
    and ($old.closedAt != null)
    and ($old.files.pageInfo.hasNextPage == false)
    and any($old.files.nodes[]; .path == "src/auth/types.rs")
    and ($old.comments.pageInfo.hasNextPage == false)
    and any($old.comments.nodes[];
      .databaseId == 4937803331
      and .authorAssociation == "OWNER"
      and .createdAt <= $old.closedAt
      and (.body | contains("#957"))
      and (.body | contains("不合并本 PR"))
    )
    and ($prs != null)
    and ($prs.pageInfo.hasNextPage == false)
    and all($prs.nodes[]; .files.pageInfo.hasNextPage == false)
    and ([
      $prs.nodes[]
      | select(any(.files.nodes[]; .path == "src/auth/types.rs"))
      | .number
    ] | length == 0)
'
```

### `GH957-GATE-EXACT-TESTS`

每个 `-- --exact` 执行前必须先在 `set -euo pipefail` 下列出 tests，并用 `grep -Fqx` 证明完整 test path
以 `: test` 存在。listing、grep 或 exact run 任一步失败都阻断；宽 filter、部分匹配和零测试成功均无效。

```bash
set -euo pipefail
cargo test --all-features --lib -- --list \
  | grep -Fqx 'auth::types::tests::auth_method_debug_exact_output_for_all_variants: test'
cargo test --all-features --lib \
  auth::types::tests::auth_method_debug_exact_output_for_all_variants -- --exact

cargo test --all-features --lib -- --list \
  | grep -Fqx 'auth::types::tests::auth_method_debug_exact_output_for_boundary_secrets: test'
cargo test --all-features --lib \
  auth::types::tests::auth_method_debug_exact_output_for_boundary_secrets -- --exact
```

### `GH957-GATE-PR-READY`

- implementation diff 的精确 allowlist 只有 `src/auth/types.rs`，以刷新后的 `origin/main...HEAD` 和 PR
  files API 双重比较；`scripts/guards/check_pr_scope.sh` 只提供信息，因为它无条件 `exit 0`，不能作为
  scope assertion。
- PR 必须 non-draft，remote `headRefOid` 等于被验证的本地 `HEAD`，`mergeStateStatus == CLEAN` 且
  `mergeable == MERGEABLE`。PR body 必须有独立一行 `Fixes #957`，且 `closingIssuesReferences` 包含 #957。
- 预期 check set 为非空的 `Lint`、六个 `Feature Matrix (...)` jobs、`Test`、`Security Audit` 与
  `Compile-check all features (including disabled modules)`。每个预期 check 必须出现并成功，所有额外
  check 也必须处于成功终态；`statusCheckRollup.contexts(first:100)` 必须没有下一页。空集合、pending、
  skipped、neutral、cancelled、未知类型或 pagination 均失败。
- GraphQL `reviewThreads(first:100)` 必须没有下一页且 unresolved count 精确为 0；最终 open-PR/files
- GraphQL `reviewThreads(first:100)` 必须没有下一页且 unresolved count 精确为 0；最终 open-PR/files
  查询也必须没有下一页，且 `src/auth/types.rs` overlap set 精确只有当前 implementation PR。随后必须对
  同一 `headRefOid` 收集独立 reviewer verdict，并重新执行 `GH957-GATE-MAINTAINER-WAIVER`。任何
  API/query/error/null 都失败，不允许降级为 warning。

Canonical command（随后必须用同一 `HEAD_SHA` 收集 independent review 和 waiver evidence）：

```bash
set -euo pipefail
: "${PR:?set PR to the numeric implementation pull request}"
git fetch --no-tags origin main
HEAD_SHA=$(git rev-parse HEAD)
EXPECTED_CHECKS='[
  "Lint",
  "Feature Matrix (lite, lite)",
  "Feature Matrix (sqlite-redis, sqlite,redis)",
  "Feature Matrix (postgres-redis, postgres,redis)",
  "Feature Matrix (sqlite-redis-observability, sqlite,redis,metrics,tracing)",
  "Feature Matrix (postgres-redis-s3-observability, postgres,redis,s3,metrics,tracing)",
  "Feature Matrix (current-ci-bundle, postgres,sqlite,redis,s3,metrics,tracing,websockets,analytics)",
  "Test",
  "Security Audit",
  "Compile-check all features (including disabled modules)"
]'
diff -u \
  <(printf '%s\n' src/auth/types.rs) \
  <(git diff --name-only origin/main...HEAD | LC_ALL=C sort)
date -u +%Y-%m-%dT%H:%M:%SZ
gh pr view "$PR" --repo majiayu000/litellm-rs \
  --json headRefOid,author,isDraft,mergeStateStatus,mergeable,body,files,closingIssuesReferences \
| jq -e --arg head "$HEAD_SHA" '
  . as $pr
  | ($pr.headRefOid == $head)
    and ($pr.isDraft == false)
    and ([ $pr.files[].path ] | sort == ["src/auth/types.rs"])
    and ($pr.mergeStateStatus == "CLEAN")
    and ($pr.mergeable == "MERGEABLE")
    and ($pr.body | test("(?im)^\\s*Fixes\\s+#957\\s*$"))
    and any($pr.closingIssuesReferences[]?; .number == 957)
'

gh api graphql -F pr="$PR" -f query='query($pr:Int!) {
  repository(owner:"majiayu000", name:"litellm-rs") {
    pullRequest(number:$pr) {
      headRefOid
      commits(last:1) {
        nodes {
          commit {
            oid
            statusCheckRollup {
              contexts(first:100) {
                pageInfo { hasNextPage }
                nodes {
                  __typename
                  ... on CheckRun { name status conclusion }
                  ... on StatusContext { context state }
                }
              }
            }
          }
        }
      }
      reviewThreads(first:100) {
        pageInfo { hasNextPage }
        nodes { isResolved }
      }
    }
    pullRequests(states:OPEN, first:100) {
      pageInfo { hasNextPage }
      nodes {
        number
        files(first:100) {
          pageInfo { hasNextPage }
          nodes { path }
        }
      }
    }
  }
}' | jq -e \
  --arg head "$HEAD_SHA" \
  --argjson pr_number "$PR" \
  --argjson expected "$EXPECTED_CHECKS" '
  .data.repository as $repo
  | $repo.pullRequest as $pr
  | $pr.commits.nodes as $commits
  | $commits[0].commit as $commit
  | $commit.statusCheckRollup.contexts as $contexts
  | [
      $contexts.nodes[]
      | if .__typename == "CheckRun" then
          {name: .name, success: (.status == "COMPLETED" and .conclusion == "SUCCESS")}
        elif .__typename == "StatusContext" then
          {name: .context, success: (.state == "SUCCESS")}
        else
          {name: null, success: false}
        end
    ] as $checks
  | ($pr != null)
    and ($pr.headRefOid == $head)
    and ($commits | length == 1)
    and ($commit.oid == $head)
    and ($contexts.pageInfo.hasNextPage == false)
    and ($expected | length > 0)
    and ($checks | length > 0)
    and (($expected - [ $checks[].name ]) | length == 0)
    and all($checks[]; .success == true)
    and ($pr.reviewThreads.pageInfo.hasNextPage == false)
    and ([ $pr.reviewThreads.nodes[] | select(.isResolved == false) ] | length == 0)
    and ($repo.pullRequests.pageInfo.hasNextPage == false)
    and all($repo.pullRequests.nodes[]; .files.pageInfo.hasNextPage == false)
    and ([
      $repo.pullRequests.nodes[]
      | select(any(.files.nodes[]; .path == "src/auth/types.rs"))
      | .number
    ] | sort == [$pr_number])
'

# Collect current-head independent reviewer evidence, then run
# GH957-GATE-MAINTAINER-WAIVER without changing HEAD_SHA or PR.
```

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 JWT 固定输出 | `AuthMethod` custom `Debug` | named tests + full-string equality + boundary pair |
| P2 API key 固定输出 | `AuthMethod` custom `Debug` | named tests + full-string equality + boundary pair |
| P3 session 固定输出 | `AuthMethod` custom `Debug` | named tests + full-string equality + boundary pair |
| P4 变体可区分 | custom `Debug` + tests | exact `Jwt`/`ApiKey`/`Session`/`None` strings |
| P5 固定占位符且无派生标识 | custom `Debug` | full-string equality only |
| P6 认证行为不变 | 不修改 auth execution | focused auth tests + full suite |

## 数据流

HTTP credential extraction → `AuthMethod` → `AuthSystem::authenticate` debug event → custom `Debug` →
只输出 method kind + `[REDACTED]` → 原有认证验证流程继续执行。

## 备选方案

- 删除认证 debug 日志：能降低泄露面，但失去 method-level 诊断信息，且其他 `Debug` 调用仍可能泄露，拒绝。
- 在日志调用点手工匹配变体：只保护一个调用点，未来其他 `{:?}` 仍不安全，拒绝。
- 记录凭证 prefix/hash：仍产生可关联或可枚举信息，不符合产品不变量，拒绝。
- 引入通用 secret wrapper：范围大于本 issue，后续可单独设计；本次采用最小 custom `Debug` 修复。

## 风险

- Security: 变更降低凭证落日志风险；custom implementation 必须覆盖所有携带 secret 的变体。
- Compatibility: `Debug` 文本不是稳定公共协议，但依赖精确 debug 字符串的测试可能需要同步更新。
- Maintenance: enum 新增变体会触发 non-exhaustive match 编译失败，强制显式决定日志策略。

## 测试计划

- [ ] Unit: `auth_method_debug_exact_output_for_all_variants` 对 JWT/API key/session/`None` 做完整字符串相等。
- [ ] Unit: `auth_method_debug_exact_output_for_boundary_secrets` 对每个 secret 变体使用至少两个不同/边界输入。
- [ ] Focused: 对两个唯一 test path 分别先执行 fail-closed `-- --list | grep -Fqx '<full-path>: test'`，再使用
  `-- --exact` 执行；全部命令在 `set -euo pipefail` 下运行，避免宽 filter 的空匹配或替代测试通过。
- [ ] Review: 检查 `origin/main...HEAD -- src/auth/types.rs`，确认实现与测试 oracle 均在提交 diff 中。
- [ ] Bypass: 扫描全部 `src/**/*.rs` 并逐项处置每个相关命中。
- [ ] Repository: format、check、strict Clippy、全量 tests、exact scope、overlap 与 fail-closed PR readiness gates。

## 回滚方案

不得 revert 到派生字段输出。若 custom formatter 导致紧急兼容问题，安全回退是临时移除
`AuthSystem::authenticate` 的该 debug event，同时保留 custom `Debug`，随后 forward-fix 安全格式；绝不恢复原始
凭证日志。
