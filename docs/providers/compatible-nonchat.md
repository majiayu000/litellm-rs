# 兼容供应商的非聊天接口

通用 `openai_compatible` 配置已能调用向量、图片和音频接口。具名供应商只开放核对过的协议；不能因为聊天兼容 OpenAI，就假设其所有端点都兼容。

本批复用已有传输和路由，不新增配置字段：

| 配置选择器 | 本批接入的能力 | 默认 API base |
| --- | --- | --- |
| `groq` | audio/speech、audio/transcriptions、audio/translations | `https://api.groq.com/openai/v1` |
| `together` / `together_ai` | embeddings、images/generations、audio/speech、audio/transcriptions、audio/translations | `https://api.together.ai/v1` |
| `deepinfra` | embeddings、images/generations | `https://api.deepinfra.com/v1/openai` |
| `fireworks` / `fireworks_ai` | embeddings | `https://api.fireworks.ai/inference/v1` |
| `openrouter` | embeddings | `https://openrouter.ai/api/v1` |
| `nebius` | embeddings | `https://api.tokenfactory.nebius.com/v1` |
| `nvidia_nim` | embeddings | `https://integrate.api.nvidia.com/v1` |
| `lm_studio` | embeddings | `http://localhost:1234/v1` |

在现有 `providers[].models` 中配置该供应商的真实模型 ID，再请求对应网关接口。表中的能力是传输协议范围，不表示每个模型都能执行全部任务：所选模型、账户权限和可用性仍由供应商约束。不支持的 capability 在路由阶段拒绝；上游模型/参数错误保留错误类型，429 的 Retry-After 秒数继续传递给重试链路。已有显式自定义价格和预算检查继续生效，缺价不会自动视为免费。

图片编辑/变体、moderations 和视频未在这些具名配置中开放。DeepInfra 的上述官方音频文档展示的是 `/v1/inference/{model}` 原生协议，因此本批不将它声明成相同 base 下的 OpenAI 音频接口。Fireworks 图片工作流也使用独立路径，不能直接挂到 `/images/generations`。

