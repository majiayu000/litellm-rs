# LiteLLM 差距补齐清单

更新日期：2026-10-04（北京时间）。后续 [#1441](https://github.com/majiayu000/litellm-rs/pull/1441) 与 [#1442](https://github.com/majiayu000/litellm-rs/pull/1442) 已验收合并；当前接受 main 为 `46476786a09b6030ab58aee4c4e8acb173c9b7b2`。当前 3 个开放 issue（#1440、#1402、#1403）、2 个开放 PR（#1443、#1444）；后者仍是发行准备。下方历史快照保留当时状态，当前回执以本节和后续验收记录为准。

本文件是 F01–F18 的执行台账。已合并与已发布分别记录；当前正式版本仍为 v0.7.0。下方保留原始验收条件、实际源码/检查/合并记录和剩余事项。

本清单记录此次审计发现的全部缺口。参考 BerriAI/litellm 的实现和官方协议，模型事实以供应商官方资料为准；不把价格表中的模型自动当作可调用模型。

状态：待开始 → 进行中 → 待验收 → 完成。只有代码、测试和关联 PR 均有可查证结果时才标为完成；合并和发布单独记录。每完成一项，在本文件更新状态、证据及剩余限制。每个实现 PR 对应一个 issue。修复已有 PR 时沿用原分支；不建立竞争 PR。表中既有测试数量来自关联 PR 的验证记录，后续代码批次已按各条目记录执行 Rust 验证；最新提交的 CI 状态单独列在下方。

| ID | 优先级 | 功能 / 问题 | 验收条件 | 状态 | Issue / PR / 验证 |
| --- | --- | --- | --- | --- | --- |
| F01 | P1 | 恢复模型价格自动更新 | 从有效上游引用解析不可变提交；新增模型进入 unreviewed；保留人工决定；更新后的三个目录文件一致；重复运行无差异 | 完成 | [#1364](https://github.com/majiayu000/litellm-rs/issues/1364)；51 项 Python 测试通过；当前源新增 14 条 unreviewed，4,555 条已有决定不变；重复同步/check 通过；Rust 默认全量检查通过；[PR #1366](https://github.com/majiayu000/litellm-rs/pull/1366)；已隔离 Voyage 缺价测试数据并通过 gateway/sqlite 定向测试；补充空证据源拒绝测试；四条上游峰谷价格已转换为运行时格式，峰/谷与预算上限回归通过；全部 CI 通过，review 已解决，已合并 `fcc8f1b1` |
| F02 | P1 | Cloudflare 错误语义 | HTTP 401/429/5xx 和 success=false 返回对应错误；畸形成功响应不能变成空成功；覆盖真实 HTTP 模拟测试 | 完成 | [#1365](https://github.com/majiayu000/litellm-rs/issues/1365) / [PR #1367](https://github.com/majiayu000/litellm-rs/pull/1367)；91 项相关测试、默认全量测试/check/clippy 通过；全部 CI 通过、无未解决 review；已合并 `01f15630`；一次 Gemini 测试超时，定向复测 9 项通过且同提交 CI 重跑通过，未将偶发超时误报为已修复 |
| F03 | P1 | Cloudflare 现代聊天协议与流式 | 使用官方兼容接口；文本、工具、多模态、用量、流式及中途错误按支持能力正确传递；文档和能力声明一致 | 完成 | [#1368](https://github.com/majiayu000/litellm-rs/issues/1368)；[PR #1369](https://github.com/majiayu000/litellm-rs/pull/1369)；聊天/流式本地 HTTP 测试、默认全量测试/check/clippy 通过；审查后的 88 项 provider 测试与 clippy 通过；全部 CI 通过，review 已解决；已合并 `4b4a4dd4` |
| F04 | P1 | Bedrock 逐模型能力与计费 | 审核 generic_converse 全部条目；修正上下文、输出、视觉、推理、profile 与价格；消除通用虚假默认值和重复价格源 | 完成 | [#1370](https://github.com/majiayu000/litellm-rs/issues/1370)；逐项核对 39 张 AWS 模型卡及官网定价；修正通用参数、移除 Sonic 聊天声明、统一计费来源；382 项 Bedrock 测试通过；[PR #1371](https://github.com/majiayu000/litellm-rs/pull/1371)；默认全量测试/check/clippy 及全部 CI 已通过，无未解决 review；已合并 `e77391fe` |
| F05 | P1 | 原生 Responses 请求通路 | 支持该协议的供应商真正调用原生 endpoint；保留工具、推理、原生事件、用量及错误；经过现有鉴权、路由、预算和记录链路 | 完成 | [#1372](https://github.com/majiayu000/litellm-rs/issues/1372) / [#1441](https://github.com/majiayu000/litellm-rs/pull/1441)；最终 head `e9dea54c557810bd3006b28c8f4a803d71a0fe68` 的 15 项 CI 成功、审查线程解决后合并 `fe036814c932654c9cc03e8ed8e7a36fe1572b25`，issue 已关闭。原生 JSON/SSE、function/custom、推理和既有生命周期之外，新增同步有界 OpenAI file_search；按工具/模型上下文预留，终态 distinct completed call 按每次 $0.0025 结算，未知账单保留预留。42 项原生 HTTP、7 项 callback 及默认全套/相关 clippy 通过。后台工具、其他托管工具及非标准档位仍排除；尚未发布。 |
| F06 | P1 | Responses 模型与供应商路由 | OpenAI、Copilot、Bedrock 原生协议分别按官方 endpoint/model 支持矩阵路由；Responses-only 模型和工具不被送往 chat/completions | 完成 | [#1375](https://github.com/majiayu000/litellm-rs/issues/1375) / [PR #1377](https://github.com/majiayu000/litellm-rs/pull/1377)，验收 head `e1623efe`。111 个 OpenAI 端点证据、Copilot 动态 supported_endpoints、Bedrock Runtime/Mantle 地址、签名和计费身份已接通；Responses-only 不回退聊天。修正自动合并重引入的 Chat 能力回退，按精确 provider/pricing_key 保留独立目录审核决定，重建摘要。52 项 Python、同步/check、18 项原生 HTTP 及联合 clippy 通过。该 head 的 15 项 CI 全绿、线程清零后以 merge commit `ac0ad6a9` 合并，保留堆叠祖先；#1375 已关闭，#1379 base 已改为 main。Copilot/Bedrock 生命周期仍不在首批范围。 |
| F07 | P1 | Responses 跨副本持久状态 | 两个网关实例可读/删同一授权响应；重启后可恢复记录；租户隔离、TTL、后台状态及取消语义有测试 | 完成 | [#1378](https://github.com/majiayu000/litellm-rs/issues/1378) / [#1379](https://github.com/majiayu000/litellm-rs/pull/1379)；验收 head `c44b48d4`，全部 15 项 CI 成功、审查线程解决后合并 `93d51613`。 POST 前 SQL 持久化结算责任；恢复只 GET，不重新生成。SQLite runtime/server/worker 停止后两个新 startup worker 恢复并只结算一次；不是实际 Postgres 重启、OS kill 或通用 exactly-once 证明。后台 provider/model 限额要求共享 SQL/Redis；进程内 key budget 拒绝，未知用量保留 reserved_unknown。当前 head 的 Main Full 37126573698 另核对成功。 |
| F08 | P1 | Anthropic 原生 Messages 网关 | 提供 /v1/messages；工具、thinking、流式、错误、用量和鉴权符合原生协议；文档/OpenAPI/路由一致 | 完成 | [#1380](https://github.com/majiayu000/litellm-rs/issues/1380) / [PR #1381](https://github.com/majiayu000/litellm-rs/pull/1381)，当前 `bb6ecc10` 已退出草稿：JSON/SSE、count_tokens、缓存 TTL、有界直接 web 工具及显式/继承 US 地区计费已接通；未知用量保留承诺但账单记为 unpriced。22 项集成、默认完整检查、gateway/sqlite/mcp 完整测试（9780 通过/1 忽略）、clippy 和 all-features check 通过。前轮五项审查修复已推送，默认 7270 项库测试/1 忽略及联合完整检查通过；all-features check 属此前提交。新轮 Haiku 缓存价格、终态用量/JSON stop_reason、start envelope 与 guardrail 健康归因五项修复已推送，25 项 HTTP、52 项 Python/重复同步、默认完整及 gateway/sqlite/mcp 完整检查通过，review 清零；随后原分支整合 f62c5adb，当前 `bb6ecc10` 的 15 项 CI 全绿且 review 清零，已合并 `7336a20e` 并自动关闭 #1380；本轮 25 项原生 Messages HTTP 测试通过；另 Sonnet 4.5 1M beta 审查经官方 2026-04-30 退役说明确认不适用，保留 200k。 |
| F09 | P2 | Responses compact 与 Gemini 文档覆盖 | 接通 /v1/responses/compact；Gemini 已有 generateContent/streamGenerateContent 路由，补 OpenAPI 覆盖并复核现有鉴权/流式/用量测试 | 完成 | [#1382](https://github.com/majiayu000/litellm-rs/issues/1382) / [#1383](https://github.com/majiayu000/litellm-rs/pull/1383)；验收 head `d61979f9`，全部 15 项 CI 成功、审查线程解决后合并 `35bd932a`。 已以 main 为 base，不再依赖开放 F07。compact 只选 OpenAI；按目录模型输出上限和 OpenAI 隐式缓存写入费预留，缺少上限拒绝。读/写缓存用量严格校验并按实际费用结算；OpenAPI 限定 default tier 并声明 cache_write_tokens。当前精确提交的默认全套、两套 clippy 和 HTTP 结果见验证清单；新增回归确认普通 Anthropic 请求不预留未请求的缓存写入，观测缓存写入仍按实际价结算。Gemini 八个既有路径只补契约。 |
| F10 | P1 | 模型退役与目录一致性 | 去除已退役模型的可调用声明；优先修 Cloudflare；复核全部现有静态供应商目录、示例、价格和能力来源 | 完成 | [#1373](https://github.com/majiayu000/litellm-rs/issues/1373) / [#1442](https://github.com/majiayu000/litellm-rs/pull/1442)；最终 head `0853f9d959a8cc7bb6b9fd6f5306db8262b656e8` 的 15 项 CI 成功、全部 9 个线程解决后合并 `46476786a09b6030ab58aee4c4e8acb173c9b7b2`，issue 已关闭。后续逐条核对 Azure AI 41 个 mapped identity、GitHub Models 16 条、Lambda、Nova/Meta/v0、音频及非 Gemini Vertex；确认退役与未验证协议分别记录，保留历史价格。Azure 原生工具同步/SSE 实际解析与 Phi 能力限制一致；默认 Azure 自定义 OpenAI-like 部署可用。默认全套、相关 feature 检查/clippy、Azure 123 项和真实 HTTP 回归通过，Python 54 项及固定源核验通过。见 [逐条审计](../audit/remaining-static-model-catalog-2026-10-04.md)；尚未发布，不宣称付费账号实测。 |
| F11 | P2 | 兼容供应商的非聊天能力 | OpenAI-compatible 通路补齐 embeddings/images/audio 的实际调度；只声明经过协议验证的能力；未知/不支持能力返回明确错误 | 进行中 | 原首批 #1389、Groq #1399 已合并；本轮自托管 #1412、云 #1418、聚合 #1424、百川 #1426、Ark #1428、W&B #1432、CompactifAI #1434、多模态聚合 #1430 已验收。中文：验收 head `fa1c8eab`，全部 15 项 CI 成功、审查线程解决后合并 `9ca1164c`。 xAI：验收 head `5ca350aa`，全部 15 项 CI 成功、审查线程解决后合并 `7b9dbb67`。 延迟审查的 language/format 修复：验收 head `0f5ff352`，全部 15 项 CI 成功、审查线程解决后合并 `3c0d1610`。 具名 selector 按实际协议/模型/价格开放；异步媒体、克隆、URL 抓取和未经证实收费仍不宣称支持。后续未核验能力按现有 compatible-nonchat 文档继续拆 bounded issue。 |
| F12 | P2 | 自定义供应商注册 | 外部实现可通过公开 API 注册并被路由，无须修改内部 Provider 枚举；覆盖构造、能力、错误和流式测试 | 完成 | [#1384](https://github.com/majiayu000/litellm-rs/issues/1384)；外部接口已接入现有 Provider/Deployment/Router，all-features 编译通过；4 项外部集成测试通过（注册/路由、模型能力、流式错误、未实现能力/缺价）；默认完整测试/check/clippy、gateway/sqlite/扩展 provider clippy 均通过（默认库 7,217 项通过、1 项忽略）；已提交 [PR #1387](https://github.com/majiayu000/litellm-rs/pull/1387)；审查补充健康检查回调路由回归；全部 CI 通过、review 已解决，已合并 `f24aa58f` |
| F13 | P2 | MCP 网关 | 把现有 MCP 能力接入 HTTP 网关；工具发现/调用与资源/提示词、鉴权、连接关闭有端到端测试；传输支持如实列出 | 完成 | [#1388](https://github.com/majiayu000/litellm-rs/issues/1388) / [PR #1391](https://github.com/majiayu000/litellm-rs/pull/1391)，`a1f718bb`：MCP 2026-07-28 无状态 POST、工具/资源/提示词、MRTR/SSE、命名权限、Origin/出站限制、并发/超时和断流已接通。新增前缀同名路由、配置导出脱敏、空/旧通用权限拒绝及 Tasks taskId 路由头回归；最新 22 项路由测试、fmt 和特性 all-target clippy 通过。前一提交 bf8434ab 默认完整检查（7219 通过/1 忽略）及 gateway/sqlite/mcp 完整测试/clippy 通过；提交 a1f718bb 的 15 项 CI 全绿，review 清零；已合并 ceff7f39。无旧协议、OAuth 获取、聚合或外部工具计费；无网关会话亲和要求。范围见 [文档](https://github.com/majiayu000/litellm-rs/blob/a1f718bb771f5974d212450c813ac4c87bd85b9d/docs/gateway/mcp.md)。 |
| F14 | P2 | A2A 网关 | 接通 agent card、任务提交/查询/取消及事件流；代理鉴权、租户隔离和错误有端到端测试 | 完成 | [#1392](https://github.com/majiayu000/litellm-rs/issues/1392) / [PR #1393](https://github.com/majiayu000/litellm-rs/pull/1393)，`9a3d19be`：补 server Message context、任意 JSON data、畸形 URL 导出脱敏、终态订阅拒绝及 SSE 禁止缓冲。41 项路由、默认完整检查、gateway/sqlite/a2a/mcp 完整测试（9990 通过/1 忽略，另有集成/doc）及 clippy 通过，五条对应 review 已解决；此前 c29db734 的 15 项 CI 全绿；最新六条协议 review 及其验证见本行后续记录。进程内归属需实例亲和、重启拒绝旧归属，无 push/list/扩展卡片/代理计费。见 [不可变范围文档](https://github.com/majiayu000/litellm-rs/blob/c29db734e312f040c4ef62d3225bb4000f2a4f91/docs/gateway/a2a.md)。 最新 9a3d19be 六条新 review 已处理、43 项路由、默认完整 7267/1 忽略和 gateway/sqlite/a2a/mcp 完整 9992/1 忽略及 clippy 通过，已推送；9a3d19be 的 15 项 CI 全绿、review 清零，已合并 `7919b463` 并自动关闭 #1392；F18 单独验证发行产物。 |
| F15 | P2 | Realtime 网关 | 接通 WebSocket 双向代理；供应商配置、鉴权、事件/关闭/错误传递有测试；首批支持范围明确 | 完成 | [#1397](https://github.com/majiayu000/litellm-rs/issues/1397) / [#1405](https://github.com/majiayu000/litellm-rs/pull/1405)；验收 head `5d691d1f`，全部 15 项 CI 成功、审查线程解决后合并 `20640a48`。 [不可变范围文档](https://github.com/majiayu000/litellm-rs/blob/5d691d1f61f2beef12dc6d7971818ddfadb32f6d/docs/gateway/realtime.md)。首批只支持 OpenAI 手动文本/音频/客户端函数 WebSocket。明确排除自动 VAD、转写会话、图片、托管 MCP、切换模型、开启内容检查的会话和崩溃结算。逐响应检查 live 身份/部署/健康/准入；完整 session ack 核对手动模式、输出上限和工具范围。终态与所有转发操作的供应商错误保留健康分类，未知状态不计成功；不可表达的 key cap 在上游连接前返回400且不扣供应商健康。当前默认全套和定向/两套 clippy 结果按精确提交归档；4cc4e4fc 额外全套的既有 moderation 挂起不计通过。 |
| F16 | P2 | 过时声明与未落地子系统 | 逐项核对 subsystem_registry 和 README；完成上述能力后同步状态，清理已到移除版本的废弃接口，避免“声明支持却不可用” | 待验收 | [#1402](https://github.com/majiayu000/litellm-rs/issues/1402) / [#1443](https://github.com/majiayu000/litellm-rs/pull/1443)；移除未用 core Realtime 库及 MCP/A2A 旧客户端/导出，保留真实配置/错误/domain 类型。37 条 registry 对照真实 startup/routes/features/README，旧协议指南改为实际 HTTP 入口。默认全套、相关特性测试和两套 clippy 已本地通过；当前 head 的发行特性 CI 成功，CI Fast moderation 鉴权测试超时正在同提交重跑；无未解决审查线程。当前未合并、未发布。见 [核对记录](../audit/subsystem-reconciliation-2026-10-04.md)。 |
| F17 | P2 | 可复现的 LiteLLM 对比基准 | 同机器、同模拟上游和相同负载比较吞吐/延迟/错误率/内存；保存命令、版本和样本，不用 Rust 语言推断性能结论 | 完成 | [#1394](https://github.com/majiayu000/litellm-rs/issues/1394) / [PR #1395](https://github.com/majiayu000/litellm-rs/pull/1395) 已于 2026-10-03 合并（`cb76a186`）。同机、同上游、4 workers、并发 64 的三轮对照及直连基线共 9 个样本零请求错误；原始数据/环境/限制随报告提交。12 项运行器行为测试、14 项现有 benchmark 契约测试、默认完整 Rust 检查及全部 CI 通过，审查线程已解决。结果仅适用于报告中的本地模拟负载，不宣称通用倍数，见 [已合并报告](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison.md)及[原始证据包](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison-20261003.json.gz)。 |
| F18 | P1 | 发布与安装产物 | 所有已验收能力进入版本发行包；验证 crate、安装说明及容器内容；记录实际发布版本/提交，不能只凭 main 已合并声称已发布 | 进行中 | [#1403](https://github.com/majiayu000/litellm-rs/issues/1403) / [#1444](https://github.com/majiayu000/litellm-rs/pull/1444)；0.8.0 版本/Cargo.lock/changelog/install 已准备。默认全套及 clippy、精确 shipped-profile 全套及 clippy、clean package 与空根 debug 安装通过；安装包 8 项协议、实际 ARM64 release 容器 11 项协议及 appuser/config/health 验证通过。预验收 SHA、checksum、镜像 ID 和可复现脚本见 [发行验收](../gateway/release-0.8.0-verification.md)。最终 accepted commit、跨平台产物与真实 GitHub/crates/GHCR/Homebrew 发布尚未完成，正式版本仍 v0.7.0。 |

## 执行与验收记录

- 2026-10-03：建立清单；GitHub 当前无开放 issue/PR 与上述工作重复。先执行 F01，然后 F02–F04，再处理协议、状态和网关能力。独立功能拆分提交，避免将全部变化塞入一个 PR。
- 所有构建与测试在本任务独立 worktree 执行。每个 PR 准备好前执行仓库要求的格式、检查、全量测试及 clippy；合并前确认 CI 全绿且 review threads 已解决。
- 离线模拟测试与需要供应商账户的真实调用分开记录。没有凭据或没有运行过的真实调用不得标为通过。

- 2026-10-03 复核更正：Gemini 原生生成和流式接口在实际路由中已存在，F09 改为文档/契约覆盖，不重复实现。

- 2026-10-03 F07 早期记录（已被 aa1dedb6 后续实现取代）：当时共享响应记录可重启读取，但后台结算责任尚不可恢复。后续 c44b48d4 的持久恢复、当前 CI 与审查已验收并合并 93d51613；恢复只 GET、不重发 POST。此段保留早期记录，不代表当前阻断。

## 后续执行顺序

先完成台账校正及已有 PR 的阻断处理，再完成 Responses 与 Messages 的计费和恢复缺口，随后收尾剩余目录、供应商及 Realtime。F16 随功能合并同步核对，F18 在选定发行提交上集中验收。该顺序不要求等待无依赖的工作，也不授权扩大首批协议范围。

Responses 旧堆叠 `#1374 → #1377 → #1379 → #1383` 只记录历史依赖；前三项已经合并，F09 已以 main 为 base。当前执行状态见下表实际回执；#1372 已通过 #1441 的单项 file_search 验收；其他托管工具仍在已记录的排除范围。

## 历史提交与 CI 基线

下表保留历史远端 CI 快照截止 2026-10-03 17:12（北京时间）；后续本地修改及已推送变动以上表为准。合并前再次核对当前 head、CI 和 review。#1390 为本次文档修订前的提交。

| PR | 远端 head | 此提交检查 | 剩余阻断 / 合并结果 |
| --- | --- | --- | --- |
| #1374 | `b67fe01a` | 15/15 项成功 | 快照时 0 条未解决 review；其余检查未计成功 |
| #1377 | `89c53df7` | 0/1 项成功 | 快照时 0 条未解决 review；其余检查未计成功；堆叠 PR 自动仅 convergence，另运行整链完整 CI |
| #1379 | `52820df8` | 0/1 项成功 | 快照时 0 条未解决 review；其余检查未计成功；堆叠 PR 自动仅 convergence，另运行整链完整 CI |
| #1381 | `febe160c` | 2/15 项成功 | 快照时 6 条未解决 review；其余检查未计成功 |
| #1383 | `2918fa38` | 0/1 项成功 | 快照时 0 条未解决 review；其余检查未计成功；堆叠 PR 自动仅 convergence，另运行整链完整 CI |
| #1390 | `48537ef5` | 2/3 项成功 | 快照时 4 条未解决 review；其余检查未计成功 |
| #1393 | `c29db734` | 15/15 项成功 | 快照时 6 条未解决 review；其余检查未计成功 |
| #1405 | `607e8c44` | 15/15 项成功 | 快照时 8 条未解决 review；其余检查未计成功 |
| #1406 | `3dfdcb12` | 13/15 项成功 | 快照时 0 条未解决 review；其余检查未计成功；随后原分支整合 main，最新本地检查进行中 |
| #1408 | `3129db8b` | 15/15 项成功 | 快照时 2 条未解决 review；其余检查未计成功 |
| #1410 | `ea8d5723` | 14/15 项成功 | 快照时 0 条未解决 review；其余检查未计成功；随后原分支整合 main，最新本地检查进行中 |
| #1411 | `3ea5f99f` | 0/3 项成功 | 快照时 3 条未解决 review；其余检查未计成功；随后三条已回复并解决 |
| #1412 | `e8634282` | 5/15 项成功 | 快照时 2 条未解决 review；其余检查未计成功 |
| #1413 | `9557578d` | 2/14 项成功 | 快照时 2 条未解决 review；其余检查未计成功 |
| #1407 | `689954f5` | 15/15 项成功 | review 清零后已合并 decce90f |

该历史快照时 main 已整合到 `f62c5adb`；最新 main 见文件开头。合并、功能验收和发行候选检查分别记录；堆叠 PR 仅 convergence 不等同完整 CI，被取消和排队的检查不计成功。

## 台账与现有 PR 收口

- [x] 保留 F01–F18 的原始验收范围，修正 F12/F17 已合并状态，恢复 F10 Mistral 和 F11 首批合并证据。
- [x] 补齐 Groq PR 链接，将未合并的 MCP/A2A 文档改为不可变提交链接，修正基准报告路径。
- [x] 在 #1390 原分支整合 main `cb76a186`，解决台账冲突并保留基准证据；文档意见逐条核实后处理。
- [x] 清单已提交推送到 #1390（`4628f8d9`），校验表格列数、18 项原始验收条件及不可变链接目标；远端渲染另行核查。
- [x] #1399 最新 head `05585aec` 的 CI 全绿且线程清零，已于 2026-10-03 12:20 合并为 `d6ab3e72`；F11 总项继续进行。
- [ ] 记录后续验收所用的具体 main/PR 提交；被取消的检查在实际验收提交上补齐，不为过期提交重复运行无关测试。

## F10 模型目录剩余工作

关联 issue #1373；先修已有 #1385，再继续尚未核验的静态目录。

- [x] 在 #1385 原分支整合 main 并解决冲突，保留 #1376 和 #1396 已合并修正；已合并 `680e70cd`。
- [x] 修复 Gemini 公共模型校验仍按前缀接受退役 ID 的路径，补当前模型接受、退役模型拒绝回归。
- [x] 在 Gemini 原生 generateContent/streamGenerateContent 路由中验证实际供应商目录和模型 surface，证明退役模型不会发往上游。
- [x] 核对 Anthropic SDK 与直接 provider 对当前模型的参数约束，补采样、thinking 和 prefill 的一致性回归；以供应商官方资料为依据处理 review。
- [ ] 以现有逐条目录基线枚举剩余静态供应商；每项记录官方来源、审核日期、可调用结论和未确认原因。deprecated 与已退役分开，历史价格不自动恢复 callable。
- [ ] 同步受影响的示例、别名、能力、上下文和价格；未证实支持的能力不新增声明。每批实现沿用对应 issue，保存验证和 PR。
- [ ] 全部静态目录审核完毕、各入口一致且相关 PR 验收后，才将 F10 标为完成；保留账户/区域及真实调用未验证限制。

## F13 MCP 验收收尾

关联 #1388 / #1391。范围保持 MCP 2026-07-28 Streamable HTTP；不追加旧协议兼容、OAuth 获取、多服务器聚合或外部工具计费。

- [x] 对照最新代码验证 URL userinfo 拒绝已生效，保留回归并处理对应审查线程。
- [x] 处理 URL query 携带凭据时的 HTTPS 边界，以及畸形 URL 导出时未脱敏的问题；测试使用虚构凭据。
- [x] 在 id-less 通知提前返回前处理已移除的初始化方法，验证旧初始化通知不会误转发。
- [x] 请求 ID 只接受字符串或整数，覆盖小数和 null 拒绝；对照[官方基础协议](https://modelcontextprotocol.io/specification/2026-07-28/basic/index)。
- [x] 处理读取请求体早于并发准入的问题，选择最小可行的准入或读取超时修复，覆盖慢请求体和释放配额。
- [x] 打通公开 API-key 创建到 mcp.<server> 路由授权的最小权限链路，覆盖正确服务器允许、其他服务器拒绝。
- [x] #1391 a1f718bb 的路由、特性检查及 15 项 CI 已通过，review 清零后合并 ceff7f39；F18 单独验收发行包。

## F14 A2A 验收收尾

关联 #1392 / #1393。首批保持现有五方法与 Agent Card；进程内归属、实例亲和、无代理费用计量等限制继续明确记录。

- [x] 保存并验证 task 与 context 的关联，覆盖同一调用者拥有两个上下文但提交错误配对的拒绝场景。
- [x] 按协议定义验证 TaskState，覆盖非法状态和终态 SSE 关闭；避免未知状态被当成执行中。
- [x] 核对 Content-Type 错误、已知但未支持的方法、push 方法与未知方法的错误区别，保留原生上游错误语义。
- [x] 核对 Agent Card 的 A2A-Version header/query 协商，补不支持版本及缺失版本的协议测试。
- [x] 对照 [A2A 1.0 JSON-RPC 规范](https://a2a-protocol.org/v1.0.0/specification/#95-error-handling) 处理 ErrorInfo 审查：区分 SHOULD 和 MUST，不把其他绑定的要求直接套到 JSON-RPC；保留解释和最终处理结果。
- [x] 复核公开密钥创建入口能否配置文档中的 a2a.<agent> 权限；如有同类集成缺口，在现有 PR 内修复并补最小权限回归。
- [x] c29db734 的 41 项路由、默认与联合特性完整检查、clippy 通过，对应五条 review 已解决，15 项 CI 全绿。
- [x] 收尾新增六条协议 review（中断态关闭、Task artifacts、混合 SSE 行尾、取消终态、非法 RPC id、historyLength），9a3d19be 的完整检查、15 项 CI 和 review 均通过，已合并 7919b463；F18 单独验收 a2a 发行特性。

## F05 至 F07 原生 Responses

- [x] 原生请求 #1374 和逐供应商路由 #1377 已合并。
- [x] 持久生命周期/恢复结算 #1379 的 c44b48d4 标准 CI 与 Main Full 都通过，审查解决后 merge 合并 93d51613；#1378 关闭。
- [x] F09 已改为 main；旧 #1374 → #1377 → #1379 仅为历史依赖，不能作为当前待合并链。
- [x] F09 当前独立验收并合并，#1382 已关闭。
- [x] #1372 单项有界 file_search 的预算预留、JSON/SSE 及实际结算已通过 #1441 验收；其他托管工具仍明确拒绝。

## F08 Anthropic Messages

关联 #1380 / #1381；复用已有原生传输，不重复实现路由。

- [x] 核对托管工具的非令牌费、工具循环新增输入和已支持的特殊费率，补预算预留及实际用量结算；未完成计费的能力不能静默按普通文本计费。
- [x] 保留 count_tokens 对图片、文档、工具和 thinking 的预估，以及冷缓存 TTL 预留；覆盖超预算时生成尚未发起的行为。
- [x] 覆盖 JSON/SSE 累计用量、缓存写入/命中、工具调用、thinking、流中错误和断连，验证不会重复结算或漏记已知费用。
- [x] 复核原生状态码/Retry-After、鉴权、模型权限、内容检查、OpenAPI 与实际路由；首批限制与原生 Messages 文档一致；当前 head 完整 CI 已通过并合并 7336a20e。

## F09 compact 与 Gemini 契约

关联 #1382 / #1383。F07 已合并，当前 base 是 main；没有开放的上游依赖。

- [x] 保留 owner/部署/账户绑定、原生错误/用量和 opaque 输出；只选择 OpenAI 的 compaction 实现。
- [x] 模型输出上限与 OpenAI 隐式缓存写入费用进入已有预留；provider/model/key 超预算先拒绝生成，观测缓存写入按实际价结算。
- [x] OpenAPI 声明 default tier 和 cache_write_tokens；八个 Gemini 既有路径与实际路由一致。
- [x] 当前精确提交的默认全套、HTTP 与两套 clippy，以及普通 Anthropic 请求不增加无缓存意图预留的回归，见验证清单。
- [x] 当前 head 的全部 CI/review 已验收，合并及关联 issue 关闭见上表。

## F11 剩余兼容供应商

首批 #1386 / #1389 已完成；Groq #1398 / #1399 已合并（`d6ab3e72`）。#1435 / #1436 只完成媒体协议审计，未完成全部媒体适配。下一单协议实现已建 [#1440](https://github.com/majiayu000/litellm-rs/issues/1440)：Nebius 图片请求/响应及计费；其余供应商缺口仍按现有审核矩阵逐批处理，每批开始前先搜索 issue/PR。

- [x] 将具名 registry 选择器逐项列入现有 compatible-nonchat 文档，记录已验证能力、官方协议路径及尚未核验范围，不能用聊天兼容性推断其他端点。
- [ ] 核对剩余 embeddings/images/audio 能力；仅同协议、同 base 的能力复用现有通路，原生专用路径单独判断是否属于本项必要实现。
- [ ] 每批覆盖 factory → Router → 实际 HTTP，以及模型级能力、JSON/二进制/multipart、状态/Retry-After、用量和预算；不支持能力须明确拒绝。
- [x] 保留 Groq 具体音频模型路由、WAV 默认、真实时长结算及十秒下限回归；缺价不视为免费。
- [ ] 逐批登记剩余供应商的核验结论和限制；只有整个约定范围核验完毕才关闭 F11，不以单个批次 issue 关闭代替完成。

## F15 Realtime

关联 #1397 / #1405。[不可变支持范围](https://github.com/majiayu000/litellm-rs/blob/5d691d1f61f2beef12dc6d7971818ddfadb32f6d/docs/gateway/realtime.md)：手动 OpenAI 文本/音频/客户端函数 WebSocket。排除自动 VAD、转写会话、图片、托管 MCP、模型切换、开启内容检查的会话和崩溃结算。

- [x] 原生 transport、公开鉴权/模型权限、出站限制、逐响应费用/预算、JWT/key/user/team 重新授权已验证。
- [x] 每次创建检查当前 deployment 存在/enabled/配置与新 router 健康及 RPM/TPM/parallel；endpoint/凭据/模型变化要求重连。
- [x] 提高/移除 key cap 可使用新的模型范围；session ack 核对手动模式、请求上限、工具和报告模型。
- [x] 终态即使无可选 usage 或结算失败仍传递；未知状态不记成功。初始化、终态和其他转发操作保留真实供应商错误；零输出 live key 是授权拒绝。
- [x] 仅协议内容检查图片，函数 schema/metadata 不被误拒绝；未知费用保留预算且不伪记实际账单。
- [x] 当前源码检查、全部 CI、审查线程与合并回执已验收；F18 单独验收正式产物。

## F16 声明和废弃接口

已建立 #1402；首批 #1404 已合并 e6677a0b。随对应功能合并更新声明，最终集中复核。

- [ ] 逐项核对 subsystem_registry、README、Cargo feature、网关挂载和发行特性，区分库类型、默认启用、可选路由和已发布能力。
- [ ] 处理写明 0.7 移除但仍存在的废弃接口；根据已落地替代入口做最小清理，不扩展成架构重写或新增兼容层。
- [ ] MCP/A2A/Realtime 等文档中的范围、认证、存储、计费和部署限制与实际行为一致；测试仅针对真实变更，不为文字调整添加机械测试。
- [ ] 记录本项 PR 和验证后关闭 F16；新增声明不能领先于对应实现验收。

## F18 发布和安装验收

已建立 #1403；先准备版本变更及可审阅产物。是否已经发布必须以实际发行记录为准。

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

已完成的 F01–F04、F06–F09、F12–F15、F17 不重复实施，仅参加受影响回归和最终发行验收。F07、F09、F15 仅限各自已接受并记录的范围，F05 内置工具和 F11 未完成能力仍单独处理。F17 当前样本只支持已记录的本地非流式聊天结论；生产负载或其他协议基准不作为本轮新增任务。

## 本次清单整理记录

- 2026-10-03 11:44：依据远端 main、19 个相关 PR 的代码/元数据与审查、发行记录及基准证据整理后续任务；补查 #1399 全部 CI 已通过。仅修改本文件，未重新运行 Rust 测试，未提交、推送、合并或发布。

- 2026-10-03 12:20：用户授权三路并行执行；#1390 清单已推送并整合 main，#1399 已核验后合并。MCP #1391、目录 #1385 在各自 worktree 修复；Responses 后台恢复正在检查预算持久性边界。

- 2026-10-03 13:10：#1385 最新提交 `b745bb9d` 的 15 项 CI 全绿且 review 已解决，已合并 `680e70cd`。F10 其余目录继续进行；三路下一轮推进 F06、F15 及网关收尾。MCP 新增两条 review 正在处理，A2A 本地验证仍在进行，未将二者标为完成。

- 2026-10-03：MCP 补充修复 `63a6c466` 已推送，最新 CI 待验收；A2A `c3bd992f` 完整检查通过后已推送，继续处理新审查。Messages 补 web 工具预算及现有 api.chat 权限复用，17 项定向测试（含 5 项共享夹具）通过，当时完整检查进行中。F06 Copilot/Bedrock 原生路由与 F15 WebSocket 实现由另外两路推进，未提前标完成。

- 2026-10-03 14:02 继续执行：F06 原 PR #1377 已推送 `822f5eb6`，OpenAI 111 模型证据、Copilot 动态端点和 Bedrock Runtime/Mantle 原生路由与价格来源已接入；158 项 OpenAI、42 项 Copilot、400 项 Bedrock/价格、12 项原生路由及默认完整检查通过。生命周期能力仍由 F07 单独验收。
- F08 原 PR #1381 已推送 `bb84357b`：公开 `api.chat` 权限贯通 Messages 中间件和 handler；web search/fetch 强制显式 max_uses 并为新增上下文保守预留。17 项定向测试（12 项路由与 5 项共享夹具）、默认 fmt/check/test/clippy 及 gateway/sqlite clippy 通过；运行时工具和特殊费率尚未全部支持，保持草稿。
- F11 第三批 [#1400](https://github.com/majiayu000/litellm-rs/issues/1400) / [PR #1401](https://github.com/majiayu000/litellm-rs/pull/1401)，提交 `7e24a14f`：接通 OpenRouter、Nebius、NVIDIA NIM、LM Studio 的已核验 embeddings 路由，记录全部 62 个兼容目录选择器的审核范围。14 项定向测试（含 5 项共享夹具）、默认完整检查（7266 项库测试通过、1 忽略）、gateway/sqlite clippy 通过。Nebius 图片协议字段不同，仍未声明支持；其余目录保持未核验，F11 总项不关闭。

- 2026-10-03 14:49：逐行对齐 F06/F07/F08/F11/F13–F16/F18 最新已推送证据，补 #1401 合并结果及 #1404/#1405。历史记录仅代表当时状态；当前阻断以上表和当前 head 为准。清单修改只做文档校验，不重复运行 Rust 构建。

- 2026-10-03 15:23：F13 #1391 验收合并 ceff7f39，完成但未发布；更新 F08 geo 与扩展边界、F09 整链验证、F11 第三批范围链接。整合 main 时清单冲突标记曾误提交至 d0f78a9d，本次立即恢复已审核台账并重新校验 18 项和冲突标记；该中间提交不作验收依据。

- 2026-10-03 15:48：修正 F07 早期记录与当前恢复实现的时序，七条 Realtime 修复按已推送记录，同时保留新的 CI 价格断言失败；A2A 更新为 7f206f27 联合 MCP 完整检查通过。补 F05 input_tokens 和矩阵集成任务，F13 加入免重复实施项。文档校验通过，未将进行中的新检查标为成功。

- 2026-10-03 17:21：恢复 F14/F15 不可变范围链接与排除项，将已完成的 cached-audio 和 c29db734 A2A 验证与新 review 分开；记录 #1407 合并、#1411–#1414 新 PR、Messages 新审查和 Responses 整链重跑，保持未验收状态。


## 本轮 issues / PR 分析与处理

本次续办起点为 11 个开放 issue、9 个开放 PR；最新快照 2026-10-04T06:09:12+08:00。开放 PR 沿用原分支；已合并 xAI 的延迟审查用同一 issue 的最小后续 PR #1439，先处理实际协议/授权/账务问题，再按依赖接受。未通过、取消和旧提交结果分开保存。

### 处理结果与判断

- CompactifAI、多模态聚合商和持久 Responses 已接受；xAI 在独立 #1438 通过 15 项 CI 后接受；合并后延迟提出的 language 需要 format=true 审查另由 #1439 修复，旧 issue 重新打开后按该修复验收。中文 PR 在接受 xAI/main 后最终 diff 只包含中文供应商范围，原生 STT 不作为另一项混入。
- compact 的 100-output 预留不足、混合 provider 选择和 cache-write 漏计均以真实 HTTP/预算/结算回归修复；缺少可靠输出上限时先拒绝。
- Realtime 修复 live 部署、身份/cap、ack/工具/模型、合法终态和错误归因；完整 default、定向特性和对应 clippy 的精确结果归档。已有 moderation 挂起未证明根因，不计扩展全套通过。
- 发行准备与实际发行分开验收。没有新 tag、registry 发布或付费供应商调用，不能把 main 上的功能写成已发布。

### 各 PR 处置

| PR | 范围 | 精确结果 |
| --- | --- | --- |
| [#1377](https://github.com/majiayu000/litellm-rs/pull/1377) | #1375 / F06 Responses | 验收 head `e1623efe`，全部 15 项 CI 成功、审查线程解决后合并 `ac0ad6a9`。 |
| [#1379](https://github.com/majiayu000/litellm-rs/pull/1379) | #1378 / F07 | 验收 head `c44b48d4`，全部 15 项 CI 成功、审查线程解决后合并 `93d51613`。 |
| [#1381](https://github.com/majiayu000/litellm-rs/pull/1381) | #1380 / F08 Messages | 验收 head `bb6ecc10`，全部 15 项 CI 成功、审查线程解决后合并 `7336a20e`。 |
| [#1383](https://github.com/majiayu000/litellm-rs/pull/1383) | #1382 / F09 | 验收 head `d61979f9`，全部 15 项 CI 成功、审查线程解决后合并 `35bd932a`。 |
| [#1390](https://github.com/majiayu000/litellm-rs/pull/1390) | 台账 / 本报告 | 本次文档修订；保留原 18 项验收条件，补当前证据/范围/剩余事项；文档自身 CI/review 与最终合并结果以 GitHub 为准。 |
| [#1393](https://github.com/majiayu000/litellm-rs/pull/1393) | #1392 / F14 A2A | 验收 head `9a3d19be`，全部 15 项 CI 成功、审查线程解决后合并 `7919b463`。 |
| [#1405](https://github.com/majiayu000/litellm-rs/pull/1405) | #1397 / Realtime | 验收 head `5d691d1f`，全部 15 项 CI 成功、审查线程解决后合并 `20640a48`。 |
| [#1406](https://github.com/majiayu000/litellm-rs/pull/1406) | F16 analytics/cache | 验收 head `fc72de28`，全部 15 项 CI 成功、审查线程解决后合并 `8a253b42`。 |
| [#1408](https://github.com/majiayu000/litellm-rs/pull/1408) | F10 DeepSeek/xAI/Cohere | 验收 head `7019848e`，全部 15 项 CI 成功、审查线程解决后合并 `bd8db0a8`。 |
| [#1410](https://github.com/majiayu000/litellm-rs/pull/1410) | F16 重复 native provider | 验收 head `c8324b26`，全部 15 项 CI 成功、审查线程解决后合并 `ba3add3f`。 |
| [#1411](https://github.com/majiayu000/litellm-rs/pull/1411) | retry/SDK 清理 | 验收 head `3ea5f99f`，全部 15 项 CI 成功、审查线程解决后合并 `c3d0977f`。 |
| [#1412](https://github.com/majiayu000/litellm-rs/pull/1412) | #1409 / F11 自托管 | 验收 head `da8fa630`，全部 15 项 CI 成功、审查线程解决后合并 `aa081f3b`。 |
| [#1413](https://github.com/majiayu000/litellm-rs/pull/1413) | #1403 / F18 | 验收 head `e0d742a9`，全部 14 项 CI 成功、审查线程解决后合并 `5249741d`。 |
| [#1414](https://github.com/majiayu000/litellm-rs/pull/1414) | F10 Perplexity | 验收 head `1732c83e`，全部 15 项 CI 成功、审查线程解决后合并 `19a18375`。 |
| [#1416](https://github.com/majiayu000/litellm-rs/pull/1416) | F10 Azure/Voyage | 验收 head `19df4e8f`，全部 15 项 CI 成功、审查线程解决后合并 `ed1e83ed`。 |
| [#1417](https://github.com/majiayu000/litellm-rs/pull/1417) | F10 fal | 验收 head `319fefd9`，全部 15 项 CI 成功、审查线程解决后合并 `5753956a`。 |
| [#1418](https://github.com/majiayu000/litellm-rs/pull/1418) | #1415 / F11 云兼容 | 验收 head `c354fd6f`，全部 15 项 CI 成功、审查线程解决后合并 `b19e8fc9`。 |
| [#1419](https://github.com/majiayu000/litellm-rs/pull/1419) | F10 Replicate | 验收 head `683bc29d`，全部 15 项 CI 成功、审查线程解决后合并 `ca0833ee`。 |
| [#1421](https://github.com/majiayu000/litellm-rs/pull/1421) | #1420 / 中文 | 验收 head `fa1c8eab`，全部 15 项 CI 成功、审查线程解决后合并 `9ca1164c`。 |
| [#1423](https://github.com/majiayu000/litellm-rs/pull/1423) | F10 Stability/BFL | 验收 head `44b5f730`，全部 15 项 CI 成功、审查线程解决后合并 `afdeb820`。 |
| [#1424](https://github.com/majiayu000/litellm-rs/pull/1424) | #1422 / F11 聚合供应商 | 验收 head `fc11c6d8`，全部 15 项 CI 成功、审查线程解决后合并 `1723787d`。 |
| [#1426](https://github.com/majiayu000/litellm-rs/pull/1426) | #1425 / F11 地区 | 验收 head `bb85cf44`，全部 15 项 CI 成功、审查线程解决后合并 `fec51c5d`。 |
| [#1428](https://github.com/majiayu000/litellm-rs/pull/1428) | #1427 / F11 原生 | 验收 head `c8c78d98`，全部 15 项 CI 成功、审查线程解决后合并 `564e070c`。 |
| [#1430](https://github.com/majiayu000/litellm-rs/pull/1430) | #1429 / 聚合 | 验收 head `0603aa0b`，全部 15 项 CI 成功、审查线程解决后合并 `e7da3653`。 |
| [#1432](https://github.com/majiayu000/litellm-rs/pull/1432) | #1431 / F11 W&B/余下选择器 | 验收 head `d7a0765b`，全部 15 项 CI 成功、审查线程解决后合并 `b63a8f8f`。 |
| [#1434](https://github.com/majiayu000/litellm-rs/pull/1434) | #1433 / CompactifAI | 验收 head `ede71579`，全部 15 项 CI 成功、审查线程解决后合并 `a3ddcacd`。 |
| [#1436](https://github.com/majiayu000/litellm-rs/pull/1436) | #1435 / 媒体审计文档 | 验收 head `65b83cbf`，全部 3 项 CI 成功、审查线程解决后合并 `5ff9ceca`。 |
| [#1438](https://github.com/majiayu000/litellm-rs/pull/1438) | #1437 / 原生xAI转写 | 验收 head `5ca350aa`，全部 15 项 CI 成功、审查线程解决后合并 `7b9dbb67`。 |
| [#1439](https://github.com/majiayu000/litellm-rs/pull/1439) | xAI language 格式化延迟审查修复（原 #1438 已合并） | 验收 head `0f5ff352`，全部 15 项 CI 成功、审查线程解决后合并 `3c0d1610`。 |

### 路线图剩余事项与下一步

原四个开放总项不是整个路线图的全部剩余工作：F11 的媒体协议审计已完成，但以下原生能力仍未适配。为它的下一项 Nebius 图片实现新增 #1440，不重开已完成审计的 #1435，也不把 xAI 转写修复视为 xAI 图片/TTS 完成。

| Issue | 已有证据与剩余范围 | 处理顺序 |
| --- | --- | --- |
| [#1372](https://github.com/majiayu000/litellm-rs/issues/1372) / F05 | 原生 JSON/SSE、推理、客户端函数/custom 工具、现有授权/预算以及 F07 持久生命周期已接受。原 issue 要求原生内置工具；当前托管搜索、容器、图片生成和远程 MCP 等因账务边界尚未建立而明确拒绝，所以不能关闭。 | 先按官方单项协议建立可靠的请求费用上限和终态计费单位，再接通一种内置工具并用真实 mock gateway/预算/结算验证；不把传输保留等同完整收费支持。 |
| [#1373](https://github.com/majiayu000/litellm-rs/issues/1373) / F10 | 当前目录 154 callable、439 pricing_only、3976 unreviewed；callable 行没有已过 deprecation_date，但这不证明所有静态运行时声明已核验。Azure mapped authority 35 条、GitHub Models、Nova/Meta/v0、剩余音频/非 Gemini Vertex 等仍需官方逐条核对。 | 先按现有条目及官方 shutdown/model 文档核验 provider/pricing_key/能力/限额；仅确认退役才取消 callable，保留历史价格。每批独立 issue/PR，不新增规则引擎。 |
| [#1440](https://github.com/majiayu000/litellm-rs/issues/1440) / F11 下一批 | [已接受媒体协议审计](https://github.com/majiayu000/litellm-rs/blob/3c0d1610680b9ee64c7f5f754d02a41eec2a3e5e/docs/audit/compatible-media-followup-2026-10-03.md)记录 Nebius width/height 与 data/id 协议差异，具名选择器尚未开放图片。OpenRouter 媒体、xAI 图片/TTS、NVIDIA 独立部署和 Fireworks 等仍有未适配或未确认范围，F11 保持进行中。 | 下一实现 PR 仅处理 Nebius 图片：重新核对官方合同、沿用现有 factory/Router/gateway 做字段与响应转换、真实 HTTP/预算/费用结算回归；其余能力按矩阵逐协议另立 issue，不因本批完成而关闭整个 F11。 |
| [#1402](https://github.com/majiayu000/litellm-rs/issues/1402) / F16 | 多个过期 manager、analytics/cache、重复 provider、observability/retry 表面已清理。旧 core::realtime 4 文件未被 server 引用；MCP/A2A 配置及错误类型有真实引用，兼容 client/export 尚待追踪。registry observability 与 core 的 module-only 注释、MCP 会话说明有过时描述；38 条 registry 声明已入源码清单，引用存在不等于运行验收。 | 先以已接受协议为基线删除确实无用的过期 API；保留 auth/storage/proxy 真实使用的 domain/config/error 类型，再同步 registry/README/Cargo/入口声明并运行必要特性检查。 |
| [#1403](https://github.com/majiayu000/litellm-rs/issues/1403) / F18 | 发行准备代码和精确 shipped-profile 检查可验收；现有版本 v0.7.0 的发布日期为 2026-09-30。历史 ARM64 dirty 预验证不是最终候选。 | 固定全部已接受代码与版本；制作不可变 crate/跨平台二进制/镜像，验证干净安装及本地 mock 协议冒烟；真实发布后登记 tag/commit/checksum/digest/platforms，才关闭。 |

这五个开放 issue 是当时已排定的剩余工作快照，其中 #1440 是 F11 的下一单协议实现，不代表整个 F11 的所有缺口只剩这一项。只有对应 PR 的当前 CI/review、合并及自身范围完成后才关闭 issue。该快照时 #1372 的原生内置工具验收条件未满足；后续完成记录见本文件顶部及 #1441 回执。

### 最新远端快照

| PR | 当前 head | CI 成功 | 未解决审查 | 状态 |
| --- | --- | --- | --- | --- |
| [#1390](https://github.com/majiayu000/litellm-rs/pull/1390)（本报告） | 新提交另行核对 | 独立 CI | 文档验收后解决 | 最终状态以 GitHub 为准 |

归档快照捕获于 06:09，早于 #1440 和本次两条文档审查修正；只适用于所列 head 和时间点，文档自身新提交须独立验收。合并前重新核对 expected head、非 Draft、MERGEABLE、全部 CI 成功及全部线程 resolved。

### 可复核证据与限制

最终组合源码 `3a7b8341` 的 8 项检查全部通过：完整默认套件（7134 库测试通过、1 忽略，另集成/doc）、146 项相关 HTTP、57 项 Realtime、fmt/check/两套 clippy 与同步。实际已接受 main `3c0d1610` 与该组合的完整 Git tree 相同，等价核对命令/哈希归档；本台账新提交只改三份文档/证据文件。

[命令/提交/结果记录](verification-2026-10-03-pr-triage.md)与[完整日志/可携带清单/CI回执/SHA-256](verification-2026-10-03-pr-triage.tar.gz)随原台账 PR 入库。源码构建只在本任务独立 worktree；root 的既有未提交文件保留。当前组必须完成才入成功归档，失败/未完成/空筛选与旧 head 单列。

未运行实际 Postgres 重启或 OS kill；SQLite 双 startup worker 与持久收据测试不等于所有部署 exactly-once。没有供应商付费调用或最终候选产物发布。文档只校验原 18 项验收条件、范围/链接、表格和日志归档，不重复运行 Rust。


### 2026-10-04 #1372 bounded file-search follow-up

Reuse the existing native OpenAI transport, input counting, pricing snapshot,
provider/model/key reservations and settlement. File-search call limits remain
native `max_tool_calls`; terminal completed output calls contribute $0.0025 each.
Background tool recovery and other hosted tools remain outside this accepted
implementation. Details: [native billing scope](../providers/native-responses-billing.md).

Fresh checks on this implementation: fmt/check/default full tests/default and
`gateway,sqlite` all-target clippy passed; all 42 `native_responses_routes` HTTP
tests passed. Build/test debug information and incremental artifacts were disabled
via `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`;
features and behavior were unchanged. The final 15 CI checks and review threads passed on e9dea54c, then #1441
merged as fe036814 and closed #1372. Release remains separate. The first
release-CI attempt timed out; the identical-head rerun passed, with no proven
concurrency-hang root cause.
### 2026-10-04 F10 remaining static catalog review

The [remaining static catalog audit](../audit/remaining-static-model-catalog-2026-10-04.md) accounts for all 41 Azure AI mapped identities, all 16 former GitHub Models, Lambda, Nova/Meta/v0 defaults, native audio records and non-Gemini Vertex metadata. Confirmed shutdowns are separate from unverified protocols. Native-only Azure protocols are pricing-only; price amounts are unchanged. Default and relevant feature suites plus clippy passed locally; final-head CI/reviews passed and #1442 merged as 46476786, closing #1373. No paid supplier availability or release is claimed.

### F16 final removal batch (2026-10-04)

旧 core Realtime 库、MCP/A2A 客户端及过期导出/聚合配置已移除；保留当前网关消费的配置、错误、domain/schema 支持。停用选择器的空目录 hook 和 Meta/v0 特殊策略已删除。全部 37 条当前 core registry 声明已对照 startup/routes/features/README，见 [逐项核对](../audit/subsystem-reconciliation-2026-10-04.md)。本地验证和最终 CI/合并状态按本项 PR 记录；尚未发布新版本。
