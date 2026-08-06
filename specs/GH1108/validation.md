# Validation Contract

## Linked Specs

- Product contract: [`product.md`](product.md)
- Architecture and implementation ownership: [`tech.md`](tech.md)
- Execution tasks and handoff gates: [`tasks.md`](tasks.md)

This file owns the complete product-to-test mapping, verification data flow, test plan, and
versioned exact-head coverage-checker contract for GH1108. Moving these requirements out of
`tech.md` does not weaken or defer them.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | neutral exact records + Developer overlay | `cargo test --locked google_model_catalog_2026_07_exact_ids`；大小写/前后缀负例均不命中。 |
| B-002 | `gemini35.rs`/`gemini36.rs` limits 与 paid Standard pricing | `cargo test --locked google_model_catalog_2026_07_metadata`；断言 Developer paid Standard per-million 与 stored per-1k/cost，并断言 Batch/Flex/Priority 无同值声明。 |
| B-003 | Developer evidence filter | `cargo test --locked google_model_catalog_2026_07_dispositions`；retired/shutdown/unverified/other-product 不公开。 |
| B-004 | frozen 17-ID disposition ledger | `cargo test --locked google_model_catalog_2026_07_dispositions` 对 17 行 exact ID/disposition/full URL set/reviewed_at/reason 逐字段 exact-equal；`gemini-2.0-flash-thinking-exp` shutdown date/reason 精确为 2025-12-02；七个 available 继续公开，六个 shutdown、一个 retired、三个 unverified 不公开，unverified 不得由实现自行升级。 |
| B-005 | shared chat/native sampling normalizers | `cargo test --locked gemini_2026_07_deprecated_sampling_rejected` 与 `cargo test --locked gemini_native_2026_07_preflight`；chat typed temperature/top_p omitted/JSON-null 均为 absent，flattened top_k null 被消费；native generationConfig temperature/topP/topK null 被消费；两 lane 的任一 non-null 均 budget/network 前 error，final JSON 四种 key 均不存在。 |
| B-006 | one-shot chat normalization + native terminal-content preflight | `cargo test --locked gemini_2026_07_prefill_rejected`、`cargo test --locked gemini_native_2026_07_preflight` 与 `cargo test --locked gemini_public_entrypoint_contract`；chat fixture 覆盖 interleaved System/Developer、non-text rejection 与 final-model cases；native 4 prefix × unary/stream 覆盖 missing/empty/malformed contents、trailing blank、terminal explicit user/omitted role success、explicit model/null/unknown-string/other-non-string role与 ambiguous shape rejection、systemInstruction 不遮蔽 model、negative network=0；chat/completions unary+stream 与 Responses sync/stream/background 都命中 shared selected-provider gate；Batch/其他 capability source invariant 证明 non-routable。 |
| B-007 | exact positive param allowlist + Responses two-stage provenance + post-selection/cache-safe stream metadata + sink parity | `cargo test --locked gemini_2026_07_request_contract_parity`、`cargo test --locked gemini_2026_07_responses_contract`、`cargo test --locked gemini_2026_07_stream_metadata` 与 `cargo test --locked gemini_2026_07_cache_hit_preflight`；provider/preflight/map/serializer param-name 集合精确等于 `{max_tokens, stop, stream}`，逐项断言 maxOutputTokens/stopSequences/stream transport sink，并断言 temperature/top_p/top_k/tools/tool_choice/response_format/max_completion_tokens 不在集合；Responses sync/stream/background adapter 的 canonical dual token fields 保持现状，typed non-serialized provenance 不进入 request/extra_body/upstream；`responses.rs` 与 `responses/lifecycle_tests.rs` 的全部 `ResponsesApiRequest` 完整 struct literals 必须随 DTO provenance 字段同步、保持编译，并显式初始化 missing provenance fixture；只有 final selected exact GH1108 Gemini direct/alias/fallback 才拒绝 non-null top_k、接受 null/omitted，并单次归一化 Responses token origin，OpenAI/OpenAI-like alias/fallback、其他 provider 与其他 Gemini 的字段和值及 serialized upstream 与 baseline 相等；wire unknown/non-bool 在 DTO boundary 拒绝但不消费合法 object；existing builder 生成、只含 include_usage=true 的 canonical metadata 在 pre-selection 不被 take/drop，且无额外 usage preference state；final selected Gemini exact identity 才消费 canonical `include_usage=true` 并到达 `ChatCompletionStream`，upstream body 无 stream_options/include_usage；OpenAI/OpenRouter canonical hook input/output 逐字段相等，selection failure 不修改原请求；cache regression 证明旧 key policy 会删除 stream_options，但 lookup/store 在 key access 前 safe bypass，已填充 cache 也不能让 selected new Gemini 的 non-stream + metadata 绕过 typed error/network=0，合法无 metadata cache hit 保持。 |
| B-008 | neutral + embedded runtime paid Standard pricing parity | `cargo test --locked gemini_2026_07_cost`、`cargo test --locked gemini_2026_07_runtime_pricing` 与 `cargo test --locked gemini_2026_07_runtime_spend`；两个 prefixed Developer rows/values/limits/source exact，provider=gemini unprefixed exact lookup 可解析且 chat/native reserve+settle fixed usage 与 neutral cost 相等；provider=vertex_ai、unprefixed row、Batch/Flex/Priority 均 fail closed/未声明，unknown policy snapshot 不变。 |
| B-009 | immutable stable snapshot | `cargo test --locked google_model_catalog_2026_07_stability`；重复/并发查询结果完全相等、升序、无重复。 |
| B-010 | closed opt-in/credential actual-env matrix | `cargo test --locked live_gemini_gate_matrix`；13 个 `env_clear` + exact-env 子进程逐行覆盖 GOOGLE/GEMINI aliases、GOOGLE precedence、各 alias 对 Vertex precedence、Vertex pair/service-account/partial pair rejection 与 disabled/missing-key paths；只有 opt-in+Developer key 命中预期 fake source，其他 counter=0；普通 parallel tests 零 `set_var`/`remove_var`，并行 child counter/env/artifact 无交叉泄漏。 |
| B-011 | deterministic failure classification + complete paginated list evidence + typed live observations/keyed aggregation | `cargo test --locked live_gemini_failure_mapping_precedence`、`cargo test --locked live_gemini_list_pagination`、`cargo test --locked live_gemini_result_classification` 与 `cargo test --locked live_gemini_observations`；table-driven fixture 穷举 execution-terminal event、Google reason、ProviderError variant、HTTP fallback 与 local protocol sources，证明 classification precedence/first-execution-terminal-event-wins 且无 substring branch；list fixture 从 empty token 遍历至无 next token，覆盖 later-page match、repeated/malformed token、page failure、100-page bound 与 cross-page duplicate；两个 exact IDs 各有 static/list/get/minimal-call 共 8 个 per-model required records，完整 traversal 派生的两条 list observations分别保持 requested_exact_id/model/page-count/exact-match 一致并独立 hash/persist；auth/timeout/cancel 无响应不构造 observation，partial 只含真实 facts；hash-only/缺字段 passed record 被拒；transport deadline 精确为 failed/network/transport_timeout，natural missing `(step, model)` 精确为 failed/protocol/missing_required_step。 |
| B-012 | credential redaction including tracing | `cargo test --locked live_gemini_redaction` 安装 captured tracing subscriber；sentinel 在 tracing/stdout/stderr/error Display+Debug/config+result Debug/artifact 中零命中。 |
| B-013 | Vertex overlay/read paths | `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" && test -z "$(git diff --name-only "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" -- ':(glob)src/core/providers/vertex_ai/**')"` 必须零输出/零退出；synthetic prohibited-path fixture 证明任一 Vertex production path 使 gate 非零退出，catalog snapshot fixture 独立证明 neutral record 的 Vertex overlay 未变。 |
| B-014 | two-axis execution/finalization outcome + durable incremental persistence | `cargo test --locked live_gemini_runner_cancellation_persists_incrementally`、`cargo test --locked live_gemini_cancel_then_final_persist_failure`、`cargo test --locked live_gemini_artifact_sink` 与 `cargo test --locked live_gemini_observation_canonicalization`；正常 cancel flush 返回 committed incomplete；cancel 后 final persist failure 返回 typed uncommitted failed/protocol/artifact_persistence_failed 且 execution_terminal 保留 externally_cancelled、last snapshot reloadable/bytes 不变、无 false final artifact/no later network；none/deadline/interruption × persisted/failure table 全覆盖；其余 atomic path/permission/retention/new-run-id/digest invariants 保持。 |
| B-015 | existing model and Responses compatibility | `cargo test --locked gemini_provider`、migration snapshot 与 `cargo test --locked gemini_2026_07_responses_contract`；只允许 fixture 声明的 advertised-ID delta，并锁定 OpenAI/OpenAI-like alias/fallback、其他 provider 与其他 Gemini 的 Responses canonical dual token fields、values、serialization 与 upstream payload baseline parity。 |
| B-016 | evidence manifest validation | `cargo test --locked google_model_catalog_2026_07_evidence`；missing/conflicting/stale/unofficial evidence 初始化失败。 |
| B-017 | provider-callable public capability + model-feature closed sets + exact-model dispatch | `cargo test --locked google_model_catalog_2026_07_metadata` 与 `cargo test --locked gemini_2026_07_capability_dispatch`；分别对两个模型断言 capability=`{ChatCompletion, ChatCompletionStream, GeminiGenerateContent}`、supports_tools=false、feature=`{MultimodalSupport, StreamingSupport, SystemInstructions}` 集合相等，并以 chat/unary-stream native selector、image inlineData/stream endpoint/systemInstruction source fixtures 证明正向 sink；显式断言 ToolCalling/FunctionCalling/JsonMode、ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding、CodeExecution/BatchProcessing/Realtime、Computer Use、audio/image generation、Live/Interactions 均未广告；`supports_capability_for_model` 以 final exact model 查询 neutral registry，使两个新 ID 不可被 ToolCalling/FunctionCalling route 选择，同时无 exact record 的既有 Gemini model 仍回落 provider-wide behavior。 |

