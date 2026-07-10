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
   session identifier 日志只有在逐项指向 #969 后才能保持 scope split。

## Implementation Preconditions

1. `product.md` 与 `tech.md` 必须获得绑定 spec PR 当前 head 的 trusted-human 安全审查。reviewer 必须与
   PR author、agent 和 bot 身份不同，并在 `APPROVED` review body 中包含精确 marker
   `GH957-SPEC-SECURITY-SCOPE-APPROVED`；agent 自检、agent 代交或普通评论不能替代该 gate。
2. Draft PR #954 已修改 `src/auth/types.rs` 并包含同一实现候选。开始实现前必须将 #954 安全地拆分或
   标记 superseded。关闭时的 supersession comment 必须明确写出 PR #954 已 superseded，链接 #957、
   active successor PR 及其 closure-time head；关闭后不得有任何 open PR 修改同一实现文件，T4 才能
   创建新的 implementation diff。
3. 最终 implementation PR 在合并前必须再次取得绑定当前 head 的 trusted-human auth/security review，
   review body 包含精确 marker `GH957-IMPLEMENTATION-SECURITY-SCOPE-APPROVED`。spec approval 不能复用为
   implementation approval。

## Fail-closed Gate Contracts

### `GH957-TRUSTED-HUMAN-LOGIN-ALLOWLIST`

Machine-facing committed declaration: `GH957_TRUSTED_HUMAN_LOGINS_JSON='[]'`。该空数组是当前唯一允许的值；
只有后续 spec commit 才能用真实 human GitHub login 修改它，并且该新 head 必须重新审查。canonical gate
会在自身内部以 `readonly` 重新声明同一值，不读取 runtime reviewer allowlist。

### `GH957-GATE-HUMAN-APPROVAL`

- committed trusted-human login allowlist 的唯一权威值是 canonical command 内的
  `GH957_TRUSTED_HUMAN_LOGINS_JSON='[]'`。当前值显式为空，因此 T2/T8 必须 fail closed。不得通过环境变量、
  CLI 参数或其他 runtime input 增加 reviewer；维护者必须先编辑并提交该 literal，写入真实的 GitHub human
  login，然后让更新后的 spec head 重新通过独立人类审查。当前 spec 不预设或虚构任何 trusted person。
- reviewer login 必须同时属于上述 committed allowlist，且 GitHub `authorAssociation` 为 `OWNER`、
  `MEMBER` 或 `COLLABORATOR`。allowlist membership 不能替代 association/User/non-author/non-bot 检查，
  metadata 缺失也不能降级放行。
- 使用 GraphQL 拉取 PR author、`headRefOid` 和最多 100 条 reviews，并断言没有下一页。至少一条 review
  必须同时满足：`APPROVED`、`commit.oid == headRefOid`、精确 marker、trusted association、actor type 为
  `User`、login 不等于 PR author 且不是 bot/agent。review 必须由该人类 reviewer 本人提交；agent review
  或 agent 对 review 的转述不构成证据。
- gate evidence 必须记录查询 UTC、PR number、`headRefOid`、reviewer login、association、review commit
  与 marker；任一字段缺失或 reviews pagination 未闭合都失败。

Canonical command（`PR`、`EXPECTED_HEAD`、`APPROVAL_MARKER` 由 T2/T8 固定；trusted-human allowlist 只能
通过 committed literal 修改，不能接受 runtime override）：

```bash
set -euo pipefail
readonly GH957_TRUSTED_HUMAN_LOGINS_JSON='[]'
: "${PR:?set PR to the numeric pull request number}"
: "${EXPECTED_HEAD:?set EXPECTED_HEAD to the reviewed 40-character SHA}"
: "${APPROVAL_MARKER:?set APPROVAL_MARKER}"
case "$APPROVAL_MARKER" in
  GH957-SPEC-SECURITY-SCOPE-APPROVED|GH957-IMPLEMENTATION-SECURITY-SCOPE-APPROVED) ;;
  *) exit 1 ;;
esac
test "$(git rev-parse HEAD)" = "$EXPECTED_HEAD"
date -u +%Y-%m-%dT%H:%M:%SZ
gh api graphql -F pr="$PR" -f query='query($pr:Int!) {
  repository(owner:"majiayu000", name:"litellm-rs") {
    pullRequest(number:$pr) {
      author { __typename login }
      headRefOid
      reviews(first:100) {
        pageInfo { hasNextPage }
        nodes {
          state
          body
          commit { oid }
          authorAssociation
          author { __typename login }
        }
      }
    }
  }
}' | jq -e \
  --arg head "$EXPECTED_HEAD" \
  --arg marker "$APPROVAL_MARKER" \
  --argjson trusted_humans "$GH957_TRUSTED_HUMAN_LOGINS_JSON" '
  .data.repository.pullRequest as $pr
  | ($pr != null)
    and ($pr.headRefOid == $head)
    and ($pr.author.__typename == "User")
    and ($pr.reviews.pageInfo.hasNextPage == false)
    and ($trusted_humans | type == "array" and length > 0)
    and ([
      $pr.reviews.nodes[]
      | select(
          .state == "APPROVED"
          and .commit.oid == $head
          and (.body | split("\n") | map(rtrimstr("\r")) | index($marker) != null)
          and .author.__typename == "User"
          and .author.login != $pr.author.login
          and ((.author.login | test("\\[bot\\]$"; "i")) | not)
          and (.author.login as $login | ($trusted_humans | index($login)) != null)
          and (
            .authorAssociation == "OWNER"
            or .authorAssociation == "MEMBER"
            or .authorAssociation == "COLLABORATOR"
          )
        )
    ] | length > 0)
'
```

