# 模型目录核对与更新（2026-10-01）

原目录没有全部跟上当前型号。本次更新价格快照、已核验的新型号和相关参数/能力映射；不把价格目录里的每一条记录自动视为可调用模型。

## 范围和逐条证据

- 基线：`3341e54a`（v0.7.0）。独立工作树 `litellm-rs-model-refresh`，分支 `codex/model-catalog-refresh-20261001`。
- LiteLLM 上游固定到 `025292e75bda0381174a751da320645c971e06c7`，该价格文件提交日期为 2026-09-30。使用已有同步工具和分类账，没有新增运行时配置或兼容层。
- 旧目录 3,475 条，新目录 4,555 条：新增 1,338、字段更新 1,210、保持 2,007、从快照移除 258。当前价格数据有 135 个 provider 标签，包含区域、部署类型和别名。
- [逐条清单](model-catalog-2026-10-01.entries.json)覆盖新旧快照并集的每一个精确 key，记录差异字段、证据层级和运行时分类。完整字段值仍以仓库中的价格 JSON 及 Git diff 为准。
- `official_fields_reviewed` 只表示该行的特定字段对照过官网；其余行是固定上游版本的逐条比对。没有声称 4,555 条均经过逐项官网核验或付费 API 调用。
- `removed_from_snapshot` 只表示本次数据源不再提供该行，不单凭它断言厂商已下线。`unreviewed` / `pricing_only` 保留原有能力隔离原则。

## 已落实的型号与行为更新

