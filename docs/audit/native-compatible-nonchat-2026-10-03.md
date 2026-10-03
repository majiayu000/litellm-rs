# Native protocol boundaries for compatible selectors — 2026-10-03

Issue #1427, F11 bounded batch. Baseline `1723787d9c659264dbb6622ee41328bcc3b3af30`. Covers Volcengine, MiniMax, SambaNova and Hyperbolic. No paid calls or account/model availability verification.

Decision: adapt the existing text embeddings transport for Volcengine and correct MiniMax's documented OpenAI base. Ark requires an input array, so a single text string becomes a one-element array at the transport boundary. Reuse existing factory/Router, usage, errors and budget processing. A provider-wide generic adapter or new configuration is unnecessary. The native APIs below need distinct field/response/billing handling; their existence alone does not certify the current standard transport.

| Selector | Verified official contract | Result and remaining work |
| --- | --- | --- |
| `volcengine` | Ark `/api/v3/embeddings`: Bearer JSON model/Endpoint ID plus input string array, float vectors and prompt/total token usage. Input max 256 nonempty texts, each within model token limit and 100,000 UTF-8 bytes. | Enable text embeddings, normalize single-string input. No dimensions, token-array input or base64-output guarantee. Multimodal `/embeddings/multimodal` has typed content and a different response contract. |
| `volcengine` images/audio | Seedream `/api/v3/images/generations` exists, with sequential-image controls, model-dependent sizes and per-image errors/usage. Voice products use separate protocols. | Image `n` does not map directly to sequential generation across all current models; generic success parsing cannot yet represent every partial-error/usage field. Keep images/audio undeclared pending explicit adaptation rather than silently ignoring parameters or charging failed outputs. |
| `minimax` | Official OpenAI SDK reference uses `https://api.minimax.io/v1`. Current ASR uses `/v1/speech_to_text` multipart with language in an HTTP header, JSON text/duration billed by seconds. TTS uses `/v1/t2a_v2`, nested voice/audio settings, hex audio and base_resp status. Images use native request/response envelopes. | Correct catalog default host; preserve explicit custom base. Standalone audio/image capability remains unadvertised until request transformation, status handling, output conversion and billing are wired. ASR similarity to multipart alone is insufficient. No embeddings protocol in reviewed current index. |
| `sambanova` | Current English feature docs explicitly restrict E5-Mistral embeddings and Whisper transcriptions/translations to SambaStack. Cloud deprecation record removes E5-Mistral on 2026-04-06. | Keep default public SambaCloud non-chat capabilities unchanged. Dedicated SambaStack support needs an explicit deployment scope; do not use older Japanese cloud-endpoint pages as current cloud availability proof. No image generation/TTS protocol established. |
| `hyperbolic` | Old `docs.hyperbolic.xyz` now redirects to a new Hyperbolic GPU infrastructure documentation site. Current official index describes rentals, storage, Kubernetes and account APIs. | Hosted inference embeddings/images/audio contract is unconfirmed in current material. Keep capabilities unchanged. Redirect/absence is not sufficient evidence to remove existing chat or claim retirement; dedicated serving protocols require separate evidence. |

No callable models or prices are inferred from a price table. Existing explicit prices are still required for unknown deployments; a documented model ID in an API example is not a claim that every account can currently call it.

## Validation

Local HTTP tests cover actual factory/Router at `/api/v3/embeddings`, array/string input, model identity, vectors/token usage, 400/429 contracts and unsupported modes. Gateway budget regression covers missing prices, rejection before dispatch and actual usage settlement. Passed: 26 HTTP/gateway tests; fmt/check; default complete tests (7,141 library passed, 1 ignored, plus integration/doc suites); default and gateway/sqlite all-target clippy; gateway/sqlite complete tests (9,514 library passed, 1 ignored, plus integration/doc suites). Full tests used `--test-threads=2`. These are local protocol simulations, not cloud or SambaStack calls.

## Source evidence

Retrieved 2026-10-03; SHA-256 hashes identify the downloaded documents. Mutable sources can change. The Volcengine API document was additionally read through official indexed page content because portions are client rendered.

- [volc-embed](https://api.volcengine.com/api-docs/view?action=Embeddings&serviceCode=ark&version=2024-01-01): SHA-256 `6c766633ab07c044c6673ad2109193d5f9f66fe028bf84a2b4d0c14e505e6e32`.
- [volc-images](https://docs.volcengine.com/docs/ark/image-generation-api?lang=en): SHA-256 `7d71d42df869cb167735f011d4e615ba36e1a6ad0d30a21c6d24abd5a2532575`.
- [minimax-index](https://platform.minimax.io/docs/llms.txt): SHA-256 `4864ebab0e904024639b47b642b27c940b96d953ae157536073a1510458e6bbd`.
- [minimax-openai](https://platform.minimax.io/docs/api-reference/text-openai-api.md): SHA-256 `ef2370c6558c72bb5c285e7c54749448e43904d1d05a5b3bab16597410063864`.
- [minimax-asr](https://platform.minimax.io/docs/api-reference/speech-to-text.md): SHA-256 `4c7c0d00394c38b0f48e97607351705ebffa35fdf0f3754e05667a4656900a17`.
- [minimax-speech](https://platform.minimax.io/docs/api-reference/speech-t2a-http.md): SHA-256 `b1525d9f1b5fe77927584ead7644e9704a7cf0758956c7f0807f3e5e13fdba4d`.
- [minimax-images](https://platform.minimax.io/docs/api-reference/image-generation-t2i.md): SHA-256 `22683b9f905175c6ff561ef71d291e61dbf3f29c5ca7c1b25fb28ef60cb4f2bb`.
- [sambanova-audio-current](https://docs.sambanova.ai/docs/en/features/audio.md): SHA-256 `c42cd1863b300425783a5dc18aa837e8c511bebacc66b39ebf61a3b0dcededd0`.
- [sambanova-embed-current](https://docs.sambanova.ai/docs/en/features/embeddings.md): SHA-256 `c744e8a76156518acadc74f22edefc21eebd58831b42e1f516999aa6444273f4`.
- [sambanova-retirement](https://docs.sambanova.ai/docs/en/models/deprecations.md): SHA-256 `ddccc28a762c84624f1a6b0748df95a8aa002193cdadc5f639a78c34de523ffe`.
- [hyperbolic-current-index](https://www.hyperbolic.ai/docs/llms.txt): SHA-256 `646bf005f3d9b8c5097b6798ed5b5fd6814aaa3962beb2047de0246decd8287c`.
- [Historical Japanese SambaNova translation page](https://docs.sambanova.ai/docs/ja/api-reference/endpoints/translation): older cloud URL example is superseded for availability by the current English feature restrictions above.
