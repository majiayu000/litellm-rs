# Task Plan

## Linked Issue

GH-1108 / #1108

## Spec Packet

- Product: `specs/GH1108/product.md`
- Tech: `specs/GH1108/tech.md`
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
      `specs/GH1108/tasks.md`. Done when: product invariants are contiguous
      `B-001..B-017`; tech Product-to-Test Mapping and task `Covers:` union both cover the
      full set; planned-changes manifest is issue=1108/complete=true; official Developer
      sources are recorded，17-row frozen disposition ledger 的 exact
      ID/status/full URL set/reviewed_at/reason 已获批准；manifest 包含 versioned coverage checker/test，tech 已定义
      八类 exact selector/span 和 fail-closed negative fixtures；GH1112 implementation
      dependency and no-Vertex-inference boundary are explicit；最终 spec head 已获得
      maintainer 明确批准，且批准证据绑定该 exact head。Verify:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108`、
      `python3 checks/check_workflow.py --repo .`、`git diff --check`.

- [ ] `SP1108-T2` Covers: B-001, B-002, B-003, B-004, B-008, B-009, B-013, B-015, B-016, B-017. Owner: neutral catalog implementation owner. Dependencies: SP1108-T1 spec PR and GH1112 implementation merged. Done when: detailed catalog evidence below is satisfied. Verify: detailed commands below pass.
      Fresh `origin/main`,
      duplicate evidence and `implement` route gate allowed; merged GH1112 paths/API
      match the tech manifest or an amendment is merged first. Files:
      `src/core/providers/google/models/registry.rs`,
      `src/core/providers/google/models/catalog/{mod.rs,gemini35.rs,gemini36.rs}`,
      `src/core/providers/google/models/tests.rs`; all Gemini/Vertex consumers read-only.
      Done when: both GA exact IDs、limits、provider-callable exact closed capability/feature
      sets（capability 仅 ChatCompletion/ChatCompletionStream、supports_tools=false；
      feature 精确为 MultimodalSupport/StreamingSupport/SystemInstructions，并分别绑定
      inlineData image/stream endpoint/systemInstruction serializer；显式排除
      ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding 以及
      ToolCalling/FunctionCalling/JsonMode）、Gemini
      Developer API paid Standard per-million pricing and official evidence are present
      in the Developer overlay；Batch/Flex/Priority 不复用或宣称该定价；every
      pre-refresh Developer chat ID 的 fixture 与 tech frozen ledger 17 行逐字段
      exact-equal（7 available_exact、6 shutdown、1 retired、3 unverified；
      `reviewed_at=2026-07-26`，仅使用表内 ai.google.dev URLs/reasons），implementation
      不得自行分类或升级 unverified；retired/shutdown/unverified entries are not
      advertised; Developer pre/post snapshot is stable, sorted and duplicate-free;
      Vertex overlay and production paths are byte-for-byte unchanged. Verify:
      `cargo test --locked google_model_catalog_2026_07`、
      `cargo test --locked gemini_2026_07_cost`、
      `test "$(git rev-parse HEAD)" = "$IMPLEMENTATION_HEAD_SHA" &&
       test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       ':(glob)src/core/providers/vertex_ai/**')"`、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T3` Covers: B-005, B-006, B-007, B-015. Owner: shared contract + Gemini consumer owner. Dependencies: SP1108-T2 stable committed head. Done when: detailed contract evidence below is satisfied. Verify: detailed commands below pass.
      No other
      writable neutral-catalog owner. Files:
      `src/core/providers/google/models/request_contract.rs`,
      `src/core/providers/gemini/provider.rs`,
      `src/core/providers/gemini/provider_tests.rs`,
      `src/core/providers/gemini/client.rs`,
      `src/core/models/openai/requests.rs`,
      `src/server/routes/ai/token_policy.rs`,
      `src/server/routes/ai/chat_tests.rs`,
      `tests/gemini_router_fallback_routes.rs`; T2 catalog records read-only；
      `src/server/routes/ai/chat.rs`、`src/server/routes/ai/chat_streaming.rs`、
      `src/core/types/chat.rs` 是 read-only context，不在 writable manifest。
      Done when: exact new-model contract removes `temperature`/`top_p`/`top_k` from
      supported params；typed temperature/top_p omitted/JSON-null 按 `Option` absent；
      flattened extra_body/extra_params 的 `top_k: Value::Null` 由 shared normalizer
      消费并删除，任何 non-null 值（包括默认数值）在 auth/network 前拒绝，final body
      无 temperature/topP/top_k/topK；`normalize_gemini_contents` 只执行一次 role/content
      normalization，直接产出 serializer-ready contents/systemInstruction，prefill gate
      与 serializer 都不读取 raw messages；meaningful System+Developer text 按原 messages
      顺序组成 `systemInstruction.parts` 且都不进 contents，Developer non-text/不可表示
      payload pre-network error；developer+user 同时保留 instruction/user；
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
      positive allowlist 仍只有三项；既有 Gemini
      ToolResult/ToolUse 序列化与完整 callability 归 GH1111、非本任务 acceptance，且
      GH1108 implementation 不依赖 GH1111；no family-substring inheritance or silent
      drop remains for this contract. Verify:
      `cargo test --locked gemini_2026_07`、
      `cargo test --locked gemini_router_fallback`、network-counter negatives、
      `test -z "$(git diff --name-only
       "$IMPLEMENTATION_BASE_SHA...$IMPLEMENTATION_HEAD_SHA" --
       src/server/routes/ai/chat.rs src/server/routes/ai/chat_streaming.rs
       src/core/types/chat.rs)"`、
      `cargo fmt --all -- --check`、`cargo check --locked`.