## Verification Data Flow

```text
official Developer evidence fixture (offline, immutable)
  -> GH1112 neutral Google catalog validation
  -> Developer availability filter
  -> stable Gemini models()
  -> neutral paid Standard cost facts

embedded model_prices_extended exact Developer rows
  -> provider-aware PricingService(provider=gemini, exact ID)
  -> OpenAI chat / legacy completions / Responses reservation + settlement
  -> native generateContent / streamGenerateContent reservation + settlement
  -> provider-aware pricing endpoint
  -> Vertex lookup remains missing

OpenAI-compatible chat / legacy completions / Responses request
  -> Responses only: two-stage route-local contract
       -> capture trusted top_k Missing/Null/Value + max_output_tokens origin
          in typed non-serialized provenance
       -> preserve existing canonical max_tokens + max_completion_tokens fields
       -> never place provenance/top_k in extra_body, RequestContext, cache key, or serialization
  -> provider-neutral gateway builder (preserve stream_options)
  -> unary response-cache policy before key access
       -> stream_options present: safe bypass lookup/store
       -> absent: existing cache identity/hit behavior
  -> alias/fallback resolution + final deployment selection
  -> Gemini exact-model capability dispatch through neutral registry
  -> post-selection token policy
       -> selected exact GH1108 Gemini:
          consume provenance, reject non-null top_k, single-normalize Responses token origin
          + validate/consume stream_options
          + canonical include_usage=true settlement metadata
       -> other provider/model: drop provenance without changing canonical fields/serialization
          and preserve stream_options unchanged
  -> shared model request contract / provider allowlist
       -> supported params
       -> preflight validation
       -> final Developer request body
  -> ChatCompletionStream transport (no provenance/top_k/stream_options in upstream body)
  -> existing Developer endpoint + query API key

Gemini-native request (4 prefixes x unary/stream)
  -> shared native exact-model preflight
       -> consume null deprecated generationConfig fields / reject non-null
       -> normalize role: explicit user/model; omitted -> user
       -> terminal normalized user passes; explicit model/unknown/ambiguous fails
  -> runtime pricing reservation
  -> GeminiProvider defense-in-depth idempotent native preflight
  -> generateContent / streamGenerateContent transport

explicit live opt-in + Developer credential
  -> per-model static snapshot (2 records)
  -> one official list response -> two independently keyed list observations
  -> per-model get (2 records)
  -> per-model minimal call (2 records)
  -> typed attempt/termination + optional redacted observations/digests
  -> await durable/temp-injected artifact sink after every logical step
  -> aggregate by closed 8-key (step, model) set
  -> result (pass | incomplete | classified failure)
```

