# 已接 embeddings 选择器的媒体协议复核

日期：2026-10-03；基线 `b19e8fc9bd7c2b2baf3cf056aab306aa67b9e28a`；issue #1435。本批只有协议审计与文档变更，没有新增运行时能力，也没有供应商账户调用。F11 不能因每个选择器有一条审核记录就标为实现完成。

| 选择器 | 已验证的供应商事实 | 当前运行时边界与最小后续工作 |
| --- | --- | --- |
| openrouter | [独立 Image API](https://openrouter.ai/docs/guides/overview/multimodal/image-generation)提供生成与 usage/cost；[TTS](https://openrouter.ai/docs/guides/overview/multimodal/tts)是 `/api/v1/audio/speech` 返回二进制，默认 PCM，多数模型按字符、部分按生成时长计费；[STT](https://openrouter.ai/docs/guides/overview/multimodal/stt)是 `/api/v1/audio/transcriptions`，支持 JSON `input_audio` 和 ≤25 MB 的 OpenAI 兼容 multipart；按时长或 tokens，返回可选 usage.seconds/tokens/cost | 已实现 embeddings。不能继续把其他项笼统写成没有协议；STT 的小文件上传可以沿用既有 multipart，仍须核对各模型参数/格式支持与实际用量结算；大文件和 JSON 专用输入另行适配。TTS 须明确不同模型的字符/时长单位，图片须核对实际 usage/cost；不扩视频、克隆或 server tools。 |
| xai | [图像参考](https://docs.x.ai/developers/rest-api-reference/inference/images)具有 `/v1/images/generations`，返回 `usage.cost_in_usd_ticks`；[官方价格](https://docs.x.ai/developers/pricing)按图片及分辨率计价，未承诺通用响应必需的 created；[TTS](https://docs.x.ai/developers/model-capabilities/audio/text-to-speech)为 `/v1/tts`、text/voice_id/language/output_format；[STT](https://docs.x.ai/developers/model-capabilities/audio/speech-to-text)为 `/v1/stt` multipart、返回 duration 和 words[].text | 当前选择器 Chat 模型登记与媒体调度不同，未接这些媒体能力；不能只加静态 capability。后续需原生请求字段/路径及响应转换、具体媒体模型身份和计费；没有确认独立 embeddings。F06 Responses-only 路由不在本批实现。 |
| nvidia_nim | [SDXL REST](https://docs.api.nvidia.com/nim/reference/stabilityai-stable-diffusion-xl-infer)使用 `ai.api.nvidia.com/v1/genai/stabilityai/stable-diffusion-xl`、text_prompts/width/height；[Speech NIM ASR](https://docs.nvidia.com/nim/speech/26.05.0/reference/api-references/asr/http-asr.html)有独立部署的 `/v1/audio/transcriptions`；[Riva 端口说明](https://docs.nvidia.com/deeplearning/riva/user-guide/docs/quick-start-guide.html)区分 gRPC/HTTP/WebSocket | 默认 `integrate.api.nvidia.com/v1` 的已验证范围仍是 embeddings。不能把独立部署的 Speech NIM 或不同主机的 genai 路径当成默认兼容 base 的音频/图片支持。后续应先明确部署地址及模型，再接实际协议和计费；不将图片/音频输入的语言模型误认作媒体生成。 |
| fireworks / fireworks_ai | 当前[官方索引](https://docs.fireworks.ai/llms.txt)与[音视频输入文档](https://docs.fireworks.ai/guides/video-audio-inputs)描述多模态 chat 和 dedicated 部署；[SDXL ControlNet FAQ](https://docs.fireworks.ai/faq-new/models-inference/how-do-i-control-output-image-sizes-when-using-sdxl-controlnet)仍描述原生尺寸字段 | 已验证 embeddings；既有图片独立路径仍需适配。索引未取得足够独立 ASR/TTS 端点及当前账户可用证据，不能从 chat 音频输入推导 `/audio/*`。这也不是退役证据，保留“未确认”。后续需当前原生媒体端点、模型和计费合同。 |
| nebius | [当前官方图片示例](https://docs.tokenfactory.nebius.com/api-reference/examples/image-generation)请求 width/height，响应是 data/id 而没有 created；[官方索引](https://docs.tokenfactory.nebius.com/llms.txt)区分 inference 与 sandbox container images | 已验证 embeddings；图片仍需参数/响应适配，不能以补造上游 created 掩盖协议差异。未取得独立音频合同，不推断没有该能力。sandbox images 是容器镜像，不是模型图像生成。 |

## 与当前实现逐项对照

- `src/core/providers/openai/api_methods.rs` 的转录助手构造 multipart，可用于 OpenRouter ≤25 MB 的兼容上传；其 JSON input_audio/大文件能力和 xAI `/stt` 路径仍需各自适配。
- `src/core/providers/openai_like/provider.rs` 的音频/图片分派沿用兼容 endpoint 和通用结构。默认 `/audio/speech` 请求字段不是 xAI `/tts` 合同；TTS 默认格式也应遵循对应协议。
- `src/server/routes/ai/images/generation.rs` 在请求前产生 estimated usage，最终结算继续使用该值。启用新媒体型号前，须按具体协议保持保守预留并使用实际可验证用量/费用结算；OpenRouter 可返回 seconds/tokens/cost，xAI 图片使用返回费用或明确配置的每图价格，不能套用不存在的 token 计数；不能拿“图片HTTP成功”代替计费验收。已有图片能力同样仍需按实际型号复核计费单位。
- `src/server/routes/ai/audio/speech.rs` 需要为语音计费识别字符/时长/token；供应商级 capability 不能保证任意模型都符合已测试单位。

决策：此批保存差异，不以新框架统一原生媒体。后续按单协议采用最小既有传输适配，并测试实际 factory/Router、字段/格式、上游错误、缺价、预留与返回用量结算。公共数据价格不作为 callable 的证据；真实账户可用性未测试。

验证：核对上述官方文档与当前源文件；`git diff --check` 通过。本次只有 Markdown，没有新增或重跑 Rust 行为测试；不沿用其他 PR 的测试结果宣称这些未实现媒体已通过。

## 来源与核验时点

上述官方链接于 2026-10-03 初次核验，OpenRouter multipart 与 xAI 图片计费于 2026-10-04 再核验。文档为动态页面，后续可能变化；本 PR 不提供固定版本的下载快照，也不将本机临时文件或其哈希作为可携带证据。复核时应重新读取对应官方合同，并与指定源码基线比较。
