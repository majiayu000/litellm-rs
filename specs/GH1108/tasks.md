# Task Plan

## Linked Issue

GH-1108 / #1108

## Spec Packet

- Product: `specs/GH1108/product.md`
- Tech: `specs/GH1108/tech.md`
- Validation: `specs/GH1108/validation.md`
- PR tier: `heavy`
- Spec status: `pending_maintainer`; `implx auto` 只授权起草，不等于 spec approval

## 当前 Gate

本 task plan 可随 spec PR 合并，但 implementation 必须等待 GH1112 production neutral
Google catalog 合并并解除其 `parked` dependency。implementation owner 不得在等待期间把
catalog delta 写入旧 `src/core/providers/gemini/models/**`。此外，maintainer 必须明确
批准本 spec 的最终 commit head；draft、PR open、route gate `allowed` 或旧 head 的批准
均不能满足该 gate。

## 实现任务

- [ ] `SP1108-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017. Owner: spec/coordinator. Dependencies: linked issue; `write_spec` route gate. Done when: detailed completion evidence below is satisfied. Verify: detailed commands below pass.
      Files: `specs/GH1108/product.md`, `specs/GH1108/tech.md`,
      `specs/GH1108/validation.md`, `specs/GH1108/tasks.md`. Done when: product invariants
      are contiguous `B-001..B-017`; validation Product-to-Test Mapping and task `Covers:` union both cover the
      full set; planned-changes manifest is issue=1108/complete=true; official Developer
      sources are recorded，17-row frozen disposition ledger 的 exact
      ID/status/full URL set/reviewed_at/reason 已获批准；manifest 包含 versioned coverage checker/test，validation 已定义
      十七类 exact LLVM function policy（含 native request preflight、runtime pricing
      authority、Responses provenance 五段、model capability dispatch 与 cache-hit
      preflight）和 fail-closed negative fixtures；GH1112 implementation
      dependency and no-Vertex-inference boundary are explicit；最终 spec head 已获得
      maintainer 明确批准，且批准证据绑定该 exact head；本 spec PR footer 保持
      `Refs #1108`，只有后续完成实现与验收的 implementation PR 才使用 `Fixes #1108`。
      Verify:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
      `python3 checks/check_workflow.py --repo .`、`git diff --check`.

