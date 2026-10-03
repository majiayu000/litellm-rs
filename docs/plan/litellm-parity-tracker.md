# LiteLLM 差距补齐清单

更新日期：2026-10-03 11:44（北京时间）。起点：`e9cf6a4b`（模型更新 PR #1360 已合并）。本次核查 main 为 `cb76a186`。

本文件是继续执行 F01–F18 的唯一台账。已完成 F01–F04、F12、F17；其余工作按下方复选清单推进。功能验收、合并和发布分别记录；当前发行仍为 v0.7.0，尚不包含本轮新增成果。

本清单记录此次审计发现的全部缺口。参考 BerriAI/litellm 的实现和官方协议，模型事实以供应商官方资料为准；不把价格表中的模型自动当作可调用模型。

状态：待开始 → 进行中 → 待验收 → 完成。只有代码、测试和关联 PR 均有可查证结果时才标为完成；合并和发布单独记录。每完成一项，在本文件更新状态、证据及剩余限制。每个实现 PR 对应一个 issue。修复已有 PR 时沿用原分支；不建立竞争 PR。表中既有测试数量来自关联 PR 的验证记录，本次整理未重新执行 Rust 测试；最新提交的 CI 状态单独列在下方。

| ID | 优先级 | 功能 / 问题 | 验收条件 | 状态 | Issue / PR / 验证 |
| --- | --- | --- | --- | --- | --- |
| F01 | P1 | 恢复模型价格自动更新 | 从有效上游引用解析不可变提交；新增模型进入 unreviewed；保留人工决定；更新后的三个目录文件一致；重复运行无差异 | 完成 | [#1364](https://github.com/majiayu000/litellm-rs/issues/1364)；51 项 Python 测试通过；当前源新增 14 条 unreviewed，4,555 条已有决定不变；重复同步/check 通过；Rust 默认全量检查通过；[PR #1366](https://github.com/majiayu000/litellm-rs/pull/1366)；已隔离 Voyage 缺价测试数据并通过 gateway/sqlite 定向测试；补充空证据源拒绝测试；四条上游峰谷价格已转换为运行时格式，峰/谷与预算上限回归通过；全部 CI 通过，review 已解决，已合并 `fcc8f1b1` |
| F02 | P1 | Cloudflare 错误语义 | HTTP 401/429/5xx 和 success=false 返回对应错误；畸形成功响应不能变成空成功；覆盖真实 HTTP 模拟测试 | 完成 | [#1365](https://github.com/majiayu000/litellm-rs/issues/1365) / [PR #1367](https://github.com/majiayu000/litellm-rs/pull/1367)；91 项相关测试、默认全量测试/check/clippy 通过；全部 CI 通过、无未解决 review；已合并 `01f15630`；一次 Gemini 测试超时，定向复测 9 项通过且同提交 CI 重跑通过，未将偶发超时误报为已修复 |
| F03 | P1 | Cloudflare 现代聊天协议与流式 | 使用官方兼容接口；文本、工具、多模态、用量、流式及中途错误按支持能力正确传递；文档和能力声明一致 | 完成 | [#1368](https://github.com/majiayu000/litellm-rs/issues/1368)；[PR #1369](https://github.com/majiayu000/litellm-rs/pull/1369)；聊天/流式本地 HTTP 测试、默认全量测试/check/clippy 通过；审查后的 88 项 provider 测试与 clippy 通过；全部 CI 通过，review 已解决；已合并 `4b4a4dd4` |
| F04 | P1 | Bedrock 逐模型能力与计费 | 审核 generic_converse 全部条目；修正上下文、输出、视觉、推理、profile 与价格；消除通用虚假默认值和重复价格源 | 完成 | [#1370](https://github.com/majiayu000/litellm-rs/issues/1370)；逐项核对 39 张 AWS 模型卡及官网定价；修正通用参数、移除 Sonic 聊天声明、统一计费来源；382 项 Bedrock 测试通过；[PR #1371](https://github.com/majiayu000/litellm-rs/pull/1371)；默认全量测试/check/clippy 及全部 CI 已通过，无未解决 review；已合并 `e77391fe` |
| F05 | P1 | 原生 Responses 请求通路 | 支持该协议的供应商真正调用原生 endpoint；保留工具、推理、原生事件、用量及错误；经过现有鉴权、路由、预算和记录链路 | 进行中 | [#1372](https://github.com/majiayu000/litellm-rs/issues/1372)；OpenAI 原生传输、预算、内容检查和回调通路已接入；10 项集成测试与 4 项单测通过，现有 Responses 8 项回归通过；默认全量 test/check/clippy 和 gateway/sqlite clippy 通过；[草稿 PR #1374](https://github.com/majiayu000/litellm-rs/pull/1374)；原生存储在后续草稿 #1379 接入；后台生命周期、其他供应商矩阵及工具/文件计费未完成，不可合并 |
| F06 | P1 | Responses 模型与供应商路由 | OpenAI、Copilot、Bedrock 原生协议分别按官方 endpoint/model 支持矩阵路由；Responses-only 模型和工具不被送往 chat/completions | 进行中 | [#1375](https://github.com/majiayu000/litellm-rs/issues/1375)；[草稿 PR #1377](https://github.com/majiayu000/litellm-rs/pull/1377)；111 个 OpenAI 模型/快照的官方端点已进入现有目录与路由，Responses-only/非聊天模型不再误走聊天；158 项 OpenAI 测试、12 项原生路由测试、51 项 Python 测试和默认全量 test/check/clippy 通过；Copilot/Bedrock 尚待接入 |
| F07 | P1 | Responses 跨副本持久状态 | 两个网关实例可读/删同一授权响应；重启后可恢复记录；租户隔离、TTL、后台状态及取消语义有测试 | 进行中 | [#1378](https://github.com/majiayu000/litellm-rs/issues/1378)；已将聊天适配路径的生命周期接入 SQL，10 项生命周期测试（包含双网关及重启）、3 项数据库竞争/清理测试、9 项路由回归（包括跨网关取消执行中请求）通过，[草稿 PR #1379](https://github.com/majiayu000/litellm-rs/pull/1379)；原生同步/流式存储、查询、输入列表、删除已接入租户/部署/账户绑定，19 项原生路由测试通过，含部署固定、上下文预算和策略变更拦截；72 项预算测试串行通过（并行时一项日志捕获断言失败，未声称已修复并发日志测试）；原生后台单次结算、跨网关取消及带游标的流式恢复已实现；19 项原生路由、9 项适配路由及默认全量 test/check/clippy、gateway/sqlite clippy 通过；store=false 只留十分钟临时元数据；仍缺进程重启后的后台费用恢复，九分钟内无最终用量按现有预留金额规则结算；全项保持进行中 |
| F08 | P1 | Anthropic 原生 Messages 网关 | 提供 /v1/messages；工具、thinking、流式、错误、用量和鉴权符合原生协议；文档/OpenAPI/路由一致 | 进行中 | [#1380](https://github.com/majiayu000/litellm-rs/issues/1380)；原生 JSON/SSE、鉴权与密钥令牌限制、原生 HTTP/流式错误、内容检查和累计用量已接入；[草稿 PR #1381](https://github.com/majiayu000/litellm-rs/pull/1381)；13 项路由测试、5 项 OpenAPI 契约测试、缓存 TTL/搜索费用测试，以及默认全量 test/check/clippy、gateway/sqlite clippy 通过；新增原生 count_tokens 预估图片/文档/工具输入，并按最贵请求 TTL 预留冷缓存写入费；15 项 Messages 路由测试通过；托管工具非令牌费用、工具循环新增输入及特殊费率仍未全部覆盖，暂不标为完成 |
| F09 | P2 | Responses compact 与 Gemini 文档覆盖 | 接通 /v1/responses/compact；Gemini 已有 generateContent/streamGenerateContent 路由，补 OpenAPI 覆盖并复核现有鉴权/流式/用量测试 | 待验收 | [#1382](https://github.com/majiayu000/litellm-rs/issues/1382)；compact 已复用原生传输、鉴权、费用与 previous_response_id 部署/账户绑定；4 项压缩定向测试通过；Gemini OpenAPI 的 8 个路径均已实际请求通过；23 项原生 Responses、29 项 Gemini 路由和 5 项 OpenAPI 契约测试通过；默认全量 test/check/clippy 及 gateway/sqlite clippy 通过；[草稿 PR #1383](https://github.com/majiayu000/litellm-rs/pull/1383)；依赖 #1379，等待 CI 与审查 |
| F10 | P1 | 模型退役与目录一致性 | 去除已退役模型的可调用声明；优先修 Cloudflare；复核全部现有静态供应商目录、示例、价格和能力来源 | 进行中 | [#1373](https://github.com/majiayu000/litellm-rs/issues/1373)；首批 OpenAI/Azure、Cloudflare、Copilot、Bedrock 已合并 [#1376](https://github.com/majiayu000/litellm-rs/pull/1376)（`42ba2b2a`）。Mistral 批次 [#1396](https://github.com/majiayu000/litellm-rs/pull/1396) 已合并（`ea7d9414`）：修正六个 Ministral 3 视觉条目和 3B 上下文，保留五个日期快照；74 项 Mistral 测试、默认完整检查和扩展特性 clippy 通过。[PR #1385](https://github.com/majiayu000/litellm-rs/pull/1385) 清理 Anthropic/Gemini 退役或未核实型号及别名，保留历史价格；修正 Gemini 缓存价格、区域/全球健康探测与受限网络策略。Anthropic 校验和能力直接使用现行目录，SDK 保留 thinking 块后的全部文本。默认完整 test/check/clippy（7260 项库测试）、234 项 Vertex、128 项 Gemini 及扩展 clippy 已通过；最新 124 项模型工具、244 项 SDK 测试和默认 all-target clippy 通过。当前 PR 有合并冲突、3 条未解决 review，最新提交未显示 CI 检查；其余静态目录仍需逐项复核；基线见 [逐条目录基线](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/audit/model-catalog-2026-10-01.entries.json)。 当前提交 `0e8d67db`。 |
| F11 | P2 | 兼容供应商的非聊天能力 | OpenAI-compatible 通路补齐 embeddings/images/audio 的实际调度；只声明经过协议验证的能力；未知/不支持能力返回明确错误 | 进行中 | 复核更正：通用 `openai_compatible` 已有 embeddings/images/audio 实现；30 项定向单测以及 embeddings、图片编辑/变体 HTTP 测试通过；`support_matrix.rs` 是旧适配器矩阵，不代表当前运行时。[#1386](https://github.com/majiayu000/litellm-rs/issues/1386)；Together/DeepInfra/Fireworks 已按官方矩阵扩展既有能力声明，11 项 HTTP/路由测试、12 项 catalog 和 85 项 provider 测试通过；默认完整 test/check/clippy 与 gateway/sqlite clippy 通过；[PR #1389](https://github.com/majiayu000/litellm-rs/pull/1389) 已合并（`c5bf4c19`），CI 全绿且 review 已解决；其余具名供应商尚待核验，范围见 [已合并范围文档](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/providers/compatible-nonchat.md)。  第二批 [#1398](https://github.com/majiayu000/litellm-rs/issues/1398) / [PR #1399](https://github.com/majiayu000/litellm-rs/pull/1399) 补 Groq 语音合成/转写/翻译、WAV 默认值和十秒最低计费；12 项协议路由与 3 项计费测试、默认完整 test/check/clippy 和 gateway/sqlite all-target clippy 通过；其余具名供应商继续核验。 Groq 审查补具体音频模型路由及翻译真实时长结算；13 项协议/路由测试、13 项音频网关测试（含时长缺失/无效回退和十秒下限）与 gateway/sqlite all-target clippy 通过，提交 `05585aec` 的 15 项 CI 全部通过、review 已解决；已合并 [#1399](https://github.com/majiayu000/litellm-rs/pull/1399)（`d6ab3e72`）。其余供应商待核验。 |
| F12 | P2 | 自定义供应商注册 | 外部实现可通过公开 API 注册并被路由，无须修改内部 Provider 枚举；覆盖构造、能力、错误和流式测试 | 完成 | [#1384](https://github.com/majiayu000/litellm-rs/issues/1384)；外部接口已接入现有 Provider/Deployment/Router，all-features 编译通过；4 项外部集成测试通过（注册/路由、模型能力、流式错误、未实现能力/缺价）；默认完整测试/check/clippy、gateway/sqlite/扩展 provider clippy 均通过（默认库 7,217 项通过、1 项忽略）；已提交 [PR #1387](https://github.com/majiayu000/litellm-rs/pull/1387)；审查补充健康检查回调路由回归；全部 CI 通过、review 已解决，已合并 `f24aa58f` |
| F13 | P2 | MCP 网关 | 把现有 MCP 能力接入 HTTP 网关；工具发现/调用与资源/提示词、鉴权、连接关闭有端到端测试；传输支持如实列出 | 待验收 | [#1388](https://github.com/majiayu000/litellm-rs/issues/1388) / [PR #1391](https://github.com/majiayu000/litellm-rs/pull/1391)；已按 MCP 2026-07-28 改为无状态 POST，删除旧初始化/会话与后台回收器。逐请求版本/方法/名称校验、Base64 名称、MRTR/订阅 SSE、原生字段与高精度数值、服务器/端点权限、Origin/凭据隔离、HTTPS/受限出站及断流测试通过。15 项最新路由测试、gateway/sqlite/mcp 完整测试（9710 项库测试通过、1 忽略，集成/doc 通过）及最新特性 all-target clippy 通过；此前默认完整检查通过。补齐每调用者 128/全局 4096 并发限制、有限响应体绝对超时与 URL 内嵌凭据拒绝，断流/超时释放配额均通过测试。最新提交 15 项 CI 全绿，但 7 条 review 线程未解决；URL userinfo 拒绝已存在，须按当前代码核实线程，其余阻断见下方清单；不包含旧协议、OAuth 获取、多服务器聚合或外部工具费用，详见 [当前 PR 范围文档](https://github.com/majiayu000/litellm-rs/blob/fe818688e6fccc23a1f78c35efe890ed19bc4ec8/docs/gateway/mcp.md)。 当前提交 `fe818688`。 |
| F14 | P2 | A2A 网关 | 接通 agent card、任务提交/查询/取消及事件流；代理鉴权、租户隔离和错误有端到端测试 | 待验收 | [#1392](https://github.com/majiayu000/litellm-rs/issues/1392) / [PR #1393](https://github.com/majiayu000/litellm-rs/pull/1393)；A2A 1.0 JSON-RPC 五方法、卡片、权限/归属、容量及 SSE 已接通。最新审查补 Task/Message 变体身份与必填字段、活动任务提前 EOF 拒绝、ROLE_USER 校验、带凭据上游强制 HTTPS；取消任务不再受旧库专有开关限制。28 项路由测试及 gateway/sqlite/a2a all-target clippy 通过，此前 132 项 core 和默认完整检查通过；最新提交 15 项 CI 全绿，6 条 review 线程未解决。进程内归属需实例亲和，不含 push/list/扩展卡片或代理费用计量，详见 [当前 PR 范围文档](https://github.com/majiayu000/litellm-rs/blob/36c4426145177cb8fc7327f9cee81d7359fee2af/docs/gateway/a2a.md)。 补齐 Task/Message 可选 contextId、后续事件 context 固定、跨帧 SSE BOM 及有限响应 JSON 类型；28 项路由测试与特性 clippy 通过。 当前提交 `36c44261`。 |
| F15 | P2 | Realtime 网关 | 接通 WebSocket 双向代理；供应商配置、鉴权、事件/关闭/错误传递有测试；首批支持范围明确 | 进行中 | [#1397](https://github.com/majiayu000/litellm-rs/issues/1397) 已立项。此前台账记录了 OpenAI WebSocket 传输的 3 项测试、3 项分项用量测试、51 项价格同步测试及特性 clippy；但尚无实现 PR 或不可变提交链接，本次不能独立核验这些记录。先找回实现及日志，再补公开路由、鉴权、预算、双向事件和完整验证；main 尚无公开 Realtime 网关路由。 |
| F16 | P2 | 过时声明与未落地子系统 | 逐项核对 subsystem_registry 和 README；完成上述能力后同步状态，清理已到移除版本的废弃接口，避免“声明支持却不可用” | 待开始 | 依赖对应功能完成；尚无本项专属 issue/PR。执行前搜索现有工作并补建 issue；逐项核对 README、subsystem_registry、feature 和发布特性，不作无关架构重写。 |
| F17 | P2 | 可复现的 LiteLLM 对比基准 | 同机器、同模拟上游和相同负载比较吞吐/延迟/错误率/内存；保存命令、版本和样本，不用 Rust 语言推断性能结论 | 完成 | [#1394](https://github.com/majiayu000/litellm-rs/issues/1394) / [PR #1395](https://github.com/majiayu000/litellm-rs/pull/1395) 已于 2026-10-03 合并（`cb76a186`）。同机、同上游、4 workers、并发 64 的三轮对照及直连基线共 9 个样本零请求错误；原始数据/环境/限制随报告提交。12 项运行器行为测试、14 项现有 benchmark 契约测试、默认完整 Rust 检查及全部 CI 通过，审查线程已解决。结果仅适用于报告中的本地模拟负载，不宣称通用倍数，见 [已合并报告](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison.md)及[原始证据包](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison-20261003.json.gz)。 |
| F18 | P1 | 发布与安装产物 | 所有已验收能力进入版本发行包；验证 crate、安装说明及容器内容；记录实际发布版本/提交，不能只凭 main 已合并声称已发布 | 待开始 | [GitHub 最新发行 v0.7.0](https://github.com/majiayu000/litellm-rs/releases/tag/v0.7.0) 对应 `3341e54a`，不含 #1360 及本轮工作；[crates.io](https://crates.io/crates/litellm-rs/versions) 最新也是 0.7.0。尚无本项专属 issue/PR；执行前搜索并补建。需核查二进制、crate、容器和安装步骤，当前产物内容尚未验收。 |

## 执行与验收记录

- 2026-10-03：建立清单；GitHub 当前无开放 issue/PR 与上述工作重复。先执行 F01，然后 F02–F04，再处理协议、状态和网关能力。独立功能拆分提交，避免将全部变化塞入一个 PR。
- 所有构建与测试在本任务独立 worktree 执行。每个 PR 准备好前执行仓库要求的格式、检查、全量测试及 clippy；合并前确认 CI 全绿且 review threads 已解决。
- 离线模拟测试与需要供应商账户的真实调用分开记录。没有凭据或没有运行过的真实调用不得标为通过。

- 2026-10-03 复核更正：Gemini 原生生成和流式接口在实际路由中已存在，F09 改为文档/契约覆盖，不重复实现。

- 2026-10-03 F07 进展：后台查询/取消和 SSE 重连共用创建请求的结算责任，不会重放生成请求；修复内容检查缓冲 response.created 导致断线时客户端拿不到 ID 的问题。存储与费用恢复分开验收：共享响应记录已具备重启可读性，后台结算任务尚未具备重启恢复能力。

## 后续执行顺序

先完成台账校正及已有 PR 的阻断处理，再完成 Responses 与 Messages 的计费和恢复缺口，随后收尾剩余目录、供应商及 Realtime。F16 随功能合并同步核对，F18 在选定发行提交上集中验收。该顺序不要求等待无依赖的工作，也不授权扩大首批协议范围。

Responses 的现有分支依赖为 `#1374 → #1377 → #1379 → #1383`。先在整条链上完成相关验收，再按依赖顺序合并和调整下游 base；不得通过改成 ready 或关闭 issue 隐藏未完成的模型、生命周期或计费要求。

## 当前提交与 CI 基线

这是 2026-10-03 11:44 的核查快照。实施前读取最新 head、CI 和 review，后续代码变更后更新证据；不要把下面的状态直接套用到新提交。

| PR | 当前 head | 最新提交的检查 | 剩余阻断 |
| --- | --- | --- | --- |
| #1374 | `c7a47ba5` | 15 项通过 | 草稿；F06/F07 及工具计费未完成 |
| #1377 | `4322e87f` | 仅 convergence 通过 | 草稿；Copilot/Bedrock 未完成；缺完整远端 CI |
| #1379 | `e3cf2a53` | 仅 convergence 通过 | 草稿；后台费用恢复未完成；缺完整远端 CI |
| #1381 | `7f590db1` | 15 项通过 | 草稿；托管工具与特殊费率未完成 |
| #1383 | `f65674f3` | 仅 convergence 通过 | 草稿；依赖 #1379；缺完整远端 CI |
| #1385 | `0e8d67db` | 未显示检查 | 合并冲突；3 条未解决 review |
| #1391 | `fe818688` | 15 项通过 | 7 条未解决 review，须区分已修复和仍有效意见 |
| #1393 | `36c44261` | 15 项通过 | 6 条未解决 review，须对照官方协议核实 |
| #1399 | `05585aec` | 15 项通过 | 无未解决 review；尚未合并 |
| #1390 | `65c48bd1` | 未显示检查 | 台账 PR 有冲突；10 条未解决 review，其中 5 条标为 outdated |

main `cb76a186` 的 [CI Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37090053898) 中 Test 被取消；[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37090053910) 已通过。取消不等于失败，也不等于全量测试成功。堆叠 PR 仅有 convergence，不足以声称完整 GitHub CI 通过。

## 台账与现有 PR 收口

- [x] 保留 F01–F18 的原始验收范围，修正 F12/F17 已合并状态，恢复 F10 Mistral 和 F11 首批合并证据。
- [x] 补齐 Groq PR 链接，将未合并的 MCP/A2A 文档改为不可变提交链接，修正基准报告路径。
- [x] 在 #1390 原分支整合 main `cb76a186`，解决台账冲突并保留基准证据；文档意见逐条核实后处理。
- [x] 清单已提交推送到 #1390（`4628f8d9`），校验表格列数、18 项原始验收条件及不可变链接目标；远端渲染另行核查。
- [x] #1399 最新 head `05585aec` 的 CI 全绿且线程清零，已于 2026-10-03 12:20 合并为 `d6ab3e72`；F11 总项继续进行。
- [ ] 记录后续验收所用的具体 main/PR 提交；被取消的检查在实际验收提交上补齐，不为过期提交重复运行无关测试。

## F10 模型目录剩余工作

关联 issue #1373；先修已有 #1385，再继续尚未核验的静态目录。

- [ ] 在 #1385 原分支整合 main 并解决冲突，保留 #1376 和 #1396 已合并修正。
- [ ] 修复 Gemini 公共模型校验仍按前缀接受退役 ID 的路径，补当前模型接受、退役模型拒绝回归。
- [ ] 在 Gemini 原生 generateContent/streamGenerateContent 路由中验证实际供应商目录和模型 surface，证明退役模型不会发往上游。
- [ ] 核对 Anthropic SDK 与直接 provider 对当前模型的参数约束，补采样、thinking 和 prefill 的一致性回归；以供应商官方资料为依据处理 review。
- [ ] 以现有逐条目录基线枚举剩余静态供应商；每项记录官方来源、审核日期、可调用结论和未确认原因。deprecated 与已退役分开，历史价格不自动恢复 callable。
- [ ] 同步受影响的示例、别名、能力、上下文和价格；未证实支持的能力不新增声明。每批实现沿用对应 issue，保存验证和 PR。
- [ ] 全部静态目录审核完毕、各入口一致且相关 PR 验收后，才将 F10 标为完成；保留账户/区域及真实调用未验证限制。

## F13 MCP 验收收尾

关联 #1388 / #1391。范围保持 MCP 2026-07-28 Streamable HTTP；不追加旧协议兼容、OAuth 获取、多服务器聚合或外部工具计费。

- [ ] 对照最新代码验证 URL userinfo 拒绝已生效，保留回归并处理对应审查线程。
- [ ] 处理 URL query 携带凭据时的 HTTPS 边界，以及畸形 URL 导出时未脱敏的问题；测试使用虚构凭据。
- [ ] 在 id-less 通知提前返回前处理已移除的初始化方法，验证旧初始化通知不会误转发。
- [ ] 请求 ID 只接受字符串或整数，覆盖小数和 null 拒绝；对照[官方基础协议](https://modelcontextprotocol.io/specification/2026-07-28/basic/index)。
- [ ] 处理读取请求体早于并发准入的问题，选择最小可行的准入或读取超时修复，覆盖慢请求体和释放配额。
- [ ] 打通公开 API-key 创建到 mcp.<server> 路由授权的最小权限链路，覆盖正确服务器允许、其他服务器拒绝。
- [ ] 跑当前提交的路由、特性测试及 clippy，确认发现/调用、资源/提示词、MRTR、SSE、鉴权、超时和断连仍成立；解决所有有效 review 后合并并记录范围。

## F14 A2A 验收收尾

关联 #1392 / #1393。首批保持现有五方法与 Agent Card；进程内归属、实例亲和、无代理费用计量等限制继续明确记录。

- [ ] 保存并验证 task 与 context 的关联，覆盖同一调用者拥有两个上下文但提交错误配对的拒绝场景。
- [ ] 按协议定义验证 TaskState，覆盖非法状态和终态 SSE 关闭；避免未知状态被当成执行中。
- [ ] 核对 Content-Type 错误、已知但未支持的方法、push 方法与未知方法的错误区别，保留原生上游错误语义。
- [ ] 核对 Agent Card 的 A2A-Version header/query 协商，补不支持版本及缺失版本的协议测试。
- [ ] 对照 [A2A 1.0 JSON-RPC 规范](https://a2a-protocol.org/v1.0.0/specification/#95-error-handling) 处理 ErrorInfo 审查：区分 SHOULD 和 MUST，不把其他绑定的要求直接套到 JSON-RPC；保留解释和最终处理结果。
- [ ] 复核公开密钥创建入口能否配置文档中的 a2a.<agent> 权限；如有同类集成缺口，在现有 PR 内修复并补最小权限回归。
- [ ] 完成当前提交路由/特性验证并解决有效 review；合并后在 F18 单独检查发行特性是否包含 a2a，不以 all-features 编译代替发行验收。

## F05 至 F07 原生 Responses

继续已有 #1374、#1377、#1379，分别对应 #1372、#1375、#1378。

- [ ] F05：保留原生 JSON/SSE、工具、推理、未知扩展字段及原生错误；复核鉴权、路由、内容检查、预算、回调和记录链路没有因后续生命周期改动被绕过。
- [ ] F05：完成已声明支持的托管工具/文件费用维度的预留与结算，缺少可计费依据的请求返回明确错误；测试包含中途错误和缺失终态用量，不能默认为免费。
- [ ] F06：完成 Copilot、Bedrock 的官方 endpoint/model 支持核验与原生分发；保留每项官方依据。Responses-only 模型和原生工具不得回退到 chat/completions。
- [ ] F06：覆盖 OpenAI/Copilot/Bedrock 的实际 HTTP 路由、模型权限、端点不支持、流式及错误；111 个 OpenAI 模型的证据不能替代其他供应商验收。
- [ ] F07：让后台结算责任在创建进程退出后仍可恢复，优先复用现有 SQL 和预算机制；响应句柄恢复与费用恢复分别验收，不引入额外通用任务平台。
- [ ] F07：以两个网关和共享数据库验证“创建 → 终态前关闭创建实例 → 另一实例/重启接管 → 最终只结算一次”，同时证明不会重放可能计费的生成请求。
- [ ] F07：补接管竞争、取消/删除竞争、过期、租约续期失败和无终态用量回归；记录保守结算的边界，防止重复扣费或预留释放后漏记。
- [ ] F07：复核 owner、deployment、account 绑定及 TTL；store=false 的临时元数据不能保存输入输出内容，独立 SQLite 回退不能宣称跨副本持久化。
- [ ] 整体验证 store 默认行为、background、previous_response_id、查询/删除/input_items、取消和游标恢复；GET、重连或取消不能生成第二次费用。
- [ ] 补齐堆叠 PR 当前提交的完整 CI，整条链验收后按依赖顺序合并并更新 base；各 issue 仅在自身全部验收范围满足后关闭。

## F08 Anthropic Messages

关联 #1380 / #1381；复用已有原生传输，不重复实现路由。

- [ ] 核对托管工具的非令牌费、工具循环新增输入和已支持的特殊费率，补预算预留及实际用量结算；未完成计费的能力不能静默按普通文本计费。
- [ ] 保留 count_tokens 对图片、文档、工具和 thinking 的预估，以及冷缓存 TTL 预留；覆盖超预算时生成尚未发起的行为。
- [ ] 覆盖 JSON/SSE 累计用量、缓存写入/命中、工具调用、thinking、流中错误和断连，验证不会重复结算或漏记已知费用。
- [ ] 复核原生状态码/Retry-After、鉴权、模型权限、内容检查、OpenAPI 与实际路由；完成现有草稿限制后再验收并合并。

## F09 compact 与 Gemini 契约

关联 #1382 / #1383；依赖 Responses 链，Gemini 原生路由已经存在。

- [ ] 在 F07 更新后的分支上复核 compact 的所有权/部署/账户绑定、预算、异常成功响应、用量和上游错误回归。
- [ ] 复核八个 Gemini OpenAPI 路径与实际挂载一致，保留鉴权、流式和用量测试；与 F10 的退役模型拒绝规则一致。
- [ ] 在依赖落地后完成当前提交的完整 CI 和 review，按顺序合并；不把文档缺项重新实现为另一套路由。

## F11 剩余兼容供应商

首批 #1386 / #1389 已完成；Groq #1398 / #1399 待合并。剩余供应商在开始下一批时先搜索 issue/PR，按实际缺口建立有边界的 issue。

- [ ] 将具名 registry 选择器逐项列入现有 compatible-nonchat 文档，记录已验证能力、官方协议路径及尚未核验范围，不能用聊天兼容性推断其他端点。
- [ ] 核对剩余 embeddings/images/audio 能力；仅同协议、同 base 的能力复用现有通路，原生专用路径单独判断是否属于本项必要实现。
- [ ] 每批覆盖 factory → Router → 实际 HTTP，以及模型级能力、JSON/二进制/multipart、状态/Retry-After、用量和预算；不支持能力须明确拒绝。
- [ ] 保留 Groq 具体音频模型路由、WAV 默认、真实时长结算及十秒下限回归；缺价不视为免费。
- [ ] 逐批登记剩余供应商的核验结论和限制；只有整个约定范围核验完毕才关闭 F11，不以单个批次 issue 关闭代替完成。

## F15 Realtime

关联 #1397；首批范围仅 OpenAI WebSocket。

- [ ] 找回已有传输、计费改动及其测试日志，提供不可变提交或草稿 PR；无法找到时将历史局部测试保留为未核验记录，不能据此跳过实现或验收。
- [ ] 复用现有供应商配置、受限出站连接、鉴权和模型权限挂载公开 WebSocket 路由。
- [ ] 完成双向文本/音频/工具事件、背压、握手失败、原生错误、关闭码与客户端断线清理测试。
- [ ] 按供应商报告用量处理文本、音频、图像及缓存费用；明确预留、会话预算耗尽、断线和缺失用量时的行为，验证后才启用可计费会话。
- [ ] 完成 gateway/sqlite/websockets 特性测试、文档及 OpenAPI 可表达的握手契约；明确未包含 Azure/Gemini/Bedrock、WebRTC、SIP。
- [ ] 完成默认全量检查、特性检查和 CI/review，关联 #1397 的实现 PR 后验收。

## F16 声明和废弃接口

在执行前搜索并建立本项 issue；随对应功能合并更新声明，最终集中复核。

- [ ] 逐项核对 subsystem_registry、README、Cargo feature、网关挂载和发行特性，区分库类型、默认启用、可选路由和已发布能力。
- [ ] 处理写明 0.7 移除但仍存在的废弃接口；根据已落地替代入口做最小清理，不扩展成架构重写或新增兼容层。
- [ ] MCP/A2A/Realtime 等文档中的范围、认证、存储、计费和部署限制与实际行为一致；测试仅针对真实变更，不为文字调整添加机械测试。
- [ ] 记录本项 PR 和验证后关闭 F16；新增声明不能领先于对应实现验收。

## F18 发布和安装验收

在执行前搜索并建立本项 issue；先准备版本变更及可审阅产物。是否已经发布必须以实际发行记录为准。

- [ ] 确定待发布版本和不可变候选提交，逐项列出纳入的已验收能力、合并提交及仍不支持的范围。
- [ ] 确认候选提交的默认完整测试、clippy、发行特性测试和跨平台构建成功；当前 main 的取消记录不能作为成功证据。
- [ ] 核对发布 workflow 与容器使用同一已验证特性集；特别检查 a2a 是否纳入，分别冒烟验证 MCP/A2A/Realtime 及已验收原生路由。
- [ ] 核对 cargo package 的文件、版本和 feature，在干净环境按 crate 安装说明构建并运行最小示例，确认新公开 API 可用。
- [ ] 验证各声明支持平台的压缩包内容、可执行文件、版本输出及启动；检查容器架构、入口、标签、digest 和实际路由。
- [ ] 从干净环境执行安装说明中的准确命令；README、release notes、crate 和镜像标签指向同一版本与功能范围。
- [ ] 完成发布前审阅要求后执行既定发布流程；记录真实版本、tag、commit、crate 页面、二进制校验和及镜像 digest，验证可下载/安装的产物。
- [ ] 更新 F18 和每项发布记录；未进入产物的能力保持“已合并、未发布”，不标成已发布。

## 每个实现 PR 的共同验收

以下是每个实现 PR 的执行要求，不新增自动验证框架。文档单独修改只做相关文档检查。

- [ ] 在本任务独立 worktree 上实现最小修复，保持一个实现 PR 对应一个 issue，现有 PR 沿用原分支。
- [ ] 迭代时运行相关行为测试；准备好前完成 `cargo fmt --check`、`cargo check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，并覆盖变更所需 gateway/存储/协议特性。
- [ ] 记录命令、提交、结果和限制；默认测试通过、特性测试通过、仅编译通过和真实供应商调用分开记载，取消/超时不计通过。
- [ ] 逐条核实审查意见的当前代码和官方依据，修复有效问题，对已修复或不适用意见说明证据并解决线程；最新 CI 全绿且 review threads 已解决后合并。
- [ ] 每项完成时更新本文件的状态、PR、合并提交、验证链接和剩余限制；发布版本在 F18 验证后补记。

已完成的 F01–F04、F12、F17 不重复实施，仅参加受影响回归和最终发行验收。F17 当前样本只支持已记录的本地非流式聊天结论；生产负载或其他协议基准不作为本轮新增任务。

## 本次清单整理记录

- 2026-10-03 11:44：依据远端 main、19 个相关 PR 的代码/元数据与审查、发行记录及基准证据整理后续任务；补查 #1399 全部 CI 已通过。仅修改本文件，未重新运行 Rust 测试，未提交、推送、合并或发布。

- 2026-10-03 12:20：用户授权三路并行执行；#1390 清单已推送并整合 main，#1399 已核验后合并。MCP #1391、目录 #1385 在各自 worktree 修复；Responses 后台恢复正在检查预算持久性边界。
