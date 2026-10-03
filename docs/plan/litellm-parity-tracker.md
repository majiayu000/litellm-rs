# LiteLLM 差距补齐清单

更新日期：2026-10-03T22:04:04+08:00（北京时间）。起点：`e9cf6a4b`。本轮已验收合并 14 个 PR、关闭 5 个关联 issue；当前远端 main 为 `ed1e83ed`。具体 head、合并提交、剩余阻断与验证见文末本轮处置记录。

本文件是继续执行 F01–F18 的唯一台账。已完成 F01–F04、F06、F08、F12、F13、F14、F17；其余工作按下方复选清单推进。功能验收、合并和发布分别记录；当前发行仍为 v0.7.0，尚不包含本轮新增成果。

本清单记录此次审计发现的全部缺口。参考 BerriAI/litellm 的实现和官方协议，模型事实以供应商官方资料为准；不把价格表中的模型自动当作可调用模型。

状态：待开始 → 进行中 → 待验收 → 完成。只有代码、测试和关联 PR 均有可查证结果时才标为完成；合并和发布单独记录。每完成一项，在本文件更新状态、证据及剩余限制。每个实现 PR 对应一个 issue。修复已有 PR 时沿用原分支；不建立竞争 PR。表中既有测试数量来自关联 PR 的验证记录，后续代码批次已按各条目记录执行 Rust 验证；最新提交的 CI 状态单独列在下方。