- [ ] `SP1108-T2` Covers: B-001, B-002, B-003, B-004, B-008, B-009, B-013, B-015, B-016, B-017. Owner: neutral catalog + runtime pricing implementation owner. Dependencies: SP1108-T1 spec PR and GH1112 implementation merged. Done when: detailed catalog and pricing-authority evidence below is satisfied. Verify: detailed commands below pass.
      Fresh `origin/main`,
      duplicate evidence and `implement` route gate allowed; merged GH1112 paths/API
      match the tech manifest or an amendment is merged first. Files:
      `src/core/providers/google/models/registry.rs`,
      `src/core/providers/google/models/catalog/{mod.rs,gemini35.rs,gemini36.rs}`,
      `src/core/providers/google/models/tests.rs`,
      `config/model_prices_extended.json`,
      `src/core/pricing_service/authority_tests.rs`,
      `src/server/routes/ai/spend_runtime_pricing_tests.rs`,
      `src/server/routes/ai/gemini/spend.rs`; all other Gemini/Vertex consumers read-only.
      Done when: both GA exact IDs、limits、provider-callable exact closed capability/feature
      sets（capability 精确为
      ChatCompletion/ChatCompletionStream/GeminiGenerateContent、supports_tools=false；
      feature 精确为 MultimodalSupport/StreamingSupport/SystemInstructions，并分别绑定
      chat 与 native capability selector、inlineData image/stream endpoint/
      systemInstruction serializer；显式排除
      ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding 以及
      ToolCalling/FunctionCalling/JsonMode）、Gemini
      Developer API paid Standard per-million pricing and official evidence are present
      in the Developer overlay；Batch/Flex/Priority 不复用或宣称该定价；every
      pre-refresh Developer chat ID 的 fixture 与 tech frozen ledger 17 行逐字段
      exact-equal（7 available_exact、6 shutdown、1 retired、3 unverified；
      `reviewed_at=2026-07-26`，其中 `gemini-2.0-flash-thinking-exp` shutdown
      date/reason 精确为 2025-12-02，仅使用表内 ai.google.dev URLs/reasons），implementation
      不得自行分类或升级 unverified；retired/shutdown/unverified entries are not
      advertised; Developer pre/post snapshot is stable, sorted and duplicate-free;
      runtime JSON 只新增 `gemini/gemini-3.6-flash` 与
      `gemini/gemini-3.5-flash-lite` 两个 prefixed Developer rows，
      `litellm_provider=gemini`，per-token values/limits/source 与 tech exact table 相等，
      不新增 unprefixed/Vertex rows；默认 embedded provider-aware lookup 对
      `provider=gemini + unprefixed exact ID` 解析到对应 prefixed row，neutral/runtime
      fixed usage cost exact-equal；OpenAI chat/Responses/legacy completions 与 native
      unary/stream reservation/settlement 均不进入 default unpriced rejection，按
      usage/maxOutputTokens 得到精确 cost；provider=vertex_ai lookup 仍 missing。
      Vertex overlay and production paths are byte-for-byte unchanged. Verify:
      `cargo test --locked google_model_catalog_2026_07`、
      `cargo test --locked gemini_2026_07_cost`、
      `cargo test --locked gemini_2026_07_runtime_pricing`、
      `cargo test --locked gemini_2026_07_runtime_spend`、
      `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" &&
       test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       ':(glob)src/core/providers/vertex_ai/**')"`、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T3` Covers: B-005, B-006, B-007, B-015, B-017. Owner: shared contract + Gemini consumer owner. Dependencies: SP1108-T2 stable committed head. Done when: detailed contract evidence below is satisfied. Verify: detailed commands below pass.
      No other
      writable neutral-catalog owner. Files:
      `src/core/providers/google/models/request_contract.rs`,
      `src/core/providers/gemini/provider.rs`,
      `src/core/providers/gemini/provider_tests.rs`,
      `src/core/providers/gemini/client.rs`,
      `src/core/providers/capability_dispatch.rs`,
      `src/server/routes/ai/gemini.rs`,
      `src/core/models/openai/requests.rs`,
      `src/core/models/openai/responses_api.rs`,
      `src/server/routes/ai/token_policy.rs`,
      `src/server/routes/ai/chat.rs`,
      `src/server/routes/ai/chat_streaming.rs`,
      `src/server/routes/ai/chat_tests.rs`,
      `src/server/routes/ai/response_cache.rs`,
      `src/server/routes/ai/responses.rs`,
      `src/server/routes/ai/responses_stream.rs`,
      `src/server/routes/ai/responses/lifecycle.rs`,
      `src/server/routes/ai/responses/lifecycle_tests.rs`,
      `src/server/routes/ai/responses_stream_tests.rs`,
      `src/core/cache/key_generator.rs`,
      `src/core/cache/key_policy.rs`,
      `src/core/cache/llm_cache.rs`,
      `tests/gemini_router_fallback_routes.rs`,
      `tests/responses_routes.rs`; T2 catalog records read-only；
      `src/server/routes/ai/completions.rs`、`src/server/routes/ai/completions_streaming.rs`、
      `src/server/routes/ai/mod.rs`、`src/server/routes/ai/batches.rs`、
      `src/core/types/chat.rs` 是 read-only context，不在 writable manifest。
      Done when: exact new-model contract removes `temperature`/`top_p`/`top_k` from
      supported params；typed temperature/top_p omitted/JSON-null 按 `Option` absent；
      flattened extra_body/extra_params 的 `top_k: Value::Null` 由 shared normalizer
      消费并删除，任何 non-null 值（包括默认数值）在 auth/network 前拒绝，final body
      无 temperature/topP/top_k/topK；`normalize_gemini_contents` 只执行一次 role/content
      normalization，直接产出 serializer-ready contents/systemInstruction，prefill gate
      与 serializer 都不读取 raw messages；meaningful System+Developer text 按原 messages
      顺序组成 `systemInstruction.parts` 且都不进 contents，System/Developer non-text
      或不可表示 payload 均 pre-network error；developer+user 同时保留 instruction/user；
      assistant+system、assistant+developer 因 final contents=model 拒绝，
      all-system/developer、non-empty model+trailing-empty、all-empty 也拒绝，user+system
      可通过；positive param allowlist 精确等于
      `{max_tokens, stop, stream}`，分别落到 maxOutputTokens/stopSequences/stream
      transport；temperature/top_p/top_k/tools/tool_choice/response_format/
      max_completion_tokens 均排除，provider/preflight/map/serializer set-equality 与 sink
      fixture 通过；`stream_options` 只接受 closed `include_usage: bool` wire metadata，
      DTO boundary 拒绝 unknown/non-bool 但不消费合法 object；shared builder 继续生成
      只含 `include_usage=true` 的既有 canonical core metadata 并保留到 alias/fallback
      后最终 deployment 选定；不得生成、保存或暴露额外 usage preference state；
      `prepare_chat_request_for_provider` 只对 selected Gemini Developer + 两个新 exact ID
      调用 `normalize_selected_gemini_stream_metadata` 并只消费 canonical
      `include_usage=true`；direct/alias/fallback 到新模型的
      canonical fixture 都到达 ChatCompletionStream/chat_completion_stream，Gemini
      upstream body 无 stream_options/include_usage；OpenAI/OpenRouter 在
      post-selection hook 前后值相等，
      selection failure 不修改原请求；wire unknown/non-bool、所选新 Gemini 的 internal
      inconsistent metadata 与 non-stream + stream_options 均 pre-network fail closed，且
      positive allowlist 仍只有三项；Responses DTO 用 deserialize-only typed presence
      capture 区分 top_k Missing/Null/Value，并记录 max_output_tokens origin；provenance
      字段加入后，`responses.rs` 与 `responses/lifecycle_tests.rs` 中两个完整
      `ResponsesApiRequest` literals 都显式初始化 missing provenance 且保持编译；
      作为 non-serialized route-local sidecar，经 `responses.rs`、sync `chat.rs`、
      `responses_stream.rs` 或 background `responses/lifecycle.rs` 到 selected-model
      token policy，不得进入
      ChatCompletionRequest/CoreChatRequest/RequestContext.metadata/extra_body/cache key/
      provider serialization。`build_chat_request` 保持当前 provider-neutral canonical
      行为：max_output_tokens 同时映射 max_tokens 与 max_completion_tokens，top_k 不进
      extra_body；只有 alias/fallback 最终选中 Gemini Developer + 两个 exact GH1108 ID
      时，`normalize_selected_gemini_responses_provenance` 才把 Missing/Null 当 absent、
      non-null 在 budget/network 前拒绝，并把 Responses token origin 单次归一化为
      max_tokens=Some(value)、max_completion_tokens=None，最终 maxOutputTokens sink 与
      budget limit exact。OpenAI exact、OpenAI-like alias/fallback、其他 provider 与其他
      Gemini 丢弃 sidecar且 canonical field/value/serialization 与 baseline 相等；
      selection failure no mutation，所有 upstream 都无 top_k/provenance；Chat request
      extra_body 不能伪造 trusted provenance；sync/stream/background positive/negative
      fixtures 均通过；
      unary cache policy 对任何 non-stream + stream_options 在 key lookup/store 前
      safe bypass，不能依赖当前 key canonicalizer 删除 metadata 后的碰撞；回归先填充同 key
      合法 response，再证明 direct/alias/fallback 最终选中两个新模型时 cache return=0、
      network=0、stable invalid-request，合法无 metadata request 仍可 cache hit；
      native shared normalizer 只对两个 exact IDs
      生效：generationConfig 的 temperature/topP/topK absent/null 被消费、non-null 在
      budget/network 前拒绝；contents missing/empty/malformed 拒绝，trailing blank 被
      跳过；role exact user/model 保留，omitted 按 official Content default normalize
      为 user，terminal explicit-user/omitted 可通过，terminal model 拒绝，explicit
      null/unknown string/其他 non-string/ambiguous sequence fail closed，
      systemInstruction 不遮蔽 model；cleaned body 同时供 native budget 与 transport，
      provider direct defensive call 幂等。公开入口矩阵 fixture 覆盖 chat、legacy
      completions unary/stream 与 Responses sync/stream/background 均命中 selected-provider shared chat gate，
      以及 `/v1`、`/v1beta`、`/gemini/v1`、`/gemini/v1beta` × native unary/stream
      八个 endpoint shape 均命中 native gate 且 network=0；route-table/source invariant
      证明所有 completion aliases 仍绑定同 handler，Batch/其他 capability routes 对
      Gemini chat non-routable。`supports_capability_for_model` 对最终 Gemini exact model
      查询 neutral registry；两个新 ID 只对 ChatCompletion/ChatCompletionStream/
      GeminiGenerateContent eligible，ToolCalling/FunctionCalling route 不可选择；
      无 exact record 的既有 Gemini model 保持 provider-wide fallback，禁止全局删除
      Gemini ToolCalling。既有 Gemini
      ToolResult/ToolUse 序列化与完整 callability 归 GH1111、非本任务 acceptance，且
      GH1108 implementation 不依赖 GH1111；no family-substring inheritance or silent
      drop remains for this contract. Verify:
      `cargo test --locked gemini_2026_07`、
      `cargo test --locked gemini_router_fallback`、network-counter negatives、
      `cargo test --locked gemini_2026_07_responses_contract`、
      `cargo test --locked gemini_2026_07_capability_dispatch`、
      `cargo test --locked gemini_2026_07_cache_hit_preflight`、
      `cargo test --locked gemini_native_2026_07_preflight`、
      `cargo test --locked gemini_public_entrypoint_contract`、
      `test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       src/server/routes/ai/completions.rs src/server/routes/ai/completions_streaming.rs
       src/server/routes/ai/mod.rs src/server/routes/ai/batches.rs
       src/core/types/chat.rs)"`、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T4` Covers: B-010, B-011, B-012, B-014, B-016. Owner: live-smoke test/documentation owner. Dependencies: SP1108-T3 stable committed head. Done when: detailed smoke/redaction evidence below is satisfied. Verify: detailed commands below pass.
      Developer
      credential path unchanged. Files: `tests/live_gemini.rs`,
      `docs/providers/gemini.md`, `docs/providers/README.md`, `.gitignore`; production catalog/provider
      files read-only. Done when: ignored live tests require exactly
      `LITELLM_RS_LIVE_GEMINI=1`；supported Developer aliases 精确为
      `GOOGLE_API_KEY`、`GEMINI_API_KEY`，production precedence 是 GOOGLE 后 GEMINI。
      closed 13-case fixture 不在普通 parallel process 调用 `set_var`/`remove_var`，每
      case 以 `env_clear` + exact env 隔离子进程经过 production reader：disabled
      empty/GOOGLE/GEMINI；enabled empty/GOOGLE/GEMINI/both；enabled Vertex
      project+location、Vertex pair+service account、project-only、location-only；
      enabled GOOGLE+Vertex 与 GEMINI+Vertex。disabled、missing Developer key 与所有
      Vertex-only/partial cases network=0；Developer-positive cases 只命中预期 fake
      source，both 时 GOOGLE 胜出、Developer key 始终先于 Vertex；parallel child
      env/counter/artifact 无泄漏；static/list/get/
      minimal-call steps 写入 closed schema
      `{run_id, model, step, status, error_class, http_status, observed_at,
      termination_reason, attempt, observation?, observation_sha256?}`；attempt 记录
      started/network_attempted/response_received/response_parsed 实际阶段；observation 是
      deny-unknown complete-or-partial tagged union，精确覆盖 static metadata 的
      ID/catalog/lifecycle/limits/pricing/
      supports flags/capability/feature/evidence facts、list 的 returned count + exact matches、get 的
      returned exact resource/methods/limits、minimal-call 的 model version/candidates/
      finish/text/usage facts，以及 aggregate `(step, model)` key sets；passed record 必须
      complete、pair digest 一致，缺 step/model-specific facts、kind 错配、partial/none 或
      hash-only 均 failed/protocol；auth/timeout/cancel 无 response observation 时 optional
      pair 都缺失，不得伪造 success facts，partial 只保存真实 typed facts；aggregate
      required keys 精确包含两个 exact model 各自一次 static/list/get/minimal-call 共
      8 个 per-model keys，sets disjoint/union 完整，缺任一项均 failed/protocol；一次
      official list 从 empty token 遍历所有 pages 直到无 non-empty next token，完整
      traversal 才可派生两条 list observation；各自 key/model/requested_exact_id/
      pages_fetched/case-sensitive exact match 必须一致并独立 hash/persist。later-page
      match 必须命中，repeated/malformed token、任一 page failure、100-page bound 与
      cross-page duplicate 必须 failed/protocol、不得 partial false-pass；redaction 后按 exact ID 不
      case-fold、set-valued fields lexical sort/dedupe、candidate/text 保序、integer
      token counts/fixed-decimal prices、aggregate keys step/model lexical sort、recursive
      lexical JSON keys canonicalize，再对 observation 求 SHA-256；observation/digest
      同生同灭且必须重算匹配，任一事实变化必须改变 canonical observation/digest；
      `classify_live_failure` 按 tech closed table 穷举 exact execution-terminal
      event、structured
      Google reason、typed ProviderError、numeric HTTP fallback 与 local validation：
      auth/quota/not_found/protocol/network 是且仅是五类，禁止 message substring；
      classification precedence 固定为 first execution terminal → structured reason →
      ProviderError → HTTP → local protocol，deadline/cancel race 使用 atomic
      first-execution-terminal-event-wins；403 +
      RESOURCE_EXHAUSTED=quota，runner deadline=failed/network/transport_timeout，remote
      DEADLINE_EXCEEDED=failed/network/step_failed，只有 verified external cancel/
      interruption 为 no-error-class incomplete；`run_live_gemini_smoke` 注入 cancellation
      token、transport barrier 与 reloadable artifact sink，每个 step 必须
      redaction/canonicalization 后 await temp-write + atomic-replace snapshot，再开始下一
      network call；真实 cancel 停止后续网络、把 in-flight/remaining keys 记为 incomplete
      并 await flush。runner closed outcome 区分 Committed 与
      UncommittedFinalizationFailure；none/deadline/cancel/interruption ×
      persisted/persistence_failed 全矩阵确定。cancel final flush 成功时 committed
      incomplete/no-error-class；失败时 typed uncommitted outcome 固定为
      failed/protocol/artifact_persistence_failed，独立 execution_terminal 保留
      externally_cancelled，last committed snapshot reloadable/bytes 不变，不生成 false
      final artifact，且不再发起 network。runner-level fixture reload 后证明
      completed facts/digests 保留、later network=0、aggregate 非 passed；retry 使用不同
      run_id，旧 keys 不聚合；只有无 external terminal event 的 natural missing required
      step 为 failed/protocol/missing_required_step；
      manual production sink 默认写
      `artifacts/live/GH1108/<run_id>.json`，非空
      `LITELLM_RS_LIVE_GEMINI_OUTPUT_DIR` 可覆盖目录；same-directory temp + atomic
      replace、Unix 0600 best-effort/非 POSIX 平台契约、success/interruption retention、
      path/reload 均有 fixture；repository `.gitignore` 新增精确 anchored
      `/artifacts/live/GH1108/`，`grep -Fx` 与 `git check-ignore` 验证通过；offline tests
      注入 temp sink 且默认目录零写入；
      redaction fixture 安装 captured tracing subscriber，sentinel key is absent from
      tracing/stdout/stderr/error Display+Debug/config+result Debug/artifact；docs 给出
      exact opt-in command、artifact 检索/显式清理命令、migration、
      disposition and Developer/Vertex boundary；分类、脱敏、observation canonicalization、
      runner cancellation/persistence
      分别由 exact symbols `classify_live_failure`、`redact_live_artifact`、
      `canonicalize_live_observation`、`run_live_gemini_smoke` 所有。Verify:
      `cargo test --locked live_gemini_failure_mapping_precedence`、
      `cargo test --locked live_gemini_list_pagination`、
      `cargo test --locked live_gemini_runner_cancellation_persists_incrementally`、
      `cargo test --locked live_gemini_cancel_then_final_persist_failure`、
      `cargo test --locked live_gemini_artifact_sink`、
      `cargo test --locked live_gemini`、
      `cargo test --locked --test live_gemini -- --ignored` with no opt-in proves skip/
      zero-network、manual
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
      only when a human supplies credentials、documentation diff review.

