# Task Plan

## Linked Issue

GH-966 / #966

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP966-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011. Owner: coordinator. Dependencies: none. Done when: GH966
  product/tech/tasks 三件套存在，behavior invariant、implementation mapping 与 task coverage
  集合完整，SpecRail workflow 与 write-spec route gate 通过；规格 PR 使用 `Refs #966`
  并经独立 current-head review、CI、threads 与 PR gate 合并。 Verify:
  `python3 checks/check_workflow.py --repo <specrail>`；比较 product/tech/tasks 的
  `B-[0-9]{3}` 集合；GitHub evidence adapter + offline PR gate。
- [x] `SP966-T2` Covers: B-003, B-004, B-011. Owner: Phase A runtime dispatch owner.
  Dependencies: SP966-T1 and staged-plan amendment merged. Done when: typed Gemini native
  capability/request 与 closed `Provider` dispatch 接入 route，实际 HTTP sender 只能是
  selected runtime provider；native Gemini 和名称规范化后精确属于
  `gemini|googleai|googleaistudio` 的 OpenAI-like runtime 使用自身 immutable
  endpoint/key/headers/policy/client/timeout，任意标点或空格名称拒绝；non-success body
  在 provider 边界脱敏 raw/URL-encoded key；route 仅可暂时用 config 构造候选/identity，
  旧 adapter/client 不得参与 send。PR 限 tech Phase A 的 10 文件、最多 500 changed
  lines，使用 `Refs #966`。 Verify: PR #1019 exact-head 六组 feature matrix、focused tests、
  all-feature check、strict Clippy、全量 test、scope/overlap、implementation/security review、
  CI/reviewThreads/required gate 全部通过；合并后 #966 仍 open。
- [x] `SP966-T2R` Covers: B-002, B-007, B-010, B-011. Owner: Phase A endpoint-policy regression
  owner. Dependencies: SP966-T2 and this regression amendment merged. Done when: 在原 PR
  #1021、原分支 `codex/gh966-transport-classification` merge 最新 `origin/main`（禁止
  force push、禁止新建替代 PR）；connection pool 新增仅由 OpenAI-like Gemini native
  sender 使用的 opt-in ordinary/streaming 执行路径，在字符串化前保留直接 outbound URL
  拒绝与 `reqwest::Error` source chain 中的 redirect-target/DNS-rebinding policy 信号；
  policy 错误映射为固定、无敏感数据的 `Configuration`，redirect loop/普通 transport
  仍为 `Network`，timeout 仍为 `Timeout`，其他 provider 与旧 connection-pool 执行方法
  语义不变；`BaseHttpClient` 只可新增 crate-private typed request opt-in，现有方法不变。
  PR 限 tech follow-up 的 6 文件、最多 500 changed lines，使用
  `Refs #966`。 Verify: 真实本地 redirect policy 与 redirect-loop 对照、DNS-rebinding
  source、unsupported scheme、timeout、ordinary/streaming、raw/URL-encoded key 与 endpoint
  不泄露；all-feature check；strict Clippy；全量 test；scope/overlap；exact-head
  implementation + security review PASS；CI/0 unresolved threads/required gate。合并后删除
  远程分支并确认 #966 仍 open。
- [ ] `SP966-T2S` Covers: B-002, B-010. Owner: Phase B prerequisite regression owner.
  Dependencies: SP966-T2R and this prerequisite amendment merged. Done when: 独立 follow-up PR
  仅修改 `tests/gemini_sdk_routes.rs`，只删除/替换不可达的
  `public_only_gemini_route_rejects_loopback_before_connect` 历史 route-time assertion；新断言
  在 runtime provider bootstrap/configuration 阶段证明 `PublicOnly` + loopback 以明确的
  `Configuration`/SSRF 错误 fail closed 且 listener 零连接。不得恢复 route config scan，
  不得接受 405 作为安全成功，不得删除、skip 或弱化 config validation、factory、Gemini
  runtime client、Base HTTP 的底层 SSRF 覆盖。PR 最多 1 个非文档文件、500 changed lines，
  使用 `Refs #966`。 Verify: focused parent integration test；
  `test_ssrf_validation_loopback`；`GeminiConfig::test_policy_client_settings_fail_closed`；
  `base_http_client_rejects_public_loopback_base`；factory endpoint-access tests；全特性构建；
  strict Clippy；全量 test；scope/overlap；exact-head implementation/security review PASS；
  CI/0 unresolved threads/required gate。合并后删除远程分支并确认 #966 仍 open。
