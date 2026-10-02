# LiteLLM 差距补齐清单

更新日期：2026-10-03。起点：`e9cf6a4b`（模型更新 PR #1360 已合并）。

本清单记录此次审计发现的全部缺口。参考 BerriAI/litellm 的实现和官方协议，模型事实以供应商官方资料为准；不把价格表中的模型自动当作可调用模型。

状态：待开始 → 进行中 → 待验收 → 完成。只有代码、测试和关联 PR 均有可查证结果时才标为完成；合并和发布单独记录。每完成一项，在本文件更新状态、证据及剩余限制。每个实现 PR 对应一个 issue。

| ID | 优先级 | 功能 / 问题 | 验收条件 | 状态 | Issue / PR / 验证 |
| --- | --- | --- | --- | --- | --- |
| F01 | P1 | 恢复模型价格自动更新 | 从有效上游引用解析不可变提交；新增模型进入 unreviewed；保留人工决定；更新后的三个目录文件一致；重复运行无差异 | 完成 | [#1364](https://github.com/majiayu000/litellm-rs/issues/1364)；51 项 Python 测试通过；当前源新增 14 条 unreviewed，4,555 条已有决定不变；重复同步/check 通过；Rust 默认全量检查通过；[PR #1366](https://github.com/majiayu000/litellm-rs/pull/1366)；已隔离 Voyage 缺价测试数据并通过 gateway/sqlite 定向测试；补充空证据源拒绝测试；四条上游峰谷价格已转换为运行时格式，峰/谷与预算上限回归通过；全部 CI 通过，review 已解决，已合并 `fcc8f1b1` |
| F02 | P1 | Cloudflare 错误语义 | HTTP 401/429/5xx 和 success=false 返回对应错误；畸形成功响应不能变成空成功；覆盖真实 HTTP 模拟测试 | 完成 | [#1365](https://github.com/majiayu000/litellm-rs/issues/1365) / [PR #1367](https://github.com/majiayu000/litellm-rs/pull/1367)；91 项相关测试、默认全量测试/check/clippy 通过；全部 CI 通过、无未解决 review；已合并 `01f15630`；一次 Gemini 测试超时，定向复测 9 项通过且同提交 CI 重跑通过，未将偶发超时误报为已修复 |
| F03 | P1 | Cloudflare 现代聊天协议与流式 | 使用官方兼容接口；文本、工具、多模态、用量、流式及中途错误按支持能力正确传递；文档和能力声明一致 | 完成 | [#1368](https://github.com/majiayu000/litellm-rs/issues/1368)；[PR #1369](https://github.com/majiayu000/litellm-rs/pull/1369)；聊天/流式本地 HTTP 测试、默认全量测试/check/clippy 通过；审查后的 88 项 provider 测试与 clippy 通过；全部 CI 通过，review 已解决；已合并 `4b4a4dd4` |
| F04 | P1 | Bedrock 逐模型能力与计费 | 审核 generic_converse 全部条目；修正上下文、输出、视觉、推理、profile 与价格；消除通用虚假默认值和重复价格源 | 完成 | [#1370](https://github.com/majiayu000/litellm-rs/issues/1370)；逐项核对 39 张 AWS 模型卡及官网定价；修正通用参数、移除 Sonic 聊天声明、统一计费来源；382 项 Bedrock 测试通过；[PR #1371](https://github.com/majiayu000/litellm-rs/pull/1371)；默认全量测试/check/clippy 及全部 CI 已通过，无未解决 review；已合并 `e77391fe` |
| F05 | P1 | 原生 Responses 请求通路 | 支持该协议的供应商真正调用原生 endpoint；保留工具、推理、原生事件、用量及错误；经过现有鉴权、路由、预算和记录链路 | 进行中 | [#1372](https://github.com/majiayu000/litellm-rs/issues/1372)；OpenAI 原生传输、预算、内容检查和回调通路已接入；10 项集成测试与 4 项单测通过，现有 Responses 8 项回归通过；默认全量 test/check/clippy 和 gateway/sqlite clippy 通过；[草稿 PR #1374](https://github.com/majiayu000/litellm-rs/pull/1374)；原生存储在后续草稿 #1379 接入；后台生命周期、其他供应商矩阵及工具/文件计费未完成，不可合并 |
| F06 | P1 | Responses 模型与供应商路由 | OpenAI、Copilot、Bedrock 原生协议分别按官方 endpoint/model 支持矩阵路由；Responses-only 模型和工具不被送往 chat/completions | 进行中 | [#1375](https://github.com/majiayu000/litellm-rs/issues/1375)；111 个 OpenAI 模型/快照的官方端点已进入现有目录与路由，Responses-only/非聊天模型不再误走聊天；158 项 OpenAI 测试、12 项原生路由测试、51 项 Python 测试和默认全量 test/check/clippy 通过；Copilot/Bedrock 尚待接入 |
| F07 | P1 | Responses 跨副本持久状态 | 两个网关实例可读/删同一授权响应；重启后可恢复记录；租户隔离、TTL、后台状态及取消语义有测试 | 进行中 | [#1378](https://github.com/majiayu000/litellm-rs/issues/1378)；已将聊天适配路径的生命周期接入 SQL，10 项生命周期测试（包含双网关及重启）、3 项数据库竞争/清理测试、9 项路由回归（包括跨网关取消执行中请求）通过，[草稿 PR #1379](https://github.com/majiayu000/litellm-rs/pull/1379)；原生同步/流式存储、查询、输入列表、删除已接入租户/部署/账户绑定，19 项原生路由测试通过，含部署固定、上下文预算和策略变更拦截；72 项预算测试串行通过（并行时一项日志捕获断言失败，未声称已修复并发日志测试）；原生后台单次结算、跨网关取消及带游标的流式恢复已实现；19 项原生路由、9 项适配路由及默认全量 test/check/clippy、gateway/sqlite clippy 通过；store=false 只留十分钟临时元数据；仍缺进程重启后的后台费用恢复，九分钟内无最终用量按现有预留金额规则结算；全项保持进行中 |
| F08 | P1 | Anthropic 原生 Messages 网关 | 提供 /v1/messages；工具、thinking、流式、错误、用量和鉴权符合原生协议；文档/OpenAPI/路由一致 | 进行中 | [#1380](https://github.com/majiayu000/litellm-rs/issues/1380)；原生 JSON/SSE、鉴权与密钥令牌限制、原生 HTTP/流式错误、内容检查和累计用量已接入；[草稿 PR #1381](https://github.com/majiayu000/litellm-rs/pull/1381)；13 项路由测试、5 项 OpenAPI 契约测试、缓存 TTL/搜索费用测试，以及默认全量 test/check/clippy、gateway/sqlite clippy 通过；托管工具非令牌费用、媒体输入与工具费用的预算预估尚未覆盖，暂不标为完成 |
| F09 | P2 | Responses compact 与 Gemini 文档覆盖 | 接通 /v1/responses/compact；Gemini 已有 generateContent/streamGenerateContent 路由，补 OpenAPI 覆盖并复核现有鉴权/流式/用量测试 | 待验收 | [#1382](https://github.com/majiayu000/litellm-rs/issues/1382)；compact 已复用原生传输、鉴权、费用与 previous_response_id 部署/账户绑定；4 项压缩定向测试通过；Gemini OpenAPI 的 8 个路径均已实际请求通过；23 项原生 Responses、29 项 Gemini 路由和 5 项 OpenAPI 契约测试通过；默认全量 test/check/clippy 及 gateway/sqlite clippy 通过；[草稿 PR #1383](https://github.com/majiayu000/litellm-rs/pull/1383)；依赖 #1379，等待 CI 与审查 |
| F10 | P1 | 模型退役与目录一致性 | 去除已退役模型的可调用声明；优先修 Cloudflare；复核全部现有静态供应商目录、示例、价格和能力来源 | 进行中 | [#1373](https://github.com/majiayu000/litellm-rs/issues/1373)；首批修正 OpenAI/Azure 20 条退役 callable 决策；Cloudflare 清理 10 条过时/未验证记录；Copilot 清理 12 条退役或非公开可选记录；Bedrock 退役条目保留历史价格但不再进入路由；[PR #1376](https://github.com/majiayu000/litellm-rs/pull/1376)；2,797 项扩展 provider 测试、7,217 项默认库测试及全量 check/clippy 通过；全部 CI 通过、review 已解决，已合并 `42ba2b2a`；第二批已移除 Anthropic 10 个退役型号、12 个关联别名及 5 个未核实别名；Gemini 共 10 个退役或未核实条目已移出共享/Vertex 目录，Developer API 原已过滤；修正存续 Gemini 2.5 缓存价及不实固定媒体价，保留独立历史价格库；[PR #1385](https://github.com/majiayu000/litellm-rs/pull/1385)；第二批 7,795 项扩展库测试、默认全量 test/check/clippy、gateway/sqlite/扩展 provider clippy 均通过，其余静态目录逐项复核尚未完成；已有逐条基线：`docs/audit/model-catalog-2026-10-01.entries.json` |
| F11 | P2 | 兼容供应商的非聊天能力 | OpenAI-compatible 通路补齐 embeddings/images/audio 的实际调度；只声明经过协议验证的能力；未知/不支持能力返回明确错误 | 进行中 | 复核更正：通用 `openai_compatible` 已有 embeddings/images/audio 实现；30 项定向单测以及 embeddings、图片编辑/变体 HTTP 测试通过；`support_matrix.rs` 是旧适配器矩阵，不代表当前运行时。[#1386](https://github.com/majiayu000/litellm-rs/issues/1386)；Together/DeepInfra/Fireworks 已按官方矩阵扩展既有能力声明，定向 HTTP 验证进行中；其余具名供应商尚待核验，范围见 `docs/providers/compatible-nonchat.md` |
| F12 | P2 | 自定义供应商注册 | 外部实现可通过公开 API 注册并被路由，无须修改内部 Provider 枚举；覆盖构造、能力、错误和流式测试 | 待验收 | [#1384](https://github.com/majiayu000/litellm-rs/issues/1384)；外部接口已接入现有 Provider/Deployment/Router，all-features 编译通过；4 项外部集成测试通过（注册/路由、模型能力、流式错误、未实现能力/缺价）；默认完整测试/check/clippy、gateway/sqlite/扩展 provider clippy 均通过（默认库 7,217 项通过、1 项忽略）；等待 PR CI 与审查 |
| F13 | P2 | MCP 网关 | 把现有 MCP 能力接入 HTTP 网关；工具发现/调用与资源/提示词、鉴权、连接关闭有端到端测试；传输支持如实列出 | 待开始 | 目前只有 feature-gated 类型，缺 HTTP 挂载 |
| F14 | P2 | A2A 网关 | 接通 agent card、任务提交/查询/取消及事件流；代理鉴权、租户隔离和错误有端到端测试 | 待开始 | 目前只有 feature-gated 类型，缺 HTTP 挂载 |
| F15 | P2 | Realtime 网关 | 接通 WebSocket 双向代理；供应商配置、鉴权、事件/关闭/错误传递有测试；首批支持范围明确 | 待开始 | 目前没有公开网关路由 |
| F16 | P2 | 过时声明与未落地子系统 | 逐项核对 subsystem_registry 和 README；完成上述能力后同步状态，清理已到移除版本的废弃接口，避免“声明支持却不可用” | 待开始 | 依赖对应功能完成；不作无关架构重写 |
| F17 | P2 | 可复现的 LiteLLM 对比基准 | 同机器、同模拟上游和相同负载比较吞吐/延迟/错误率/内存；保存命令、版本和样本，不用 Rust 语言推断性能结论 | 待开始 | 复用 `docs/benchmarks/gateway-overhead.md` 的方法 |
| F18 | P1 | 发布与安装产物 | 所有已验收能力进入版本发行包；验证 crate、安装说明及容器内容；记录实际发布版本/提交，不能只凭 main 已合并声称已发布 | 待开始 | 当前 v0.7.0 不含 #1360；发布前准备可审阅结果 |

## 执行与验收记录

- 2026-10-03：建立清单；GitHub 当前无开放 issue/PR 与上述工作重复。先执行 F01，然后 F02–F04，再处理协议、状态和网关能力。独立功能拆分提交，避免将全部变化塞入一个 PR。
- 所有构建与测试在本任务独立 worktree 执行。每个 PR 准备好前执行仓库要求的格式、检查、全量测试及 clippy；合并前确认 CI 全绿且 review threads 已解决。
- 离线模拟测试与需要供应商账户的真实调用分开记录。没有凭据或没有运行过的真实调用不得标为通过。

- 2026-10-03 复核更正：Gemini 原生生成和流式接口在实际路由中已存在，F09 改为文档/契约覆盖，不重复实现。

- 2026-10-03 F07 进展：后台查询/取消和 SSE 重连共用创建请求的结算责任，不会重放生成请求；修复内容检查缓冲 response.created 导致断线时客户端拿不到 ID 的问题。存储与费用恢复分开验收：共享响应记录已具备重启可读性，后台结算任务尚未具备重启恢复能力。