- [ ] `SP1108-T6` Covers: B-003, B-005, B-006, B-007, B-008, B-011, B-012, B-014, B-016, B-017. Owner: coverage-checker coordinator. Dependencies: SP1108-T2 through T4 stable committed head; validation exact function-policy contract. Done when: versioned checker and negative fixtures below are implemented before SP1108-T5 read-only review. Verify: checker unit tests and exact-head invocation below pass.
      Files: `checks/gh1108_coverage_gate.py`,
      `checks/test_gh1108_coverage_gate.py`, `.github/workflows/ci-coverage.yml`;
      production/test Rust files read-only.
      Done when: checker enforces full SHA/head/ancestor/tracked-clean/LLVM JSON guards、non-empty
      changed-production denominator、all changed production sources in JSON、changed-line
      ≥80%、changed paths 是 complete manifest 子集；tech 列出的五个 read-only
      routing/context files 与 `src/core/providers/vertex_ai/**` 任一变化均
      fail closed；coverage workflow pin installer SHA + `cargo-llvm-cov@0.8.7` 并把
      GH1108 JSON/checker 接入 bounded PR path；十七个 mandatory categories 均绑定
      validation 表中 exact path + LLVM function identity，不读取 Rust source、comment
      marker 或 struct literal；prefill 的 `normalize_gemini_contents`/
      `validate_no_model_prefill` 两个 function policies 都满足，behavior fixtures 覆盖
      interleaved System/Developer 原序、developer+user 保留、
      System/Developer non-text rejection、assistant+developer final-model rejection，
      native request preflight selector 覆盖 exact-only、sampling absent/null/non-null、
      malformed/empty/trailing-empty contents、terminal explicit-user/omitted success、
      explicit-model/null/unknown-string/其他 non-string/ambiguous rejection、4 prefixes ×
      unary/stream negative network=0 与 provider defensive idempotence；runtime pricing authority
      selector 覆盖两个 prefixed Developer rows、Gemini provider-aware lookup、neutral/
      runtime fixed cost parity、chat/native reserve+settle 与 Vertex missing；
      stream metadata 的 `src/server/routes/ai/token_policy.rs` /
      `prepare_chat_request_for_provider` / `selected-deployment-stream-metadata` selector
      覆盖 selected Gemini
      direct/alias/fallback consume、OpenAI/OpenRouter preserve、selection-failure no
      mutation 与 internal-inconsistent/non-stream branches；DTO unit fixtures 另行覆盖
      unknown/non-bool wire rejection。Responses 五个 selectors 分别覆盖 typed
      non-serialized provenance capture、unary propagation、stream propagation、
      background propagation 与 selected-model normalization；fixtures 锁定 DTO top_k Missing/Null/Value、
      pre-selection dual token fields、no extra_body/upstream leak、selected GH1108
      direct/alias/fallback single-normalization，以及 OpenAI/OpenAI-like alias/fallback、
      其他 provider/其他 Gemini 的 field/value/serialization baseline parity；
      model capability dispatch selector 覆盖两个新 exact IDs 的
      三项 closed positive capabilities、ToolCalling/FunctionCalling negatives、case/prefix
      mismatch 与旧 Gemini no-record provider-wide fallback；cache-hit preflight 的
      response-cache/chat 两个 selectors 覆盖真实合法 seed/hit、旧 key collision、
      metadata-bearing lookup/store pre-key bypass、direct/alias/fallback selected-model
      invalid-request/network=0，不能以改 key/prompt 或清 cache 制造 miss。live observation 的
      `canonicalize_live_observation` selector 覆盖 redaction-first/hash-only/
      optional-pair/partial/no-response/8-key per-model missing/global-static-list rejection/
      complete-pagination-two-independent-observations/later-page-match/repeated-or-malformed-
      token/page-failure/100-page-bound/cross-page-duplicate/one-fact-different branches，
      `classify_live_failure` selector 覆盖完整 source→class table、precedence、
      first-execution-terminal-event-wins 与 no-substring branches；
      `live_interruption_persistence` category 的 `run_live_gemini_smoke` /
      `live-runner-cancellation-persistence` selector 必须由真实
      runner 驱动 barrier/cancellation/sink reload，覆盖每-step awaited atomic replace、
      cancel flush、remaining incomplete、no-later-network、new-run-id isolation 与完整
      execution-terminal × finalization matrix；cancel 后 final persist failure 必须是
      typed uncommitted failed/protocol/artifact_persistence_failed、保留 execution
      terminal、last snapshot reloadable/unchanged、无 false final artifact；并覆盖
      default/override `<run_id>.json`、permission/retention/
      offline-temp-sink branches；只构造 incomplete record 或只测 sink helper 不算覆盖；
      classification/redaction/canonicalization/interruption-persistence 独立满足；missing/
      malformed JSON、wrong tool version、missing/duplicate/wrong-path/branchless function
      或 uncovered branch 全部 fail closed。negative fixtures 证明 same-path other-function
      的 covered branch 不能满足 category，并分别证明 classification/redaction/canonicalization/
      interruption-persistence 任一
      missing/uncovered 时失败。`ResponsesApiRequest` constructor completeness 由
      `cargo check --all-targets` 与 unary/background provenance behavior fixtures 证明，
      禁止 literal scan。Verify:
      `python3 checks/test_gh1108_coverage_gate.py`；生成 pinned LLVM coverage JSON 后执行
      `python3 checks/gh1108_coverage_gate.py --repo . --base "$IMPLEMENTATION_BASE_SHA"
       --head "$IMPLEMENTATION_HEAD_SHA" --coverage-json artifacts/coverage/GH1108/coverage.json
       --output artifacts/coverage/GH1108/gate.json`.

