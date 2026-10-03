# Aggregator non-chat protocol audit — 2026-10-03

Issue #1422, F11 bounded batch. Baseline `5753956ae7519ae944eee063c35a3445d1e8800e`. The audit covers existing selectors, not models inferred from pricing files. No paid or vendor-account calls were made.

Decision: adapt existing OpenAI-like transport and factory/Router capabilities. Only NanoGPT needs an endpoint-specific base adjustment: embeddings/chat/audio use `/api/v1`, whereas compatible images use `/v1`. Reuse JSON, multipart and buffered binary implementations; no SDK, configuration field, capability-discovery layer or asynchronous job framework is needed for this scope. Native asynchronous protocols are explicitly outstanding.

## Per-selector evidence and limits

| Selector | Official protocol evidence | Result and remaining limits |
| --- | --- | --- |
| `novita` | Official embeddings schema uses model/input, floating vectors and token usage; official LangChain integration uses `https://api.novita.ai/v3/openai`. | Enable text embeddings. Configure a supported embedding ID; do not route chat models as embeddings by assumption. Qwen image API is an asynchronous `/v3/async` task contract; MiniMax speech returns nested JSON/audio metadata rather than raw binary. Both require separate adaptation. |
| `featherless` | Official `/v1/embeddings` supports string/list input, dimensions where model supports them and token usage. `/v1/audio/speech` supports default buffered binary and input-character billing. | Enable text embeddings and synchronous TTS. Use the actual model catalog/voice guide. String `default` voice works when no preset exists. Requested codec is not guaranteed; preserve actual response Content-Type. No object-valued voice cloning, SSE/base64 speech, images or transcription claim. |
| `galadriel` | Official OpenAPI publishes `/v1/embeddings` and `/v1/images/generations` with standard request/response schemas. | Enable those two protocols. Current model documentation focuses on verified chat and does not establish specific current non-chat model IDs. Account/model availability remains unverified; users must configure actual supported IDs. Image editing exists in OpenAPI but named multipart gateway dispatch is outstanding. No standalone audio evidence. |
| `nanogpt` | Official quickstart plus endpoint references distinguish `/api/v1/embeddings`, `/api/v1/audio/speech`, `/api/v1/audio/transcriptions` and root `/v1/images/generations`. | Fix incorrect `api.nanogpt.com` default to `api.nano-gpt.com/api/v1`. Enable these four modes. Text embeddings return token usage; ordinary transcription returns duration in seconds; synchronous TTS is input-character billed. Native queued `/api/tts`, music/cloning, asynchronous transcription, video and SSE audio are outside this validation. No audio translation claim. |
| `ai21` | Reviewed current official index and Jamba API reference: chat, Maestro and dedicated tools are documented. | No confirmed compatible embeddings/images/audio protocol in reviewed sources. Keep current capabilities; this is an unresolved scope, not proof of universal absence. |
| `cerebras` | Reviewed current official OpenAPI: only `/v1/chat/completions`. Audio guides use external voice services. | No new non-chat capability. Vision input is not image generation. Other standalone modes remain unconfirmed. |

Only text/floating embeddings and standard synchronous image responses are verified here. No new callable model list or price is synthesized from these schemas. Existing configured prices are required for unknown models, and existing budget behavior remains active. A model with an incompatible billing unit or asynchronous response is not certified by the provider-level protocol declaration; configure the documented synchronous models with matching price units. Upstream model/parameter errors remain errors.

## Validation

Local HTTP fixtures exercise the real factory and Router, standard vectors/usage, image payloads, binary audio, multipart transcription and duration, 400/429 errors, and rejection of unverified capabilities before any upstream call. A real NanoGPT gateway test covers missing price, budget rejection before dispatch and actual usage settlement. Passed: 23 HTTP/gateway integration tests; `cargo fmt --check`; `cargo check`; complete default tests (7,141 library tests passed, 1 ignored, plus integration/doc suites); default all-target clippy; gateway/sqlite all-target clippy and complete tests (9,514 library tests passed, 1 ignored, plus integration/doc suites). Full runs used `--test-threads=2`. Local simulations are not live model/account verification.