- [ ] `SP966-T3` Covers: B-002, B-006, B-009. Owner: Phase B route ownership owner.
  Dependencies: SP966-T2S merged. Done when: 原 PR #1023、原 Phase B 分支 merge prerequisite
  follow-up 后的最新 `origin/main`（禁止 force push、禁止新建替代 PR），再删除 selected deployment 后的
  `state.config().providers()` 反查、route-owned client construction、API key/base
  URL/headers/timeout 复制和旧 Gemini send/error helpers 全部删除；adapter 只保留
  selected provider/pricing identity 与 original requested Gemini model，native URL/budget/spend
  不使用 empty-model named deployment 的 selection key。pre-selection candidate scan 可暂留，
  但不得影响已选 snapshot。PR 仅修改 tech Phase B 的四文件集合、最多 500 changed
  lines，使用 `Refs #966`。 Verify: selected snapshot endpoint/key/header/timeout
  mutation tests；empty-model URL/pricing/model-budget/spend identity tests；focused Gemini
  SDK/fallback/spend tests；六组 feature matrix；strict Clippy；全量 test；scope/overlap；
  exact-head implementation + security review PASS；CI/reviewThreads/required gate。合并后
  删除远程阶段分支并确认 #966 仍 open。不得把 parent test 修改挤入已有 497-line Phase B diff；
  Phase B 四文件 writable scope 与 500 changed-line 上限保持不变。
- [ ] `SP966-T3O` Covers: B-007, B-010. Owner: Phase C prerequisite router-order owner. Dependencies: SP966-T3 and this model-order amendment merged. Done when: 下述 immutable order contract 完整实现。Verify: 下述 focused、repository、review 与 gate checks 在 exact head 通过。
  Implementation details: 从 Phase B 合并后的最新
  `origin/main` 创建独立 prerequisite PR，只修改 `src/core/router/unified.rs` 与
  `src/core/router/tests/router_tests.rs`；`RoutingSnapshot` 在同一 immutable generation 维护
  model-group first-insertion order 并提供 additive ordered read API，add/remove/reindex/同组重复与
  `set_model_list` 都有 router unit regressions；`model_index` lookup、同组 deployment 顺序、alias、
  selection strategy、health/lease/state preservation 保持不变，不加入 config scan 或 Gemini route 特例。
  PR 最多 2 个非文档文件、500 changed lines，使用 `Refs #966`。 Verify: focused router order tests；
  既有 router tests；`cargo fmt --all -- --check`；`cargo check --all-targets --all-features --locked`；
  strict Clippy；全量 test；scope/overlap；exact-head independent implementation review PASS；
  CI/0 unresolved threads/required gate。合并后删除远程前置分支并确认 #966 仍 open；随后原 PR #1026、
  原分支 `codex/gh966-runtime-only-discovery` merge 最新 `origin/main`（禁止 force push、禁止新建替代
  PR），在新 exact head 重跑 Phase C 全部验证。前置 diff 不进入 Phase C 四文件/500-line budget。
- [ ] `SP966-T4` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011. Owner: Phase C runtime-only discovery owner. Dependencies: SP966-T3O
  merged and original PR #1026 branch merged latest `origin/main`. Done when: Gemini candidate model/alias
  只从 immutable router deployments 派生，使用 ordered model-group API 过滤且不得 alphabetic sort，route
  不再读取 `state.config().providers()`；unary/stream snapshot、native + 三命名兼容正例、
  任意名称拒绝、fallback/budget/health/lease/spend、client cancel neutral、read failure 与
  raw/encoded key 脱敏全部覆盖；source guard 拒绝 config scan、`RouteHttpClient`、敏感
  adapter 字段与第二 sender。PR 仅修改 tech Phase C 的四文件集合、最多 500 changed
  lines，并使用 `Fixes #966`。 Verify: focused Gemini SDK/fallback/spend/execution tests；
  source guard；`cargo fmt --all -- --check`; `cargo check --all-targets --all-features
  --locked`; strict Clippy；全量 test；scope/overlap。