- [ ] `SP1108-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017. Owner: coordinator + independent security reviewer. Dependencies: SP1108-T2 through T4 and SP1108-T6 complete. Done when: detailed exact-head evidence below is satisfied. Verify: every validation Test Plan command plus runtime/remote gates passes.
      Files: read-only
      verification; findings return to owning task. Done when: focused tests, full
      fmt/check/strict Clippy/test, SpecRail gates, exact-head coverage, local independent
      review, hosted CI, GraphQL review threads and `pr_gate.py` are current and green;
      changed production Rust line coverage is at least 80%；versioned checker 证明
      catalog evidence validation、deprecated rejection、prefill rejection、native request
      preflight、runtime pricing authority、
      Responses provenance capture/unary propagation/stream propagation/background
      propagation/selected-model normalization、model capability dispatch、stream-metadata validation、
      cache-hit preflight、live classification、live redaction、
      live-observation canonicalization、live interruption persistence 的 mandatory
      LLVM function records 各自存在 branch regions 且 100% covered；coverage gate 验证完整
      `IMPLEMENTATION_BASE_SHA`/`IMPLEMENTATION_HEAD_SHA`、exact HEAD、tracked clean、
      pinned LLVM JSON 存在，并对 missing changed production source、empty denominator、missing/
      uncovered category branch fail closed；no raw credential-bearing live output is
      published; final PR uses `Fixes #1108`. Verify: every command in
      `specs/GH1108/validation.md#test-plan` plus
      `python3 checks/runtime_ledger_gate.py --checkpoint
      .specrail/runtime/current.json` and fresh GitHub evidence.