正常构建、单元测试和运行时不读取远端模型目录，也不根据 live smoke 修改 catalog。

## Test Plan

- [ ] Catalog: `cargo test --locked google_model_catalog_2026_07`
- [ ] Provider contract: `cargo test --locked gemini_2026_07`
- [ ] Responses/capability/cache closures:
      `cargo test --locked gemini_2026_07_responses_contract &&
       cargo test --locked gemini_2026_07_capability_dispatch &&
       cargo test --locked gemini_2026_07_cache_hit_preflight`
- [ ] Public entrypoint/native preflight:
      `cargo test --locked gemini_native_2026_07_preflight &&
       cargo test --locked gemini_public_entrypoint_contract`
- [ ] Embedded runtime pricing:
      `cargo test --locked gemini_2026_07_runtime_pricing &&
       cargo test --locked gemini_2026_07_runtime_spend`
- [ ] Router/network negatives: `cargo test --locked gemini_router_fallback`
- [ ] Offline live-smoke fixtures: `cargo test --locked live_gemini`
- [ ] Manual opt-in:
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
- [ ] Format/build: `cargo fmt --all -- --check && cargo check --locked`
- [ ] Strict lint: `cargo clippy --locked --all-targets -- -D warnings`
- [ ] Full suite: `cargo test --locked`
- [ ] SpecRail:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108 &&
       python3 checks/check_workflow.py --repo .`
- [ ] Diff integrity: `git diff --check`
- [ ] Durable artifact ignore policy:
      `grep -Fx '/artifacts/live/GH1108/' .gitignore &&
       git check-ignore -q artifacts/live/GH1108/probe.json`
- [ ] Remaining read-only routing contexts unchanged:
      `test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       src/server/routes/ai/completions.rs src/server/routes/ai/completions_streaming.rs
       src/server/routes/ai/mod.rs src/server/routes/ai/batches.rs
       src/core/types/chat.rs)"`