- [ ] `SP1108-T4` Covers: B-010, B-011, B-012, B-014, B-016. Owner: live-smoke test/documentation owner. Dependencies: SP1108-T3 stable committed head. Done when: detailed smoke/redaction evidence below is satisfied. Verify: detailed commands below pass.
      Developer
      credential path unchanged. Files: `tests/live_gemini.rs`,
      `docs/providers/gemini.md`, `docs/providers/README.md`; production catalog/provider
      files read-only. Done when: ignored live tests require exactly
      `LITELLM_RS_LIVE_GEMINI=1`；2×2 fixture 不在普通 parallel process 调用
      `set_var`/`remove_var`，每 case 以 `env_clear` + exact env 的隔离子进程经过
      production actual env boundary；双 unset、仅 sentinel
      `GEMINI_API_KEY`、仅 opt-in=1 且 GEMINI_API_KEY/GOOGLE_API_KEY/其他 Developer
      aliases unset 均 network counter=0，双满足只命中 fake transport，parallel child
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
      list response 可派生两条 list observation，但各自 key/model/requested_exact_id/
      case-sensitive exact match 必须一致并独立 hash/persist；redaction 后按 exact ID 不
      case-fold、set-valued fields lexical sort/dedupe、candidate/text 保序、integer
      token counts/fixed-decimal prices、aggregate keys step/model lexical sort、recursive
      lexical JSON keys canonicalize，再对 observation 求 SHA-256；observation/digest
      同生同灭且必须重算匹配，任一事实变化必须改变 canonical observation/digest；
      `classify_live_failure` 按 tech closed table 穷举 exact terminal event、structured
      Google reason、typed ProviderError、numeric HTTP fallback 与 local validation：
      auth/quota/not_found/protocol/network 是且仅是五类，禁止 message substring；
      precedence 固定为 first terminal event → structured reason → ProviderError → HTTP →
      local protocol，deadline/cancel race 使用 atomic first-terminal-event-wins；403 +
      RESOURCE_EXHAUSTED=quota，runner deadline=failed/network/transport_timeout，remote
      DEADLINE_EXCEEDED=failed/network/step_failed，只有 verified external cancel/
      interruption 为 no-error-class incomplete；`run_live_gemini_smoke` 注入 cancellation
      token、transport barrier 与 reloadable artifact sink，每个 step 必须
      redaction/canonicalization 后 await temp-write + atomic-replace snapshot，再开始下一
      network call；真实 cancel 停止后续网络、把 in-flight/remaining keys 记为 incomplete
      并 await flush，persistence failure 不得 passed；runner-level fixture reload 后证明
      completed facts/digests 保留、later network=0、aggregate 非 passed；retry 使用不同
      run_id，旧 keys 不聚合；只有无 external terminal event 的 natural missing required
      step 为 failed/protocol/missing_required_step；
      manual production sink 默认写
      `artifacts/live/GH1108/<run_id>.json`，非空
      `LITELLM_RS_LIVE_GEMINI_OUTPUT_DIR` 可覆盖目录；same-directory temp + atomic
      replace、Unix 0600 best-effort/非 POSIX 平台契约、success/interruption retention、
      path/reload 均有 fixture，offline tests 注入 temp sink 且默认目录零写入；
      redaction fixture 安装 captured tracing subscriber，sentinel key is absent from
      tracing/stdout/stderr/error Display+Debug/config+result Debug/artifact；docs 给出
      exact opt-in command、artifact 检索/显式清理命令、migration、
      disposition and Developer/Vertex boundary；分类、脱敏、observation canonicalization、
      runner cancellation/persistence
      分别由 exact symbols `classify_live_failure`、`redact_live_artifact`、
      `canonicalize_live_observation`、`run_live_gemini_smoke` 所有。Verify:
      `cargo test --locked live_gemini_failure_mapping_precedence`、
      `cargo test --locked live_gemini_runner_cancellation_persists_incrementally`、
      `cargo test --locked live_gemini_artifact_sink`、
      `cargo test --locked live_gemini`、
      `cargo test --locked --test live_gemini -- --ignored` with no opt-in proves skip/
      zero-network、manual
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
      only when a human supplies credentials、documentation diff review.