## 并行拆分

- Spec PR 与 implementation PR 分离；这是 heavy tier 的两阶段流程。
- T2 → T3 严格串行：T2 owns neutral catalog records，T3 owns shared contract 和 Gemini
  consumers；T3 只能在 T2 head、focused verification 和 clean state 记录后开始。
- T4 理论上文件独立，但依赖 T3 的最终 contract 和 credential/error shape；为避免 smoke
  固化中间 API，按 `serial_after_dependency` 执行，不与 T3 并发写。
- T6 在 T2-T4 stable head 后由 coordinator 独占 `checks/gh1108_coverage_gate.py` 与
  `checks/test_gh1108_coverage_gate.py`；不得与其他 checks writer 并发。T6 完成后 T5
  reviewer 才进入只读审查；full suite/coverage 只有 coordinator 一个 owner。
- GH1111/GH1113/GH1112 或其他 Google/Gemini writable lane 与本 implementation 禁止并发；
  overlapping neutral/consumer paths 必须串行。

## 验证

- Product invariant set:
  `B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010,
  B-011, B-012, B-013, B-014, B-015, B-016, B-017`.
- Task `Covers:` union: 同一完整集合；无 orphan invariant。
- Planned manifest: issue=1108、complete=true、包含 neutral catalog、shared contract、
  embedded runtime pricing authority/tests、Gemini consumers、native request preflight/
  public-entrypoint fixtures、Responses DTO/sync/stream/background canonical adapter/tests、
  exact-model capability dispatch、post-selection token-policy stream-metadata adapter/tests、
  chat/response-cache/key-policy cache-hit closure、live smoke、versioned coverage
  checker/tests、provider docs 与精确 artifact ignore policy owner `.gitignore`；
  tech 列出的五个 remaining routing/context files 仅为 read-only context，不扩 writable
  manifest。