| ID | 优先级 | 功能 / 问题 | 验收条件 | 状态 | Issue / PR / 验证 |
| --- | --- | --- | --- | --- | --- |
| F01 | P1 | 恢复模型价格自动更新 | 从有效上游引用解析不可变提交；新增模型进入 unreviewed；保留人工决定；更新后的三个目录文件一致；重复运行无差异 | 完成 | [#1364](https://github.com/majiayu000/litellm-rs/issues/1364)；51 项 Python 测试通过；当前源新增 14 条 unreviewed，4,555 条已有决定不变；重复同步/check 通过；Rust 默认全量检查通过；[PR #1366](https://github.com/majiayu000/litellm-rs/pull/1366)；已隔离 Voyage 缺价测试数据并通过 gateway/sqlite 定向测试；补充空证据源拒绝测试；四条上游峰谷价格已转换为运行时格式，峰/谷与预算上限回归通过；全部 CI 通过，review 已解决，已合并 `fcc8f1b1` |
| F02 | P1 | Cloudflare 错误语义 | HTTP 401/429/5xx 和 success=false 返回对应错误；畸形成功响应不能变成空成功；覆盖真实 HTTP 模拟测试 | 完成 | [#1365](https://github.com/majiayu000/litellm-rs/issues/1365) / [PR #1367](https://github.com/majiayu000/litellm-rs/pull/1367)；91 项相关测试、默认全量测试/check/clippy 通过；全部 CI 通过、无未解决 review；已合并 `01f15630`；一次 Gemini 测试超时，定向复测 9 项通过且同提交 CI 重跑通过，未将偶发超时误报为已修复 |
| F03 | P1 | Cloudflare 现代聊天协议与流式 | 使用官方兼容接口；文本、工具、多模态、用量、流式及中途错误按支持能力正确传递；文档和能力声明一致 | 完成 | [#1368](https://github.com/majiayu000/litellm-rs/issues/1368)；[PR #1369](https://github.com/majiayu000/litellm-rs/pull/1369)；聊天/流式本地 HTTP 测试、默认全量测试/check/clippy 通过；审查后的 88 项 provider 测试与 clippy 通过；全部 CI 通过，review 已解决；已合并 `4b4a4dd4` |
| F04 | P1 | Bedrock 逐模型能力与计费 | 审核 generic_converse 全部条目；修正上下文、输出、视觉、推理、profile 与价格；消除通用虚假默认值和重复价格源 | 完成 | [#1370](https://github.com/majiayu000/litellm-rs/issues/1370)；逐项核对 39 张 AWS 模型卡及官网定价；修正通用参数、移除 Sonic 聊天声明、统一计费来源；382 项 Bedrock 测试通过；[PR #1371](https://github.com/majiayu000/litellm-rs/pull/1371)；默认全量测试/check/clippy 及全部 CI 已通过，无未解决 review；已合并 `e77391fe` |
| F05 | P1 | 原生 Responses 请求通路 | 支持该协议的供应商真正调用原生 endpoint；保留工具、推理、原生事件、用量及错误；经过现有鉴权、路由、预算和记录链路 | 进行中 | [#1372](https://github.com/majiayu000/litellm-rs/issues/1372) / [PR #1374](https://github.com/majiayu000/litellm-rs/pull/1374)，`b67fe01a`：OpenAI 原生 JSON/SSE、鉴权、路由、预算、内容检查及回调已接通；function/custom 工具、内联图片/PDF/file_id 使用官方 input_tokens。未知用量保留预算而不伪记实际账单；托管工具、可变远程 URL、隐藏上下文和非标准 tier 拒绝。16 项 HTTP、5 项单测、默认完整检查（7266 通过/1 忽略）及特性 clippy 通过，15 项 CI 全绿、原 review 已解决。F06/F07/F09 整链 2918fa38 的 Main Full 和 Cross Platform 均通过；#1374 已在 15/15 CI、review 清零后合并 f62c5adb。F06/F07 按顺序整合，首批仍限制 store=false/background=false/无 predecessor。 |
| F06 | P1 | Responses 模型与供应商路由 | OpenAI、Copilot、Bedrock 原生协议分别按官方 endpoint/model 支持矩阵路由；Responses-only 模型和工具不被送往 chat/completions | 完成 | [#1375](https://github.com/majiayu000/litellm-rs/issues/1375) / [PR #1377](https://github.com/majiayu000/litellm-rs/pull/1377)，验收 head `e1623efe`。111 个 OpenAI 端点证据、Copilot 动态 supported_endpoints、Bedrock Runtime/Mantle 地址、签名和计费身份已接通；Responses-only 不回退聊天。修正自动合并重引入的 Chat 能力回退，按精确 provider/pricing_key 保留独立目录审核决定，重建摘要。52 项 Python、同步/check、18 项原生 HTTP 及联合 clippy 通过。该 head 的 15 项 CI 全绿、线程清零后以 merge commit `ac0ad6a9` 合并，保留堆叠祖先；#1375 已关闭，#1379 base 已改为 main。Copilot/Bedrock 生命周期仍不在首批范围。 |
| F07 | P1 | Responses 跨副本持久状态 | 两个网关实例可读/删同一授权响应；重启后可恢复记录；租户隔离、TTL、后台状态及取消语义有测试 | 进行中 | [#1378](https://github.com/majiayu000/litellm-rs/issues/1378) / [Draft PR #1379](https://github.com/majiayu000/litellm-rs/pull/1379)，最新 `c44b48d4`，base main，已整合 F06 和主线 `1723787d`。POST 前 SQL 持久化结算责任，恢复只 GET、不重发生成；SQL 用量/收据同事务，Redis 去重。历史实现默认完整 7269/1 忽略、316 项 Responses、3 项数据库及 165 项含隔离 Redis 的预算测试通过。真实 HTTP POST 后关闭创建 runtime/server/worker，再由两个新 startup worker 恢复一次的 SQLite 回归通过；不声称实际 Postgres 重启或 OS kill。未知用量保留 reserved_unknown，provider/model 后台预算需共享 Redis，不支持进程内 key 预算，账务不被内容 TTL/删除抹除。最新整链 `48f1b2be` 的 94 项 HTTP、同步/check、fmt 和联合 clippy 通过；[当前 F07 Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126573698)及主线检查须完成，Draft/issue 保留。 |
| F08 | P1 | Anthropic 原生 Messages 网关 | 提供 /v1/messages；工具、thinking、流式、错误、用量和鉴权符合原生协议；文档/OpenAPI/路由一致 | 完成 | [#1380](https://github.com/majiayu000/litellm-rs/issues/1380) / [PR #1381](https://github.com/majiayu000/litellm-rs/pull/1381)，当前 `bb6ecc10` 已退出草稿：JSON/SSE、count_tokens、缓存 TTL、有界直接 web 工具及显式/继承 US 地区计费已接通；未知用量保留承诺但账单记为 unpriced。22 项集成、默认完整检查、gateway/sqlite/mcp 完整测试（9780 通过/1 忽略）、clippy 和 all-features check 通过。前轮五项审查修复已推送，默认 7270 项库测试/1 忽略及联合完整检查通过；all-features check 属此前提交。新轮 Haiku 缓存价格、终态用量/JSON stop_reason、start envelope 与 guardrail 健康归因五项修复已推送，25 项 HTTP、52 项 Python/重复同步、默认完整及 gateway/sqlite/mcp 完整检查通过，review 清零；随后原分支整合 f62c5adb，当前 `bb6ecc10` 的 15 项 CI 全绿且 review 清零，已合并 `7336a20e` 并自动关闭 #1380；本轮 25 项原生 Messages HTTP 测试通过；另 Sonnet 4.5 1M beta 审查经官方 2026-04-30 退役说明确认不适用，保留 200k。 |
| F09 | P2 | Responses compact 与 Gemini 文档覆盖 | 接通 /v1/responses/compact；Gemini 已有 generateContent/streamGenerateContent 路由，补 OpenAPI 覆盖并复核现有鉴权/流式/用量测试 | 待验收 | [#1382](https://github.com/majiayu000/litellm-rs/issues/1382) / [Draft PR #1383](https://github.com/majiayu000/litellm-rs/pull/1383)，最新 `48f1b2be`，base F07 `c44b48d4`。compact 经过原生 OpenAI、权限/预算和持久记录；Gemini 八个既有路径补契约，不重复实现。历史 `2918fa38` 完整 gateway/sqlite 9653/1 忽略通过；`0050a21b` [Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37120379335)和[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37120381165)均成功，只适用于旧 head。最新 94 项 HTTP（Responses 31、adapter 10、Gemini 30、catalog 23）、fmt、同步/check、联合 clippy 通过。最新 head 的 [Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126576346)和[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37126578702)均已成功；F07 接受后再改 base 为 main，不能用旧 CI 代替新 head/base 验收。 |
| F10 | P1 | 模型退役与目录一致性 | 去除已退役模型的可调用声明；优先修 Cloudflare；复核全部现有静态供应商目录、示例、价格和能力来源 | 进行中 | [#1373](https://github.com/majiayu000/litellm-rs/issues/1373)。先前 OpenAI/Azure/Cloudflare/Copilot/Bedrock #1376、Mistral #1396、Anthropic/Gemini #1385 已合并；本轮 DeepSeek/xAI/Cohere #1408（`bd8db0a8`）、Perplexity #1414（`19a18375`）、fal #1417（`5753956a`）、Replicate #1419（`ca0833ee`）、Stability/BFL #1423（`afdeb820`）均在 exact head 的 15 项 CI 全绿和 review 清零后合并。Azure/Voyage #1416 的 `19df4e8f` 同样通过并合并 `ed1e83ed`；实际 providers-extra Azure AI 107 项为前轮 d0eb912d，最新整合为 Azure 相关 116 项、Voyage 6 项、Responses 18 项、Python 52 项及联合 clippy。不能把错误特性过滤匹配的两个测试算原生 Azure 模块验收。历史全量检查与最新定向检查分开归档。Azure mapped authority 35 条、GitHub Models、Nova/Meta/v0、剩余音频/非 Gemini Vertex 等仍需核验；历史价格不恢复 callable，图片价格不从 tokens 推断。基线见 [逐条目录记录](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/audit/model-catalog-2026-10-01.entries.json)。 |
| F11 | P2 | 兼容供应商的非聊天能力 | OpenAI-compatible 通路补齐 embeddings/images/audio 的实际调度；只声明经过协议验证的能力；未知/不支持能力返回明确错误 | 进行中 | 通用 openai_compatible 原本已有 embeddings/images/audio；按具名 provider 的已核验协议开启能力，不能靠聊天兼容推断。此前 #1389（Together/DeepInfra/Fireworks）、#1399（Groq）、#1401（OpenRouter/Nebius/NVIDIA/LM Studio）已合并；本轮自托管 #1412 合并 `aa081f3b` / #1409 关闭，聚合供应商 #1424 合并 `1723787d` / #1422 关闭。云兼容 #1418 当前 `95ea52e7`（最新已推送 c354fd6f）：保留 Nscale/OVH/Heroku embeddings，接入官方原生凭据及配置实例名预算；像素计费未支持，因此撤回 Nscale 图片 capability。941357cb 完整默认 7141/扩展 10414（各 1 忽略）及 clippy 通过，95ea52e7 的 27 项 HTTP 和 clippy 通过；两条新 review 为 HTTP input_type 文档和直接 factory 空凭据，c354fd6f 的直接 factory/credential 矩阵、27 项 HTTP、完整默认 7132/扩展 10252（各 1 忽略，另集成/doc）和两套 clippy 均通过，已推送并解决两线程，最新 CI 待验收。中文 #1421 当前 `cfbf58bf`，DashScope/Qwen total_tokens-only 使用量在 wire 边界归一化，d8346759 完整默认 7137/扩展 10410（各 1 忽略）通过，最新 28 项 HTTP/clippy 通过，等待当前 CI。地区批次 #1426（#1425）新出现且仍有两条 Baichuan 有效审查：批次截断及 429 余额/限流区分；不能合并。原生协议批次 #1427 已形成 #1428，等待当前 CI/review；全 62 个选择器的剩余核验不能因某一批次验收而关闭。 |
| F12 | P2 | 自定义供应商注册 | 外部实现可通过公开 API 注册并被路由，无须修改内部 Provider 枚举；覆盖构造、能力、错误和流式测试 | 完成 | [#1384](https://github.com/majiayu000/litellm-rs/issues/1384)；外部接口已接入现有 Provider/Deployment/Router，all-features 编译通过；4 项外部集成测试通过（注册/路由、模型能力、流式错误、未实现能力/缺价）；默认完整测试/check/clippy、gateway/sqlite/扩展 provider clippy 均通过（默认库 7,217 项通过、1 项忽略）；已提交 [PR #1387](https://github.com/majiayu000/litellm-rs/pull/1387)；审查补充健康检查回调路由回归；全部 CI 通过、review 已解决，已合并 `f24aa58f` |
| F13 | P2 | MCP 网关 | 把现有 MCP 能力接入 HTTP 网关；工具发现/调用与资源/提示词、鉴权、连接关闭有端到端测试；传输支持如实列出 | 完成 | [#1388](https://github.com/majiayu000/litellm-rs/issues/1388) / [PR #1391](https://github.com/majiayu000/litellm-rs/pull/1391)，`a1f718bb`：MCP 2026-07-28 无状态 POST、工具/资源/提示词、MRTR/SSE、命名权限、Origin/出站限制、并发/超时和断流已接通。新增前缀同名路由、配置导出脱敏、空/旧通用权限拒绝及 Tasks taskId 路由头回归；最新 22 项路由测试、fmt 和特性 all-target clippy 通过。前一提交 bf8434ab 默认完整检查（7219 通过/1 忽略）及 gateway/sqlite/mcp 完整测试/clippy 通过；提交 a1f718bb 的 15 项 CI 全绿，review 清零；已合并 ceff7f39。无旧协议、OAuth 获取、聚合或外部工具计费；无网关会话亲和要求。范围见 [文档](https://github.com/majiayu000/litellm-rs/blob/a1f718bb771f5974d212450c813ac4c87bd85b9d/docs/gateway/mcp.md)。 |
| F14 | P2 | A2A 网关 | 接通 agent card、任务提交/查询/取消及事件流；代理鉴权、租户隔离和错误有端到端测试 | 完成 | [#1392](https://github.com/majiayu000/litellm-rs/issues/1392) / [PR #1393](https://github.com/majiayu000/litellm-rs/pull/1393)，`9a3d19be`：补 server Message context、任意 JSON data、畸形 URL 导出脱敏、终态订阅拒绝及 SSE 禁止缓冲。41 项路由、默认完整检查、gateway/sqlite/a2a/mcp 完整测试（9990 通过/1 忽略，另有集成/doc）及 clippy 通过，五条对应 review 已解决；此前 c29db734 的 15 项 CI 全绿；最新六条协议 review 及其验证见本行后续记录。进程内归属需实例亲和、重启拒绝旧归属，无 push/list/扩展卡片/代理计费。见 [不可变范围文档](https://github.com/majiayu000/litellm-rs/blob/c29db734e312f040c4ef62d3225bb4000f2a4f91/docs/gateway/a2a.md)。 最新 9a3d19be 六条新 review 已处理、43 项路由、默认完整 7267/1 忽略和 gateway/sqlite/a2a/mcp 完整 9992/1 忽略及 clippy 通过，已推送；9a3d19be 的 15 项 CI 全绿、review 清零，已合并 `7919b463` 并自动关闭 #1392；F18 单独验证发行产物。 |
| F15 | P2 | Realtime 网关 | 接通 WebSocket 双向代理；供应商配置、鉴权、事件/关闭/错误传递有测试；首批支持范围明确 | 进行中 | [#1397](https://github.com/majiayu000/litellm-rs/issues/1397) / [PR #1405](https://github.com/majiayu000/litellm-rs/pull/1405)，已推送最新 `969ba9ca`。OpenAI 手动文本/音频/函数生成、逐响应 key/JWT 用户/团队授权、有效输出 cap 预留、原生事件/错误/关闭传递已接通；配置实例名用于预算，canonical identity 用于价格。八条、四条、两条以及最新五条对应 review 均已修复并解决。最新五条为中断 generation 的 RPM/TPM、原生 inf cap、同模型 session.update、零输出 key 早拒绝和预转发预算失败取消 admission。session cap 仅按上游 acknowledgment 更新，可信 completed response.done 不被独立结算错误吞掉，真实结算错误仍保留 Err/日志。2ee7911b 完整默认 7134/扩展 9913（各 1 忽略，另集成/doc）与两套 clippy 通过；首次特性完整挂起已取消，原始日志单列，既有 Responses 目标 8 项独立通过，同提交两线程全量重跑成功，不声称挂起根因已修。969ba9ca 的 30 项 Realtime、隔离 Redis admission 回归、fmt/check/同步及两套 clippy 通过，随后新增三条 review（session update 未确认时旧 cap、上游拒绝请求本地 RPM、取消终态缺 usage）；本地 eef1c949 最小修复已提交、完整检查中，尚未推送。最新 CI/review 待验收。首批无自动 VAD、转写、图片、托管 MCP、模型切换、WebRTC/SIP 或启用内容 guardrail 的会话；崩溃后结算不保证 durable exactly-once。见 [当前不可变范围](https://github.com/majiayu000/litellm-rs/blob/969ba9ca3f87c71efde665b73ae3fc858239c9ea/docs/gateway/realtime.md)。 |
| F16 | P2 | 过时声明与未落地子系统 | 逐项核对 subsystem_registry 和 README；完成上述能力后同步状态，清理已到移除版本的废弃接口，避免“声明支持却不可用” | 进行中 | [#1402](https://github.com/majiayu000/litellm-rs/issues/1402) / [首批 PR #1404](https://github.com/majiayu000/litellm-rs/pull/1404)，`6b9b4580`：删除过期且未使用的 BatchProcessor、重复 VirtualKeyManager、UserManager 和 user-management feature，保留实际网关/鉴权/用户数据通路；README 与登记表记录未发布的源码破坏性变更。默认完整检查 7266 通过/1 忽略、all-features check、gateway/sqlite 9631 项库测试/1 忽略及 clippy 通过，15 项 CI 全绿且无未解决 review，已合并 `e6677a0b`。后续 [#1406](https://github.com/majiayu000/litellm-rs/pull/1406) 删除过期 analytics/semantic cache，[#1407](https://github.com/majiayu000/litellm-rs/pull/1407) 删除闲置 observability/webhooks，[#1410](https://github.com/majiayu000/litellm-rs/pull/1410) 删除五套过期重复 native provider。三批各自默认完整、all-features、相关特性测试/clippy 均通过；#1407 最新 689954f5 的 15 项 CI 全绿、review 清零，已合并 decce90f。#1406（a3390652）/#1410（c8324b26）与该 main 的冲突已原分支解决，整合后的默认完整、all-features、相关特性完整测试/clippy 已通过并推送。#1410 首次扩展运行在 batch mock 关闭时挂起被中断，定向与单线程完整复测通过，未声称已修复偶发挂起。retry/SDK 清理 [#1411](https://github.com/majiayu000/litellm-rs/pull/1411)（3ea5f99f）默认完整 7259/1 忽略及扩展 clippy/all-features 通过，文档 review 已处理，3ea5f99f 的 15 项 CI 全绿且 review 清零，已合并 `c3d0977f`；旧 realtime 及最终声明尚待核对。 #1410 的 c8324b26 在 15 项 CI 全绿、review 清零后已合并 ba3add3f；#1406 更新迁移文档冲突为 fc72de28，该 head 15 项 CI 全绿、线程清零后已合并 `8a253b42`；F16 总项继续开放。 |
| F17 | P2 | 可复现的 LiteLLM 对比基准 | 同机器、同模拟上游和相同负载比较吞吐/延迟/错误率/内存；保存命令、版本和样本，不用 Rust 语言推断性能结论 | 完成 | [#1394](https://github.com/majiayu000/litellm-rs/issues/1394) / [PR #1395](https://github.com/majiayu000/litellm-rs/pull/1395) 已于 2026-10-03 合并（`cb76a186`）。同机、同上游、4 workers、并发 64 的三轮对照及直连基线共 9 个样本零请求错误；原始数据/环境/限制随报告提交。12 项运行器行为测试、14 项现有 benchmark 契约测试、默认完整 Rust 检查及全部 CI 通过，审查线程已解决。结果仅适用于报告中的本地模拟负载，不宣称通用倍数，见 [已合并报告](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison.md)及[原始证据包](https://github.com/majiayu000/litellm-rs/blob/cb76a1866dd7f50664cae7a1389c0c3b38861840/docs/benchmarks/litellm-comparison-20261003.json.gz)。 |
| F18 | P1 | 发布与安装产物 | 所有已验收能力进入版本发行包；验证 crate、安装说明及容器内容；记录实际发布版本/提交，不能只凭 main 已合并声称已发布 | 进行中 | [#1403](https://github.com/majiayu000/litellm-rs/issues/1403) / [Draft PR #1413](https://github.com/majiayu000/litellm-rs/pull/1413)，最新 `674f804f` 已整合 main `1723787d`，保留 Rust 1.96.1、统一 A2A-inclusive 发行特性、GHCR 登录、归档 checksum 和真实 Cargo target 文件。fmt 和 locked 精确发行特性 gateway 编译通过；当前 CI 即使全绿仍只验收准备补丁。此前 ceff7f39 基础的两种原生 ARM64 Docker 镜像、普通用户 /health=200、解包 crate 编译和干净 install/validate-config 为历史预验证，未推 registry，不代表最终候选或跨架构运行。最终版本/不可变候选、产物协议冒烟与发布仍缺。实际发行仍 [v0.7.0](https://github.com/majiayu000/litellm-rs/releases/tag/v0.7.0)（`3341e54a`，2026-09-30），未创建 tag、发布 crate/镜像或触发正式发行。 |

## 执行与验收记录

- 2026-10-03：建立清单；GitHub 当前无开放 issue/PR 与上述工作重复。先执行 F01，然后 F02–F04，再处理协议、状态和网关能力。独立功能拆分提交，避免将全部变化塞入一个 PR。
- 所有构建与测试在本任务独立 worktree 执行。每个 PR 准备好前执行仓库要求的格式、检查、全量测试及 clippy；合并前确认 CI 全绿且 review threads 已解决。
- 离线模拟测试与需要供应商账户的真实调用分开记录。没有凭据或没有运行过的真实调用不得标为通过。

- 2026-10-03 复核更正：Gemini 原生生成和流式接口在实际路由中已存在，F09 改为文档/契约覆盖，不重复实现。

- 2026-10-03 F07 早期记录（已被 aa1dedb6 后续实现取代）：当时共享响应记录可重启读取，但后台结算责任尚不可恢复。当前 aa1dedb6 已在 POST 前保存独立结算记录，恢复 GET 接管、不重发 POST；见上表及 #1379。整链集成、CI 与审查仍待完成。

## 后续执行顺序

先完成台账校正及已有 PR 的阻断处理，再完成 Responses 与 Messages 的计费和恢复缺口，随后收尾剩余目录、供应商及 Realtime。F16 随功能合并同步核对，F18 在选定发行提交上集中验收。该顺序不要求等待无依赖的工作，也不授权扩大首批协议范围。

Responses 的现有分支依赖为 `#1374 → #1377 → #1379 → #1383`。先在整条链上完成相关验收，再按依赖顺序合并和调整下游 base；不得通过改成 ready 或关闭 issue 隐藏未完成的模型、生命周期或计费要求。

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

继续已有 #1374、#1377、#1379，分别对应 #1372、#1375、#1378。

- [ ] F05：保留原生 JSON/SSE、工具、推理、未知扩展字段及原生错误；复核鉴权、路由、内容检查、预算、回调和记录链路没有因后续生命周期改动被绕过。
- [ ] F05：完成已声明支持的托管工具/文件费用维度的预留与结算，缺少可计费依据的请求返回明确错误；测试包含中途错误和缺失终态用量，不能默认为免费。
- [x] F05 独立实现：b67fe01a 已接通原生 input_tokens 的结构化媒体/文件计数；HTTP 回归覆盖计数失败、超预算不生成及可变远程输入拒绝。下游整链验收仍待完成。
- [x] F05/F06 本地集成：d9c224b8 已将模型/endpoint 矩阵合入新计费路径；F09 整链 8764e00e 本地完整检查通过，远端完整 CI 仍单独等待。
- [x] F06：完成 Copilot、Bedrock 的官方 endpoint/model 支持核验与原生分发；保留每项官方依据。Responses-only 模型和原生工具不得回退到 chat/completions。
- [x] F06：覆盖 OpenAI/Copilot/Bedrock 的实际 HTTP 路由、模型权限、端点不支持、流式及错误；111 个 OpenAI 模型的证据不能替代其他供应商验收。
- [x] F07 实现：aa1dedb6 已以独立 SQL 结算记录和共享预算收据实现退出后恢复接管，见 #1379；这一勾选只记录实现及本地测试，整链最终验收见以下未勾选项。
- [x] F07 本地回归（52820df8，SQLite）：以两个网关和共享数据库验证“创建 → 终态前关闭创建实例 → 另一实例/重启接管 → 最终只结算一次”，同时证明不会重放可能计费的生成请求。
- [ ] F07：补接管竞争、取消/删除竞争、过期、租约续期失败和无终态用量回归；记录保守结算的边界，防止重复扣费或预留释放后漏记。
- [ ] F07：复核 owner、deployment、account 绑定及 TTL；store=false 的临时元数据不能保存输入输出内容，独立 SQLite 回退不能宣称跨副本持久化。
- [ ] 整体验证 store 默认行为、background、previous_response_id、查询/删除/input_items、取消和游标恢复；GET、重连或取消不能生成第二次费用。
- [x] F06 e1623efe 的 15 项 CI 和 review 通过后，以 ac0ad6a9 合并并关闭 #1375；保留堆叠祖先，把 #1379 的 base 改为 main。
- [ ] F07/F09 当前提交在依赖改 base 后验收完整 CI；Draft 状态保留，各 issue 只在自身范围满足后关闭。

## F08 Anthropic Messages

关联 #1380 / #1381；复用已有原生传输，不重复实现路由。

- [x] 核对托管工具的非令牌费、工具循环新增输入和已支持的特殊费率，补预算预留及实际用量结算；未完成计费的能力不能静默按普通文本计费。
- [x] 保留 count_tokens 对图片、文档、工具和 thinking 的预估，以及冷缓存 TTL 预留；覆盖超预算时生成尚未发起的行为。
- [x] 覆盖 JSON/SSE 累计用量、缓存写入/命中、工具调用、thinking、流中错误和断连，验证不会重复结算或漏记已知费用。
- [x] 复核原生状态码/Retry-After、鉴权、模型权限、内容检查、OpenAPI 与实际路由；首批限制与原生 Messages 文档一致；当前 head 完整 CI 已通过并合并 7336a20e。

## F09 compact 与 Gemini 契约

关联 #1382 / #1383；依赖 Responses 链，Gemini 原生路由已经存在。

- [x] 在 F07 更新后的分支上复核 compact 的所有权/部署/账户绑定、预算、异常成功响应、用量和上游错误回归。
- [x] 复核八个 Gemini OpenAPI 路径与实际挂载一致，保留鉴权、流式和用量测试；与 F10 的退役模型拒绝规则一致。
- [x] 历史 F09 0050a21b 的 Main Full 与 Cross Platform 均成功（该次 base 为 F07）。
- [x] 最新 F09 48f1b2be 与 F07 c44b48d4 整合后，94 项 HTTP、fmt、同步/check 和联合 clippy 通过。
- [x] 最新 48f1b2be 的完整 Main Full/Cross Platform 均成功，精确 head 已核对，不能沿用旧 head 的成功。
- [ ] 在 F07 依赖落地后改 base，再验收当前主线 CI/review；不把文档缺项重新实现为另一套路由。

## F11 剩余兼容供应商

首批 #1386 / #1389 已完成；Groq #1398 / #1399 已合并（`d6ab3e72`）。剩余供应商在开始下一批时先搜索 issue/PR，按实际缺口建立有边界的 issue。

- [x] 将具名 registry 选择器逐项列入现有 compatible-nonchat 文档，记录已验证能力、官方协议路径及尚未核验范围，不能用聊天兼容性推断其他端点。
- [ ] 核对剩余 embeddings/images/audio 能力；仅同协议、同 base 的能力复用现有通路，原生专用路径单独判断是否属于本项必要实现。
- [ ] 每批覆盖 factory → Router → 实际 HTTP，以及模型级能力、JSON/二进制/multipart、状态/Retry-After、用量和预算；不支持能力须明确拒绝。
- [x] 保留 Groq 具体音频模型路由、WAV 默认、真实时长结算及十秒下限回归；缺价不视为免费。
- [ ] 逐批登记剩余供应商的核验结论和限制；只有整个约定范围核验完毕才关闭 F11，不以单个批次 issue 关闭代替完成。

## F15 Realtime

关联 #1397 / #1405；首批范围仅 OpenAI WebSocket，具体限制见上表。

- [x] 原始实现、路由、费用与验证已形成可查证提交 7b037dd8 和 PR #1405。
- [x] 接入现有供应商配置、受限出站、鉴权、模型权限与公开 WebSocket 路由；完成默认和 gateway/sqlite/websockets 全量检查。
- [x] 修复审查发现的逐响应 API-key RPM、已完成 token 的失败后 TPM 计数、上游错误关闭健康统计和握手模型映射。
- [x] 修复 pre-creation 错误事件与预留的关联、用量记录失败时终态事件传递，并核对 gpt-realtime-2 缓存音频价格依据。
- [x] 607e8c44 修复发行特性下 cached-audio JSON 浮点表示断言，定向测试/clippy 通过；运行时价格未改。
- [x] 收尾新增八条 review：输出上限继承、活动 close/failed done 健康、初始化重试、逐响应部署 RPM/TPM、客户端错误中立、middleware 路径及最新 key 状态/权限；保持未知费用不当免费。
- [x] 四条 review：JWT 用户/团队逐响应重新授权、provider pre-creation 错误健康归因、有效输出 cap 的预算预留及 OpenAI query 错误契约；已在 8d0ba253 修复、完整验证并解决线程。
- [x] 两条 review：session cap 在上游 acknowledgment 后变更、已完成 response.done 不被预算结算错误吞掉；a23f37e4 完整检查通过、推送并解决线程。
- [x] 最新五条 review：中断 generation 的部署 RPM/TPM、inf cap、同模型 session.update、零输出 key 早拒绝和预算拒绝撤销 admission；2ee7911b 完整检查通过，969ba9ca 最新 30 项 Realtime/隔离 Redis 与两套 clippy 通过，已推送并解决线程。
- [x] 969ba9ca 回归配置实例名 prod-openai 的预转发限额拒绝和实际费用结算；canonical OpenAI 仍只用于定价。
- [ ] 最新 CI/review 通过后验收，F18 单独验证发行包；不包含 WebRTC、SIP 或其他供应商。

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

已完成的 F01–F04、F06、F08、F12、F13、F14、F17 不重复实施，仅参加受影响回归和最终发行验收。F17 当前样本只支持已记录的本地非流式聊天结论；生产负载或其他协议基准不作为本轮新增任务。

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

快照时间：2026-10-03T22:04:04+08:00。开始为 12 个开放 issue、15 个开放 PR；处理期间出现 8 个新 PR，本报告累计覆盖 23 个 PR。已验收合并 14 个，关闭 #1392、#1380、#1409、#1375、#1422 共 5 个关联 issue；快照仍有 12 个开放 issue、9 个开放 PR。快照后新建/更新的事项需按其当前 head 复核，不能把此表当成持续实时状态。

### 判断与已执行处理

- 主要问题是并行分支过期、后续审查揭示真实协议/预算边界，以及台账将旧 head 的成功错算到新 head；处理优先级应由依赖和错误影响决定，不能只清空 issue 数量。
- 已在已有原分支修复 Responses 能力回退、Realtime 逐响应授权/配额/预算/事件契约、DashScope total-only usage、Heroku/OVH 官方凭据和云实例预算归属；每个修复用真实本地 HTTP/WS 或独立 Redis 行为验证。Nscale 图片因像素计费缺口撤回声明，不扩展成新计费框架。新出现 #1428 已读取范围和验证记录，按现有 PR 等待最新 CI，不创建竞争实现。
- F06 用 merge commit 保留堆叠祖先；只有合并成功后才把 F07 base 改 main，并关闭其 issue。F07/F09 与发行 PR 保持 Draft。一次 guard 暂停后脚本曾继续改 base/关闭 issue，已立即恢复，再在真实合并成功后重新执行，最终依赖状态已核对。
- 所有本轮合并都重新读取 expected head、非 Draft、mergeable、全部 CI 成功及全部 review threads resolved；没有用旧提交或取消/排队状态替代新提交验收。#1377 为 merge，其余为 squash。
- 新出现 #1426 的批次/错误审查仍阻止验收。[官方 embeddings 协议](https://platform.baichuan-ai.com/docs/text-Embedding)明示16条批次上限和超出截断，[官方错误表](https://platform.baichuan-ai.com/docs/errCode)同时将限流和余额不足列为429；本轮已检索核验。文档已明示 Baichuan 静默截断，但新增测试只有单输入，不能证明大批次安全；余额错误的 body 分类尚未覆盖。处理建议是限量拒绝而非新增分批框架，并按原生 error body 区分永久余额失败和可重试限流；完成官方核验和回归后再接受。
- 总项 #1373/#1402 及 F11 不能由一个子批次自动关闭；版本发行仍为 v0.7.0，代码合并和准备 PR 全绿均不等于产物已发布。

### 每个 PR 的处置

| PR | 对应范围 | 验收 / 当前 head | 已执行结果或剩余阻断 |
| --- | --- | --- | --- |
| [#1377](https://github.com/majiayu000/litellm-rs/pull/1377) | #1375 / Responses 路由 | `e1623efe` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `ac0ad6a9`。 |
| [#1379](https://github.com/majiayu000/litellm-rs/pull/1379) | #1378 / F07 | `c44b48d4` | Draft，base main；本地最新整链定向通过，当前主线完整 CI 验收及 lifecycle 边界复核待完成。 |
| [#1381](https://github.com/majiayu000/litellm-rs/pull/1381) | #1380 / Messages | `bb6ecc10` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `7336a20e`。 |
| [#1383](https://github.com/majiayu000/litellm-rs/pull/1383) | #1382 / F09 | `48f1b2be` | Draft，base F07；94 项 HTTP/clippy 通过，当前 Main Full/Cross Platform 均成功；F07 接受后改 base 并重新验收。 |
| [#1390](https://github.com/majiayu000/litellm-rs/pull/1390) | 台账 / 本报告 | `本次修订` | 沿用原 PR、整合 main，补精确提交/状态及可携带日志归档；四条文档审查在本次推送后解决。 |
| [#1393](https://github.com/majiayu000/litellm-rs/pull/1393) | #1392 / A2A | `9a3d19be` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `7919b463`。 |
| [#1405](https://github.com/majiayu000/litellm-rs/pull/1405) | #1397 / F15 | `969ba9ca` | 已修复五条新增审查和实例预算归属，30 项 Realtime/隔离 Redis/clippy 通过，已推送并解决线程；新 CI 待完成；新增三条审查 eef1c949 本地完整验证中。 |
| [#1406](https://github.com/majiayu000/litellm-rs/pull/1406) | #1402 / analytics/cache | `fc72de28` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `8a253b42`。 |
| [#1408](https://github.com/majiayu000/litellm-rs/pull/1408) | #1373 / DeepSeek/xAI/Cohere | `7019848e` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `bd8db0a8`。 |
| [#1410](https://github.com/majiayu000/litellm-rs/pull/1410) | #1402 / 重复 native provider | `c8324b26` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `ba3add3f`。 |
| [#1411](https://github.com/majiayu000/litellm-rs/pull/1411) | #1402 / retry/SDK | `3ea5f99f` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `c3d0977f`。 |
| [#1412](https://github.com/majiayu000/litellm-rs/pull/1412) | #1409 / 自托管 | `da8fa630` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `aa081f3b`。 |
| [#1413](https://github.com/majiayu000/litellm-rs/pull/1413) | #1403 / F18 | `674f804f` | 保持 Draft；精确发行特性编译通过；最终候选产物/安装/协议验证及实际发布仍未完成。 |
| [#1414](https://github.com/majiayu000/litellm-rs/pull/1414) | #1373 / Perplexity | `1732c83e` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `19a18375`。 |
| [#1416](https://github.com/majiayu000/litellm-rs/pull/1416) | #1373 / Azure/Voyage | `19df4e8f` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `ed1e83ed`。 |
| [#1417](https://github.com/majiayu000/litellm-rs/pull/1417) | #1373 / fal | `319fefd9` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `5753956a`。 |
| [#1418](https://github.com/majiayu000/litellm-rs/pull/1418) | #1415 / F11 云兼容 | `c354fd6f` | 已完成两条新审查最小修复；直接 factory/credential 矩阵、27 HTTP、完整默认7132/特性10252（各1忽略，另集成/doc）及两套clippy通过，已推送并解决线程；新CI待验收。 |
| [#1419](https://github.com/majiayu000/litellm-rs/pull/1419) | #1373 / Replicate | `683bc29d` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `ca0833ee`。 |
| [#1421](https://github.com/majiayu000/litellm-rs/pull/1421) | #1420 / F11 中文兼容 | `cfbf58bf` | total-only usage 和预算回归已修，28 项联合 HTTP/clippy 通过；最新 15 项 CI 未全部完成，暂不合并/关闭 issue。 |
| [#1423](https://github.com/majiayu000/litellm-rs/pull/1423) | #1373 / Stability/BFL | `44b5f730` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `afdeb820`。 |
| [#1424](https://github.com/majiayu000/litellm-rs/pull/1424) | #1422 / 聚合供应商 | `fc11c6d8` | 该 head 的 15 项 CI 全绿、review 清零后已合并 `1723787d`。 |
| [#1426](https://github.com/majiayu000/litellm-rs/pull/1426) | #1425 / F11 地区兼容 | `cded6245` | 处理期间新增，审查指出 Baichuan >16 批次可静默丢向量及 429 混淆余额/限流；代码/文档确有对应边界缺口，保留开放，需原分支最小修复和当前 CI。 |
| [#1428](https://github.com/majiayu000/litellm-rs/pull/1428) | #1427 / F11 原生重协议 | `a5ce53a1` | 新增原 PR：Volcengine Ark embeddings 的 string→array 适配、MiniMax base 修正；原 PR 记录26 HTTP、默认7132/gateway9426（各1忽略）及clippy通过，最新CI未完成，不将原生音频/images错误协议算兼容。 |

### 当前开放 issue 的关闭条件

| Issue | 当前结论 | 必须满足的关闭条件 |
| --- | --- | --- |
| [#1372](https://github.com/majiayu000/litellm-rs/issues/1372) | F05 基础 #1374 已合并，整链未最终验收 | 接受 F07/F09 的既定范围，并复核原生工具/媒体计费、未知 usage、错误和权限；托管工具等未支持范围须明确拒绝。 |
| [#1373](https://github.com/majiayu000/litellm-rs/issues/1373) | 多批目录审核已合并，F10 总项开放 | 核验剩余全部静态目录与 authority 身份，目录/示例/价格/能力一致；账户/区域/真实付费调用未验证要保留限制。 |
| [#1378](https://github.com/majiayu000/litellm-rs/issues/1378) | Draft #1379 实现和 SQLite/Redis 分项证据已有 | 当前 main-base 完整 CI/review 通过，复核 ownership/TTL/取消/竞争/未知费用边界；不将 SQLite 测试写成实际 Postgres 重启通过。 |
| [#1382](https://github.com/majiayu000/litellm-rs/issues/1382) | Draft #1383 依赖 F07 | F07 接受后更新 base，当前主线完整/平台 CI 和 scope 验收通过；Gemini 只补已有路由契约。 |
| [#1397](https://github.com/majiayu000/litellm-rs/issues/1397) | Realtime #1405 修复已推送 | 969ba9ca 最新 CI 全绿且新增审查处理完毕，范围文档与鉴权/预算/原生事件一致后合并，由 Closes 关闭。 |
| [#1402](https://github.com/majiayu000/litellm-rs/issues/1402) | 本轮多个过期接口移除批次已合并 | 核对剩余旧 Realtime 与 subsystem_registry/README/Cargo feature/路由声明；避免把删除重复实现当成模型能力验收。 |
| [#1403](https://github.com/majiayu000/litellm-rs/issues/1403) | 发行准备 #1413 保持 Draft | 固定最终版本/候选；完整测试、跨平台、crate、干净安装、镜像及已接受协议冒烟通过；真实发行 tag/commit/checksum/digest 可查后关闭。 |
| [#1415](https://github.com/majiayu000/litellm-rs/issues/1415) | 云兼容 #1418 的两条新 review 已修，当前新 head CI 待验收 | direct factory 空 credential 和 HTTP input_type 修复已完整验证/推送，需接受新 CI/review；只接受已核验 embeddings，保留像素/原生协议限制。 |
| [#1420](https://github.com/majiayu000/litellm-rs/issues/1420) | 中文供应商 #1421 已修 usage/预算 | cfbf58bf 当前完整 CI 成功、无未解决 review 后接受明确范围；排除未支持异步/ASR/账务单位。 |
| [#1425](https://github.com/majiayu000/litellm-rs/issues/1425) | 地区兼容 #1426 新增两个有效阻断 | 原分支修复 Baichuan 批次截断和永久余额错误分类，实际 factory/Router HTTP + 错误/预算回归及当前 CI 通过。 |
| [#1427](https://github.com/majiayu000/litellm-rs/issues/1427) | 已形成新 PR #1428，当前 CI 待完成 | 接受现有 Volcengine embeddings/MiniMax base 最小适配及其明确边界，核对最新 CI/review；SambaStack-only、原生 ASR/TTS/images/partial-error 不误写成标准兼容支持。 |
| [#1429](https://github.com/majiayu000/litellm-rs/issues/1429) | 新建多模态聚合供应商核验，无 PR | 后续按 bounded issue 核验 AIML/Comet/Bytez/Poe；只接受可证明的同步端点及计费，附件/原生模型运行接口不能冒充标准 images/audio；本轮不扩展实现新批次。 |

### 当前 CI / review 快照

| PR | head | CI 成功数 | 未解决线程 | base / 合并判定 |
| --- | --- | --- | --- | --- |
| #1379 | `c44b48d4` | 15/15 | 0 | Draft；main；MERGEABLE |
| #1383 | `48f1b2be` | 1/1 | 0 | Draft；codex/responses-persistent-state-20261003；MERGEABLE |
| #1390 | `7f95cdba` | 3/3 | 4 | main；CONFLICTING |
| #1405 | `969ba9ca` | 13/15 | 3 | main；MERGEABLE |
| #1413 | `674f804f` | 14/14 | 0 | Draft；main；MERGEABLE |
| #1418 | `c354fd6f` | 0/15 | 0 | main；MERGEABLE |
| #1421 | `cfbf58bf` | 14/15 | 0 | main；MERGEABLE |
| #1426 | `cded6245` | 0/0 | 2 | main；CONFLICTING |
| #1428 | `a5ce53a1` | 1/15 | 0 | main；MERGEABLE |

#1390 为本次文档修订前的 head；本次新 head 的 docs CI 需独立验收。#1418 最新 c354fd6f 已推送，完整本地检查通过，新 head CI 独立验收。CI 快照是时间点证据，不预测之后结果。

### 接下来应如何推进

1. 先接受无依赖、范围明确且当前 CI/review 全绿的 PR；Realtime、云/中文兼容当前修复各自在原分支验收。#1426 先修两个真实协议边界，不提前开启未经证明的能力。
2. Responses 按 F07 #1379 → F09 #1383 顺序验收，依次更新 base；保留 Draft，不绕过持久账务和生命周期验收。
3. 按 #1373 的剩余逐条目录及 F11 的 bounded issue 做后续批次；#1427 为后续新实现范围。F16 只清理已到移除版本且有实际替代入口的声明/接口。
4. 最后在固定不可变提交上完成 F18 产物、安装和协议冒烟，再走真实发行；不能拿旧 ARM64 预验证或 main 合并记录代替最终发行。

### 本轮实际验证与限制

[可阅读的命令/提交/结果记录](verification-2026-10-03-pr-triage.md)及[完整原始日志、可携带清单和 SHA-256 归档](verification-2026-10-03-pr-triage.tar.gz)随本 PR 入库。每个命令注明源码提交、特性和退出码，取消尝试单列，不以临时目录作为唯一证据。

| 范围 / 提交 | 实际完成 | 不能推断的结论 |
| --- | --- | --- |
| Realtime 实现 `2ee7911b` | 默认完整 7134、特性完整 9913（各1忽略，另集成/doc），fmt/check/两套clippy通过；特性全量两线程 | 首次取消不是成功；不证明既有挂起根因已修 |
| Realtime 最新 `969ba9ca` | 30项Realtime、隔离Redis admission取消回归、同步/check/fmt/default check及两套clippy通过 | 前轮完整套件不等于新head CI通过 |
| Responses 最新整链 `48f1b2be` | 94项HTTP、fmt、同步/check、联合特性clippy通过 | 最新Main Full/Cross Platform成功；仍不能代替F07接受后改base验收 |
| 云兼容 `941357cb` / `95ea52e7` | 前者完整默认7141/特性10414（各1忽略）、两套clippy；后者27项联合HTTP/fmt/clippy通过 | 最新c354fd6f完整默认7132/特性10252（各1忽略）、27HTTP/direct factory/矩阵/两套clippy均通过；仍需新CI |
| 中文兼容 `d8346759` / `cfbf58bf` | 前者完整默认7137/特性10410（各1忽略）、两套clippy；后者28项HTTP/fmt/clippy通过 | 未测试供应商账户可用性、未做付费调用 |
| Azure `d0eb912d` / `19df4e8f` | 实际providers-extra原生107项在d0；最新116项Azure相关、6Voyage、18Responses、52Python/同步/联合clippy通过 | 新head过滤116不称全为原生模块107；未核实authority不标callable |
| 发行准备 `674f804f` | fmt、locked精确发行特性gateway compile通过 | 不是最终候选crate/image/跨架构运行或真实发行 |

本次报告编辑只校验文档、18项原验收条件、链接和归档，不因文档变更重跑Rust。所有源码构建均在本任务独立worktree；root工作目录的既有未提交文件保留。