- [ ] Vertex production paths unchanged:
      `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" &&
       test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       ':(glob)src/core/providers/vertex_ai/**')"`
- [ ] Coverage checker unit tests:
      `python3 checks/test_gh1108_coverage_gate.py`
- [ ] Coverage artifact:
      `mkdir -p artifacts/coverage/GH1108 &&
       cargo llvm-cov --version > artifacts/coverage/GH1108/cargo-llvm-cov.version &&
       cargo llvm-cov --locked --all-features --workspace --branch --json
       --output-path artifacts/coverage/GH1108/coverage.json`
- [ ] Exact-head coverage gate:
      `python3 checks/gh1108_coverage_gate.py --repo . --base "$IMPLEMENTATION_BASE_SHA" --head "$IMPLEMENTATION_HEAD_SHA" --coverage-json artifacts/coverage/GH1108/coverage.json --tool-version-file artifacts/coverage/GH1108/cargo-llvm-cov.version --output artifacts/coverage/GH1108/gate.json`
- [ ] Coverage CI contract: `.github/workflows/ci-coverage.yml` pins
      `taiki-e/install-action@c44f6b046f1c29ae5918b1e0bfdbb2f1813836fd` and
      `cargo-llvm-cov@0.8.7`, asserts `cargo llvm-cov --version` exactly, and runs the
      GH1108 JSON/checker path for bounded pull-request changes, passing the captured
      one-line version attestation via `--tool-version-file`. The JSON, version attestation,
      and gate result are uploaded with immutable base/head metadata; missing or failed upload cannot be
      green. The pull-request lane uses `github.event.pull_request.head.sha` directly as
      its checkout ref and must not add a `github.sha` fallback; scheduled/manual lanes
      use their separately defined immutable refs. Existing scheduled LCOV/Codecov
      behavior may remain separate.

full suite、strict Clippy、coverage 与 SpecRail gates 在 exact implementation head 各执行
一次；reviewer 默认 inspection/focused，避免重复 full run。

## Versioned exact-head coverage checker contract