### `GH957-GATE-SUPERSESSION`

- 关闭 #954 前，先验证 successor PR 为 `OPEN`，记录其 number、URL、40 位 `headRefOid` 与 UTC。
- #954 的 closure-time comment 必须使用 marker `GH957-SUPERSESSION`，明确包含 “PR #954 is
  superseded”，并给出 `https://github.com/majiayu000/litellm-rs/issues/957`、active successor 的完整 PR URL
  和指向上述 head SHA 的完整 commit URL。comment 必须不晚于 `closedAt`；successor 必须已在
  `closedAt` 前创建且当时为 active。
- `PRE_CLOSE_AT` 必须在发布 exact comment 并关闭 #954 的紧邻前一步记录；GraphQL 必须证明
  `PRE_CLOSE_AT <= comment.createdAt <= closedAt`。
- 关闭后 GraphQL 证据必须证明 #954 为 `CLOSED`、marker/comment/timestamp 完整，并证明 open PR 中
  `src/auth/types.rs` overlap set 为空。successor 保持 `OPEN` 且 current head 未变，但它此时不得修改该文件；
  comments、open PRs 或 files 任一连接有下一页即失败。

Canonical closure oracle（comment body 必须精确等于 `EXPECTED_COMMENT`；`PRE_CLOSE_AT` 与 comment/close
命令之间不得插入其他操作）：

```bash
set -euo pipefail
: "${SUCCESSOR_PR:?set SUCCESSOR_PR to the numeric active successor PR}"
PRE_CLOSE_JSON=$(gh pr view "$SUCCESSOR_PR" --repo majiayu000/litellm-rs \
  --json number,state,url,createdAt,headRefOid)
SUCCESSOR_HEAD_AT_CLOSURE=$(printf '%s\n' "$PRE_CLOSE_JSON" | jq -er '
  select(.state == "OPEN") | .headRefOid | select(test("^[0-9a-f]{40}$"))
')
SUCCESSOR_URL="https://github.com/majiayu000/litellm-rs/pull/$SUCCESSOR_PR"
SUCCESSOR_HEAD_URL="$SUCCESSOR_URL/commits/$SUCCESSOR_HEAD_AT_CLOSURE"
EXPECTED_COMMENT="GH957-SUPERSESSION: PR #954 is superseded for https://github.com/majiayu000/litellm-rs/issues/957 by active successor $SUCCESSOR_URL at head $SUCCESSOR_HEAD_URL."
printf '%s\n%s\n' "$PRE_CLOSE_JSON" "$EXPECTED_COMMENT"
PRE_CLOSE_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)
gh pr comment 954 --repo majiayu000/litellm-rs --body "$EXPECTED_COMMENT"
gh pr close 954 --repo majiayu000/litellm-rs
printf '%s\n' "$PRE_CLOSE_AT"

gh api graphql -F successor="$SUCCESSOR_PR" -f query='query($successor:Int!) {
  repository(owner:"majiayu000", name:"litellm-rs") {
    old: pullRequest(number:954) {
      state
      closedAt
      comments(first:100) {
        pageInfo { hasNextPage }
        nodes { body createdAt }
      }
    }
    successor: pullRequest(number:$successor) {
      number
      state
      url
      createdAt
      headRefOid
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
  --argjson successor "$SUCCESSOR_PR" \
  --arg successor_url "$SUCCESSOR_URL" \
  --arg successor_head "$SUCCESSOR_HEAD_AT_CLOSURE" \
  --arg pre_close_at "$PRE_CLOSE_AT" \
  --arg expected_comment "$EXPECTED_COMMENT" '
  .data.repository as $repo
  | $repo.old.closedAt as $closed_at
  | ($repo.old != null)
    and ($repo.successor != null)
    and ($repo.old.state == "CLOSED")
    and ($closed_at != null)
    and ($repo.old.comments.pageInfo.hasNextPage == false)
    and ($repo.pullRequests.pageInfo.hasNextPage == false)
    and all($repo.pullRequests.nodes[]; .files.pageInfo.hasNextPage == false)
    and ($repo.successor.number == $successor)
    and ($repo.successor.state == "OPEN")
    and ($repo.successor.url == $successor_url)
    and ($repo.successor.headRefOid == $successor_head)
    and ($repo.successor.createdAt <= $closed_at)
    and ($pre_close_at | test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
    and any($repo.old.comments.nodes[];
      .body == $expected_comment
      and (($pre_close_at | fromdateiso8601) <= (.createdAt | fromdateiso8601))
      and ((.createdAt | fromdateiso8601) <= ($closed_at | fromdateiso8601))
    )
    and ([
      $repo.pullRequests.nodes[]
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
  查询也必须没有下一页，且 `src/auth/types.rs` overlap set 精确只有当前 implementation PR。随后对同一
  `headRefOid` 执行 `GH957-GATE-HUMAN-APPROVAL`。任何 API/query/error/null 都失败，不允许降级为 warning。

Canonical command（随后必须用同一 `HEAD_SHA` 执行 implementation approval command）：

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

EXPECTED_HEAD="$HEAD_SHA"
APPROVAL_MARKER=GH957-IMPLEMENTATION-SECURITY-SCOPE-APPROVED
export PR EXPECTED_HEAD APPROVAL_MARKER
# Run GH957-GATE-HUMAN-APPROVAL here without changing HEAD_SHA or PR.
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
