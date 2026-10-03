# Regional compatible non-chat audit — 2026-10-03

Issue #1425, F11 bounded batch. Baseline `5753956ae7519ae944eee063c35a3445d1e8800e`. Scope: Baichuan, Moonshot, DeepSeek, Xiaomi MiMo, Yi and Maritalk embeddings/images/audio. No paid calls, credentials, live account/model validation or pricing-derived callable additions.

Decision: adapt existing catalog capability/transport only for Baichuan's documented standard embeddings. Existing JSON dispatch, token usage, errors and budgets already fit that contract; no new abstraction, SDK, configuration or model-discovery system is needed. Native chat-audio contracts need a separately scoped adapter rather than a false standard audio declaration.

| Selector | Official evidence | Decision and remaining limits |
| --- | --- | --- |
| `baichuan` | Official indexed documentation for `/v1/embeddings`: Bearer JSON model/input, float data and prompt/total token usage. Documented model `Baichuan-Text-Embedding`, 512-token input, batch ≤16, 1024 dimensions. The source says excess input/batch is truncated upstream. | Enable standard text embeddings through current factory/Router. Optional dimensions/base64 are not certified. Direct page currently renders an application/contact shell, whereas official search-index content provides the schema; account access/current model availability remains unverified. No static callable model or price is added. |
| `moonshot` | Current official documentation index redirects to Kimi platform and lists current model inference, vision, tools, files and agent APIs; reviewed model page describes multimodal chat. | No confirmed independent embeddings/image-generation/audio endpoint in reviewed materials. Image/video comprehension is not generation. Keep non-chat declarations unchanged; absence from the reviewed index is not proof an undocumented service cannot exist. |
| `deepseek` | Current API reference/quickstart covers chat, Responses, FIM completion, models, files and vision. | No confirmed embeddings/images/audio protocol. Responses belongs to the native-protocol routing work, not this bounded non-chat batch. Do not translate vision/file support into image-generation capability. |
| `xiaomi_mimo` | Official speech synthesis uses assistant text plus audio parameters in `/v1/chat/completions`; voice design/cloning require additional message content. Official ASR likewise uses single audio content input to the chat endpoint. | Real audio capability exists, but generic `/audio/speech` binary or `/audio/transcriptions` multipart is a different wire contract. Keep those modes unadvertised until dedicated request/response and billing adaptation. No embeddings/image-generation endpoint established. |
| `yi` | Official docs URL includes chat usage material in server-generated RSC data, while the visible document is mostly navigation. | Standalone embeddings/images/audio remain unconfirmed; preserve existing capability declarations. Do not infer availability from pricing or model family names. |
| `maritalk` | Official embeddings/RAG guide explicitly says Maritaca does not currently provide its own embedding model and recommends using DeepInfra separately. Current intro documents chat usage. | Do not send embeddings to Maritalk or silently reroute credentials to another provider. Independent images/audio remain unconfirmed. Existing DeepInfra configuration is the separate path for that provider. |

## Validation

HTTP simulations exercise actual Baichuan factory/Router dispatch, model/input preservation, token usage, 400/429 errors and unsupported image/audio selection. A real gateway test covers missing prices, an insufficient finite budget before upstream dispatch, and actual two-token cost settlement. These are protocol fixtures, not live vendor validation. Original PR head `cded6245` verification: 20 HTTP/gateway tests, fmt/check, default complete tests (7,141 library passed, 1 ignored, plus integration/doc suites), default and gateway/sqlite all-target clippy. Gateway/sqlite complete rerun passed (9,514 library passed, 1 ignored, plus integration/doc suites). First feature full run hung in the existing `test_completions_streaming_response_sends_sse_and_done` integration test and was stopped after 1,200 seconds; its exact targeted rerun passed one test, and one complete rerun passed. No unrelated streaming code was changed, so the intermittent hang is not claimed fixed.

## Dated sources

Raw snapshot SHA-256 hashes identify retrieved evidence; documentation remains mutable. Direct shell pages and parsed indexed protocol content are distinguished above rather than treated as identical evidence.

- [baichuan-embeddings](https://platform.baichuan-ai.com/docs/text-Embedding): SHA-256 `394c08b14be71d7f7e1f2f3b7afbc8eba5e35419c7bd901400050f302a227076`.
- [moonshot-index](https://platform.moonshot.cn/docs/llms.txt): SHA-256 `5310bda697f6ae6829548add82a40b766631b284a4045b454775d3420238aaab`.
- [moonshot-models](https://platform.kimi.com/docs/models.md): SHA-256 `5c910633721b155d39514ab2f19936134adb67e3a5fa3fac27142263b61650aa`.
- [deepseek-api](https://api-docs.deepseek.com/api/deepseek-api/): SHA-256 `4d0431066f79ba2c46896a48179b745d507f3bd307f1d1ab97fd717193d22104`.
- [mimo-speech](https://mimo.mi.com/docs/en-US/quick-start/usage-guide/audio/speech-synthesis-v2.5): SHA-256 `53b28bcb9b2adea1d055d19e02ac4217a8876b105730f6a2e7c15824f9fd379a`.
- [mimo-asr](https://mimo.mi.com/docs/en-US/api/audio/Speech-Recognition): SHA-256 `00525b24de60815ec95a578e99f5b3aa3740979f9948b7db5ddabb9bd70b8dd9`.
- [yi-docs](https://platform.lingyiwanwu.com/docs): SHA-256 `cb8facc744ac2644226244f01e9dc698b60c42f9748b512fb55bfe674381f76f`.
- [maritalk-intro](https://docs.maritaca.ai/pt/introducao): SHA-256 `28d49b9afbaa48b31039f13921ca15821f188047c269f6630bf40ee145254396`.
- [maritalk-embeddings](https://docs.maritaca.ai/pt/embeddings%2BSabia-4%2BRAG): SHA-256 `4292e54cebd2d4d18aeb7e75f301e5c860f9e6f02d73f060f243efbd15f9d68e`.
- [Baichuan indexed protocol](https://platform.baichuan-ai.com/docs/text-Embedding): official page content retrieved through web search on 2026-10-03; the raw HTML snapshot above is only the current page shell.
- [DeepSeek current quickstart](https://api-docs.deepseek.com/): retrieved via the `/llms.txt` URL which returned HTML quickstart, not an actual endpoint index. No claim is based on a nonexistent llms index.

## Review corrections

The existing wire boundary rejects non-text and batches over 16 before I/O, avoiding the documented silent truncation. The official [error table](https://platform.baichuan-ai.com/docs/errCode) assigns 429 to both rate limits and exhausted account balance; the documented balance message maps to QuotaExceeded, while rate limits retain Retry-After. Tests cover pre-dispatch rejection, 16-item dispatch, and both 429 meanings. Token limits still depend on the native model/tokenizer and are not approximated locally.