implementation PR 必须新增并提交 `checks/gh1108_coverage_gate.py` 与
`checks/test_gh1108_coverage_gate.py`；本 spec PR 不实现 checker，也不需要独立 policy
JSON。checker 必须只读取 Git metadata/diff、pinned LLVM coverage JSON 与独立生成的
tool-version attestation；不得自行执行 coverage 工具，不得扫描 Rust 源码文本、注释
marker、struct literal 字符串或依赖行号 golden。checker 必须验证：

- `IMPLEMENTATION_BASE_SHA`/`IMPLEMENTATION_HEAD_SHA` 是不同的完整 40 位小写 commits，
  base 是 head ancestor，当前 `HEAD` 等于 head，tracked worktree clean，coverage JSON 存在；
- `base...head` changed paths 必须是 [`tech.md`](tech.md) complete planned-changes manifest
  的子集；[`tech.md`](tech.md) 中列出的五个 read-only routing/context files 或
  `src/core/providers/vertex_ai/**` 任一路径变化均 fail closed。Vertex neutral overlay
  是否未变仍由 catalog snapshot fixture 独立证明，不能用 path gate 替代；
- `base...head` changed production Rust executable lines 是非空分母，所有 changed
  production sources 均存在于 coverage JSON，changed-line coverage 至少 80%；
- `--tool-version-file` 必须存在、是普通文件、只含单行精确值
  `cargo-llvm-cov 0.8.7` 且无额外空白/行；coverage JSON malformed、tool version
  attestation 不匹配、required function
  record 缺失/重复、required function 没有 branch region，或任一 required function 的
  branch covered/count 不相等均非零退出；
- 成功 artifact 保存 immutable base/head、changed-line manifest、coverage JSON SHA256、
  tool-version attestation SHA256 与精确值，以及每个 required category 的 path、实际命中的
  LLVM JSON function identity、region
  数与 branch covered/count。

关键行为不能只按 path 归类。checker 内置以下 mandatory function policies；每项必须在
pinned Rust 1.96.1 生成的 LLVM JSON `data[].functions[]` 中按 exact file + deterministic
demangled selector 唯一命中，并以该 function record 自带的 regions/branches 计算覆盖率，
不读取源码。`fn:<name>` 要求 identity 最后一个非 closure path segment 与 name 完全相等
且拒绝 closure；`async_body:<name>` 要求 identity 以完整 segment 序列
`::<name>::{{closure}}` 结尾。两种 selector 都是 segment-anchored exact match，不得使用
substring/contains；同 file 出现零个或多个 match 均 fail closed。这样 async body 的
branches 绑定 generated future，而不是无 branch 的 outer constructor：

| Required category | Path | Deterministic function selector |
| --- | --- | --- |
| `catalog_evidence_validation` | `src/core/providers/google/models/registry.rs` | `fn:validate_developer_catalog_evidence` |
| `deprecated_param_rejection` | `src/core/providers/google/models/request_contract.rs` | `fn:normalize_deprecated_sampling_params` |
| `prefill_rejection` | `src/core/providers/google/models/request_contract.rs` | `fn:normalize_gemini_contents` **and** `fn:validate_no_model_prefill`；两个 function policies 都必需 |
| `native_request_preflight` | `src/core/providers/google/models/request_contract.rs` | `fn:normalize_native_gemini_request` |
| `runtime_pricing_authority` | `src/core/pricing_service/authority_tests.rs` | `fn:gemini_2026_07_runtime_pricing_authority` |
| `responses_provenance_capture` | `src/server/routes/ai/responses.rs` | `fn:responses_request_provenance` |
| `responses_unary_propagation` | `src/server/routes/ai/chat.rs` | `async_body:handle_chat_completion_with_state_and_provenance` |
| `responses_stream_propagation` | `src/server/routes/ai/responses_stream.rs` | `async_body:handle_streaming_response` |
| `responses_background_propagation` | `src/server/routes/ai/responses/lifecycle.rs` | `async_body:handle_background_response`（匹配该函数内直接 spawned async block） |
| `responses_selected_model_normalization` | `src/server/routes/ai/token_policy.rs` | `fn:normalize_selected_gemini_responses_provenance` |
| `model_capability_dispatch` | `src/core/providers/capability_dispatch.rs` | `fn:supports_capability_for_model` |
| `stream_metadata_validation` | `src/server/routes/ai/token_policy.rs` | `fn:prepare_chat_request_for_provider` |
| `cache_hit_preflight` | `src/server/routes/ai/response_cache.rs` | `fn:should_bypass_chat_cache` **and** `src/server/routes/ai/chat.rs` 的 `async_body:handle_chat_completion_internal`；两个 function policies 都必需 |
| `live_classification` | `tests/live_gemini.rs` | `fn:classify_live_failure` |
| `live_redaction` | `tests/live_gemini.rs` | `fn:redact_live_artifact` |
| `live_observation_canonicalization` | `tests/live_gemini.rs` | planned `fn:canonicalize_live_observation` |
| `live_interruption_persistence` | `tests/live_gemini.rs` | planned `async_body:run_live_gemini_smoke` |