## Source evidence

Retrieved 2026-10-03. SHA-256 values identify locally retrieved source snapshots; mutable documentation may change. Featherless pages were read through the web tool (direct snapshot downloads returned 403), so no raw snapshot hash is claimed.

- [novita-embeddings](https://novita.ai/docs/api-reference/model-apis-llm-create-embeddings.md): SHA-256 `12a0230c3de6f8a96bdf69a89ddfde1eead3199d9f8f1b3ae2be58a2d42d97ad`.
- [novita-images](https://novita.ai/docs/api-reference/model-apis-qwen-image-txt2img.md): SHA-256 `c246e8293fe281d0361c6fb0ebb9cf42aac1cd9965132d1c4276db140656f2d8`.
- [novita-speech](https://novita.ai/docs/api-reference/model-apis-minimax-speech-2.8-hd.md): SHA-256 `c5b8ee3b0b435ca3a7a00ee149e9575fff9541adb5bc0f323447314239cd7020`.
- [nano-quickstart](https://docs.nano-gpt.com/quickstart.md): SHA-256 `d3e473d01b70de54e56f9ff1a2034a6856a08bec035f726d3dcea11588878052`.
- [nano-embeddings](https://docs.nano-gpt.com/api-reference/endpoint/embeddings.md): SHA-256 `a6a660830d04420ae2b15397043eb93842c751e11b3273f012be8459bb5ccacd`.
- [nano-images](https://docs.nano-gpt.com/api-reference/endpoint/image-generation-openai.md): SHA-256 `9205068f6ae62a6733b17ed393b4b90ec3696a7b0c6ed1a317b6d76afcd785a8`.
- [nano-transcription](https://docs.nano-gpt.com/api-reference/endpoint/audio-transcriptions.md): SHA-256 `aa6ee730652b488cf89612f445abf82942e40ba5abbfca43829a1ff877674d5b`.
- [nano-speech-compatible](https://docs.nano-gpt.com/api-reference/endpoint/speech.md): SHA-256 `d1c852f25218b6518ae213900a27400dfc5a4396716958fd83654d40fadc9f02`.
- [nano-stt-guide](https://docs.nano-gpt.com/api-reference/speech-to-text.md): SHA-256 `77fdba171ad34846def4621859de3a05ecfd212da2cd266a149d784ad47203fc`.
- [galadriel-openapi](https://docs.galadriel.com/openapi.json): SHA-256 `543bac795875b1416195181179452e954724762cbe245d65c3290d7874cd6016`.
- [galadriel-models](https://docs.galadriel.com/for-agents-developers/models.md): SHA-256 `764bcc3838507a95d8e039ba7acd1412903b83b792a84df04cad0ecc9894dc74`.
- [ai21-chat](https://docs.ai21.com/reference/jamba-1-6-api-ref.md): SHA-256 `ae0f466f3983c28928764361ea5144c1a0ae267209e4054ce621def9513a04f7`.
- [cerebras-openapi](https://inference-docs.cerebras.ai/api-reference/openapi.yaml): SHA-256 `45e0b45718e1c1562bf33d944b4a346615d6e59f824e864277ac0a0a9cdebcc8`.
- [Novita official LangChain integration](https://blogs.novita.ai/build-a-sales-analytics-system-with-langchain/): official OpenAIEmbeddings base example, reviewed via web.
- [Featherless embeddings](https://featherless.ai/docs/embeddings), [speech contract](https://featherless.ai/docs/api-reference-audio-speech), [Kokoro guide](https://featherless.ai/docs/audio-speech-model-kokoro): reviewed via web.
- [AI21 current index](https://docs.ai21.com/llms.txt), [Cerebras current index](https://inference-docs.cerebras.ai/llms.txt): reviewed for additional endpoints; absence from the index is not an assertion about undocumented APIs.