核对依据（2026-10-03）：[Together 兼容矩阵](https://docs.together.ai/docs/inference/openai-compatibility)、[DeepInfra 向量](https://deepinfra.com/Qwen/Qwen3-Embedding-8B/api)、[DeepInfra 图片](https://deepinfra.com/black-forest-labs/FLUX-1-schnell/api)、[DeepInfra 音频](https://docs.deepinfra.com/apis/text-to-speech)、[Fireworks 向量](https://docs.fireworks.ai/guides/querying-embeddings-models)。

验证使用本地 HTTP 模拟服务，覆盖实际 factory 构造、现有 Router 的 capability 选择、JSON/二进制/multipart 传输和错误。未运行付费供应商实调。`registry/support_matrix.rs` 记录的是旧适配入口，不应作为当前运行时能力的唯一依据。

Groq 音频按 [转写/翻译协议](https://console.groq.com/docs/speech-to-text) 和 [Orpheus 协议](https://console.groq.com/docs/text-to-speech/orpheus) 接入（2026-10-03）。语音合成省略格式时使用 WAV，显式格式/参数仍交给上游校验；当前 Orpheus 只支持 WAV。Whisper Large V3 支持转写和翻译，Turbo 只支持转写。网关仍限上传文件与 JSON/verbose_json 响应，不包含 URL 输入或原始文本字幕响应。为得到实际时长，Groq 默认/JSON 转写和翻译请求上游 verbose_json；响应保留时长及时间戳。预算预留与结算统一使用至少十秒的计费时长，预留时长仍沿用现有文件大小估算。不会把供应商级音频能力解释成所有模型均支持音频。

Groq 路由按具体模型区分：Whisper v3/v3-turbo 可转写，仅 v3 可翻译，Orpheus 英语/沙特阿拉伯语模型可合成语音。音频模型不作为聊天部署候选。翻译成功后的计费优先使用上游返回的有效正数 duration；缺失或无效时保留已有文件大小估算，两种情况均应用十秒最低计费。

第三批（#1400）沿用现有 HTTP/Router 实现。依据：[OpenRouter embeddings](https://openrouter.ai/docs/api/api-reference/embeddings/create-embeddings)、[Nebius 官方 OpenAPI](https://api.tokenfactory.nebius.com/docs)、[NVIDIA embeddings](https://docs.api.nvidia.com/nim/reference/nvidia-llama-nemotron-embed-1b-v2-infer)、[LM Studio embeddings](https://lmstudio.ai/docs/developer/openai-compat/embeddings)，核验日期 2026-10-03。Nebius 默认 base 使用官方 Token Factory 地址；已有显式 base 配置仍优先。

本批验证文本输入和浮点向量响应。NVIDIA/OpenRouter 的供应商 `input_type` 通过 HTTP 请求的 `input_type` 字段传递（内部映射为 `task_type`），NVIDIA 的 `truncation: true/false` 分别映射 `truncate: END/NONE`；没有增加供应商配置或默认截断。NVIDIA 的维度、模型可用性仍受实际模型协议约束。LM Studio 需要本地加载 embedding 模型；本地服务不自动获得零价，继续使用已有价格配置和缺价策略。多模态输入及 base64 向量响应未在本批验证。

Nebius 图片端点虽然路径相同，官方请求使用 width/height，响应为 id/data 且没有当前通用实现必需的 created，因此本批保持明确不支持，不把路径相同视为协议等价。其独立适配仍是 F11 的剩余工作。

第四批（#1409，核验 2026-10-03）接入本地/自托管服务的已验证协议：

| 选择器 | 当前接入的非聊天能力 | 默认 API base |
| --- | --- | --- |
| `vllm` / `hosted_vllm` | embeddings、audio/transcriptions、audio/translations | `http://localhost:8000/v1` |
| `llamafile` | embeddings | `http://localhost:8080/v1` |
| `docker_model_runner` | embeddings | `http://localhost:12434/engines/llama.cpp/v1` |
| `xinference` | embeddings、images/generations、audio/speech、audio/transcriptions、audio/translations | `http://localhost:9997/v1` |
| `infinity` | embeddings | `http://localhost:7997` |
| `oobabooga` | embeddings、images/generations | `http://localhost:5000/v1` |
| `lemonade` | embeddings、images/generations、audio/speech、audio/transcriptions | `http://localhost:13305/v1` |

Infinity 的官方默认 URL 没有 `/v1` 前缀，Lemonade 是本地服务器；此前 Lemonade 的 `.social` 地址没有该协议依据，现改用官方本地地址。两者已有显式 `base_url` 配置仍优先。所有本地选择器可不设上游 key，显式 key 仍通过 Bearer 传递；网关自身鉴权保持原有规则。

本批验证文本/浮点 embeddings、图片生成的 `b64_json`、上传文件的 JSON 转写/翻译，以及不流式的 WAV/MP3 语音响应。模型 ID 是部署端的真实名称或 UID，须在既有 `providers[].models` 配置；供应商级协议声明不表示任意模型都支持它。不要把自托管成本自动当作零：未知价格仍拒绝，需使用既有显式价格配置，已有预算预留和结算继续生效。

- vLLM 转写/翻译需音频依赖和兼容模型；Whisper Turbo 不支持翻译。vLLM-Omni、模型原生语音生成和 WebSocket 不属于此协议范围。
- llamafile 需要 dedicated embedding 模型；Whisperfile/Diffusionfile 是独立服务，不能从同一 `/v1` base 推断音频或图片端点。
- Docker Model Runner 图片使用 `/engines/diffusers/v1/images/generations`，官方响应示例也缺少当前通用响应必需的 `created`，暂不宣称已接入。
- Infinity 服务主要做 embedding/rerank/classify，原先统一聊天声明已移除。图像/音频 embedding 需要 `modality` 和相应输入格式，不等于图片/音频生成；当前只验证文本 embeddings。
- Oobabooga embeddings 使用服务端配置的 embedding 模型，图片使用已加载 diffusion 模型；请求 `model` 不负责选择它们。当前图片 `n` 是 `batch_size` 别名。音频转写虽注册端点，但已核对源码包含 `FormData.getvalue` 等未验证路径，因此保留未接入状态，不能写成供应商没有音频能力。
- Lemonade embeddings 依赖 llamacpp/flm recipe；ONNX recipe 不支持。图片生成使用 diffusion 模型、`n=1` 和 `b64_json`；转写目前限 WAV 输入；语音格式依模型后端限制，OpenMOSS 建议显式 WAV。原始 PCM 的额外采样率元数据、克隆、原生 streaming 未验证。官方图像编辑/变体已存在，但具名网关 multipart 调度尚未接入；音频翻译未得到当前端点证据。
- LM Studio 当前官方兼容端点只确认聊天/Responses/completions/models/embeddings，图像输入不是图像生成。没有对未知独立音频或图片端点作否定推断。

逐项官方链接、固定源码版本和剩余限制见 [本地非聊天协议审计](../audit/local-compatible-nonchat-2026-10-03.md)。实测均为本地 HTTP 模拟：覆盖 factory/Router、实际 JSON/multipart/二进制传输、模型与参数错误、429 Retry-After，以及本地模型缺价、预算不足和按真实 usage 结算。未安装模型、未运行 GPU 推理或供应商付费调用。

第五批（#1415，核验 2026-10-03）补充 Nscale embeddings、OVHcloud embeddings 和 Heroku embeddings。Nscale 默认 base 修正为 `https://inference.api.nscale.com/v1`，OVHcloud 为 `https://oai.endpoints.kepler.ai.cloud.ovh.net/v1`；显式自定义 base 仍优先。Heroku 使用 add-on 对应的模型和 key，网关 API base 必须配置为 `${EMBEDDING_URL}/v1`（例如 `https://us.inference.heroku.com/v1`），不能直接填写 add-on 给出的裸主机 URL；[官方示例](https://devcenter.heroku.com/articles/heroku-inference-api-v1-embeddings)在该变量后追加 `/v1/embeddings`。向量请求的 `encoding_format: float` 映射为官方 `raw`，HTTP 请求使用 `input_type` 字段，内部 `task_type` 再映射为上游 `input_type`。仅核验文本输入和浮点向量；Heroku 的自定义维度、截断等非协议参数不静默删除，错误由上游返回。Nscale 图片虽有兼容协议，但当前模型按像素计价，网关尚未支持该计费单位，因此不声明图片生成能力，并在请求上游前拒绝。Heroku 凭据按显式 key、`HEROKU_API_KEY`、`INFERENCE_KEY`、`EMBEDDING_KEY` 顺序解析；多个模型资源使用显式 key 选择对应资源。OVHcloud 同样接受官方 `OVH_AI_ENDPOINTS_ACCESS_TOKEN`，显式 key 和 `OVHCLOUD_API_KEY` 优先。

官方协议与剩余限制见 [云供应商非聊天审计](../audit/cloud-compatible-nonchat-2026-10-03.md)。Baseten 专用 BEI、Friendli 专用 embeddings/images、HF 原生多任务接口不能从兼容聊天地址推断可用。Friendli serverless 转写虽有兼容路径，其用量按 input/output tokens 表达，当前音频路由按秒计费，故仍需明确适配。Heroku 图片的 aspect_ratio/output_format 也尚未映射，不扩大支持声明。本批本地 HTTP 覆盖真实 factory/Router、请求映射、400/429、缺价拒绝、预算不足和实际 usage 结算；没有付费实调。

第七批（#1422，核验 2026-10-03）接入 Novita 文本 embeddings、Featherless 文本 embeddings/同步语音、Galadriel embeddings/图片生成，以及 NanoGPT embeddings/图片生成/同步语音/转写。NanoGPT 默认地址修正为 `https://api.nano-gpt.com/api/v1`；图片按官方协议使用同一主机的 `/v1/images/generations`。显式 base 以 `/api/v1` 结尾时，图片使用相邻 `/v1`；其他自定义 base 保持原路径。

所有模型须由现有配置指定，能力声明只确认接口协议，不保证任意模型/账户均可用。Featherless 音频可能使用模型原生格式，响应的实际 Content-Type 保持传递；`voice: "default"` 可用于无预设音色的模型。NanoGPT 当前只验证普通同步转写与按字符计费的同步 TTS，不包括克隆、视频、音乐、异步作业和 SSE 音频。图片编辑/变体、音频翻译均未在本批开放。Novita 原生异步图片和 JSON 音频仍需专门适配。AI21/Cerebras 的当前官方文档未提供已确认的兼容非聊天协议，因此不扩展声明，也不把未确认写成供应商绝无该能力。详见 [逐项协议审计](../audit/aggregator-compatible-nonchat-2026-10-03.md)。

第十批（#1429，核验 2026-10-03）接入 `aiml_api`/`aiml` 与 `comet_api` 文本 embeddings。AIML 官方响应只给 `usage.total_tokens` 时，按向量无输出 tokens 的语义填入 `prompt_tokens`，不从文本长度推估；已有 `task_type` 映射为其 `input_type`。缺失/无效 usage 仍遵循解析错误合同。CometAPI 默认主机修正为 `https://api.cometapi.com/v1`，Bytez 修正为 `https://api.bytez.com/models/v2/openai/v1`。

AIML 图片响应 schema 不保证当前成功类型必需的 `created`，TTS/STT 使用原生 URL/异步任务；CometAPI 标准图片/音频接口存在，但本批尚未确认其模型计费单位与当前网关字符/秒/图片估算结算相符，保持待适配状态。Bytez 非聊天使用原生 model-run；Poe 多模态 bots 使用聊天内容。逐项证据与限制见 [多模态聚合供应商审计](../audit/multimodal-compatible-nonchat-2026-10-03.md)。

第九批（#1427，核验 2026-10-03）接入火山方舟文本 embeddings：使用现有 `/api/v3` base，按官方协议把单字符串输入规范为单元素数组，保留批量文本与浮点响应。模型或 Endpoint ID 由已有配置指定；多模态 `/embeddings/multimodal` 仍是独立协议。MiniMax 的 OpenAI 默认地址修正为 `https://api.minimax.io/v1`；它的原生图片/语音协议没有因此自动获得支持。

SambaNova 当前官方文档明确将 embeddings 和 Whisper 音频限定于 SambaStack，默认 SambaCloud 选择器不扩展能力。Hyperbolic 当前文档站转向 GPU 租用，托管推理非聊天协议仍未确认；这不是已退役的证据。详细官方来源和剩余差异见 [原生协议边界审计](../audit/native-compatible-nonchat-2026-10-03.md)。

第八批（#1425，核验 2026-10-03）沿用现有传输接入百川文本 embeddings，官方端点为 `https://api.baichuan-ai.com/v1/embeddings`。官方已索引协议限定 `Baichuan-Text-Embedding`、每条最多 512 tokens、批次最多 16 条、固定 1024 维；超过 16 条的批次在发送前拒绝，避免上游静默截断；仅发送文本字符串或文本数组。余额不足的官方 429 映射为不可重试 QuotaExceeded，限流 429 保留 Retry-After。当前直接文档页面仅显示申请体验外壳，未实调账户可用性，不据此新增静态 callable 型号或价格。使用已获访问权限的真实模型配置和正确价格。

本批同时核验 Moonshot、DeepSeek、Xiaomi MiMo、Yi、Maritalk。MiMo 的 TTS/ASR 官方确实存在，但经聊天请求/响应中的音频协议，需要独立适配后才能挂到网关 `/audio/*`；Maritaca 官方明确推荐其他供应商的 embeddings。未确认端点保持不声明，详见 [区域供应商审计](../audit/regional-compatible-nonchat-2026-10-03.md)。

## 具名目录核验范围

下表从本批 `registry/catalog.rs` 的全部定义枚举。已核验只表示表内非聊天协议范围，不表示价格表中的型号都可调用。其余行待逐项核验，不能据当前仅声明聊天就断言供应商不提供其他能力。

| 选择器 | 非聊天核验状态 |
| --- | --- |
| `groq` | 音频合成/转写/翻译；见上方模型限制 |
| `ai21` | 已核对官方 Jamba/工具目录；未确认兼容 embeddings/images/audio，保持不声明 |
| `huggingface` | 官方兼容 router 非聊天任务需 Inference Clients/native 协议；embeddings/images/audio 未在兼容 base 接入 |
| `baseten` | 已核验：embeddings 属专用 BEI 部署 /sync/v1，默认 Model APIs base 未确认该能力；图片/音频为独立部署协议，未接入 |
| `together` | embeddings/images/audio |
| `together_ai` | 同 together |
| `fireworks` | embeddings；图片原生路径待适配，独立 ASR/TTS 当前合同未确认；[媒体复核](../audit/compatible-media-followup-2026-10-03.md) |
| `fireworks_ai` | 同 fireworks |
| `perplexity`（已移出目录） | Sonar 聊天端点已退役，具名选择器在构造阶段拒绝；[F10 审核证据](../audit/perplexity-sonar-retirement-2026-10-03.md)。Agent/Responses、搜索与 embeddings 不因此自动获得支持，仍待各自核验/适配 |
| `cerebras` | 官方 OpenAPI 仅列 chat/completions；独立 embeddings/images/audio 未确认 |
| `openrouter` | embeddings；已确认独立 Image/TTS/STT 合同，原生输入及实际媒体计费尚待适配；[媒体复核](../audit/compatible-media-followup-2026-10-03.md) |
| `deepinfra` | embeddings/images；音频原生路径待适配 |
| `deepseek` | 核对当前官方 API；Responses/文件/视觉是独立范围，未确认 embeddings/images/audio |
| `novita` | 文本 embeddings；原生异步图片及 MiniMax 音频协议仍待适配 |
| `nvidia_nim` | embeddings；图片为另一 genai 主机/路径，音频为独立 Speech NIM 部署，默认 base 不扩声明；[媒体复核](../audit/compatible-media-followup-2026-10-03.md) |
| `nebius` | embeddings；图片 width/height 与 data/id 响应待适配，独立音频未确认；[媒体复核](../audit/compatible-media-followup-2026-10-03.md) |
| `nscale` | embeddings；图片像素计价未接入，能力暂不声明；官方 .com base，模型/图片退役窗口见 cloud 审计 |
| `hyperbolic` | 旧文档现跳 GPU 租用新站，未找到当前托管推理非聊天协议；保持未确认，不凭重定向断言退役 |
| `featherless` | 文本 embeddings、按字符计费的同步语音；格式/voice 依模型，未接克隆/SSE |
| `galadriel` | 官方 OpenAPI 的 embeddings、images/generations；须配置实际可用模型，未实调账户目录 |
| `sambanova` | 当前官方 embeddings/Whisper 仅 SambaStack，公共云未开放；旧日文云端点说明不可当现行证据 |
| `heroku` | 文本 embeddings；float→raw、HTTP input_type→上游 input_type；须模型 add-on 对应 URL/key；图片参数待适配 |
| `friendliai` | 已核验：serverless 转写按 tokens 用量，与当前音频秒计费不同，未接入；dedicated embeddings/images 不在默认 base |
| `meta_llama` | 待核验；本批未扩展非聊天声明 |
| `v0` | 待核验；本批未扩展非聊天声明 |
| `amazon_nova` | 待核验；本批未扩展非聊天声明 |
| `github` | 待核验；本批未扩展非聊天声明 |
| `xai` | 已确认原生 images、/tts、/stt；请求/响应与计费尚待接入，未确认独立 embeddings；[媒体复核](../audit/compatible-media-followup-2026-10-03.md) |
| `vllm` | embeddings、音频转写/翻译；须部署对应 pooling/Whisper 模型，Turbo 不支持翻译 |
| `hosted_vllm` | 同 vllm；已有显式 API key 会发送 Bearer |
| `lm_studio` | 文本 embeddings；当前官方端点清单未确认图片生成/独立音频协议，不扩展声明 |
| `llamafile` | embeddings；需要 embedding 模型与 server/embedding 模式，Whisperfile 是独立协议 |
| `docker_model_runner` | embeddings；需 embedding 模型/运行标志；Diffusers 图片独立路径和响应待适配 |
| `xinference` | embeddings、images/generations、audio/speech/transcriptions/translations；须启动相应模型 UID |
| `infinity` | 文本 embeddings；默认根路径 /embeddings；移除原来虚假的聊天声明；图像/音频 embedding 待扩展输入 |
| `oobabooga` | embeddings、images/generations；依赖已加载模型；音频转写源实现异常待上游确认 |
| `moonshot` | 核对当前官方模型/端点目录；未确认独立 embeddings/images/audio，图像/视频理解不等于生成 |
| `dashscope` | 待核验；本批未扩展非聊天声明 |
| `qwen` | 待核验；本批未扩展非聊天声明 |
| `baichuan` | 官方已索引标准文本 embeddings 协议；账户可用性未实调，输入 ≤512 tokens/批量 ≤16，固定1024维 |
| `minimax` | 修默认 OpenAI base 为 api.minimax.io/v1；ASR /speech_to_text、TTS /t2a_v2、图片原生 JSON 待独立适配 |
| `volcengine` | 文本 embeddings（/api/v3），单字符串转数组；多模态向量/Seedream图片参数及部分失败、音频另待适配 |
| `xiaomi_mimo` | 官方 TTS/ASR 经 chat/completions 音频契约；通用 /audio/* 尚未适配，embeddings/images 未确认 |
| `zhipu` | 待核验；本批未扩展非聊天声明 |
| `zai` | 待核验；本批未扩展非聊天声明 |
| `lemonade` | embeddings、images/generations、audio/speech/transcriptions；修正本地默认 base；编辑/变体待网关调度接入 |
| `linkup` | 当前官方为 Search/Fetch/Research/Tasks/Extract；无兼容非聊天证据，旧 chat 声明待 F10 复核；[依据](../audit/remaining-compatible-selectors-2026-10-03.md) |
| `poe` | 图片/音频 bots 经 chat/completions 返回内容；未确认标准 embeddings/images/audio 路由，不能互相替代 |
| `wandb` | 已核验当前 CoreWeave Serverless：修正 inference 默认地址；仅 Chat/Models 合同，独立非聊天未确认；[依据](../audit/remaining-compatible-selectors-2026-10-03.md) |
| `nanogpt` | embeddings、images/generations、同步 speech/transcriptions；图片 /v1，其他 /api/v1；异步/克隆未接 |
| `aiml_api` | 文本 embeddings；total-only usage 规范为输入tokens；图片created缺项、原生/异步语音待适配 |
| `aiml` | 同 aiml_api（独立选择器共享已验证协议） |
| `aleph_alpha` | 官方 SDK 有兼容 embeddings；默认 host/部署模型未确认，尚未启用；旧 semantic_embed 不等同兼容协议；[依据](../audit/remaining-compatible-selectors-2026-10-03.md) |
| `anyscale` | 当前官方是部署型 Ray Serve/vLLM；旧共享 endpoint 与非聊天能力未确认，不推断退役；[依据](../audit/remaining-compatible-selectors-2026-10-03.md) |
| `bytez` | 修正 OpenAI base /models/v2/openai/v1；非聊天原生 model-run 协议及运行时计费待适配 |
| `comet_api` | 文本 embeddings；修正主机 api.cometapi.com；图片/音频协议存在但计费/usage差异尚待收口 |
| `compactifai` | Whisper 转写；一分钟最低计费同时用于预留/结算，其他模型不因此获得音频能力；见下方 |
| `maritalk` | 官方明确无自身 embeddings，推荐第三方 DeepInfra；图片/音频协议未确认 |
| `siliconflow` | 待核验；本批未扩展非聊天声明 |
| `yi` | 官方页面/RSC 只确认聊天资料，独立 embeddings/images/audio 未确认 |
| `lambda_ai` | 官方确认共享推理于 2025-09-25 退役，交 F10 #1373 清理；不扩非聊天能力；[依据](../audit/remaining-compatible-selectors-2026-10-03.md) |
| `ovhcloud` | 文本 embeddings；官方统一 oai.endpoints base；图片/音频原生协议尚未接入 |

选择器别名沿用 `canonical_catalog_name`，例如 hugging_face、aimlapi、ai21_chat 等不另建重复审核项。

## CompactifAI 转录

2026-10-03 核验，issue #1433。[官方模型卡](https://docs.compactif.ai/models/whisper_large_v3_turbo_slim/)确认 `cai-whisper-large-v3-turbo-slim`，[API 参考](https://docs.compactif.ai/api_reference/#audio-transcriptions)定义 multipart `/v1/audio/transcriptions`。现有配置可使用 `compactifai`，默认 base 为 `https://api.compactif.ai/v1`。只开放该模型的转写；它不作为 Chat、翻译或 TTS 候选。供应商其他聊天模型不因此获得转写能力。

默认/JSON 请求上游 `verbose_json` 以取得 duration；该转换只为保留实际计费时长，沿用现有网关 JSON/verbose_json 响应范围，不宣称音频流式、SRT/VTT 或翻译支持。[官方定价](https://docs.compactif.ai/pricing/#speech-to-text-pricing)明确逐秒计费且至少一分钟。既有同一 time-pricing 入口对预算预留和结算都应用至少 60 秒，不增设配置；预留依旧使用现有文件大小时长估算，结算采用有效上游 duration。尚未提供内置价格时必须使用既有显式每秒价格配置，缺价拒绝，不把自定义测试费率当成官方价格。

决策：适配既有 OpenAI multipart、Router 和 time pricing，不引入 SDK 或通用计费规则框架。接口原始依据与下载哈希见 [#1432 的先前审计](https://github.com/majiayu000/litellm-rs/pull/1432)；本节取代其中的 ASR 未接入状态。未调用供应商账户。HTTP 测试覆盖 multipart/duration、429、具体模型隔离；网关测试覆盖短/长音频、一分钟最低预留阻止超预算调用、缺价拒绝与实际费用结算。

计费审查回归覆盖压缩输入被估算为 2 秒、预留最低 60 秒、返回实际 75 秒、预算只够 70 秒的场景。现有结算允许记录已经发生的超预算费用：provider/model 都计入实际 75 秒，下一请求在上游调用前被拒绝。该测试并不证明文件大小估算是时长上界，也不证明任意后端故障下账务可持久恢复；不增加重复记账或回填框架。