- Spec phase:
  `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
  `python3 checks/check_workflow.py --repo .`、`git diff --check`.
- Implementation phase: focused tests during iteration；exact final head 只执行一次 full
  fmt/check/strict Clippy/test/coverage，并把 raw output 保存到 artifact。

## Handoff Notes

- 当前 implementation blockers 是 GH1112 production dependency 与 maintainer 对最终
  spec head 的明确批准；`route_gate allowed` 不等于批准。
- 2026-07-26 verified official facts：
  - `gemini-3.6-flash`: 1,048,576 / 65,536，Gemini Developer API paid Standard
    1.50 / 7.50 USD per million。
  - `gemini-3.5-flash-lite`: 1,048,576 / 65,536，Gemini Developer API paid Standard
    0.30 / 2.50 USD per million。
  - `gemini-2.0-flash-thinking-exp`: exact changelog shutdown date 2025-12-02。
- Google Developer release/lifecycle/pricing 证据不等于 Vertex availability。
- gateway 运行时预算/结算不调用 neutral catalog cost，而加载 embedded
  `model_prices_extended.json`；两个新 ID 因此必须同时拥有 exact prefixed Developer
  rows。provider=gemini lookup、chat/native reserve+settle 与 neutral fixed cost parity
  由 T2 锁定；不得增加 unprefixed/Vertex rows，provider=vertex_ai 保持 missing。
- deprecated sampling fields 对 omitted/JSON-null 按 absent 处理；non-null 使用 typed
  pre-network rejection；其中 flattened top_k null 必须被 shared normalizer 消费并从
  extra_params 删除，禁止 silent drop/透传。
- one-shot `normalize_gemini_contents` 直接产出 contents/systemInstruction；gate 与
  serializer 共用结果。meaningful System+Developer text 按 raw message 顺序组成
  systemInstruction.parts 且不进 contents；System/Developer non-text 或不可表示 payload
  均 fail closed，developer+user 不丢内容。System/Developer extraction 不能遮蔽 terminal model，
  assistant+developer、空 contents 与 final model fail closed。
- public entrypoint matrix 除 chat 外还包含 legacy completions unary/stream 与 Responses
  sync/stream/background，它们都必须到 shared selected-provider chat gate；native `/v1`、`/v1beta`、
  `/gemini/v1`、`/gemini/v1beta` × unary/stream 八个 shape 走独立 shared native
  normalizer，在 budget/network 前消费 null sampling fields；terminal omitted role
  按 official default=user 通过，explicit model/null/unknown/nonrepresentable/ambiguous
  拒绝。
  Batch 与其他 capability routes 已证明不能路由到 Gemini chat。
- Responses sync/stream/background 使用 two-stage contract：DTO 捕获 typed、trusted、
  non-serialized top_k presence/value 与 max_output_tokens origin；route-local sidecar 不进
  canonical request、RequestContext、extra_body、cache key 或 serialization。
  `build_chat_request` 保持现有 max_tokens/max_completion_tokens 双字段；只有最终 selected
  exact GH1108 Gemini consumer 才拒绝 non-null top_k，并把 token origin 单次归一化为
  max_tokens only 后进入 Gemini maxOutputTokens。OpenAI/OpenAI-like alias/fallback、其他
  provider 与其他 Gemini 的 field/value/serialization 保持 baseline parity。
- 新模型 positive params 精确为 `{max_tokens, stop, stream}`；tools/tool_choice 仅有现存
  passthrough、无 serializer consumer，因此与 response_format/max_completion_tokens 一并
  排除。stream_options 是 gateway settlement metadata，不是第四个 param；builder 保留到
  final selection 的是只含 include_usage=true 的既有 canonical core metadata，不存在
  额外 usage preference state；只有 selected Gemini Developer + 两个新 exact ID 在
  token-policy hook 消费。
  direct/alias/fallback 的 canonical include_usage=true 到达 stream transport 但不
  进 upstream body；OpenAI/OpenRouter hook input/output 相等，selection failure 不修改
  请求，selected
  new-Gemini 非法或 non-stream 组合 fail closed。
- unary response cache 的旧 key policy 会删除 stream_options；T3 固定在 key lookup/store
  前 safe bypass metadata-bearing non-stream 请求，再由最终 selected-model hook 判定。
  regression 必须真实 seed 同 key cache，证明 direct/alias/fallback cache return=0、
  network=0/error 稳定，并保持无 metadata 合法 cache hit。
- 两个新模型 capability 精确为 ChatCompletion/ChatCompletionStream/
  GeminiGenerateContent，features 仅
  MultimodalSupport/StreamingSupport/SystemInstructions，不广告
  ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding 或
  ToolCalling/FunctionCalling/JsonMode，supports_tools=false；Google 产品支持不等于当前
  provider callability，相关能力留给 GH1111/后续契约。`capability_dispatch.rs` 必须按
  final exact model 查询 neutral registry；两个新 ID 的 ToolCalling/FunctionCalling route
  eligibility 为 false，但 no-record 既有 Gemini model 继续 provider-wide fallback，不能
  全局删除 Gemini ToolCalling。
- live artifact closed schema 含 run_id/attempt/termination 与 status-dependent optional
  observation/digest pair；passed 必须 complete，auth/timeout/cancel 无响应不得伪造
  observation，partial 只存真实 facts。两个 exact model 各自 static/list/get/minimal-call
  形成 8 个 `(step, model)` required keys；list 必须完成全部 pagination，later-page model
  可见且 repeated/malformed token、page failure、100-page bound/cross-page duplicate
  fail closed；完整 traversal 的两条 observations 仍独立绑定 exact ID，一个模型/global
  record 不能替代另一个；实际 facts 经 credential
  redaction 后 canonicalize，pair/digest 必须一致且事实变化产生不同记录；error class
  使用 closed source table 与 first-execution-terminal-event-wins，禁止 substring；transport
  timeout 是 failed/network，verified external cancel/interruption 才是 no-error-class
  incomplete。真实 runner cancellation fixture 必须证明逐 step await atomic snapshot、
  cancel flush、reload 保留、later network=0 与 new run_id isolation；artifact
  finalization 是独立 required commit gate。cancel 后 flush 成功返回 committed
  incomplete；失败返回 typed uncommitted failed/protocol/
  artifact_persistence_failed，同时保留 execution cancellation 与 last committed
  snapshot，不声称 final artifact 已持久化。manual sink 默认
  durable `artifacts/live/GH1108/<run_id>.json`，支持 explicit output-dir override、
  atomic replace、权限/retention 契约与 docs 检索/清理；implementation 必须提交 exact
  `.gitignore` pattern `/artifacts/live/GH1108/`，offline sink 使用 temp。closed
  13-case actual-env matrix 必须分别锁定 GOOGLE/GEMINI aliases、GOOGLE precedence、
  Developer-over-Vertex precedence 和 Vertex-only/partial zero-network，普通 parallel
  process 零 set_var/remove_var；captured tracing redaction fixture 必须通过。
- 17-ID frozen disposition ledger 是批准输入（7 available/6 shutdown/1 retired/
  3 unverified，reviewed_at 2026-07-26）；implementation fixture 必须逐字段 exact-equal，
  不得自行升级 unverified。
- Vertex production unchanged 由对 `src/core/providers/vertex_ai/**` 真正非零退出的
  fail-closed path gate 证明；neutral catalog 内 Vertex overlay 仍由 snapshot fixture
  独立证明。
- `checks/gh1108_coverage_gate.py`/test 是 implementation manifest 的必交付物；必须在
  T5 read-only review 前由 coordinator 实现，不能以本 spec 里的临时 heredoc 替代。
- live smoke 仍处于 manual validation 阶段；不得直接做 cron/CI automation。
- manual artifacts 是 retained persistent files；rollback 不自动删除，必须检索 default
  与所有 override directories 后显式归档/清理，保留文件时继续维持 ignore protection。
- 若 merged GH1112 API/path 与本 tech manifest 不同，先修订 spec，不猜 alias/wrapper。
- 当前 spec PR 始终使用 `Refs #1108`，不关闭仍待实现的 issue；只有满足 T2-T6 与 T5
  exact-head acceptance 的 implementation PR 才使用 `Fixes #1108`。
- 不修改 GH1111 tool loop、GH1113 pricing authority 或 unknown-cost semantics；GH1108
  不依赖 GH1111，也不声称 tool-loop 完整 callability。