- [ ] `SP966-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008,
  B-009, B-010, B-011. Owner: integration verification owner + independent reviewer.
  Dependencies: SP966-T4 exact head ready. Done when: Phase C exact-head CI 全绿、0 unresolved
  review threads、独立 spec-vs-implementation/security review PASS、required PR gate PASS；合并后
  #966 closed、远程实现分支删除，main 与 closure audit 一致。 Verify: GitHub evidence
  adapter；offline `pr_gate.py --mode required`；merged SHA checks；issue/PR queue closure audit。
  旧 full checkpoint 仅作参考，不得作为 alias/cancel/read-failure 或 gate 证据。

## 并行拆分

- T2、T2R、T2S、T3、T3O、T4 是严格串行的 implementation/regression PR；T2R 是 Phase A 合并后暴露的
  endpoint-policy 回归修复，T2S 是 Phase B exact-head 暴露的历史 route assertion 修正；两者都必须在
  T3 前合并；T3O 是 #1026 review 暴露的 immutable model-group order 前置修正，必须先合并，再由原 #1026
  分支 merge 最新 `main`。每阶段必须合并后，下一阶段才继续。
- 每个阶段最多 10 个非文档文件、500 changed lines；三阶段 writable union 为 tech spec
  明列的 11 个文件，T2R 另限定为 tech spec 明列的 6 个文件，T2S 另限定为 parent integration
  test 1 个文件，T3O 另限定为 router core/test 2 个文件；T2S 不扩大 Phase B、T3O 不扩大 Phase C 的
  四文件 scope 或 500-line budget。不修改 799 行的
  `execution.rs` 或 budget API。
- T5 的只读 reviewer/security lane 可与 coordinator 的 final verification 并行；reviewer 不写
  production/test 文件，只在 exact head 给 verdict，并由 reviewer 身份解析 review threads。
- writable worker 使用独立 worktree 且单一 owner；同一阶段不并行修改 `gemini.rs`、
  provider adapter 或 integration fixture。若 scope 超限，先更新并合并规范，不得临时
  扩大文件集合或弱化测试。

## 验证

- Product、tech mapping 与 tasks `Covers:` union 精确为 B-001 至 B-011，无 orphan 或
  undeclared invariant。
- 本 amendment PR 只包含 `specs/GH966/tech.md` 与 `specs/GH966/tasks.md`；不改变 product
  behavior invariants。
- Phase A/T2R/T2S/B/T3O/C 每个 PR 的 fresh exact-head focused、feature matrix、check、
  strict Clippy、全量 test、scope/overlap、CI、reviewThreads 与 offline gate 全部通过；Phase A、
  T2R、T2S、Phase B 与 T3O 合并后 issue 保持 open，Phase C 合并后 #966 closed。

## Handoff Notes

- selected runtime provider 是唯一执行器；禁止再从 Gateway config 复原认证、endpoint 或
  client。
- `GeminiRouteProvider` 只可保留 selected provider name + original requested Gemini model 的
  budget/spend identity，不可保存 API key、base URL、headers、timeout、client，亦不可把
  named deployment model 当作请求 model。
- 兼容范围仅为显式命名 `gemini|googleai|googleaistudio` 的 OpenAI-compatible runtime；
  不得扩大为任意实例。
- upstream error 脱敏必须在持有 runtime key 的 provider 内完成，覆盖 raw 与
  URL-encoded key；route 不得取回 key。
- spec amendment、regression follow-up 与 Phase B 使用 `Refs #966`；仅 Phase C 使用
  `Fixes #966`。
- Phase B prerequisite 只把失效的 route-time loopback assertion 替换为 bootstrap/configuration
  fail-closed evidence；#1023 必须在该 follow-up 合并后 merge 最新 `main`，不得用扩大 Phase B diff、
  压缩测试或恢复 config reconstruction 绕过 500-line gate。
- #1026 的 `discussion_r3587044607` 不改变 canonical empty-model 三字段 identity；
  `discussion_r3587080223` 由 T3O 的 immutable model-group ordered API 与 router regressions 处理。
  T3O 合并后必须在原 #1026 分支 merge 最新 `main`，不得在 Phase C route 内以 alphabetic sort、config scan、
  新 PR 或扩大四文件 diff 规避顺序 blocker。
