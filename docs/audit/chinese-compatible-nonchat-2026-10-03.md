# 中国兼容供应商非聊天协议审计

核验日：2026-10-03。基线 `aa081f3bf774b6e596072cdc2687a2fe2a3edff9`。issue #1420。范围：dashscope/qwen、zhipu、zai、siliconflow 的 embeddings/images/audio。未进行账户实调，模型事实依据官方协议，价格不自动成为可调用证据。

决策：复用现有 factory、OpenAILike 传输、Router、缺价与预算机制；新增已证明的静态能力和智谱默认 WAV。无需新 SDK、配置或抽象。未兼容的返回结构和计费单位保留明确限制，不以“路径相同”代替协议审核。

| 选择器 | 官方证据与本次接入 | 模型/协议限制与剩余工作 |
| --- | --- | --- |
| dashscope / qwen | [OpenAI-compatible embeddings](https://www.alibabacloud.com/help/en/model-studio/embedding-interfaces-compatible-with-openai)：`/compatible-mode/v1/embeddings`，Bearer，model/input/encoding_format/dimensions，标准 data 与 usage | 文本 v3/v4 型号与维度/批次数按区域选择；v4常用默认1024维、最多10段，每段8192tokens。旧 dashscope 域名仍有效，新 workspace 域名通过已有 base 配置使用。多模态 embedding、sparse 输出不支持本兼容端点，图片与音频原生协议未接入；不从 Qwen 品牌或价格行泛化能力。 |
| zhipu embeddings | [文本嵌入](https://docs.bigmodel.cn/api-reference/模型-api/文本嵌入)：`/api/paas/v4/embeddings`，embedding-2/3，标准向量与 prompt/total tokens | 只测试文本输入/float；embedding-3 默认2048维，可选256/512/1024/2048，每段最多3072tokens/最多64条；embedding-2固定1024维。更多编码不宣称支持。 |
| zhipu images | [同步图像生成](https://docs.bigmodel.cn/api-reference/模型-api/图像生成)：`/api/paas/v4/images/generations`，model/prompt/quality/size，created/data/url | GLM-Image与CogView各模型尺寸/质量不同；本次保留标准字段并由上游验证。异步生成、扩展水印/用户字段和编辑未映射，不更改供应商默认水印策略。 |
| zhipu speech | [GLM-TTS](https://docs.bigmodel.cn/api-reference/模型-api/文本转语音)：`/api/paas/v4/audio/speech`，model/input/voice，非流式WAV/PCM字节；[定价](https://docs.bigmodel.cn/cn/guide/start/pricing)按字符 | 显式默认 WAV，显式 PCM 不改写，避免通用 MP3 默认与该模型不匹配。GLM-TTS按2元/万字符收费；运行时复用用户配置的准确币种/字符价格，不新增未经转换的USD价格。Unicode字符数量预算回归通过；克隆按次计费与streaming不在本批。 |
| zhipu ASR | [语音转文本](https://docs.bigmodel.cn/api-reference/模型-api/语音转文本) 返回text等字段，无usage；[官方定价](https://docs.bigmodel.cn/cn/guide/start/pricing)按输入tokens，输出不计费 | 不接入当前按秒的转写预算/结算；官方“约每秒”换算不是精确计费契约。真实token计量与保守结算方案另待实现。 |
| zai | [同步图片](https://docs.z.ai/api-reference/image/generate-image) 的 `/api/paas/v4/images/generations` 返回created/data/url，接入；[ASR](https://docs.z.ai/api-reference/audio/audio-transcriptions) 是独立音频协议 | 只声明当前官方已核验的同步图片。GLM-ASR 无当前可结算token用量，不提前挂载。国际文档未确认embeddings/TTS，不直接复制国内智谱能力。 |
| siliconflow embeddings | [创建向量](https://docs.siliconflow.cn/docs/api/embeddings-post)：`/v1/embeddings`，Bearer、model/input，标准data/usage | 接入文本浮点向量；模型名称取账户当前模型列表，维度/长度受模型限制。没有从价格表推断支持型号。 |
| siliconflow images/audio | [图片](https://docs.siliconflow.cn/docs/api/images-generations-post) 返回images/timings/seed；[语音合成](https://docs.siliconflow.cn/docs/api/audio-speech-post) 和 [转写](https://docs.siliconflow.cn/docs/api/audio-transcriptions-post) 已有端点 | 图片参数image_size/batch_size和返回值与当前统一schema不同，不先声明。TTS支持mp3/opus/wav/pcm但不同模型费用单位尚待逐项核验；转写回复只有text，计量策略尚未确认。保留未接入状态，不能描述为供应商没有能力。 |

## 验证

真实本地 HTTP 服务经过 create_provider 与 Router 选择。测试检查 DashScope 的 compatible-mode 路径、智谱/ZAI 的 paas/v4 路径、vector usage、URL 图片、不流式语音格式与二进制内容、400/429 Retry-After及不支持操作拒绝。真实网关语音预算测试覆盖缺价拒绝、预算不足不调用上游、`你好`按2个Unicode字符结算而非6个UTF-8字节。没有改通用预算公式、没有新增供应商配置、没有付费调用。

## 官方页面快照

以下SHA-256定位本次下载内容。网页未来变化需重新核验；不在仓库复制供应商文档全文。

| 页面 | SHA-256 |
| --- | --- |
| [zhipu-index](https://docs.bigmodel.cn/llms.txt) | `94dc81e3bef8fb4290d48e4d4366e7f1ce55134e9b1b189140be6371feec0016` |
| [zai-index](https://docs.z.ai/llms.txt) | `eb829f723defccea7c06148093f5229d8b760a2e9dca5102fe38c12c63a144af` |
| [dashscope-embedding](https://www.alibabacloud.com/help/en/model-studio/embedding-interfaces-compatible-with-openai) | `a0f5a43ab167071a1d53ad1c2b4819d6fe1aaa60f8c860588a7d68ed765ccfcd` |
| [zhipu-embedding](https://docs.bigmodel.cn/api-reference/模型-api/文本嵌入.md) | `d465ffcadcccb27f6991431449200ed80dd33109c9f5988f39ffcdf9ad222c74` |
| [zhipu-images](https://docs.bigmodel.cn/api-reference/模型-api/图像生成.md) | `6ae5d322a8d57225db466c8d28d1b50262ec6d2b10b7a1c3c9446f0362d595f7` |
| [zhipu-tts](https://docs.bigmodel.cn/api-reference/模型-api/文本转语音.md) | `d054d8cbae59195aa5880bf00517fcf0186bf128ddc635f401d109f59afa0408` |
| [zhipu-asr](https://docs.bigmodel.cn/api-reference/模型-api/语音转文本.md) | `2b24721b7a8809ba51ede57072567cefe60de22013ba683426c7360298a8b3b0` |
| [zai-images](https://docs.z.ai/api-reference/image/generate-image.md) | `d41c331887b06f344dacd545e19bb56b70baac4cce9ea680faa30a58c0966145` |
| [zai-asr](https://docs.z.ai/api-reference/audio/audio-transcriptions.md) | `7b85cd2bb0a692102608aceb490258ca550ec9a559214f592b7eb746057047f0` |
| [zhipu-pricing](https://docs.bigmodel.cn/cn/guide/start/pricing.md) | `7224d9566404ed86a97dbfb41cf1703f1f31a25d2b4e91953018ceb5be638d0b` |
| [siliconflow-embeddings-post](https://docs.siliconflow.cn/docs/api/embeddings-post) | `4a0b9f40b988c263f20872d208d38f8a02cf4a7222f4fa4fc61e52b96b753d71` |
| [siliconflow-images-generations-post](https://docs.siliconflow.cn/docs/api/images-generations-post) | `6b5dd1a5aeb5a9afdaae6dc6b38d567c14c9cad4d2737d4aeddd5efe28520cfa` |
| [siliconflow-audio-speech-post](https://docs.siliconflow.cn/docs/api/audio-speech-post) | `e51879dd1b78314f9c5a5188dac21ffb948f7a61ca6b98665398642f38ae4211` |
| [siliconflow-audio-transcriptions-post](https://docs.siliconflow.cn/docs/api/audio-transcriptions-post) | `27f9012f6f6735ad740e1d33ec9f3dfdbbd96d7aa1fcc7b35f2a3a93e1e4cef5` |

DashScope/Qwen embeddings 的官方兼容接口仅返回 `usage.total_tokens`；在 embeddings 响应边界将其映射为 prompt_tokens，completion_tokens 为 0，不伪造缺失总量。真实 HTTP fixture 使用该原生形状，同时断言 total/prompt/completion 为 2/2/0。来源：[阿里云兼容 embeddings 接口](https://help.aliyun.com/en/model-studio/embedding-interfaces-compatible-with-openai)。

2026-10-04 核验：[国内 Zhipu 图片 API](https://docs.bigmodel.cn/api-reference/%E6%A8%A1%E5%9E%8B-api/%E5%9B%BE%E5%83%8F%E7%94%9F%E6%88%90) 的 user_id 与国际 Z.AI 图片协议都要求 6–128 字符。两者已分别依据各自文档核对；标准 user 在既有图片序列化边界映射到 user_id，上游执行长度校验。真实 HTTP 回归观察两个选择器的 user_id 与 user 缺省。