- [ ] `SP1108-T6` Covers: B-003, B-005, B-006, B-007, B-011, B-012, B-014, B-016. Owner: coverage-checker coordinator. Dependencies: SP1108-T2 through T4 stable committed head; tech exact selector contract. Done when: versioned checker and negative fixtures below are implemented before SP1108-T5 read-only review. Verify: checker unit tests and exact-head invocation below pass.
      Files: `checks/gh1108_coverage_gate.py`,
      `checks/test_gh1108_coverage_gate.py`; production/test Rust files read-only.
      Done when: checker enforces full SHA/head/ancestor/tracked-clean/LCOV guards、non-empty
      changed-production denominator、all changed production sources in LCOV、changed-line
      ≥80%、changed paths 是 complete manifest 子集；chat.rs/chat_streaming.rs/
      core/types/chat.rs read-only contexts 与 `src/core/providers/vertex_ai/**` 任一变化均
      fail closed；八个 mandatory categories 均绑定 tech 表中 exact path + Rust symbol + marker
      span，prefill 的 `normalize_gemini_contents`/`validate_no_model_prefill` 两个
      selectors 都满足并覆盖 interleaved System/Developer 原序、developer+user 保留、
      Developer non-text rejection、assistant+developer final-model rejection，
      stream metadata 的 `src/server/routes/ai/token_policy.rs` /
      `prepare_chat_request_for_provider` / `selected-deployment-stream-metadata` selector
      覆盖 selected Gemini
      direct/alias/fallback consume、OpenAI/OpenRouter preserve、selection-failure no
      mutation 与 internal-inconsistent/non-stream branches；DTO unit fixtures 另行覆盖
      unknown/non-bool wire rejection。live observation 的
      `canonicalize_live_observation` selector 覆盖 redaction-first/hash-only/
      optional-pair/partial/no-response/8-key per-model missing/global-static-list rejection/
      one-list-response-two-independent-observations/one-fact-different branches，
      `classify_live_failure` selector 覆盖完整 source→class table、precedence、
      first-terminal-event-wins 与 no-substring branches；
      `live_interruption_persistence` category 的 `run_live_gemini_smoke` /
      `live-runner-cancellation-persistence` selector 必须由真实
      runner 驱动 barrier/cancellation/sink reload，覆盖每-step awaited atomic replace、
      cancel flush、remaining incomplete、no-later-network、persistence failure 与
      new-run-id isolation，以及 default/override `<run_id>.json`、permission/retention/
      offline-temp-sink branches；只构造 incomplete record 或只测 sink helper 不算覆盖；
      classification/redaction/canonicalization/interruption-persistence 独立满足；missing/
      malformed selector、DA/BRDA、span 或 uncovered branch 全部 fail closed。negative
      fixtures 证明 same-path other-symbol 与 same-symbol outside-marker 的 covered branch
      不能满足 category，并分别证明 classification/redaction/canonicalization/
      interruption-persistence 任一
      missing/uncovered 时失败。Verify:
      `python3 checks/test_gh1108_coverage_gate.py`；生成 LCOV 后执行
      `python3 checks/gh1108_coverage_gate.py --repo . --base "$IMPLEMENTATION_BASE_SHA"
       --head "$IMPLEMENTATION_HEAD_SHA" --lcov artifacts/coverage/GH1108/lcov.info
       --output artifacts/coverage/GH1108/gate.json`.