| 提供商/接口 | 更新 | 依据与边界 |
|---|---|---|
| OpenAI | GPT‑6 Astra、Sol、6.1 Sol、Luna；GPT Image 2.5 Flare/Sunburst | [官方模型目录](https://developers.openai.com/api/docs/models)；文本模型 1,050,000 上下文、128,000 输出。Astra/6.1 Sol 的 Chat Completions 参数列表不宣称工具调用；工具应走 Responses。Sol/Luna 在 Chat Completions 使用工具时需要 `reasoning_effort=none`。 |
| Azure OpenAI | 四个 GPT‑6 规范型号进入已核验可调用分类 | [Microsoft 模型表](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-sold-directly-by-azure)；区域报价行不自动升级为可调用模型，仍需部署与配额。 |
| Azure AI | Cohere Rerank 4 Pro/Fast，采用微软文档的大小写 ID | 同上；重排不暴露聊天参数，不把每次搜索价格写成 token 单价。原测试引用的 V3.5 已从上游快照移除，改用已核验 V4。 |
| Anthropic | Claude Fable 5.1、Opus 5.5、Sonnet 5.5 | [模型总览](https://platform.claude.com/docs/en/models/overview)、[价格](https://platform.claude.com/docs/en/about-claude/pricing)；1M/128K、缓存价格、精确型号识别；Fable/Opus 不允许关闭 thinking；Sonnet 的关闭前置思考映射为 `between_tools`，仅 high 及以下。 |
| Gemini / Vertex | Gemini 3.8 Flash，以及工具层上下文查询和价格日程 | [模型卡](https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash)、[价格](https://ai.google.dev/gemini-api/docs/pricing)；修复 Vertex 新价格页地址导致优惠截止日失效的问题，测试覆盖 2027-01-01 UTC 切换。 |
| DeepSeek | `deepseek-flash` 规范 ID、V4.1 Flash 价格及现有 V4 Flash 别名 | [官方价格](https://api-docs.deepseek.com/quick_start/pricing)；非高峰输入/输出/缓存每百万 $0.15/$0.60/$0.003，高峰翻倍；Vision 与输出上限同步。保留现有时间计费表达方式。 |
| xAI | Grok 4.7，500K 上下文、xhigh 推理参数 | [Grok 4.7](https://docs.x.ai/developers/models/grok-4.7)；同步缓存和长上下文价格。 |
| Mistral | 托管 `zai-glm-5-3` | [官方模型卡](https://docs.mistral.ai/models/zai-glm-5-3)；1M/128K，输入/输出每百万 $1.4/$4.4。Medium 3.5、Small 4 等已有型号保留。 |
| Cohere | 修正 Command R/R+ 08-2024 静态报价 | [价格](https://cohere.com/pricing)；R 为 $0.15/$0.60，R+ 为 $2.5/$10（每百万），与现有正式报价覆盖保持一致；已有 Command A+、Embed 4、Rerank 4 无需另造别名。 |
| Voyage | Rerank 3、Rerank 3 Lite | [重排文档](https://docs.voyageai.com/docs/reranker)；32K 输入；已有 Voyage 4 / Code 4 保留。 |
| ElevenLabs | Scribe v2 | [官方型号](https://elevenlabs.io/docs/overview/models)；加入已有文件转录接口，不假装新增实时传输。 |
| Deepgram | 对照 Nova 3 / Aura 2 | 现有原生目录已包含这两代；价格随快照同步。 |
| Bedrock | Claude Fable 5.1 / Opus 5.5 / Sonnet 5.5、四个 GPT‑6 型号；修正 Nova 2 Lite/Premier 的静态限制和报价 | [AWS 模型卡](https://docs.aws.amazon.com/bedrock/latest/userguide/model-cards.html)；Sonnet 5.5 仅 Global，Fable 5.1 为 US/Global，Opus 5.5 包括 AU/JP；GPT‑6.1 Sol 仅 US runtime profile、131,072 输出及 10% 区域价格增幅。价格表中的其他地域/参考报价不代表运行时入口可用。 |
| Cloudflare | 8 个当前聊天型号，见下表 | 逐个查询 [官方模型卡](https://developers.cloudflare.com/workers-ai/models/)；原生适配器尚未实现流式/工具/视觉转发，新增条目不宣称这些能力；未公布的输出上限设为未知。新增收费模型不再被成本函数固定算作 0。 |
| GitHub Copilot | GPT‑6 Astra / Sol / Luna、`claude-opus-5.5` | [官方 CLI 型号 ID](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference)；账号/客户端限制不能照抄原厂，新增条目以 0/None 表示未知。未取得账号目录，未验证 HTTP 可用性；此证据仅核验公开 ID。 |
| BFL | FLUX.2 Max、Pro Preview/固定版、Flex、Klein 4B、Klein 9B Preview/固定版 | [官方生成端点](https://docs.bfl.ai/quick_start/generating_images)；复用已有异步生成接口，校验原生宽高参数；没有扩展新的编辑协议。 |
| fal | FLUX.2 Pro/Flex、Recraft V4/Pro、Ideogram V4 | 各型号官方卡；固定价格 Recraft 为 $0.04/$0.25，按百万像素/质量计费的型号不虚构固定单价，token 成本接口明确返回不支持。 |
| Replicate | FLUX.2 Pro | [官方模型卡](https://replicate.com/black-forest-labs/flux-2-pro)；复用已有预测接口；按图像维度计费，token 成本接口不误报 0。 |
| Stability | 检查现有 Stable Image Core/Ultra、SD 3.5 | 已有当前生成接口型号；上游价格快照刷新。 |

Cloudflare 本次逐型号核验：

| 精确 ID | 上下文 | 输入/输出每百万 USD |
|---|---:|---:|
| `@cf/zai-org/glm-5.3` | 1,048,576 | 1.4 / 4.4 |
| `@cf/zai-org/glm-5.3-flash` | 1,048,576 | 0.15 / 0.50 |
| `@cf/deepseek-ai/deepseek-v4-flash-0731` | 1,048,576 | 0.44 / 1.32 |
| `@cf/deepseek-ai/deepseek-v4-pro-0813` | 1,048,576 | 1.32 / 3.96 |
| `@cf/google/gemma-4-26b-a4b-it` | 256,000 | 0.10 / 0.30 |
| `@cf/moonshotai/kimi-k2.7-code` | 262,144 | 0.95 / 4.0 |
| `@cf/openai/gpt-oss-120b` | 128,000 | 0.35 / 0.75 |
| `@cf/qwen/qwen3-30b-a3b-fp8` | 32,768 | 0.0509 / 0.335 |

## 仍然不能等同于“全库每个模型均已验证最新可调用”

1. 通用 OpenAI 兼容提供商接受模型 ID 透传，不维护完整静态白名单；价格同步不等于官方所有新接口已实现。新传输（例如 GPT‑Live、专用 OCR、音乐/实时音频）本次没有冒充聊天模型开放。
2. Copilot 完整账号目录、企业私有部署、区域配额和本地下载模型需要对应运行环境才能确认。没有读取或输出任何真实凭据，也没有执行付费推理。
3. 历史静态条目不等于仍可调用。Cloudflare 已公布部分旧模型在 2026-05-30 下线；本次添加当前型号，但没有重构旧枚举及全库历史型号的生命周期管理。参考 [下线公告](https://developers.cloudflare.com/changelog/post/2026-05-08-planned-model-deprecations/)。
4. DeepSeek 官方高峰规则排除中国公共假日；现有运行时仅表达星期和 UTC 小时，假日例外仍不能精确结算。本次改正新报价，未引入日历规则系统。
5. DeepSeek 官方已公告 `deepseek-chat` / `deepseek-reasoner` 在 2026-07-24 停用；裸 ID 和带 `deepseek/` 前缀的四条历史价格记录保留基线值，不套用 V4.1 的新字段，清单标为 `historical_alias_retired`。它们仍为 `unreviewed`，这些历史值不代表当前报价或可调用性。[官方更新日志](https://api-docs.deepseek.com/updates/)。
6. Nova Premier 的官方模型卡标为 Legacy，且同页 EOL 日期存在矛盾；本次只校正 25K 输出和 US profile，不据此推断实际下线日。[AWS 模型卡](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-amazon-nova-premier.html)。
7. Bedrock 的其他旧 generic 条目、Copilot 未公布的 HTTP 型号参数、图像复杂账单仍有未验证范围。清单保留 `unreviewed`，不能把上游字段当作原生适配器的执行保证。

## 每个通用提供商入口的核对结果

下表逐个列出 `registry/catalog.rs` 中现有入口。除明确列出的静态特殊目录外，入口使用模型 ID 透传；有价格行的随本次上游快照逐行同步。无价格行表示没有可同步的固定公开价格，不表示免费或未实现。

| 入口 | 处理 | 当前同名价格行数 |
|---|---|---:|
| `ai21` | 模型 ID 透传；比较上游价格快照 | 12 |
| `aiml` | 模型 ID 透传；比较上游价格快照 | 13 |
| `aiml_api` | 模型 ID 透传；比较上游价格快照 | 0 |
| `aleph_alpha` | 模型 ID 透传；比较上游价格快照 | 0 |
| `amazon_nova` | 静态特殊目录及报价已核对 | 4 |
| `anyscale` | 模型 ID 透传；比较上游价格快照 | 12 |
| `baichuan` | 模型 ID 透传；比较上游价格快照 | 0 |
| `baseten` | 模型 ID 透传；比较上游价格快照 | 27 |
| `bytez` | 模型 ID 透传；比较上游价格快照 | 0 |
| `cerebras` | 模型 ID 透传；比较上游价格快照 | 7 |
| `comet_api` | 模型 ID 透传；比较上游价格快照 | 0 |
| `compactifai` | 模型 ID 透传；比较上游价格快照 | 0 |
| `dashscope` | 模型 ID 透传；比较上游价格快照 | 47 |
| `deepinfra` | 模型 ID 透传；比较上游价格快照 | 134 |
| `deepseek` | 模型 ID 透传；比较上游价格快照 | 17 |
| `docker_model_runner` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `featherless` | 模型 ID 透传；比较上游价格快照 | 0 |
| `fireworks` | 模型 ID 透传；比较上游价格快照 | 0 |
| `fireworks_ai` | 模型 ID 透传；比较上游价格快照 | 335 |
| `friendliai` | 模型 ID 透传；比较上游价格快照 | 7 |
| `galadriel` | 模型 ID 透传；比较上游价格快照 | 0 |
| `github` | 静态特殊目录及报价已核对 | 0 |
| `groq` | 模型 ID 透传；比较上游价格快照 | 19 |
| `heroku` | 模型 ID 透传；比较上游价格快照 | 4 |
| `hosted_vllm` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `huggingface` | 模型 ID 透传；比较上游价格快照 | 0 |
| `hyperbolic` | 模型 ID 透传；比较上游价格快照 | 16 |
| `infinity` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `lambda_ai` | 模型 ID 透传；比较上游价格快照 | 20 |
| `lemonade` | 模型 ID 透传；比较上游价格快照 | 5 |
| `linkup` | 模型 ID 透传；比较上游价格快照 | 2 |
| `llamafile` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `lm_studio` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `maritalk` | 模型 ID 透传；比较上游价格快照 | 0 |
| `meta_llama` | 静态特殊目录及报价已核对 | 4 |
| `minimax` | 模型 ID 透传；比较上游价格快照 | 21 |
| `moonshot` | 模型 ID 透传；比较上游价格快照 | 17 |
| `nanogpt` | 模型 ID 透传；比较上游价格快照 | 0 |
| `nebius` | 模型 ID 透传；比较上游价格快照 | 60 |
| `novita` | 模型 ID 透传；比较上游价格快照 | 135 |
| `nscale` | 模型 ID 透传；比较上游价格快照 | 16 |
| `nvidia_nim` | 模型 ID 透传；比较上游价格快照 | 3 |
| `oobabooga` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `openrouter` | 模型 ID 透传；比较上游价格快照 | 484 |
| `ovhcloud` | 模型 ID 透传；比较上游价格快照 | 15 |
| `perplexity` | 模型 ID 透传；比较上游价格快照 | 77 |
| `poe` | 模型 ID 透传；比较上游价格快照 | 0 |
| `qwen` | 模型 ID 透传；比较上游价格快照 | 0 |
| `sambanova` | 模型 ID 透传；比较上游价格快照 | 8 |
| `siliconflow` | 模型 ID 透传；比较上游价格快照 | 0 |
| `together` | 模型 ID 透传；比较上游价格快照 | 0 |
| `together_ai` | 模型 ID 透传；比较上游价格快照 | 90 |
| `v0` | 静态特殊目录及报价已核对 | 3 |
| `vllm` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `volcengine` | 模型 ID 透传；比较上游价格快照 | 14 |
| `wandb` | 模型 ID 透传；比较上游价格快照 | 30 |
| `xai` | 模型 ID 透传；比较上游价格快照 | 57 |
| `xiaomi_mimo` | 模型 ID 透传；比较上游价格快照 | 4 |
| `xinference` | 本地部署决定型号；不替用户下载/替换模型 | 0 |
| `yi` | 模型 ID 透传；比较上游价格快照 | 0 |
| `zai` | 模型 ID 透传；比较上游价格快照 | 16 |
| `zhipu` | 模型 ID 透传；比较上游价格快照 | 25 |

企业/部署入口 `databricks`、`snowflake`、`oci`、`watsonx`、`sagemaker` 使用用户配置的部署 ID；`ollama` 支持本机动态目录，`custom_api` 使用配置。它们没有一个能脱离用户部署声明的“全局最新型号”。`azure`、`azure_ai`、`vertex_ai` 同时受部署/区域约束；本次的公开目录核验不代替权限验证。

## 每个价格 provider 标签的比对结果

这是数据覆盖表，不是库支持的提供商白名单。逐模型详情见 JSON 清单。

| 价格 provider | 原条数 | 当前条数 | 新增 | 更新 | 移除 |
|---|---:|---:|---:|---:|---:|
| `01ai` | 2 | 2 | 0 | 0 | 0 |
| `agentcore` | 1 | 1 | 0 | 0 | 0 |
| `ai21` | 12 | 12 | 0 | 0 | 0 |
| `aihubmix` | 0 | 72 | 72 | 0 | 0 |
| `aiml` | 13 | 13 | 0 | 0 | 0 |
| `amazon_nova` | 4 | 4 | 0 | 0 | 0 |
| `anthropic` | 28 | 25 | 4 | 12 | 7 |
| `anyscale` | 12 | 12 | 0 | 0 | 0 |
| `apiserpent` | 2 | 2 | 0 | 0 | 0 |
| `assemblyai` | 2 | 2 | 0 | 0 | 0 |
| `aws_polly` | 4 | 4 | 0 | 0 | 0 |
| `azure` | 220 | 305 | 96 | 151 | 11 |
| `azure_ai` | 112 | 141 | 39 | 85 | 10 |
| `azure_text` | 3 | 3 | 0 | 0 | 0 |
| `baseten` | 11 | 27 | 16 | 2 | 0 |
| `bedrock` | 269 | 293 | 41 | 119 | 17 |
| `bedrock_converse` | 151 | 219 | 69 | 132 | 1 |
| `bedrock_mantle` | 15 | 47 | 32 | 10 | 0 |
| `bing_grounding` | 1 | 1 | 0 | 1 | 0 |
| `black_forest_labs` | 8 | 8 | 0 | 0 | 0 |
| `cerebras` | 7 | 7 | 2 | 0 | 2 |
| `chatgpt` | 10 | 14 | 4 | 8 | 0 |
| `cloudflare` | 30 | 36 | 6 | 30 | 0 |
| `codestral` | 2 | 2 | 0 | 0 | 0 |
| `cognition` | 3 | 3 | 0 | 0 | 0 |
| `cohere` | 35 | 30 | 1 | 1 | 6 |
| `cohere_chat` | 1 | 0 | 0 | 0 | 1 |
| `crusoe` | 7 | 7 | 0 | 0 | 0 |
| `darkbloom` | 2 | 2 | 0 | 0 | 0 |
| `dashscope` | 45 | 47 | 2 | 0 | 0 |
| `databricks` | 49 | 62 | 23 | 28 | 10 |
| `dataforseo` | 1 | 1 | 0 | 0 | 0 |
| `deepgram` | 37 | 43 | 6 | 0 | 0 |
| `deepinfra` | 135 | 134 | 0 | 6 | 1 |
| `deepseek` | 15 | 17 | 2 | 9 | 0 |
| `duckduckgo` | 1 | 1 | 0 | 0 | 0 |
| `elevenlabs` | 4 | 5 | 1 | 0 | 0 |
| `exa_ai` | 1 | 1 | 0 | 0 | 0 |
| `fal_ai` | 71 | 210 | 139 | 0 | 0 |
| `featherless_ai` | 2 | 2 | 0 | 0 | 0 |
| `firecrawl` | 1 | 1 | 0 | 0 | 0 |
| `fireworks_ai` | 314 | 335 | 23 | 40 | 2 |
| `fireworks_ai-embedding-models` | 7 | 7 | 0 | 0 | 0 |
| `friendliai` | 2 | 7 | 7 | 0 | 2 |
| `gemini` | 84 | 85 | 17 | 45 | 16 |
| `gigachat` | 6 | 7 | 2 | 0 | 1 |
| `github_copilot` | 33 | 31 | 0 | 4 | 2 |
| `gmi` | 17 | 16 | 0 | 1 | 1 |
| `google_pse` | 1 | 1 | 0 | 0 | 0 |
| `gradient_ai` | 13 | 13 | 0 | 0 | 0 |
| `groq` | 27 | 19 | 2 | 0 | 10 |
| `heroku` | 4 | 4 | 0 | 0 | 0 |
| `hyperbolic` | 16 | 16 | 0 | 0 | 0 |
| `inception` | 1 | 2 | 1 | 0 | 0 |
| `jina_ai` | 1 | 1 | 0 | 1 | 0 |
| `lambda_ai` | 20 | 20 | 0 | 0 | 0 |
| `lemonade` | 5 | 5 | 0 | 0 | 0 |
| `libertai` | 12 | 12 | 0 | 0 | 0 |
| `linkup` | 2 | 2 | 0 | 0 | 0 |
| `llamagate` | 16 | 16 | 0 | 0 | 0 |
| `meta` | 3 | 6 | 3 | 3 | 0 |
| `meta_llama` | 4 | 4 | 0 | 4 | 0 |
| `minimax` | 21 | 21 | 0 | 0 | 0 |
| `mistral` | 105 | 95 | 17 | 42 | 27 |
| `moonshot` | 29 | 17 | 1 | 0 | 13 |
| `morph` | 2 | 2 | 0 | 0 | 0 |
| `nebius` | 30 | 60 | 30 | 30 | 0 |
| `nimble` | 1 | 1 | 0 | 0 | 0 |
| `nlp_cloud` | 2 | 2 | 0 | 0 | 0 |
| `novita` | 135 | 135 | 0 | 2 | 0 |
| `nscale` | 16 | 16 | 0 | 0 | 0 |
| `nvidia_nim` | 3 | 3 | 0 | 0 | 0 |
| `oci` | 44 | 44 | 0 | 9 | 0 |
| `ollama` | 29 | 29 | 0 | 0 | 0 |
| `openai` | 227 | 210 | 13 | 121 | 30 |
| `openrouter` | 100 | 484 | 386 | 83 | 2 |
| `ovhcloud` | 15 | 15 | 0 | 15 | 0 |
| `palm` | 6 | 6 | 0 | 0 | 0 |
| `parallel_ai` | 2 | 4 | 2 | 2 | 0 |
| `perplexity` | 47 | 77 | 30 | 10 | 0 |
| `pinstripes` | 6 | 6 | 0 | 6 | 0 |
| `prism` | 0 | 2 | 2 | 0 | 0 |
| `publicai` | 9 | 9 | 0 | 0 | 0 |
| `qwen_ai_platform` | 0 | 47 | 47 | 0 | 0 |
| `qwencloud` | 0 | 45 | 45 | 0 | 0 |
| `recraft` | 2 | 2 | 0 | 0 | 0 |
| `reducto` | 2 | 2 | 0 | 0 | 0 |
| `replicate` | 40 | 40 | 0 | 2 | 0 |
| `runwayml` | 14 | 14 | 0 | 0 | 0 |
| `sagemaker` | 6 | 6 | 0 | 0 | 0 |
| `sail` | 0 | 12 | 12 | 0 | 0 |
| `sambanova` | 19 | 8 | 0 | 0 | 11 |
| `sarvam` | 1 | 1 | 0 | 0 | 0 |
| `scaleway` | 17 | 15 | 2 | 2 | 4 |
| `scx-ai` | 2 | 2 | 0 | 0 | 0 |
| `searxng` | 1 | 1 | 0 | 0 | 0 |
| `serper` | 1 | 1 | 0 | 0 | 0 |
| `snowflake` | 37 | 37 | 0 | 0 | 0 |
| `soniox` | 2 | 2 | 0 | 0 | 0 |
| `stability` | 25 | 25 | 0 | 0 | 0 |
| `tavily` | 2 | 2 | 0 | 0 | 0 |
| `tencent` | 2 | 3 | 1 | 0 | 0 |
| `tensormesh` | 10 | 10 | 0 | 0 | 0 |
| `text-completion-codestral` | 2 | 2 | 0 | 0 | 0 |
| `text-completion-inception` | 1 | 1 | 0 | 0 | 0 |
| `text-completion-openai` | 6 | 6 | 0 | 5 | 0 |
| `tinyfish` | 1 | 1 | 0 | 0 | 0 |
| `together_ai` | 67 | 90 | 40 | 37 | 17 |
| `transcribe` | 0 | 1 | 1 | 0 | 0 |
| `typesafe` | 0 | 3 | 3 | 0 | 0 |
| `v0` | 3 | 3 | 0 | 0 | 0 |
| `vercel_ai_gateway` | 101 | 99 | 0 | 2 | 2 |
| `vertex_ai` | 28 | 55 | 27 | 13 | 0 |
| `vertex_ai-ai21_models` | 5 | 5 | 0 | 5 | 0 |
| `vertex_ai-anthropic_models` | 37 | 36 | 6 | 28 | 7 |
| `vertex_ai-deepseek_models` | 3 | 3 | 0 | 3 | 0 |
| `vertex_ai-embedding-models` | 10 | 10 | 0 | 4 | 0 |
| `vertex_ai-image-models` | 8 | 0 | 0 | 0 | 8 |
| `vertex_ai-language-models` | 38 | 34 | 2 | 22 | 6 |
| `vertex_ai-llama_models` | 11 | 12 | 1 | 2 | 0 |
| `vertex_ai-minimax_models` | 1 | 1 | 0 | 1 | 0 |
| `vertex_ai-mistral_models` | 19 | 19 | 0 | 10 | 0 |
| `vertex_ai-moonshot_models` | 1 | 1 | 0 | 1 | 0 |
| `vertex_ai-openai_models` | 3 | 3 | 0 | 3 | 0 |
| `vertex_ai-qwen_models` | 4 | 4 | 0 | 4 | 0 |
| `vertex_ai-text-models` | 2 | 2 | 0 | 0 | 0 |
| `vertex_ai-video-models` | 7 | 8 | 1 | 7 | 0 |
| `vertex_ai-zai_models` | 2 | 3 | 1 | 2 | 0 |
| `volcengine` | 12 | 14 | 2 | 4 | 0 |
| `voyage` | 32 | 35 | 3 | 0 | 0 |
| `wandb` | 35 | 30 | 5 | 23 | 10 |
| `watsonx` | 29 | 31 | 2 | 6 | 0 |
| `xai` | 36 | 57 | 42 | 12 | 21 |
| `xiaomi_mimo` | 2 | 4 | 2 | 0 | 0 |
| `you_com` | 1 | 1 | 0 | 0 | 0 |
| `zai` | 14 | 16 | 2 | 0 | 0 |
| `zhipu` | 25 | 25 | 0 | 0 | 0 |

## 验证

验收工作树已普通合并 current main `307f04b7`。下面记录合成后的实际结果，不把原 PR 的旧验证当成当前结果。

- `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`：40 项通过。
- `cargo check --features providers-extra,providers-extended`：通过。current main 在同一公开组合下原有 5 个 Gemini helper cfg 编译错误；本次仅使这 5 个共享 helper 对 `providers-extended` 可见，错误/密钥脱敏路径保持不变。
- `cargo test --features providers-extra,providers-extended --no-fail-fast -- --test-threads=2`：通过；库单元测试 8,637 通过、1 忽略，集成测试与文档测试通过。旧 `public_api_compat` 的网关/存储部分增加 `storage` cfg，3 个原生兼容测试继续在库组合执行。
- 完整网关组合 `postgres,sqlite,redis,s3,metrics,tracing,websockets,analytics,providers-extra,providers-extended`：`cargo check` 通过；`cargo test --lib --tests --bins --features <组合> --no-fail-fast` 的集成/命令行测试全部通过，库首次 11,104 通过、1 项未修改的 500ms 健康探测测试超时、1 忽略。该项单独复测通过；`cargo test --lib --features <组合> -- --test-threads=2` 最终 11,105 通过、1 忽略，没有改变超时阈值。
- `cargo clippy --all-targets --features <完整网关组合> -- -D warnings`：通过。没有声称无网关的 native 组合通过严格 Clippy；该组合的现有未使用网关 helper 警告仍然存在，没有新增全局 allow。
- `cargo fmt --check`、`git diff --check`：通过。
- `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`：通过；4,437 条上游记录 + 207 条覆盖记录，合并后 4,555 条。并集 4,813 个精确 key 的字段差异与 authority/decision 生成结果独立核验一致。

忽略项未视为已执行。测试不包含真实厂商付费 API 冒烟；官网只核验公开型号及选定字段，不能证明账号、区域或私有部署实际可调用。


## 2026-10-03 静态目录退役复核（F10 第二批）

- Anthropic：移除 Opus 4.1、Opus 4、Sonnet 4、Haiku 3.5、Sonnet 3.5、Opus 3、Sonnet 3、Haiku 3、Claude 2.1、Instant 1.2，以及指向它们的 12 个别名。健康探测改用仍可用的 Haiku 4.5。Sonnet 4.5 仅弃用、到 2026-11-30 才退役，本批保留。
- Claude 4.6 起使用无日期的固定型号 ID，移除 5 个未经官方证实的日期/latest 别名，保留官方 pre-4.6 短别名。此前审计文档中的这些别名声明由本条更正。
- Gemini：移除 1.0 Pro、1.5 Pro/Flash/Flash-8B、2.0 Flash experimental/thinking experimental、3 Pro Image preview；另外移除没有可核实公开 API ID 的 `gemini-3-pro`、`gemini-3-pro-deep-think`、`gemini-3.1-flash`，这三项不作有退役日期的断言。Developer API 原有过滤；本次消除共享注册表和 Vertex/experimental surface 中的残留。
- Vertex 额外传输放行名单中的 1.5/2.0 型号也已移除；旧 ID 在网络调用前返回 ModelNotFound，历史费用查询仍可用。健康探测改用现有 Gemini 3.7 Flash；crate quick start 与 Claude 公共别名工具同步到保留型号。
- Gemini 2.5 Pro/Flash/Flash-Lite 仍可用。按官方标准文本缓存价修正为 $0.125/$0.03/$0.01 每百万 tokens；媒体按 tokens 计费，移除目录中未经证实的固定每张图/每秒价格。中央价格库仍负责运行时分档、多模态计费，本批不建立第二套计费机制。
- 历史价格库和历史模型家族分类保持独立，不因取消可调用声明而删除既有历史查询记录。真实供应商网络调用未运行。
- 后续审查：公共 Gemini 校验改用精确目录；原生 generateContent/streamGenerateContent 在发送前按 Developer/Vertex surface 拒绝退役及不可用型号。SDK Claude 5 请求复用原生参数/prefill 校验，按 [Claude Messages 参数文档](https://platform.claude.com/docs/en/api/http/beta/messages/create) 和 [API primer](https://platform.claude.com/docs/en/claude_api_primer) 保留 InvalidRequest 分类。默认完整 test/check/clippy、格式检查通过（库 7,266 通过、1 忽略）；gateway/sqlite/providers-extra/providers-extended 的 Gemini 226 项、Vertex 254 项及 all-target clippy 通过。新原生测试同时覆盖 JSON/SSE，确认拒绝请求不触发本地 HTTP 连接。

依据（2026-10-03 核对）：[Claude 退役表](https://platform.claude.com/docs/en/about-claude/model-deprecations)、[Claude ID 规范](https://platform.claude.com/docs/en/about-claude/models/model-ids-and-versions)、[Gemini 退役表](https://ai.google.dev/gemini-api/docs/deprecations)、[Gemini 更新记录](https://ai.google.dev/gemini-api/docs/changelog)、[Vertex 生命周期](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/model-versions)、[Gemini 定价](https://ai.google.dev/gemini-api/docs/pricing)。

## 2026-10-03 Mistral 能力与固定快照复核

Ministral 3 的 3B/8B/14B 都支持视觉；3B 的上下文上限应为 262,144，而非现有的 128,000/131,072。已修正三个 latest 与三个 2512 条目。价格单位核对后仍为每百万输入/输出分别 0.10、0.15、0.20 美元，不作无依据改价。

三个 Ministral 2512 及两个 Magistral 2509 的固定 ID 不应在发请求时改写成 latest，现保留调用者选择的日期快照。Pixtral Large、Pixtral 12B、Mistral Nemo、Devstral 2、Magistral Medium 1.2 和 Mistral Small 3.2 的官方卡片当前标注 deprecated，并非 retired；未据此删除仍可用的目录记录。

依据：[Ministral 3B](https://docs.mistral.ai/models/ministral-3-3b-25-12)、[8B](https://docs.mistral.ai/models/ministral-3-8b-25-12)、[14B](https://docs.mistral.ai/models/ministral-3-14b-25-12)、[官方参数中的精确上下文长度](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512/blob/cfcb068fa7c44114cf77a462357c6cdcd2c304b4/params.json)、[Magistral Medium 1.2](https://docs.mistral.ai/models/magistral-medium-1-2-25-09)、[生命周期规则](https://docs.mistral.ai/inference/model-lifecycle)。测试仅使用本地请求转换，不代表供应商账户实调。