GH1112 merged API 若不能采用这些 deterministic function selectors/paths，必须先 amend spec、
manifest 与 checker，不得让 checker 猜 alias。function record 缺失、重复、来自错误 path、
无 branch regions 或存在任一 uncovered branch，都不得满足 policy。每个 category 的所有
function policies 都必须达到 100% branch coverage；`live_classification`、
`live_redaction`、`live_observation_canonicalization` 与
`live_interruption_persistence` 分开判定，不能互相替代。

`checks/test_gh1108_coverage_gate.py` 使用 synthetic Git diff/LLVM JSON fixtures，至少覆盖：

- happy path 与 full-SHA/head/ancestor/tracked-clean/coverage-JSON guards；
- changed path 不在 complete manifest、[`tech.md`](tech.md) 中列出的五个 read-only
  routing/context files 任一变化、任一
  `src/core/providers/vertex_ai/**` 变化均非零退出；synthetic allowed path 仍通过；
- missing changed source、empty denominator、line coverage <80%、malformed JSON、missing/
  non-file/multiline/wrong tool-version attestation、missing/duplicate function、wrong-file
  function、async outer constructor only、ambiguous async closure、zero branch region 与
  uncovered function branch；
- same-path other function 的 covered branch 不能满足任何 category；checker test 还必须
  证明它从不打开 production Rust source 文件；
- catalog、deprecated、prefill、native preflight、runtime pricing authority、
  Responses provenance capture/unary propagation/stream propagation/background propagation/selected-model
  normalization、stream metadata、model capability dispatch、cache-hit preflight 的任一
  selector 缺失或 uncovered 均失败；
- prefill fixtures 覆盖 interleaved System/Developer 原序 parts、developer+user
  instruction/content 均保留、System/Developer non-text/不可表示 payload pre-network rejection、
  assistant+developer final-model rejection，且证明 normalizer/serializer 不二次读 raw roles；
- DTO unit fixtures 独立覆盖 unknown/non-bool wire rejection，证明合法 object 未被消费；
  Responses fixtures 分别覆盖：
  - DTO deserialize-only typed provenance 的 top_k Missing/Null/Value 与
    max_output_tokens origin，且 serde output 不含 provenance；
  - `cargo check --all-targets` 编译全部 in-crate `ResponsesApiRequest` constructors；
    `responses_request_provenance_defaults` behavior fixture 分别通过 unary 与 background
    constructor 构造请求并断言 missing provenance，Serde fixture 独立断言 omitted/null/value；
    不扫描 `ResponsesApiRequest {` 字符串；
  - pre-selection adapter 保持 `max_tokens=Some(value)` 与
    `max_completion_tokens=Some(value)`，top_k 不进入 extra_body；
  - selected exact GH1108 Gemini sync/stream/background direct/alias/fallback 才执行 non-null top_k
    typed invalid-request/network=0、null/omitted absent 和 token single-normalization，
    最终 maxOutputTokens sink 且 upstream 无 top_k/provenance；
  - background non-null top_k 等 selected-model preflight failure 等待 terminal 后断言单次
    store mutation 得到 `status=failed` + stable typed `error.code`/`error.message`，GET 可见、
    network=0，且 cancelled record 不被 late task 覆盖；`error=None` 必须失败；
  - OpenAI exact、OpenAI-like alias/fallback、其他 provider 与其他 Gemini 的 canonical
    field/value 和 serialized upstream baseline parity，selection failure no mutation；
  - Chat Completions extra_body 不能伪造 trusted Responses provenance；
  capability-dispatch fixtures 覆盖两个新 exact IDs 的三项正能力、ToolCalling/
  FunctionCalling 负能力、case/prefix mismatch，以及无 exact neutral record 的既有 Gemini
  model provider-wide fallback；
