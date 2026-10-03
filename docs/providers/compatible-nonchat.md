# 兼容供应商的非聊天接口

通用 `openai_compatible` 配置已能调用向量、图片和音频接口。具名供应商只开放核对过的协议；不能因为聊天兼容 OpenAI，就假设其所有端点都兼容。

本批复用已有传输和路由，不新增配置字段：

| 配置选择器 | 本批接入的能力 | 默认 API base |
| --- | --- | --- |
| `together` / `together_ai` | embeddings、images/generations、audio/speech、audio/transcriptions、audio/translations | `https://api.together.ai/v1` |
| `deepinfra` | embeddings、images/generations | `https://api.deepinfra.com/v1/openai` |
| `fireworks` / `fireworks_ai` | embeddings | `https://api.fireworks.ai/inference/v1` |

在现有 `providers[].models` 中配置该供应商的真实模型 ID，再请求对应网关接口。表中的能力是传输协议范围，不表示每个模型都能执行全部任务：所选模型、账户权限和可用性仍由供应商约束。不支持的 capability 在路由阶段拒绝；上游模型/参数错误保留错误类型，429 的 Retry-After 秒数继续传递给重试链路。已有显式自定义价格和预算检查继续生效，缺价不会自动视为免费。

图片编辑/变体、moderations 和视频未在这些具名配置中开放。DeepInfra 的上述官方音频文档展示的是 `/v1/inference/{model}` 原生协议，因此本批不将它声明成相同 base 下的 OpenAI 音频接口。Fireworks 图片工作流也使用独立路径，不能直接挂到 `/images/generations`。

核对依据（2026-10-03）：[Together 兼容矩阵](https://docs.together.ai/docs/inference/openai-compatibility)、[DeepInfra 向量](https://deepinfra.com/Qwen/Qwen3-Embedding-8B/api)、[DeepInfra 图片](https://deepinfra.com/black-forest-labs/FLUX-1-schnell/api)、[DeepInfra 音频](https://docs.deepinfra.com/apis/text-to-speech)、[Fireworks 向量](https://docs.fireworks.ai/guides/querying-embeddings-models)。

验证使用本地 HTTP 模拟服务，覆盖实际 factory 构造、现有 Router 的 capability 选择、JSON/二进制/multipart 传输和错误。未运行付费供应商实调。`registry/support_matrix.rs` 记录的是旧适配入口，不应作为当前运行时能力的唯一依据。
