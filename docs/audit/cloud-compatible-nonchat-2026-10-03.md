# 云供应商非聊天协议核验：第一批

日期：2026-10-03。基线 `f62c5adb5071063665fbb683620fa3b28fd00e7d`，issue #1415。只审核当前具名选择器的 embeddings/images/audio 协议，不把价格目录或模型网页当作所有端点的可调用证据。真实账户调用未运行。

决策：复用既有 OpenAI-compatible JSON 传输、Router 能力选择及预算；仅补静态能力、两处错误默认地址和 Heroku 参数映射。不引入 SDK、配置面或统一适配框架。专用部署、多任务原生协议及不同计费单位不伪装成兼容端点。

| 选择器 | 官方证据及已验证协议 | 本次改动与剩余限制 |
| --- | --- | --- |
| nscale | [embeddings](https://docs.nscale.com/api-reference/inference/create-embeddings)、[images](https://docs.nscale.com/api-reference/inference/create-image)：Bearer、`inference.api.nscale.com`、`/v1/embeddings` 文本请求与向量/usage、`/v1/images/generations` model/prompt/n/size 与 created/data/b64_json | 接入 embeddings，修正旧 `.ai` base。图片协议已核验，但模型按像素计价，当前网关未支持该计费单位，撤下 ImageGeneration 声明并在传输前拒绝。调用所配置模型，未从定价猜能力。图片额外 quality/style/response_format 未验证，音频无当前端点证据，维持不支持。官方 [模型页](https://docs.nscale.com/docs/ai-services/models) 宣布包括 FLUX.1-schnell 在内的部分型号将于 2026-11-02 至 11-04 退役；当前日期尚未退役，网关不新增静态型号，运营方须按账户 model list 配置。 |
| ovhcloud | [BGE-M3 官方协议](https://www.ovhcloud.com/en/public-cloud/ai-endpoints/catalog/bge-m3/) 使用 `oai.endpoints.kepler.ai.cloud.ovh.net/v1/embeddings`、Bearer、标准向量响应与 usage | 接入文本 embeddings，并修正原无协议依据的默认 base。官方页面列文本/模型批次与维度限制，按模型选择，不作通用维度保证。图像、ASR/TTS 产品存在但本批未验证同一 base 的兼容请求/返回/计费，保持未接入，不能理解成供应商无此能力。 |
| heroku | [embeddings](https://devcenter.heroku.com/articles/heroku-inference-api-v1-embeddings)：model/input、可选 input_type、encoding_format raw/base64、embedding_type float；向量响应与 token usage | 接入文本 float（映射 raw）和 task_type（映射 input_type）。模型 add-on 提供对应 URL/key，用户使用既有显式 base。官方限制单次最多 96 段、每段 2048 字符，推荐少于 512 tokens；模型约束由上游验证。base64 结果类型尚未接入。 |
| heroku images | [官方图片协议](https://devcenter.heroku.com/articles/heroku-inference-api-v1-images-generations) 使用 aspect_ratio、output_format、negative_prompt，返回 created/data/b64_json | 路径和响应相似，但当前统一 size/response_format 不能直接表达全部语义；本批未声明。音频端点未得到当前官方证据。 |
| baseten | [BEI 示例](https://docs.baseten.co/examples/bei) 明确专用 `model-<id>.api.baseten.co/environments/production/sync/v1/embeddings` 与 Bearer；[转写](https://docs.baseten.co/reference/inference-api/predict-endpoints/transcription-api) 是部署特定预测协议 | 默认 `inference.baseten.co/v1` 只确认 Model APIs，不能把专用 BEI 端点能力挂到默认选择器。已有通用 openai_compatible 可显式配置 BEI base；具名非聊天暂不扩展。图片/TTS/转写部署示例存在，仍需各端点契约与计费适配。 |
| friendliai | [serverless transcription](https://friendli.ai/docs/openapi/model-apis/audio-transcriptions)、[dedicated embeddings](https://friendli.ai/docs/openapi/dedicated/inference/embeddings)、[dedicated images](https://friendli.ai/docs/openapi/dedicated/inference/image-generations) | serverless 转写返回 input_tokens/output_tokens，现有网关音频链路按秒预留/结算，不先声明以免错误账单。dedicated embedding/image 位于 `/dedicated/v1`，需要部署 endpoint ID，不能从当前 `/v1` 默认推断。默认聊天 base 与官方 `/serverless/v1` 的差异记录为后续修复，未在非聊天分支顺手改动。 |
| huggingface | [Inference Providers 官方说明](https://huggingface.co/docs/inference-providers/main/index) 明示 compatible router 的 embeddings/images/speech 需走原生客户端；[TEI](https://huggingface.co/docs/text-embeddings-inference/quick_tour) 是独立可部署 `/v1/embeddings` 服务 | 当前选择器默认 router 不声明这些非聊天能力。TEI 可使用通用兼容配置，不将独立自托管服务冒充云 router。Responses 原生端点为另一任务范围。 |

## 实现与验证范围

Heroku 的 add-on `EMBEDDING_URL` 是主机地址，网关 API base 配置为 `${EMBEDDING_URL}/v1`，使既有 transport 追加 `/embeddings` 后与官方路径一致；区域/自定义端点同样适用。

Nscale/OVHcloud/Heroku 均使用既有 `Provider::OpenAILike` 分派，模型名按现有 identity 处理，无新模型别名。Heroku 未知参数不吞掉；400 和 429/Retry-After 保留原错误合同。测试使用真实本地 HTTP 服务器而非只检查常量：三家 embedding 选择/请求/usage、Nscale 图片传输前拒绝、Heroku 参数转换、不支持操作拒绝。网关测试确认缺价不等于免费、剩余预算不足不发上游请求、成功后按返回 2 tokens 结算到配置名 `prod-nscale`（供应商类型仍为 `nscale`，定价使用该 canonical identity）。没有供应商账户或模型推理实调。

原始官方页面下载及 SHA-256 列于下方，便于确定此次观察内容；不是复制供应商文档全文。由于网页可变，后续维护需重新核验。

| 页面 | SHA-256 |
| --- | --- |
| [nscale-embeddings](https://docs.nscale.com/api-reference/inference/create-embeddings.md) | `f5b787923b21a8b275b8f660b38f84ba99f52ca4e8d22e52bfb98c66acb7e0e1` |
| [nscale-images](https://docs.nscale.com/api-reference/inference/create-image.md) | `370a1405237b77cd7a26dfcde99db5b4326173a7fabd981903371e19601d2164` |
| [nscale-models](https://docs.nscale.com/docs/ai-services/models.md) | `455d980bc10a9437e0401e7cda5f9cfaafbfc635b6cfeb45a7e76155b977bcfd` |
| [friendli-transcription](https://friendli.ai/docs/openapi/model-apis/audio-transcriptions.md) | `38566709b5a28aed03c3242303b55fb1101aa45e52c8b7d5c4cbc1e588f2a755` |
| [friendli-embeddings](https://friendli.ai/docs/openapi/dedicated/inference/embeddings.md) | `60c912e38f70cd9b113d675b2dfd7f74b63517126029351eaa0c14d06c66c1ea` |
| [friendli-images](https://friendli.ai/docs/openapi/dedicated/inference/image-generations.md) | `91244b362bc71e8f7e671b5a8982d66fb9c36d94450d75e1b7b2c55f481a827c` |
| [baseten-embedding](https://docs.baseten.co/examples/bei.md) | `67fc88b5c84734030166def65295f7b3d2927ca2418f1dfc1a44b79edc663fe0` |
| [baseten-audio](https://docs.baseten.co/reference/inference-api/predict-endpoints/transcription-api.md) | `604bc7a44122e50b915113295eeaafa7f337c311f6968f97cd51eb0bd59e8ea5` |
| [ovh-embeddings](https://www.ovhcloud.com/en/public-cloud/ai-endpoints/catalog/bge-m3/) | `ae93ff0a51793e64751440fd9aaff6ded1586941a69d7e912f89da88240aa506` |
| [heroku-images](https://devcenter.heroku.com/articles/heroku-inference-api-v1-images-generations) | `df252faa317f58b1ddf07e27a316c8400fe4133b77b4971e98cce0e4ac9eb66f` |
| [heroku-embeddings](https://devcenter.heroku.com/articles/heroku-inference-api-v1-embeddings) | `556e0021b1178ff4bf621bc21c1476dc0ebb3f524e501f2b2db301e929d69570` |

凭据核验：Heroku 官方 [add-on 说明](https://devcenter.heroku.com/articles/heroku-inference)给出 `INFERENCE_KEY`，[embeddings 示例](https://devcenter.heroku.com/articles/heroku-inference-api-v1-embeddings)使用 `EMBEDDING_KEY`；OVHcloud [BGE-M3 示例](https://www.ovhcloud.com/en-au/public-cloud/ai-endpoints/catalog/bge-m3/)使用 `OVH_AI_ENDPOINTS_ACCESS_TOKEN`。复用既有 alternate_auth_env_vars 和 credential precedence；显式 key 仍优先，未新增配置或解析层。