- stream behavior fixtures 覆盖 selected Gemini direct/alias/fallback 的 canonical
  include_usage=true happy path、selected Gemini internal-inconsistent metadata 与 non-stream
  + stream_options、OpenAI/OpenRouter post-selection input/output equality、selection
  failure no mutation；任何
  conditional consume/preserve/fail-closed branch 未命中均失败；
- cache selectors 必须以真实 cache seed/hit path 覆盖：无 metadata 合法 hit；同 key 的
  non-stream + canonical stream_options 在 key lookup/store 前 bypass；direct/alias/fallback
  最终选中两个新 exact model 时 cache-return=0/network=0/stable invalid-request；旧 key
  canonicalizer 删除 stream_options 的 collision 事实也必须由 fixture 锁定，不能通过换
  prompt/model、清 cache 或修改 identity 避开；
- native selector fixtures 覆盖 exact-model-only、三个 generationConfig fields 的
  absent/null/non-null、contents missing/empty/malformed、trailing blank、terminal
  explicit-user/omitted-role success、explicit-model/null/unknown-string/
  other-non-string/ambiguous rejection、systemInstruction 不遮蔽 terminal model 与 idempotent
  defense-in-depth；
  route fixture 另覆盖 4 prefixes × unary/stream，并证明 budget/network counter=0；
- runtime-pricing selector fixtures 覆盖两个 exact Developer prefixed rows 的 provider-aware
  lookup、fixed usage cost、chat/native reservation + settlement 与 provider=vertex_ai
  missing；unprefixed/Vertex row、wrong unit/source/value 或任一路径落入 unpriced policy
  都 fail closed；
- observation selector fixtures 覆盖 redaction-before-canonicalization、optional pair
  consistency/recomputed digest、auth/timeout/cancel no-response、typed partial facts、
  hash-only/complete-required-for-passed rejection、8 个 per-model required keys 逐项缺失、
  global static/list replacement rejection、完整 paginated traversal 派生的两条
  observations 各自 model/requested_exact_id/pages_fetched/exact-match 一致；pagination
  fixture 覆盖 later-page match、repeated/malformed token、page failure、100-page bound 与
  cross-page duplicate，以及 one-fact-different canonical/digest branches；
- env-gate fixtures 用 closed table 的 13 个 `env_clear` + exact-env child 经过
  production reader，分别覆盖 GOOGLE/GEMINI keys、双 key GOOGLE precedence、各 key
  对 Vertex precedence、Vertex pair/service-account/partial-pair zero-network 与
  disabled/missing-key paths；证明 parallel parent 零 `set_var`/`remove_var`、network
  counters 与 artifact paths 无泄漏；
- classification selector fixtures 穷举完整 source→class table、precedence、
  first-execution-terminal-event-wins 与 no-message-substring branches；
- interruption/persistence selector fixtures 必须由真实 runner 驱动 barrier +
  cancellation token + reloadable artifact sink，覆盖每-step awaited atomic replace、
  cancel flush、remaining incomplete、no-later-network、new-run-id isolation，以及
  none/deadline/cancel/interruption × persisted/persistence_failed closed matrix；特别覆盖
  cancel 后 final flush failure 返回 typed uncommitted
  failed/protocol/artifact_persistence_failed、保留 execution terminal、last committed
  snapshot reloadable/unchanged 且不存在 false final artifact；sink fixtures 另覆盖
  default/override `<run_id>.json`、same-dir temp replace、
  Unix 0600/其他平台 contract、success/interruption retention、path traversal rejection 与
  offline temp sink/default-dir-zero-write；只构造 incomplete record 或只覆盖 sink helper
  不能满足该 category；
- classification、redaction、observation canonicalization、interruption persistence 任一
  missing/uncovered 均失败，其他三类 covered 不能替代缺失类。