- [ ] `SP1108-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012, B-013, B-014, B-015, B-016, B-017. Owner: coordinator + independent security reviewer. Dependencies: SP1108-T2 through T4 and SP1108-T6 complete. Done when: detailed exact-head evidence below is satisfied. Verify: every tech-spec Test Plan command plus runtime/remote gates passes.
      Files: read-only
      verification; findings return to owning task. Done when: focused tests, full
      fmt/check/strict Clippy/test, SpecRail gates, exact-head coverage, local independent
      review, hosted CI, GraphQL review threads and `pr_gate.py` are current and green;
      changed production Rust line coverage is at least 80%；versioned checker 证明
      catalog evidence validation、deprecated rejection、prefill rejection、
      stream-metadata validation、live classification、live redaction、
      live-observation canonicalization、live interruption persistence 的 mandatory
      symbol/marker spans 各自存在 changed
      branch records 且 100% covered；coverage gate 验证完整
      `IMPLEMENTATION_BASE_SHA`/`IMPLEMENTATION_HEAD_SHA`、exact HEAD、tracked clean、
      LCOV 存在，并对 missing changed production source、empty denominator、missing/
      uncovered category branch fail closed；no raw credential-bearing live output is
      published; final PR uses `Fixes #1108`. Verify: every command in the tech-spec Test
      Plan plus `python3 checks/runtime_ledger_gate.py --checkpoint
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
  Gemini consumers、post-selection token-policy stream-metadata adapter/tests、live smoke、
  versioned coverage
  checker/tests 与 provider docs；chat.rs/chat_streaming.rs/core/types/chat.rs 仅为
  read-only context，不扩 writable manifest。
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
- Google Developer release/lifecycle/pricing 证据不等于 Vertex availability。
- deprecated sampling fields 对 omitted/JSON-null 按 absent 处理；non-null 使用 typed
  pre-network rejection；其中 flattened top_k null 必须被 shared normalizer 消费并从
  extra_params 删除，禁止 silent drop/透传。
- one-shot `normalize_gemini_contents` 直接产出 contents/systemInstruction；gate 与
  serializer 共用结果。meaningful System+Developer text 按 raw message 顺序组成
  systemInstruction.parts 且不进 contents；Developer non-text/不可表示 payload fail
  closed，developer+user 不丢内容。System/Developer extraction 不能遮蔽 terminal model，
  assistant+developer、空 contents 与 final model fail closed。
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
- 两个新模型 features 仅 MultimodalSupport/StreamingSupport/SystemInstructions，不广告
  ContextCaching/SearchGrounding/VideoUnderstanding/AudioUnderstanding 或
  ToolCalling/FunctionCalling/JsonMode，supports_tools=false；Google 产品支持不等于当前
  provider callability，相关能力留给 GH1111/后续契约。
- live artifact closed schema 含 run_id/attempt/termination 与 status-dependent optional
  observation/digest pair；passed 必须 complete，auth/timeout/cancel 无响应不得伪造
  observation，partial 只存真实 facts。两个 exact model 各自 static/list/get/minimal-call
  形成 8 个 `(step, model)` required keys；一次 list response 的两条 observations 仍独立
  绑定 exact ID，一个模型/global record 不能替代另一个；实际 facts 经 credential
  redaction 后 canonicalize，pair/digest 必须一致且事实变化产生不同记录；error class
  使用 closed source table 与 first-terminal-event-wins，禁止 substring；transport
  timeout 是 failed/network，verified external cancel/interruption 才是 no-error-class
  incomplete。真实 runner cancellation fixture 必须证明逐 step await atomic snapshot、
  cancel flush、reload 保留、later network=0 与 new run_id isolation。manual sink 默认
  durable `artifacts/live/GH1108/<run_id>.json`，支持 explicit output-dir override、
  atomic replace、权限/retention 契约与 docs 检索/清理；offline sink 使用 temp。2×2 gate
  matrix 必须以 env-isolated child actual boundary 测试、普通 parallel process 零
  set_var/remove_var；captured tracing redaction fixture 必须通过。
- 17-ID frozen disposition ledger 是批准输入（7 available/6 shutdown/1 retired/
  3 unverified，reviewed_at 2026-07-26）；implementation fixture 必须逐字段 exact-equal，
  不得自行升级 unverified。
- Vertex production unchanged 由对 `src/core/providers/vertex_ai/**` 真正非零退出的
  fail-closed path gate 证明；neutral catalog 内 Vertex overlay 仍由 snapshot fixture
  独立证明。
- `checks/gh1108_coverage_gate.py`/test 是 implementation manifest 的必交付物；必须在
  T5 read-only review 前由 coordinator 实现，不能以本 spec 里的临时 heredoc 替代。
- live smoke 仍处于 manual validation 阶段；不得直接做 cron/CI automation。
- 若 merged GH1112 API/path 与本 tech manifest 不同，先修订 spec，不猜 alias/wrapper。
- 不修改 GH1111 tool loop、GH1113 pricing authority 或 unknown-cost semantics；GH1108
  不依赖 GH1111，也不声称 tool-loop 完整 callability。
