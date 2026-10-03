# 兼容供应商的非聊天接口

通用 `openai_compatible` 配置已能调用向量、图片和音频接口。具名供应商只开放核对过的协议；不能因为聊天兼容 OpenAI，就假设其所有端点都兼容。

本批复用已有传输和路由，不新增配置字段：

| 配置选择器 | 本批接入的能力 | 默认 API base |
| --- | --- | --- |
| `groq` | audio/speech、audio/transcriptions、audio/translations | `https://api.groq.com/openai/v1` |
| `together` / `together_ai` | embeddings、images/generations、audio/speech、audio/transcriptions、audio/translations | `https://api.together.ai/v1` |
| `deepinfra` | embeddings、images/generations | `https://api.deepinfra.com/v1/openai` |
| `fireworks` / `fireworks_ai` | embeddings | `https://api.fireworks.ai/inference/v1` |

在现有 `providers[].models` 中配置该供应商的真实模型 ID，再请求对应网关接口。表中的能力是传输协议范围，不表示每个模型都能执行全部任务：所选模型、账户权限和可用性仍由供应商约束。不支持的 capability 在路由阶段拒绝；上游模型/参数错误保留错误类型，429 的 Retry-After 秒数继续传递给重试链路。已有显式自定义价格和预算检查继续生效，缺价不会自动视为免费。

图片编辑/变体、moderations 和视频未在这些具名配置中开放。DeepInfra 的上述官方音频文档展示的是 `/v1/inference/{model}` 原生协议，因此本批不将它声明成相同 base 下的 OpenAI 音频接口。Fireworks 图片工作流也使用独立路径，不能直接挂到 `/images/generations`。

核对依据（2026-10-03）：[Together 兼容矩阵](https://docs.together.ai/docs/inference/openai-compatibility)、[DeepInfra 向量](https://deepinfra.com/Qwen/Qwen3-Embedding-8B/api)、[DeepInfra 图片](https://deepinfra.com/black-forest-labs/FLUX-1-schnell/api)、[DeepInfra 音频](https://docs.deepinfra.com/apis/text-to-speech)、[Fireworks 向量](https://docs.fireworks.ai/guides/querying-embeddings-models)。

验证使用本地 HTTP 模拟服务，覆盖实际 factory 构造、现有 Router 的 capability 选择、JSON/二进制/multipart 传输和错误。未运行付费供应商实调。`registry/support_matrix.rs` 记录的是旧适配入口，不应作为当前运行时能力的唯一依据。

Groq 音频按 [转写/翻译协议](https://console.groq.com/docs/speech-to-text) 和 [Orpheus 协议](https://console.groq.com/docs/text-to-speech/orpheus) 接入（2026-10-03）。语音合成省略格式时使用 WAV，显式格式/参数仍交给上游校验；当前 Orpheus 只支持 WAV。Whisper Large V3 支持转写和翻译，Turbo 只支持转写。网关仍限上传文件与 JSON/verbose_json 响应，不包含 URL 输入或原始文本字幕响应。为得到实际时长，Groq 默认/JSON 转写和翻译请求上游 verbose_json；响应保留时长及时间戳。预算预留与结算统一使用至少十秒的计费时长，预留时长仍沿用现有文件大小估算。不会把供应商级音频能力解释成所有模型均支持音频。

Groq 路由按具体模型区分：Whisper v3/v3-turbo 可转写，仅 v3 可翻译，Orpheus 英语/沙特阿拉伯语模型可合成语音。音频模型不作为聊天部署候选。翻译成功后的计费优先使用上游返回的有效正数 duration；缺失或无效时保留已有文件大小估算，两种情况均应用十秒最低计费。
