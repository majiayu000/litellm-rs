# Remaining static model catalog review — 2026-10-04

Issue #1373. This closes the enumerated static-catalog review after the earlier
bounded audits. It establishes declarations and protocol coverage, not account
access. No paid supplier requests were made. Historical price amounts remain
unchanged; no unreviewed pricing row is promoted to callable.

## Evidence and implementation decision

Adapt the existing registry, classification ledger and native clients. No new
lifecycle engine, alias table, configuration or transport is introduced.
The existing native Bedrock path remains available for supported Nova models.
An explicitly configured OpenAI-compatible deployment remains available for
customer-hosted models. Neither option certifies a supplier account.

- [GitHub Models retirement](https://docs.github.com/en/github-models): entire service retired 2026-07-30; see the [16-ID audit](github-models-retirement-2026-10-03.md). Copilot is separate.
- [Lambda staff notice](https://deeptalk.lambda.ai/t/sunsetting-chat-sunsetting-inference/4744): shared Chat and Inference API retired 2025-09-25; remove `lambda_ai` from the default catalog, retain price rows.
- [AWS lifecycle](https://docs.aws.amazon.com/bedrock/latest/userguide/model-lifecycle-legacy.html) and [Nova 2 Lite contract](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-amazon-nova-2-lite.html): Bedrock model IDs do not establish the invented `api.nova.amazon.com` OpenAI transport. Disable that selector, retaining historical Nova pricing helpers. Premier EOL is 2026-09-14. Nova 2 Lite, Pro, Lite and Micro are not marked retired by this audit.
- [Current Meta Model API](https://dev.meta.ai/docs/models) and [Llama distinction](https://dev.meta.ai/help/about-model-api/llama-and-model-api): current hosted models are separate from downloadable Llama. No current official evidence establishes the old `api.llama.com/compat/v1` transport or its short IDs; disable its default selector. This is an unverified-protocol decision, not a claim that open-weight Llama retired.
- [Historical Vercel announcement](https://vercel.com/blog/v0-composite-model-family), [former Model API page](https://v0.dev/docs/v0-model-api), [former AI SDK provider page](https://ai-sdk.dev/providers/ai-sdk-providers/vercel), and [staff documentation thread](https://community.vercel.com/t/how-to-use-the-v0-model-api-in-your-own-tools-the-links-are-blank/31215): the model page now redirects to Platform API, the SDK page is 404, and current SDK source lacks the former provider. The search index alone does not establish availability. Remove invented `v0-default` and disable the unverified default selector. Historical `v0-1.0-md`, `v0-1.5-md`, `v0-1.5-lg` are not promoted into the current runtime catalog. No independent service shutdown is claimed. No default/provider alias silently rewrites an ID.
- [Deepgram model options](https://developers.deepgram.com/docs/model) and [TTS REST](https://developers.deepgram.com/reference/text-to-speech/speak-request): retain `nova-3`, `nova-3-general`, `nova-3-medical` (transcription), `aura-2-thalia-en` (speech). These use native audio units, not token prices.
- [ElevenLabs removal notice](https://elevenlabs.io/docs/changelog/2026/6/8) and [current models](https://elevenlabs.io/docs/overview/models): `scribe_v1` removed 2026-07-09. Remove it and its unverified experimental preview from the advertised list. Retain `scribe_v2`, `eleven_v3`, `eleven_multilingual_v2` with their audio-only capabilities. Removal of the experimental declaration does not claim an independently published shutdown date.

## Azure AI mapped authority

All 41 callable identities present at the start are accounted for here, including
the six already covered by the previous static-native review and the remaining
35 mapped identities. Source: [current Azure models](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-sold-directly-by-azure),
[partner models](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-from-partners),
and [Azure lifecycle](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-retirement-schedule).
Future retirement dates and deprecation do not establish shutdown.
Native-only entries become pricing-only because this adapter cannot execute their
protocol, even when the supplier still offers them. The remaining entries have
explicit endpoint/capability declarations. Bind those declarations once at
startup; price modes cannot reinstate chat or tools. In particular the official
Azure Llama 3.3/4 and Phi contracts do not claim tool calling. Text embedding
requests do not gain image-input capability from the supplier's wider offering.
Rerank retains its catalog identity; the gateway still has no Azure rerank dispatch.
FLUX retains the existing generation-only implementation.

| Exact identity | Decision / implemented surface |
| --- | --- |
| `Cohere-embed-v3-english` | embeddings |
| `Cohere-embed-v3-multilingual` | embeddings |
| `FLUX-1.1-pro` | image_generation |
| `FLUX.1-Kontext-pro` | image_generation |
| `Llama-3.3-70B-Instruct` | chat_completions |
| `Llama-4-Maverick-17B-128E-Instruct-FP8` | chat_completions |
| `Llama-4-Scout-17B-16E-Instruct` | chat_completions |
| `Phi-4` | chat_completions |
| `Phi-4-mini-instruct` | chat_completions |
| `Phi-4-mini-reasoning` | chat_completions |
| `Phi-4-multimodal-instruct` | chat_completions |
| `Phi-4-reasoning` | chat_completions |
| `claude-fable-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-haiku-4-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-opus-4-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-opus-4-6` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-opus-4-7` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-opus-4-8` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-opus-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-sonnet-4-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-sonnet-4-6` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `claude-sonnet-5` | pricing_only: Current Foundry model uses native Messages; azure_ai has no Messages adapter. |
| `Cohere-rerank-v4.0-fast` | rerank |
| `Cohere-rerank-v4.0-pro` | rerank |
| `embed-v-4-0` | embeddings |
| `gpt-5.4` | chat_completions |
| `gpt-5.4-mini` | chat_completions |
| `gpt-5.4-nano` | chat_completions |
| `gpt-5.4-pro` | pricing_only: Responses-only model; azure_ai has no native Responses adapter. |
| `gpt-5.5` | chat_completions |
| `gpt-oss-120b` | chat_completions |
| `grok-4` | chat_completions |
| `grok-4.3` | chat_completions |
| `grok-code-fast-1` | chat_completions |
| `mistral-document-ai-2512` | pricing_only: Current OCR model uses document/image-to-text protocol; azure_ai has no OCR adapter. |

## Nova and Meta: each former static identity

| Former selector | Exact IDs | Result |
| --- | --- | --- |
| amazon_nova | `amazon.nova-2-lite-v1:0`, `amazon.nova-pro-v1:0`, `amazon.nova-lite-v1:0`, `amazon.nova-micro-v1:0`, `amazon.nova-premier-v1:0` | No default OpenAI transport claim. Historical prices retained; Premier EOL separately verified. |
| meta_llama | `llama4-scout`, `llama4-maverick`, `llama3.3-70b`, `llama3.2-1b`, `llama3.2-3b`, `llama3.2-11b-vision`, `llama3.2-90b-vision`, `llama3.1-8b`, `llama3.1-405b`, `llama3.1-70b` | Remove unverified hosted short-ID declarations; downloadable weights and other hosting services are separate. |

## Non-Gemini Vertex: each metadata identity

[Google partner shutdown notices](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/deprecations/partner-models)
confirm Claude 3 Opus shutdown 2025-08-01, Claude 3 Haiku 2026-08-23,
Claude 3.5 Sonnet v2 2026-02-19 and both Jamba 1.5 models 2026-02-27.
[Open-model notices](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/deprecations/open-models)
are scoped to exact MaaS IDs; their future October 21 dates do not retire arbitrary
Llama strings or self-deployed endpoints.
The native client's published model list is Gemini-only. Its historical partner
helpers previously accepted enum metadata through a generic `predict` request;
that is not evidence of current native Claude/MaaS protocols. This audit limits
executable native requests and gateway model capabilities to the existing
verified Gemini catalog. Partner enum metadata remains historical helper data;
it cannot enable transport, token counting or route capabilities. Reimplementing
partner protocols is outside this retirement review.

| Historical metadata ID | Accounted result |
| --- | --- |
| `claude-opus-4-7` | No exact current native protocol verification; no callable declaration |
| `claude-opus-4-6@20260114` | No exact current native protocol verification; no callable declaration |
| `claude-opus-4-5@20251110` | No exact current native protocol verification; no callable declaration |
| `claude-sonnet-4-6` | No exact current native protocol verification; no callable declaration |
| `claude-sonnet-4-5@20250929` | No exact current native protocol verification; no callable declaration |
| `claude-haiku-4-5@20251001` | No exact current native protocol verification; no callable declaration |
| `claude-sonnet-4@20250514` | No exact current native protocol verification; no callable declaration |
| `claude-3-opus@20240229` | Shutdown 2025-08-01 |
| `claude-3-sonnet@20240229` | No exact current native protocol verification; no callable declaration |
| `claude-3-haiku@20240307` | Shutdown 2026-08-23 |
| `claude-3-5-sonnet@20241022` | Shutdown 2026-02-19 |
| `meta/llama3-70b-instruct-maas` | No exact current native protocol verification; no callable declaration |
| `meta/llama3-8b-instruct-maas` | No exact current native protocol verification; no callable declaration |
| `meta/llama-3.1-405b-instruct-maas` | No exact current native protocol verification; no callable declaration |
| `meta/llama-3.1-70b-instruct-maas` | No exact current native protocol verification; no callable declaration |
| `meta/llama-3.2-90b-vision-instruct-maas` | No exact current native protocol verification; no callable declaration |
| `meta/llama-4-scout-17b-16e-instruct` | No exact current native protocol verification; no callable declaration |
| `meta/llama-4-maverick-17b-128e-instruct` | No exact current native protocol verification; no callable declaration |
| `ai21/jamba-1.5-large` | Shutdown 2026-02-27 |
| `ai21/jamba-1.5-mini` | Shutdown 2026-02-27 |
| `ai21/jamba-2-instruct` | No exact current native protocol verification; no callable declaration |
| `mistral/mistral-large-2411` | No exact current native protocol verification; no callable declaration |
| `mistral/mistral-nemo` | No exact current native protocol verification; no callable declaration |
| `mistral/codestral-2501` | No exact current native protocol verification; no callable declaration |

OpenAI/Azure audio, Groq, xAI and CompactifAI were accounted for in earlier F10/F11
batches. Their current exact audio routing is not expanded here. Generic
passthrough providers and historical price rows are not static callable catalogs.

## Validation

- Default complete suite: 7,126 library tests passed, one ignored; integration and doctests passed.
- `gateway,sqlite,providers-extra` complete suite: 10,201 library tests passed, one ignored; integration and doctests passed.
- Default and feature all-target clippy (`-D warnings`), `cargo check --locked`, and formatting passed.
- 54 pricing/catalog Python tests and pinned-source sync check passed. Price amounts are unchanged.
- Tests cover native-only Azure mapping rejection, explicit capability projection, removal of retired audio declarations, unverified selector rejection, and Vertex rejection before authentication/transport while retaining customer-endpoint token counting.
- PR CI/review is still required; this file does not claim a merge or release.

## Final transport reconciliation

Azure AI native JSON responses now retain complete tool/function calls, and SSE
responses retain indexed tool deltas and fragmented legacy function arguments.
Legacy function request fields use the existing model-specific parameter gate;
non-tool models reject them before transport. Local HTTP fixtures exercise both
native response paths. The OpenAILike Azure AI fallback retains explicit custom model names and its
model-less configured-name chat/stream route without fabricating a catalog or
pricing identity; known pricing-only IDs still undergo the authority check.
The provider-specific GH837 migration guide now documents disabled Nova/Meta/v0
selectors and retired GitHub Models instead of promising catalog equivalence.
