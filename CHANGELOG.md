# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-08-12

### Added
- feat(providers): preserve Meta Llama and v0 catalog policies (#1165)
- feat(providers): wire native Ollama factory (#1164)
- feat(responses): add Codex tool protocol compatibility (#1160)
- feat(gemini): refresh developer API model catalog (#1159)
- Merge pull request #1125 from majiayu000/impl/gh1105-model-alias-priority
- feat(gateway): implement GH1105 model aliases and priorities
- Merge pull request #1094 from majiayu000/impl/gh837-t13-github-0-6-policy
- Merge pull request #1122 from majiayu000/workflow/gh1107-retained-branch-disposition
- feat(specrail): gate retained branch dispositions
- Merge pull request #1110 from majiayu000/codex/gh1107-codex-responses-compat
- feat(responses): accept Codex wire items (GH1107 T1)
- feat(providers): add GitHub Models catalog policy for 0.6 (GH837 T13)
- Merge pull request #1090 from majiayu000/codex/gh838-guardrails-ip-access
- Merge pull request #1093 from majiayu000/impl/gh837-amazon-nova-0-6-policy
- feat(providers): add Amazon Nova catalog policy
- feat(providers): deprecate custom API for 0.6
- feat(core): wire guardrails and IP access for GH838
- feat(admin): add built-in dashboard (#1085)
- feat(observability): wire external lifecycle callbacks (#1078)
- feat: adopt SpecRail workflow pack (#1071)
- feat(gemini): execute SDK routes through runtime providers (#1019)
- Merge pull request #996 from majiayu000/codex/issue-968-propagate-shared-endpoint-access
- feat: propagate provider endpoint access contract
- Merge pull request #994 from majiayu000/codex/issue-968-wire-gateway-runtime-policy
- feat: stage provider endpoint access configuration
- feat: enforce endpoint policy for gateway OpenAI runtimes
- feat(security): add policy-aware provider HTTP client (#985)
- feat(security): add provider endpoint policy foundation (#984)
- feat(spend): expose unpriced model metrics
- Merge pull request #901 from majiayu000/harness/issue-831-unpriced-usage-record
- feat(keys): track unpriced usage records
- feat(pricing): add usage-aware dry run (#899)
- feat(config): add unpriced model policy (#897)
- feat(anthropic): allow compatible model ids (#704)
- feat(deepseek): update v4 model metadata
- feat(catalog): refresh gemini and cohere models
- feat(openai): add gpt-5.5 catalog entries
- feat(bedrock): introduce unified catalog
- feat(bedrock): wire native provider

### Fixed
- fix(security): upgrade jsonwebtoken to patched 10.x (#1167)
- fix(core)!: resolve unwired subsystem dispositions (#1163)
- fix(gemini): complete Google tool result loop (#1158)
- fix(vertex-ai): unify Google pricing authority (#1156)
- fix(pricing): preserve Google unit semantics
- fix(gemini): keep pricing surface-specific
- fix(pricing): preserve Google exact lookup semantics
- fix(vertex-ai): resolve canonical catalog models
- fix(vertex-ai): preserve canonical pricing IDs
- fix(pricing): resolve exact Google provider rows
- fix(vertex-ai): unify Google pricing authority
- Merge pull request #1155 from majiayu000/impl/gh1128-guardrail-input-minimal
- fix(guardrails): scan tool definitions and partial args
- fix(guardrails): close structured input bypasses
- fix(guardrails): scan structured chat input
- Merge pull request #1154 from majiayu000/impl/gh1127-stream-output-guardrails-minimal
- fix(guardrails): address streaming review feedback
- fix(guardrails): enforce streaming output checks
- fix(storage): gate S3-only helpers by feature (#1151)
- fix(files): enforce tenant-isolated ownership and access (GH1130) (#1149)
- Merge pull request #1144 from majiayu000/codex/gh1129-usage-normalization-recovery
- fix(streaming): ignore empty usage sentinels for billing
- fix(streaming): ignore empty usage sentinels for billing
- fix(streaming): distinguish output from heartbeat frames
- fix(gemini): isolate cloned stream usage state
- fix(streaming): keep Gemini usage state private
- fix(streaming): invalidate malformed Gemini usage
- fix(billing): reject malformed final stream usage
- fix(billing): fail closed on malformed provider usage
- Merge pull request #1139 from majiayu000/impl/gh1132-dev-config-pricing
- fix(config): make dev pricing example usable
- fix(config): reject padded model alias names
- fix(gateway): preserve alias compatibility edges
- fix(gateway): route aliases across specialized endpoints
- fix(gateway): reject duplicate generated deployment ids
- fix(gateway): address PR1125 hosted review
- fix(gateway): satisfy GH1105 strict clippy
- Merge pull request #1124 from vrieswang/issue-1123-factory-auto-detect
- fix(factory): auto-detect private network for localhost catalog providers
- fix(specrail): allow verified merged retained branches
- fix(specrail): revalidate duplicate evidence at gate time
- fix(router): align credential provenance with provider construction
- fix(responses): close final Codex review gaps
- fix(responses): close final Codex review gaps
- fix(responses): address Codex wire review gaps
- fix(responses): enforce Codex T1 wire boundary
- fix(providers): address T13 review findings on github deprecation
- fix(providers): expose Amazon Nova catalog metadata
- fix(providers): unify Amazon Nova model authority
- fix(providers): wire Amazon Nova catalog runtime
- fix(guardrails): fail closed on unsupported policy config
- Merge pull request #1083 from majiayu000/security/gh1066-callback-runtime-hardening
- fix(opentelemetry): serialize export state handoff
- fix(opentelemetry): reap completed export tasks
- Merge pull request #1086 from majiayu000/refactor/gh965-t012-provider-error-redaction
- fix(errors): preserve redacted provider response details
- fix(errors): redact provider gateway responses
- fix(observability): harden Datadog export boundary (#1081)
- fix(callbacks): preserve terminal delivery and shutdown
- Merge pull request #1080 from majiayu000/security/gh1066-stream-terminal-hardening
- fix(observability): report streaming terminal delivery failures
- fix(observability): harden webhook admission redaction
- fix(auth): reject corrupt persisted user state (#1049)
- fix(teams): reject deletes for missing rows (#1061)
- fix(teams): reject updates for missing rows (#1058)
- fix(teams): fail closed on corrupt legacy data (#1055)
- fix(mcp): enforce initialization lifecycle (#1043)
- fix(batch): canonicalize persisted status encoding (#1052)
- fix(auth): reject corrupt API key JSON (#1046)
- fix(deps): update anyhow to 1.0.103 (#1034)
- fix(router): preserve model group insertion order (#1029)
- fix(gemini): preserve endpoint policy errors (#1021)
- fix(vertex-ai): shrink provider runtime state (#1016)
- fix(bedrock): narrow failed dependency retries (#1014)
- fix(bedrock): preserve InvokeAgent compatibility (#1013)
- fix(security): reject private access to official OpenAI endpoint (#1012)
- fix(bedrock): close protocol review gaps (#1011)
- fix(security): close provider endpoint policy gaps (#1010)
- fix(providers): activate endpoint policy runtime (#1007)
- fix(routes): enforce endpoint policy for remaining AI proxies (#1006)
- fix(routes): enforce endpoint policy for direct AI proxies (#1005)
- fix(azure-ai): enforce native endpoint policy (#1003)
- fix(azure): enforce native endpoint policy (#1001)
- Merge pull request #1000 from majiayu000/codex/issue-968-native-anthropic-gemini-vertex
- fix(providers): enforce native endpoint policy
- fix(providers): enforce policy for shared runtime extras (#999)
- fix(providers): wire shared providers to endpoint policy (#997)
- fix: allow policy-bound private OpenAI test paths
- fix: stage private endpoint access until all routes are wired
- fix: retain DNS validation for unwired providers
- fix: use standard image edit test module
- fix(storage): restrict API key owner deletion (#983)
- fix(auth): redact session identifiers from logs (#982)
- fix(auth): redact AuthMethod debug credentials (#981)
- fix(auth): distinguish infrastructure failures from invalid credentials (#980)
- fix(auth): make API key lifecycle database authoritative (#979)
- fix(auth): reject API keys with invalid owners (#978)
- fix(providers): derive Tier-1 capabilities from catalog (#977)
- fix(router): honor provider health check config (#976)
- fix(router): apply per-provider retry policy (#975)
- fix(openai): preserve legacy functions in upstream payloads (#972)
- fix(build): pin Rust toolchain (#956)
- fix(providers): restore deepgram feature gate
- fix(providers): keep gradient_ai pricing rows
- fix(providers): remove stale gradient_ai cfg
- fix(providers): drop google_pse pricing metadata
- Merge pull request #926 from majiayu000/harness/runtime-wf-github-issue-pr-d657b8f7
- fix(error): normalize OpenAI middleware errors
- fix(error): scope OpenAI extractor handlers
- fix(error): normalize OpenAI route boundary errors
- Merge pull request #929 from majiayu000/harness/runtime-wf-github-issue-pr-c6556130
- fix(cache): bound eviction sampling queue
- fix(cache): preserve indexed eviction semantics
- fix(cache): guard stale eviction metadata
- fix(cache): address in-memory eviction feedback
- fix(cache): remove global in-memory lru mutex
- fix(error): address HTTP mapping review feedback
- fix(error): unify gateway HTTP mappings
- fix(error): unify HTTP error mapping
- fix(ai): reserve image proxy budgets atomically
- fix(spend): fail closed unpriced runtime models
- fix(core): classify unwired subsystems (#889)
- fix(rate-limit): fail closed on Redis degradation (#887)
- fix(routes): map unconfigured batch image to client errors (#890)
- fix(error): preserve rate-limit response headers
- fix(images): require model before image generation authz
- fix(server): let CORS preflight bypass auth
- fix(cache): wire response cache runtime (#804)
- fix(anthropic): preserve rich request content (#802)
- fix(anthropic): keep stream chunk message id (#801)
- fix(openai): preserve upstream error envelopes (#800)
- fix: wire http metrics into prometheus output (#790)
- fix: add OpenAI-compatible files routes (#768)
- Merge pull request #734 from majiayu000/fix/issue-728-support-matrix
- fix: unify provider route support matrix
- Merge pull request #733 from majiayu000/fix/issue-729-provider-capability
- fix(provider): enforce capability dispatch contract
- Merge pull request #732 from majiayu000/fix/issue-726-pricing-service
- fix: apply tiered pricing in provider route
- fix: address pricing authority review gaps
- fix: cover xai pricing authority
- fix: converge pricing service spend authority
- Merge pull request #731 from majiayu000/fix/issue-725-provider-instantiation
- fix: preserve custom catalog provider construction
- fix: gate catalog selectors through provider registry
- fix: converge provider instantiation registry
- Merge pull request #730 from majiayu000/fix/issue-724-canonical-type-tree
- fix: forward OpenAI stream options
- fix: converge canonical chat model types
- Merge pull request #721 from majiayu000/fix/issue-713-provider-contract
- fix: align provider contract docs
- Merge pull request #720 from majiayu000/fix/issue-711-budget-reserve-settle
- fix: settle no-usage completion reservations
- fix: close response stream budget gaps
- fix: settle stream budget review gaps
- fix: align core router budget fallback
- fix: tighten budget fallback review handling
- fix: close budget reservation review gaps
- fix: price provider budget reservations
- fix: make bedrock budget reservations model aware
- fix: align budget reservations with provider output caps
- fix: add atomic budget reservations
- fix(router): install atomic routing snapshots (#719)
- fix(router): harden deployment reservations (#717)
- Merge pull request #712 from majiayu000/fix/main-ci-quinn-proto-audit
- fix(ci): update quinn-proto audit advisory
- fix(pricing): tolerate LiteLLM schema drift (#708)
- fix(cost): price Xiaomi MiMo anthropic-compatible routes (#706)
- fix(server): require admin role for budget mutation endpoints (#683)
- fix(provider): wire replicate native dispatch (#698)
- fix(provider): wire fal ai native image dispatch (#697)
- fix(provider): wire Cohere native dispatch (#696)
- fix(provider): wire gemini native dispatch (#695)
- fix(provider): wire vertex ai native dispatch (#694)
- fix(provider): wire github copilot native dispatch (#693)
- fix(streaming): avoid shared request timeout (#692)
- fix(server): reject chat completions when budget is exhausted
- fix(providers): preserve choice index, usage, logprobs and refusal in streaming
- fix(anthropic): reject n!=1 and preserve cache usage details
- fix(provider): catalogify OpenAI-like adapter providers
- fix(server): record budget spend and key usage for chat completions
- fix(cost): error on missing model pricing instead of charging zero
- fix(config): surface not-yet-implemented cache and rate-limit config
- fix(auth): fail closed when no auth method is enabled (#685)
- fix(audio): fail closed for unwired routes (#680)
- fix(audio): enforce multipart upload limits (#678)
- fix(config): redact secrets in config exports (#679)
- fix(ci): restore clippy baseline (#682)
- Merge pull request #673 from majiayu000/harness/runtime-wf-github-issue-pr-63a99a35
- fix(rate-limit): reserve before auth verification
- Merge pull request #671 from majiayu000/split/issue-599-azure-native-dispatch-wire
- fix(provider): patch Azure review follow-ups (#672)
- fix(provider): wire Azure native dispatch
- Merge pull request #668 from majiayu000/split/issue-599-azure-native-dispatch
- fix(provider): harden Azure native parity
- Merge pull request #670 from majiayu000/split/issue-653-rate-limit-reservation-core
- fix(rate-limit): add releaseable reservations
- Merge pull request #667 from majiayu000/harness/runtime-wf-github-issue-pr-5fabd280
- fix(provider): migrate openai stream callers
- fix(provider): add bounded streaming request helper
- fix(security): resolve outbound hostnames for SSRF guard (#662)
- fix(bedrock): reject invalid tool call arguments (#656)
- fix(bedrock): stabilize streaming chunk metadata (#654)
- fix(bedrock): harden runtime model-id fallback (#640)
- fix(router): distinguish unsupported capability (#661)
- fix(config): fail fast for explicit config files (#660)
- fix(providers): report requested factory provider (#659)
- fix(ci): scope release token permissions (#663)
- fix(config): preserve pricing source on merge
- fix(sse): parse Gemini uppercase finish reasons
- fix(guardrails): redact moderation error bodies
- fix(config): preserve auto migrate overlay intent
- fix(router): prefer xai key for api base overrides
- fix(openai): advertise forwarded chat params
- fix(health): reuse readiness aggregates
- fix(openai): preserve streaming audio metadata and reasoning deltas
- fix(storage): simplify Redis init status
- fix(providers): reconcile Mistral Nova xAI and Llama catalogs
- fix(anthropic): lock opus alias metadata
- Merge pull request #613 from majiayu000/codex/issue-611-startup-migrations
- fix(storage): respect budget degradation during schema check
- fix(storage): address migration review feedback
- fix(storage): migrate sqlite fallback on startup
- fix(storage): make startup migrations configurable
- Merge pull request #609 from majiayu000/codex/pricing-review-fixes
- fix(pricing): reject nan pricing inputs
- fix(pricing): validate negative costs and duration dispatch
- Merge pull request #607 from majiayu000/codex/phase0-provider-dispatch
- fix(provider): enforce dispatch classification contract
- Merge pull request #598 from majiayu000/codex/phase0-cache
- fix(cache): reject unwired gateway cache config
- Merge pull request #597 from majiayu000/codex/phase0-pricing
- fix(pricing): reject missing pricing costs
- fix(storage): migrate database during startup
- fix(openai): forward typed chat parameters
- fix(bedrock): enforce model parameter policy (#589)
- fix(ollama): add missing audio field to streaming ChatDelta literals (#585)
- fix(health): treat configured-but-unknown providers as not-ready (#583)
- Merge pull request #586 from majiayu000/fix/issue-556-runtime-degradation
- fix(runtime): fail-fast on enabled-but-failed dependencies (#556)
- fix(ollama): add missing audio field to streaming ChatDelta literals
- fix(bedrock): drop redundant model-id re-parse and allow IAM credential chain
- fix(bedrock): reject prompt arns from converse fallback
- fix(bedrock): stream converse tool calls
- fix(bedrock): parse converse stream events
- fix(bedrock): use converse body for streaming
- fix(bedrock): stream converse catalog models
- fix(bedrock): encode runtime model ids
- fix(bedrock): preserve converse tool calls
- fix(bedrock): allow runtime-resolved model ARNs
- fix(health): treat configured-but-unknown providers as not-ready (#555)
- fix bedrock family hint parsing
- fix bedrock sigv4 clippy lint
- Merge pull request #568 from majiayu000/codex/issue552-openai-stream-audio
- fix(openai): preserve streaming audio deltas
- fix(config): reject unsupported sticky router fields (#562)
- fix(security): stop logging raw upstream response bodies (#551)
- fix(chat): preserve audio in core message round trips (#550)
- fix(server): drain budget persistence on shutdown (#548)
- fix(storage): return retrievable s3 file ids (#547)
- fix(responses): include reasoning items in completed stream output (#546)
- fix(cache): keep volatile extras out of keys (#545)
- fix(cache): preserve nested schema ids in keys (#544)
- fix(oauth): redact generic oidc error bodies (#543)
- fix(responses-stream): emit reasoning_summary_text events for delta.thinking (#532)
- fix(oauth): redact response bodies in error logs (#504)
- fix(http): wire HttpServer::shutdown_signal into start() and close storage (#527)
- fix(http): stop disabling pricing source when semantic_cache is enabled (#502)
- fix(cache): split llm_cache tests + fix invalidate_chat user_specific (#531)
- fix(openai_like): route Tier-1 responses through OpenAIResponseTransformer (#528)
- fix(rate-limit): bound fallback DashMap to prevent memory DoS (#525)
- fix(s3): wire S3Config credentials and endpoint into the AWS client (#523)
- fix(storage): validate file_id in LocalStorage to block path traversal (#524)
- fix(anthropic-stream): extract cache_creation/read tokens in message_delta (#526)
- fix(sse): map Anthropic/Gemini finish reasons in default parse_finish_reason (#500)
- fix(openai): propagate tool_calls and function_call through stream delta (#498)
- fix(cache): include output-affecting fields in chat key, bump schema to v3 (#506)

### Changed
- refactor(router): converge completion and SDK runtime (#1162)
- refactor(cost): deprecate legacy compatibility surface (#1161)
- refactor(google): share Gemini catalog with Vertex (#1157)
- Merge pull request #1121 from majiayu000/impl/gh965-t013b-credential-provenance
- refactor(router): normalize credential provenance
- refactor(completion): route unary facade through runtime (#1098)
- Merge pull request #1096 from majiayu000/impl/gh965-t017-retry-helper-deprecation
- refactor(errors): deprecate six provider retry helpers for 0.6 (GH965 D1E-c)
- perf(providers): map GitHub ModelInfos directly from catalog entries
- Merge pull request #1092 from majiayu000/impl/gh837-custom-api-0-6-deprecation
- refactor(sdk): deprecate legacy provider error (T023b) (#1074)
- refactor(errors): add canonical provider redaction (T023a) (#1073)
- Merge pull request #1070 from majiayu000/refactor/gh965-d1e-a1-canonical-retry
- refactor(errors): converge provider retry facts
- refactor(router): pin canonical runtime generations
- refactor(gemini): discover providers from runtime router (#1026)
- refactor(providers): remove unreachable codestral module (#1028)
- refactor(gemini): keep selected runtime identity (#1023)
- Merge pull request #995 from majiayu000/codex/issue-968-normalize-base-config-literals
- refactor: normalize BaseConfig construction
- refactor(providers): delete 14 approved GH837 orphan modules (#971)
- refactor(providers): delete unwired oci module (#952)
- refactor(providers): delete unwired huggingface module (#951)
- refactor(providers): delete unwired langgraph module (#950)
- Merge pull request #949 from majiayu000/harness/gh837-delete-deepl
- refactor(providers): delete unwired deepl module
- Merge pull request #948 from majiayu000/harness/gh837-delete-datarobot
- refactor(providers): delete unwired datarobot module
- Merge pull request #947 from majiayu000/harness/gh837-delete-spark
- refactor(providers): delete unwired spark module
- Merge pull request #946 from majiayu000/harness/gh837-delete-gradient-ai
- refactor(providers): remove unreachable gradient_ai module
- Merge pull request #945 from majiayu000/harness/gh837-delete-google-pse
- refactor(providers): remove unreachable google_pse module
- Merge pull request #944 from majiayu000/harness/gh837-delete-nlp-cloud
- refactor(providers): remove unreachable nlp_cloud module
- Merge pull request #942 from majiayu000/harness/gh842-share-chat-request
- perf(chat): share chat request budget views
- Merge pull request #941 from majiayu000/harness/gh837-delete-triton
- refactor(providers): remove unreachable triton module
- Merge pull request #940 from majiayu000/harness/gh837-delete-predibase
- refactor(providers): remove unreachable predibase module
- Merge pull request #939 from majiayu000/harness/gh837-delete-ragflow
- refactor(providers): remove unreachable ragflow module
- Merge pull request #938 from majiayu000/harness/gh837-delete-morph
- refactor(providers): remove unreachable morph module
- Merge pull request #937 from majiayu000/harness/gh837-delete-manus
- refactor(providers): remove unreachable manus module
- Merge pull request #936 from majiayu000/harness/gh837-delete-exa-ai
- refactor(providers): remove unreachable exa_ai module
- Merge pull request #935 from majiayu000/harness/gh837-delete-databricks
- refactor(providers): remove unreachable databricks module
- Merge pull request #934 from majiayu000/harness/gh837-delete-clarifai
- refactor(providers): remove unreachable clarifai module
- Merge pull request #932 from majiayu000/harness/gh837-delete-baseten
- refactor(providers): remove unreachable baseten module
- Merge pull request #931 from majiayu000/harness/gh837-delete-petals
- refactor(providers): remove unreachable petals module
- Merge pull request #930 from majiayu000/harness/gh837-delete-ai21
- refactor(providers): remove unreachable ai21 module
- Merge pull request #927 from majiayu000/harness/runtime-wf-github-issue-pr-c95565e9
- Merge pull request #928 from majiayu000/harness/runtime-wf-github-issue-pr-cc57a939
- refactor(providers): remove unreachable gigachat module
- perf(ai): remove redundant context clones
- perf(ai): share request context handles
- refactor(providers): remove unreachable firecrawl module
- Merge pull request #923 from majiayu000/codex/issue-837-delete-empower
- refactor(providers): remove unreachable empower module
- Merge pull request #922 from majiayu000/codex/issue-837-delete-vercel-ai
- refactor(providers): remove unreachable vercel ai module
- Merge pull request #921 from majiayu000/codex/issue-842-request-context-arc
- perf(context): share request context extensions
- Merge pull request #920 from majiayu000/codex/issue-837-delete-sap-ai
- refactor(providers): remove unreachable sap ai module
- Merge pull request #919 from majiayu000/codex/issue-842-key-manager-arc
- perf(keys): share hmac secret across key manager clones
- Merge pull request #918 from majiayu000/codex/issue-837-delete-topaz
- refactor(providers): remove unreachable topaz module
- Merge pull request #916 from majiayu000/codex/issue-840-execution-gate
- refactor(ai): route execution through budgeted entrypoints
- Merge pull request #915 from majiayu000/codex/issue-840-image-proxy-budgeted
- refactor(ai): route image proxy budgets through executor
- Merge pull request #914 from majiayu000/codex/issue-840-response-cache-budgeted
- refactor(ai): route cache pricing through budgeted executor
- Merge pull request #913 from majiayu000/codex/issue-840-gemini-budgeted
- refactor(ai): route gemini budgets through executor
- Merge pull request #912 from majiayu000/codex/issue-840-responses-stream-budgeted
- refactor(ai): route responses stream budgets through executor
- Merge pull request #911 from majiayu000/codex/issue-840-images-budgeted
- refactor(ai): route image generation budgets through executor
- Merge pull request #910 from majiayu000/codex/issue-840-audio-budgeted
- refactor(ai): route audio budgets through executor
- Merge pull request #909 from majiayu000/codex/issue-840-chat-budget-manager-access
- refactor(ai): use budgeted key reservations in chat routes
- Merge pull request #908 from majiayu000/codex/issue-840-embeddings-budgeted
- refactor(ai): route embeddings budgets through executor
- Merge pull request #907 from majiayu000/codex/issue-840-availability-routes
- refactor(ai): route availability checks through budgeted
- Merge pull request #906 from majiayu000/codex/issue-840-chat-stream-driver
- refactor(ai): route chat streams through settled finalizer
- Merge pull request #905 from majiayu000/codex/issue-840-settled-stream
- refactor(ai): centralize stream settlement finalizer
- Merge pull request #895 from majiayu000/harness/runtime-wf-github-issue-pr-ab9b0b2b
- refactor(ai): add budgeted executor scaffold
- refactor(ai): centralize budgeted provider calls
- refactor(user-management): split type tests for GH727
- refactor(sync): split versioned map tests for GH727
- refactor(sync): split concurrent vec tests for GH727
- refactor(observability): split metrics tests for GH727
- refactor(openai): split client tests for GH727
- refactor(tests): split moderation routes for GH727
- refactor(anthropic): split client tests for GH727
- refactor(langfuse): split types tests for GH727
- refactor(integrations): split manager tests for GH727
- refactor(gemini): split provider tests for GH727 (#867)
- refactor(virtual-keys): split type tests for GH727 (#866)
- refactor(budget): split alert tests for GH727 (#865)
- refactor(user): split user type tests for GH727
- refactor(teams): split manager tests for GH727
- refactor(config): split server config tests for GH727
- refactor(audio): split audio type tests for GH727
- refactor(tests): split auth middleware integration suite for GH727
- refactor(net): split client utils tests for GH727
- refactor(config): split helper tests for GH727
- refactor(metrics): split aggregate tests for GH727
- refactor(v0): split provider tests for GH727 (#856)
- refactor(observability): split type tests for GH727
- refactor(analytics): split report tests for GH727
- refactor(cache): split type tests for GH727
- refactor(bedrock): split provider tests for GH727 (#852)
- refactor(monitoring): split type tests for GH727 (#851)
- refactor(validation): split request validator for GH727 (#850)
- refactor(oauth): split session module for GH727 (#849)
- refactor(router): split strategy implementation tests for GH727
- refactor(event): split event tests for GH727
- refactor(observability): split OpenTelemetry integration
- refactor(utils): split DataUtils tests for GH727 (#824)
- refactor(storage): split SeaORM team repository for GH727 (#823)
- refactor(cost): extract cost type tests for GH727 (#822)
- refactor(server): extract teams route tests for GH727 (#821)
- refactor(security): extract security type tests for GH727 (#820)
- refactor(providers): split unified provider error facade for GH727 (#819)
- refactor(analytics): split analytics types facade for GH727 (#818)
- refactor(bedrock): project model config from catalog for GH727 (#816)
- refactor(sdk): split SDK types facade for GH727 (#815)
- refactor(vertex): split client modules for GH727 (#814)
- Merge pull request #723 from majiayu000/fix/issue-715-provider-failure-policy
- refactor: split provider retry policy
- style(cache): use validate method syntax
- refactor(openai-models): re-export Usage / *TokensDetails from canonical types (#535)
- refactor(types): collapse FunctionCallDelta / ToolCallDelta to canonical (#538)
- refactor(providers/base): extract shared ProviderModelEntry struct (#537)
- refactor(pricing): collapse pricing::Usage into core::types::responses::Usage (#536)




### Added
- Wired the `ollama` selector to its native chat, streaming, embeddings, tools, and health runtime behind `providers-extended`, with policy-bound public/private endpoint handling for #837.
- Wired `guardrails` into canonical chat request/response execution with default-on prompt-injection protection and an explicit `guardrails.enabled: false` opt-out.
- Added gateway `ip_access` configuration and registered its middleware ahead of authentication, handlers, and provider execution; default empty rules remain allow-all.
- Wired `enterprise.audit_logging` into request audit middleware with structured JSON stderr/file output, and aligned the observability facade with configured Langfuse/OpenTelemetry/Datadog lifecycle callbacks.
- Added real default-off Cargo gates for the module-only `a2a`, `mcp`, and `webhooks` experimental libraries.
- Added the `pricing.unpriced_model_policy` and `pricing.unpriced_fallback_cost_per_1k_tokens` configuration surface for #831 fail-closed unpriced-model enforcement; `pricing.allow_degraded` remains startup-only.
- Added `gateway_unpriced_events_total{provider,model_bucket,policy,outcome}` and `gateway_unpriced_spend_total{provider,model_bucket,policy,outcome}` Prometheus metrics for unpriced-model rejects, router candidate exclusions, and fallback settlements.

### Changed
- Breaking behavior: runtime requests for unpriced models now fail closed by default even when `pricing.allow_degraded=true`; deployments that intentionally allow unpriced traffic must set `pricing.unpriced_model_policy=allow_unpriced` and configure a finite `pricing.unpriced_fallback_cost_per_1k_tokens`.

### Deprecated
- Deprecated the unreachable `core::batch::BatchProcessor`, duplicate `core::virtual_keys::VirtualKeyManager`, `core::semantic_cache`, `core::analytics`, default-off `core::a2a`/`core::mcp`/`core::realtime`/`core::webhooks`, and optional `core::user_management::UserManager` public surfaces for the 0.6 line. They remain available behind their documented compatibility/features until the approved 0.7 removal; see `docs/architecture/GH838-subsystem-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `amazon_nova` native module for the 0.6.0 line while preserving its symbols, constructors, and runtime behavior. The catalog policy now records the same Amazon Nova endpoint/auth contract, five canonical models, token pricing, multimodal/tool metadata, and provider capabilities as the native implementation. Direct Rust imports should migrate to the `amazon_nova` catalog selector before the planned 0.7.0 native demotion; see `docs/providers/GH837-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `github` native module for the 0.6.0 line while preserving its symbols, constructors, and runtime behavior. The catalog policy now records the same `GITHUB_MODELS_API_BASE` (`https://models.inference.ai.azure.com`) endpoint/Bearer `GITHUB_TOKEN` auth contract, all 16 GitHub Models, token pricing, multimodal/tool metadata, and provider capabilities as the native implementation, with health served by the OpenAI-compatible catalog route. Direct Rust imports should migrate to the `github` catalog selector before the planned 0.7.0 native demotion; see `docs/providers/GH837-migration-0.6-to-0.7.md`.
- Deprecated the `providers-extended` public `custom_api` module and its `CustomHttpxConfig`, `CustomApiErrorMapper`, and `CustomHttpxProvider` exports for the 0.6.0 line. Existing symbols, signatures, and runtime behavior remain available in 0.6.x, but arbitrary URL/method/template/parser support is no longer a product goal and the surface is scheduled for removal in 0.7.0. See `docs/providers/GH837-migration-0.6-to-0.7.md` for alternatives.

### Removed
- Removed the module-only `topaz` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::topaz` must remove that import or restore the old implementation from git history.
- Removed the module-only `sap_ai` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::sap_ai` must remove that import or restore the old implementation from git history.
- Removed the module-only `vercel_ai` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::vercel_ai` must remove that import or restore the old implementation from git history.
- Removed the module-only `empower` provider implementation from the `providers-extended` surface for #837. It had no gateway factory or dispatch path, so runtime provider selection is unchanged; downstream crates directly importing `litellm_rs::core::providers::empower` must remove that import or restore the old implementation from git history.

### Fixed
- Fixed IP access denial so blocked requests no longer execute the downstream service before returning `403 Forbidden`.
- Redis-backed distributed rate limiting now fails closed by default when Redis commands fail, emits `rate_limiter_degraded_total{operation,mode}`, and keeps the old local fallback only behind `rate_limit.redis_failure_mode: fail_open_local`.

## [0.5.0] - 2026-04-30

### Added
- Merge pull request #412 from majiayu000/feat/provider-model-refresh-2026-04-21
- feat(models): update model catalogs for OpenAI, Anthropic, and Zhipu AI (#388)
- feat(router): add zai prefix alias to zhipu routing
- feat(router): add moonshot/minimax/zhipu dynamic and prefix routing
- feat(router): add atomic routing metrics counters (#376)
- feat(anthropic): add beta headers, structured outputs, and built-in tool types (#324)
- feat(openai): add store/metadata/service_tier params, update image models, mark deprecated (#322)
- feat: replace wildcard re-exports with explicit pub use in lib.rs (#315)
- feat(providers): reject unknown provider type strings with clear error at parse time (#311)
- feat(mistral): add missing params - frequency_penalty, presence_penalty, n, parallel_tool_calls, guardrails (#302)
- feat(core): enable user_management module with stub DB implementations (#296)
- feat: add CI job to compile-check disabled modules (#295)
- feat: enable virtual_keys module with stub database implementations (#292)
- feat(openai): add GPT-5.4 family and fix GPT-4.1 context window (#287)
- feat(gemini): add Gemini 3.1 models, fix systemInstruction and tool call handling (#291)
- feat(mistral): overhaul model catalog with 36+ current models (#290)
- feat: add reasoning_effort parameter and Developer message role for o-series models (#289)
- feat(anthropic): add claude-sonnet-4-6, claude-haiku-4-5; fix opus-4-6 limits and thinking serialization (#288)
- feat(openai-like): forward extra_params to upstream provider (#286)
- feat(config): implement YAML env var substitution in Config::from_file (#285)
- feat(mcp): add lightweight JSON Schema validation for tool arguments (#212) (#232)
- feat(a2a): add periodic health checks and exclude Unknown agents from routing (#213) (#227)
- feat(storage): add cache-aside pattern for API key verification (#207) (#228)
- feat(router): add structured tracing for routing decisions (#229)
- feat(config): add environment variable support for cache/rate-limit/enterprise (#66)
- feat(config): add schema_version field to GatewayConfig (#68)
- feat(examples): add hello example and fix broken bin references (#57)

### Fixed
- fix(cli): add gateway release entrypoint
- fix(router): execute with capability-aware deployments
- fix(auth): normalize brute-force lockout keys
- Merge pull request #455 from majiayu000/fix/issue-408-rate-limit-stable-client-key
- fix(rate-limit): ignore untrusted auth headers
- Merge pull request #454 from majiayu000/fix/issue-407-cors-validation-gate
- fix(server): fail fast on invalid cors config
- Merge pull request #453 from majiayu000/fix/issue-409-embedding-array-validation
- fix(embeddings): reject non-string array input
- Merge pull request #452 from majiayu000/fix/issue-413-sdk-chat-model
- fix(sdk): preserve explicit chat model
- Merge pull request #451 from majiayu000/fix/issue-414-vertex-gemini3-models
- fix(vertex-ai): route Gemini 3 models
- Merge pull request #450 from majiayu000/fix/issue-424-gemini-thinking-pricing
- fix(pricing): align Gemini thinking cost
- Merge pull request #449 from majiayu000/fix/issue-436-storage-file-config
- fix(storage): honor configured file storage
- Merge pull request #448 from majiayu000/fix/issue-438-streaming-deployment-lifecycle
- fix(ai): hold deployment leases for streams
- Merge pull request #447 from majiayu000/fix/issue-439-openai-error-envelope
- fix(ai): return OpenAI error envelopes
- Merge pull request #446 from majiayu000/fix/issue-433-filtered-key-pagination
- fix(keys): paginate filtered key listings
- Merge pull request #445 from majiayu000/fix/issue-432-key-admin-promotion
- fix(keys): block non-admin management permission grants
- fix(models): align GPT-5.4 Pro token limits
- Merge pull request #418 from majiayu000/fix/gstack-health-2026-04-24
- fix(deps): address TLS migration review
- Merge pull request #441 from majiayu000/fix/issue-434-key-manager
- Merge pull request #444 from majiayu000/fix/issue-440-virtual-key-persistence
- fix(storage): keep virtual key last-used monotonic
- fix: align Homebrew release automation (#429)
- fix(storage): require explicit sqlite fallback (#442)
- fix: add utility pricing for Gemini flash variants (#425)
- fix(storage): preserve virtual key spend during usage updates
- fix(storage): reject placeholder vector backends (#443)
- fix(storage): avoid virtual key usage races
- fix(keys): bound last used cache
- fix(storage): persist virtual keys
- fix(keys): share key manager across requests
- fix(release): publish gateway binary only (#431)
- fix(deps): eliminate vulnerable TLS and YAML chains
- fix(sdk): wire execute_stream_request to provider dispatch (#396) (#402)
- fix(sdk): parse data URI to extract correct media_type for Anthropic multimodal (#401)
- fix(sdk): implement atomic round-robin rotation in LoadBalancer (#397)
- fix(auth): guard is_admin_route() against prefix confusion (SEC-04) (#393)
- fix(auth): replace prefix match with exact equality in is_public_route (#390)
- fix(errors): replace Box<dyn Error> with typed errors at trait boundaries (#384)
- fix(a2a): auto-trigger agent health checks before routing (#381)
- fix(mcp): add optional JSON Schema validation for MCP tool parameters (#380)
- fix(storage): wrap multi-step DB operations in SeaORM transactions (#377)
- fix(storage): add cache-aside invalidation on API key usage write (#378)
- fix(streaming): add CancellationToken to cancel provider streams on client disconnect (#379)
- fix(providers): wire 6 unreachable provider types into factory (#374)
- fix(security): redact sensitive fields in Debug impls for config structs (#369)
- fix(auth): tighten password reset rate limit to 5 requests per 15 minutes (#371)
- fix(rust): add rust-toolchain.toml pinning stable channel (#368)
- fix(router): wire min_requests and success_threshold into circuit breaker (#367)
- fix(providers): implement 5 missing from_config_async branches (#365)
- fix(a2a): replace hardcoded request ID=1 with unique IDs (#361)
- fix(streaming): add VecDeque buffer size limit to prevent OOM (#362)
- fix(responses): address 6 correctness issues from code review (#329)
- fix(responses): resolve CI failures in Responses API implementation (#328)
- fix(macros): remove dead helper functions from provider_config! macro (#321)
- fix(dead_code): resolve 55 of 56 dead_code suppressions (#278) (#320)
- fix(lib): restore FunctionCall and ToolCall to public re-exports (#319)
- fix(errors): replace .unwrap() in production hot paths (#261) (#312)
- fix(openai): update capability lists for GPT-5.4, o3, o4-mini (#274) (#306)
- fix(config): replace hardcoded default string comparison in StorageConfig merge logic (#310)
- fix(config): remove dead hot_reload entries from ConfigPresets (#308)
- fix(core): gate user_management behind storage feature flag (#300)
- fix: deep-merge reasoning object and make effort/max_tokens mutually exclusive (#301)
- fix(openrouter): add HTTP-Referer/X-Title headers and wire reasoning param (#299)
- fix: implement user_management DB ops and wire TeamManager to persistent storage (#298)
- fix: gate virtual_keys module behind gateway feature flag (#294)
- fix: resolve critical TODOs in teams, redis pubsub, and monitoring (#293)
- fix(security): migrate API key hashing to HMAC-SHA256 with server secret (#254)
- fix(auth): implement basic RBAC with admin/user roles in check_permission (#242) (#251)
- fix(security): enforce minimum 32-byte JWT secret length (#240) (#250)
- fix(security): reject empty OAuth allowed_origins instead of permitting all (#241) (#247)
- fix(router): add circular alias and fallback cycle detection (#214) (#234)
- fix(streaming): add idle timeout to SSE streams to prevent zombie connections (#205)
- fix(auth): reject empty JWT secret on startup instead of warn (#204)
- fix(auth): separate access and refresh token verification (#203)
- fix(router): use min_requests and success_threshold in circuit breaker (#200)
- fix(streaming): cancel upstream provider stream on client disconnect (#198)
- fix(provider): add missing from_config_async branches for catalog-covered provider types (#197)
- fix(config): change Redis default to enabled=false (#196)
- fix(streaming): handle SSE errors with proper error events instead of HTTP 200 (#185)
- fix(storage): replace relative ./data path with absolute path in local file storage (#184)
- fix(config): fix boolean merge one-way override in CacheConfig (#183)
- fix(a2a): replace hardcoded request ID=1 with atomic counter (#182)
- fix(config): validate port range to reject values >65535 (#181)
- fix(auth): add input validation for API key creation (#180)
- fix(router): rename CostBased strategy to PriorityBased (#178)
- fix(storage): implement 4 unimplemented S3 methods (#177)
- fix(streaming): add VecDeque buffer capacity limit to prevent OOM (#176)
- fix(security): redact secrets in Debug impl for AuthConfig and ProviderConfig (#175)
- fix(auth): add rate limiting to password reset endpoint (#174)
- fix(storage): replace hardcoded relative SQLite path with platform-aware default_sqlite_path() (#156)
- fix(perf): throttle api_key last_used DB writes to every 5 minutes (#153)
- fix(storage): apply max_connections config to Redis connection pool (#148)
- fix(storage): remove dead BatchOperations referencing nonexistent Database enum (#147)
- fix(security): mask usernames in login log messages to prevent PII leak (#146)
- fix(provider): replace from_f64().unwrap() with safe error handling across providers (#130)
- fix(auth): use transactional reset_password_with_token to eliminate TOCTOU race (#129)
- fix(perf): replace blocking parking_lot::Mutex with tokio::sync::Mutex in memory cache (#133)
- fix(api): forward stream_options field in chat completion requests (#131)
- fix: remove unwrap() panic in Mistral transform_request (closes #77) (#127)
- fix: remove unwrap() panics in vertex_ai provider (closes #78) (#128)
- fix: remove unwrap() panic in S3 cache storage_class parse (closes #76) (#126)
- fix: remove unwrap() panics in openai provider (closes #79) (#125)
- fix(server): mount missing auth/keys/teams/budget/health routes in create_app (#112)
- fix(provider): OpenAILikeProvider::name() returns actual provider name (#117)
- fix(middleware): X-Request-ID generated twice and not returned in responses (#111)
- fix(api): unify pricing routes from /api/v1/ to /v1/ prefix (#123)
- fix(cache): log Redis write failure in dual-cache set_with_size (#121)
- fix(middleware): remove no-op CorsMiddleware implementation (#120)
- fix(budget): eliminate TOCTOU race in create_budget() via Entry API (#116)
- fix(error): replace wildcard with explicit match arms for 11 GatewayError variants (#115)
- fix(sync): eliminate read-modify-write race in AtomicValue::update() (#114)
- fix(security): add ownership verification to API key CRUD endpoints (IDOR) (#110)
- fix(security): SSRF protection for custom API endpoint_url (#109)
- fix(auth): add IP-based rate limiting to /auth/login endpoint (#108)
- fix(api): GET /auth/me incorrectly registered as POST method (#113)
- fix(security): CORS empty origins list no longer defaults to wildcard '*' (#107)
- fix(auth): wrap password reset token ops in database transaction (#73)
- fix(server): add X-Forwarded-For trusted proxy validation (#72)
- fix(auth): replace unwrap_or_else with proper error handling in auth middleware (#67)
- fix(config): correct boolean merge logic in config system (#65)
- fix(deps): consolidate reqwest to single version 0.12.x (#48)
- fix(deps): upgrade quinn-proto to fix CVE-2026-0037 (#50)
- fix(deps): upgrade rand from 0.8 to 0.9 (#47)
- fix(lint): resolve 314 collapsible_if warnings for clippy 1.94.0 (#49)
- fix(security): add rate limiting and unify error messages for registration (#42)
- fix(security): reject session auth until proper session store is implemented (#41)
- fix(security): reject refresh tokens in authenticate_jwt (#39)
- fix(security): use SHA-256 for rate limit key hashing (#40)
- fix(ci): pin rust toolchain and add PR guardrails (#34)
- fix(security): consolidated security hardening — audit fixes, auth hash, env validation, route bypass, OAuth, concurrency (#33)
- fix(error): preserve provider identity in map_http_status_to_error (FUT-59) (#22)
- fix: harden boundary guard and stabilize router/error mapping integration (#14)
- fix(sse): map reasoning_content to thinking delta (#11)

### Changed
- style(config): format serde_norway migration cleanup
- refactor(deps): use explicit maintained crate names
- refactor(router): remove redundant dead-code zai/ prefix check (#405)
- refactor(providers): split LLMProvider into focused sub-traits (#383)
- fix(a2a): auto-trigger agent health checks before routing (#381)
- refactor: split factory.rs into registry, resolver, builder, coordinator modules (#317)
- refactor(config): split gateway.rs tests and fix pricing source path (#318)
- refactor(errors): split utils.rs (1435 lines) into focused sub-modules (#316)
- refactor(deps): replace async-trait with native AFIT in core traits (#246) (#252)
- refactor(provider): eliminate unwrap() in provider request/response paths (#245) (#248)
- refactor(provider): remove associated types from LLMProvider trait (#238)
- refactor: extract test modules from oversized gateway_error files (#221) (#237)
- refactor(config): consolidate default values into single source of truth (#235)
- refactor(error): simplify From<ProviderError> to use GatewayError::Provider directly (#233)
- refactor(storage): add transaction wrapping and optimistic locking for DB operations (#206) (#230)
- refactor(config): replace Arc<Config> with AtomicValue for atomic hot reload (#209) (#226)
- refactor(provider): consolidate 5 dispatch macros into single parametric macro (#224)
- refactor(quality): eliminate unwrap() calls in auth and security paths (#215) (#231)
- refactor(storage): remove dead legacy migration files (#222)
- refactor(error): consolidate GatewayError from 29 to 15 variants (#160)
- refactor(provider): remove orphan LLMProvider implementations (#159)
- refactor: split openai/transformer.rs into focused sub-modules (#154)
- refactor(provider): remove standalone impls for catalog-covered providers (#151)
- refactor(provider): remove dead Provider enum variants without factory paths (#150)
- refactor(provider): consolidate OpenAI dual LLMProvider implementations (#149)
- refactor: remove deprecated legacy config types (#132)
- refactor: remove duplicate LiteLLMError and OpenAIError type definitions (#124)
- refactor: 3-phase architectural refactoring (God Module, Type, Error) (#58)
- perf(observability): shorten record_request write lock hold time (#19)
- perf(recovery): remove blocking mutexes in circuit breaker async path (#15)
- perf(health): replace std rwlock with async monitor locks (#20)
- perf(cache): remove deep clone in hit path via Arc payload (#16)
- refactor(streaming): dedupe done marker handling for pilot providers (#24)




### Removed
- Removed the legacy `google-gateway` binary from Cargo, release archives, CI artifacts, and Docker images. The published gateway distribution now focuses on the main `gateway` executable.

## [0.4.2] - 2026-02-28

### Fixed
- fix(ci): fallback to grep when ripgrep is unavailable




## [0.4.1] - 2026-02-28

### Fixed
- fix(clippy): satisfy strict lints in audio service and router tests




## [0.4.0] - 2026-02-28




### Changed
- **Provider Infra**: `BaseConfig::for_provider()` now delegates environment loading with the original provider input while keeping normalized default resolution in one place, removing duplicated normalization flow.
- **Provider Infra**: `BaseConfig::provider_env_key()` env-key normalization now explicitly covers trimmed/case-variant provider input via regression test.
- **Provider Infra**: `BaseConfig::provider_env_key()` now normalizes provider names internally, and `from_env()` reuses normalized env helpers directly to remove duplicated normalization flow.
- **Provider Infra**: Centralized provider environment variable key/value resolution in `BaseConfig` helpers (`provider_env_key`, `env_value`) to remove repeated env lookup formatting.
- **Provider Infra**: Centralized endpoint URL construction in `BaseConfig::build_endpoint()` and reused it for chat/embeddings endpoints to remove duplicated formatting logic.
- **Provider Infra**: Centralized default API version assignment in `BaseConfig::default_api_version()` to remove repeated provider-specific conditionals.
- **Provider Infra**: `BaseConfig::for_provider` now normalizes provider names (trim + lowercase) before catalog/fallback resolution to prevent casing/spacing drift.
- **Provider Infra**: Removed legacy alias fallback in `BaseConfig` and kept canonical provider-name defaults only to avoid alias drift.
- **Provider Infra**: Extracted `legacy_default_base_url()` helper in `BaseConfig` to isolate non-catalog fallback mapping and simplify maintenance while preserving behavior.
- **Provider Infra**: `BaseConfig::for_provider` now consults Tier-1 provider catalog defaults first, reducing duplicated base URL definitions while preserving existing fallback behavior.
- **Provider Infra**: Removed the unused `CommonProviderConfig` duplicate from `core::providers::shared`, keeping provider base config responsibilities centralized in `core::providers::base` and reducing schema duplication.

### Added
- **Provider Tests**: Added B1 batch coverage to validate `aiml_api`, `anyscale`, `bytez`, and `comet_api` selectors and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.
- **Provider Tests**: Added B2 batch coverage to validate `compactifai`, `aleph_alpha`, `yi`, and `lambda_ai` selector and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.
- **Provider Tests**: Added B3 batch coverage to validate `ovhcloud`, `maritalk`, `siliconflow`, and `lemonade` selector and creation paths resolve through Tier-1 catalog to `OpenAILike` providers.

## [0.3.0] - 2026-02-05

### Added
- **Agent Coordinator**: New `core::agent` module for managing concurrent agent lifecycles with cancellation, timeouts, and stats.
- **Utilities**: Added `utils::event` publish/subscribe broker and `utils::sync` concurrent containers.

### Changed
- **Providers**: Migrated `ai21`, `amazon_nova`, `datarobot`, and `deepseek` to pooled HTTP provider hooks.
- **HTTP Client**: Standardized pooled client usage and shared client caching across core/providers.
- **Routing**: Refined provider routing and OpenAI-compatible request/response handling.

### Fixed
- **Auth Context**: Corrected user/api-key context propagation in auth routes and middleware.
- **SSRF Validation**: DNS resolution failures no longer hard-fail SSRF checks while preserving IP safety.
- **Observability**: Prometheus label handling now safely maps provider identifiers.
- **Concurrency**: Event broker handles zero capacity; VersionedMap retry now guarantees progress under contention.
- **Packaging**: Track core cache sources and add root README for crates.io.

## [0.1.3] - 2025-09-18

### Fixed
- **docs.rs Build**: Fixed documentation build failure on docs.rs by excluding `vector-db` feature
  - Added `all-features = false` to `package.metadata.docs.rs` configuration
  - Explicitly listed features that work with docs.rs read-only filesystem
- **Internationalization**: Translated all Chinese comments and documentation to English
  - Cleaned 40+ files with hundreds of Chinese comments
  - Improved accessibility for international developers
  - Maintained technical accuracy in all translations

### Changed
- **Configuration**: Updated `Cargo.toml` metadata for better docs.rs compatibility
- **Documentation**: All code comments are now in English

## [0.1.1] - 2025-7-28

### Fixed
- **Security**: Excluded sensitive configuration file `config/gateway.yaml` from published package
- **Package**: Only include example configuration files (`.example`, `.template`) in published crate
- **Privacy**: Prevent accidental exposure of API keys and secrets in published package

## [0.1.0] - 2025-07-28

### Added
- Initial release of Rust LiteLLM Gateway
- High-performance AI Gateway with OpenAI-compatible APIs
- Intelligent routing and load balancing capabilities
- Support for multiple AI providers (OpenAI, Anthropic, Google, etc.)
- Enterprise features including authentication and monitoring
- Actix-web based web server with async/await support
- PostgreSQL and Redis integration for data persistence and caching
- Comprehensive configuration management via YAML
- Rate limiting and request throttling
- WebSocket support for real-time communication
- Prometheus metrics integration
- OpenTelemetry tracing support
- Vector database integration (Qdrant)
- S3-compatible object storage support
- JWT-based authentication system
- Docker and Kubernetes deployment configurations
- Comprehensive API documentation
- Integration tests and examples

### Features
- **Core Gateway**: OpenAI-compatible API endpoints
- **Multi-Provider Support**: Seamless integration with various AI providers
- **Load Balancing**: Intelligent request distribution
- **Caching**: Redis-based response caching
- **Monitoring**: Prometheus metrics and OpenTelemetry tracing
- **Authentication**: JWT-based security
- **Rate Limiting**: Configurable request throttling
- **WebSocket**: Real-time streaming support
- **Storage**: PostgreSQL for persistence, S3 for object storage
- **Vector DB**: Qdrant integration for embeddings
- **Deployment**: Docker, Kubernetes, and systemd configurations

[Unreleased]: https://github.com/majiayu000/litellm-rs/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/majiayu000/litellm-rs/compare/v0.1.3...v0.3.0
[0.1.3]: https://github.com/majiayu000/litellm-rs/compare/v0.1.1...v0.1.3
[0.1.1]: https://github.com/majiayu000/litellm-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/majiayu000/litellm-rs/releases/tag/v0.1.0
