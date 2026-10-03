# 已接 embeddings 选择器的媒体协议复核

日期：2026-10-03；基线 `b19e8fc9bd7c2b2baf3cf056aab306aa67b9e28a`；issue #1435。本批只有协议审计与文档变更，没有新增运行时能力，也没有供应商账户调用。F11 不能因每个选择器有一条审核记录就标为实现完成。

| 选择器 | 已验证的供应商事实 | 当前运行时边界与最小后续工作 |
| --- | --- | --- |
| openrouter | [独立 Image API](https://openrouter.ai/docs/guides/overview/multimodal/image-generation)提供生成与 usage/cost；[TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts)是 `/api/v1/audio/speech` 返回二进制，默认 PCM，多数模型按字符、部分按生成时长计费；[STT](https://openrouter.ai/docs/guides/overview/multimodal/stt)是 `/api/v1/audio/transcriptions`，JSON `input_audio`，按时长或 tokens，返回 usage.seconds/tokens/cost | 已实现 embeddings。不能继续把其他项笼统写成没有协议；但 STT 不能直接套用 multipart，TTS 不能把所有型号统一按字符，图片不能忽略最终实际 usage。最小后续分别适配 JSON 转录、明确语音计费单位/原始音频元数据、图像模态 usage 结算；不扩视频、克隆或 server tools。 |
| xai | [图像参考](https://docs.x.ai/developers/rest-api-reference/inference/images)具有 `/v1/images/generations`、模态 token usage，未承诺通用响应必需的 created；[TTS](https://docs.x.ai/developers/model-capabilities/audio/text-to-speech)为 `/v1/tts`、text/voice_id/language/output_format；[STT](https://docs.x.ai/developers/model-capabilities/audio/speech-to-text)为 `/v1/stt` multipart、返回 duration 和 words[].text | 当前选择器 Chat 模型登记与媒体调度不同，未接这些媒体能力；不能只加静态 capability。后续需原生请求字段/路径及响应转换、具体媒体模型身份和计费；没有确认独立 embeddings。F06 Responses-only 路由不在本批实现。 |
| nvidia_nim | [SDXL REST](https://docs.api.nvidia.com/nim/reference/stabilityai-stable-diffusion-xl-infer)使用 `ai.api.nvidia.com/v1/genai/stabilityai/stable-diffusion-xl`、text_prompts/width/height；[Speech NIM ASR](https://docs.nvidia.com/nim/speech/26.05.0/reference/api-references/asr/http-asr.html)有独立部署的 `/v1/audio/transcriptions`；[Riva 端口说明](https://docs.nvidia.com/deeplearning/riva/user-guide/docs/quick-start-guide.html)区分 gRPC/HTTP/WebSocket | 默认 `integrate.api.nvidia.com/v1` 的已验证范围仍是 embeddings。不能把独立部署的 Speech NIM 或不同主机的 genai 路径当成默认兼容 base 的音频/图片支持。后续应先明确部署地址及模型，再接实际协议和计费；不将图片/音频输入的语言模型误认作媒体生成。 |
| fireworks / fireworks_ai | 当前[官方索引](https://docs.fireworks.ai/llms.txt)与[音视频输入文档](https://docs.fireworks.ai/guides/video-audio-inputs)描述多模态 chat 和 dedicated 部署；[SDXL ControlNet FAQ](https://docs.fireworks.ai/faq-new/models-inference/how-do-i-control-output-image-sizes-when-using-sdxl-controlnet)仍描述原生尺寸字段 | 已验证 embeddings；既有图片独立路径仍需适配。索引未取得足够独立 ASR/TTS 端点及当前账户可用证据，不能从 chat 音频输入推导 `/audio/*`。这也不是退役证据，保留“未确认”。后续需当前原生媒体端点、模型和计费合同。 |
| nebius | [当前官方图片示例](https://docs.tokenfactory.nebius.com/api-reference/examples/image-generation)请求 width/height，响应是 data/id 而没有 created；[官方索引](https://docs.tokenfactory.nebius.com/llms.txt)区分 inference 与 sandbox container images | 已验证 embeddings；图片仍需参数/响应适配，不能以补造上游 created 掩盖协议差异。未取得独立音频合同，不推断没有该能力。sandbox images 是容器镜像，不是模型图像生成。 |

## 与当前实现逐项对照

- `src/core/providers/openai/api_methods.rs` 的转录助手构造 multipart，不能直接发送 OpenRouter 的 JSON input_audio，也不能自动改成 xAI `/stt`。
- `src/core/providers/openai_like/provider.rs` 的音频/图片分派沿用兼容 endpoint 和通用结构。默认 `/audio/speech` 请求字段不是 xAI `/tts` 合同；TTS 默认格式也应遵循对应协议。
- `src/server/routes/ai/images/generation.rs` 在请求前产生 estimated usage，最终结算继续使用该值。启用按模态 token/cost 计费的新媒体型号前，须让预留保持保守、结算使用实际可验证用量；不能拿“图片HTTP成功”代替计费验收。已有图片能力同样仍需按实际型号复核计费单位。
- `src/server/routes/ai/audio/speech.rs` 需要为语音计费识别字符/时长/token；供应商级 capability 不能保证任意模型都符合已测试单位。

决策：此批保存差异，不以新框架统一原生媒体。后续按单协议采用最小既有传输适配，并测试实际 factory/Router、字段/格式、上游错误、缺价、预留与返回用量结算。公共数据价格不作为 callable 的证据；真实账户可用性未测试。

验证：核对上述官方文档与当前源文件；`git diff --check` 通过。本次只有 Markdown，没有新增或重跑 Rust 行为测试；不沿用其他 PR 的测试结果宣称这些未实现媒体已通过。

## 可复查下载

下面列出成功保存的原始内容哈希；未成功下载的页面只提供官方链接，不伪造快照。原始文件在 `/tmp/litellm-media-followup-sources`。

| 来源 | SHA-256 |
| --- | --- |
| [xai-image](https://docs.x.ai/developers/rest-api-reference/inference/images.md) | `29936d730ae04a61fef85f9488224bd2309c9140418a06133a9d20025a9a3f4c` |
| [xai-voice](https://docs.x.ai/developers/rest-api-reference/inference/voice.md) | `1fff1df5652a33a616f386800648be019bd210036522b2768ffbbc0f6a96aee2` |
| [xai-pricing](https://docs.x.ai/developers/pricing.md) | `be9f024e79261c02d9e22a6845e3b991ccb4cc13b867d63381aa4b8535ceec93` |
| [fireworks-index](https://docs.fireworks.ai/llms.txt) | `172663c62a95268fc4830fe83c3e0f009ec72fd5d7439913563184d8be24f768` |
| [nebius-index](https://docs.tokenfactory.nebius.com/llms.txt) | `3cf8bf6b3f567b792a596d834fcd8752c3fe20a126dcbe09c7f4b04309e73e1e` |
| [fireworks-audio](https://docs.fireworks.ai/guides/video-audio-inputs.md) | `78fbdcbc188d3d833630a20e8d59ce9a97b629cf9be6bb9b819fcb3352d52688` |
| [fireworks-image](https://docs.fireworks.ai/faq-new/models-inference/how-do-i-control-output-image-sizes-when-using-sdxl-controlnet.md) | `05557d3cd6d0b90511520a65ba8e1ac2fbba891331aabe6eae67ccd274e51923` |
| [nebius-image](https://docs.tokenfactory.nebius.com/api-reference/examples/image-generation.md) | `3b0118ed767bcf5e82658fef8276047dcfd70c250b2aa249194ea4dcde7a4cfd` |
| [xai-tts](https://docs.x.ai/developers/model-capabilities/audio/text-to-speech.md) | `456d7a3ae3b1f51306e3d3a5f5779dff667d0a83aa1721b947da927a09c58645` |
| [xai-stt](https://docs.x.ai/developers/model-capabilities/audio/speech-to-text.md) | `cfb5cb2c4e145ea1f36168dfec0a106e437194e0d3421848bd534edbb0e65efc` |
