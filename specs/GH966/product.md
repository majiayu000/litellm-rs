# Product Spec

## Linked Issue

GH-966 / #966

complexity: medium

## 用户问题

Gemini SDK-compatible route 已通过统一路由器选出一个具体 provider deployment，但实际发送请求前又扫描
Gateway provider 配置，复制 endpoint、认证、headers 与 timeout，并创建第二个 HTTP client。路由器记录的
健康、fallback、并发 lease 与预算身份因此可能对应一个 provider，而真正收到请求的是另一个重新构造的
执行器；配置热替换或同名 deployment 会进一步放大这种身份漂移。

## 目标

- 统一路由器选中的不可变 runtime provider snapshot 是 Gemini SDK-compatible 请求的唯一执行者。
- 普通与流式 Gemini SDK 请求复用 selected provider 已构造的认证、endpoint policy、headers、timeout 与 client。
- fallback、预算、健康、并发 lease 和 spend 始终归属同一个 selected deployment/provider/model。
- 保留明确配置为 `gemini`、`googleai` 或 `googleaistudio` 的既有 OpenAI-compatible provider 兼容路径。

## 非目标

- 不改变 Gemini SDK-compatible HTTP path、请求/响应 JSON 或 SSE wire format。
- 不改变普通 OpenAI-compatible chat-completions route 的行为。
- 不新增请求级 endpoint、认证、headers 或 timeout override。
- 不重新设计 `UnifiedRouter` 的策略、重试次数、预算算法或定价数据。
- 不扩大任意 OpenAI-compatible provider 对 Gemini native wire protocol 的支持范围。

## Behavior Invariants

1. B-001 每次普通或流式 Gemini SDK-compatible 请求只能由统一路由器本次选中的 runtime provider
   snapshot 执行；route 不得再按 Gateway 配置反查或重建执行器。
2. B-002 实际出站请求必须使用 selected provider 构造时固定的 endpoint、认证、custom headers、timeout、
   endpoint access policy 与 HTTP client；路由选择后修改 Gateway 配置不得改变该请求的目标或凭据。
3. B-003 native Gemini runtime provider 必须直接执行 `v1`/`v1beta` 的 `generateContent` 与
   `streamGenerateContent`，保持请求 JSON、API key query、custom headers、SSE `alt=sse` 和响应透传语义。
4. B-004 仅明确命名为 `gemini`、`googleai` 或 `googleaistudio` 的 OpenAI-compatible runtime provider
   可以执行 Gemini native wire protocol，并使用自身不可变配置；其他 OpenAI-compatible provider 必须不可选。
5. B-005 若没有支持 Gemini native wire protocol 且匹配请求模型的 runtime deployment，route 必须返回明确
   的未配置/不支持错误，不得扫描配置、换用默认 provider 或创建临时 client 降级。
6. B-006 native URL、预算预留与 spend 必须使用 selected runtime provider 的名称和客户端原始请求的 Gemini
   model；fallback 排除、健康成功/失败与并发 lease 必须使用本次选择的 deployment id（router selection key
   仅用于定位候选）。这些身份必须属于同一次选择尝试，不得把空 `models` 兼容 deployment 的 provider 名称
   当作 Gemini 请求 model，也不得来自第二份配置。
7. B-007 普通请求的可重试 upstream 错误、provider/model 预算拒绝和未定价策略必须沿用统一路由器现有重试
   与 fallback 语义；失败执行器与被记录/排除的 deployment 必须相同。
8. B-008 流式请求在 response headers 成功后必须持有 selected deployment lease，直到流结束、读取失败或
   客户端取消时恰好释放一次；正常结束记录健康成功，上游读取失败记录健康失败，客户端取消对 upstream
   健康保持 neutral（不记成功或失败），并按取消前已观察到的 usage/输出结算 selected provider/model spend。
9. B-009 route 级 provider adapter 可以保留定价与 spend 所需的只读身份，但不得持有 API key、base URL、
   headers、timeout 或 route-owned HTTP client。
10. B-010 源码与回归测试必须阻止 Gemini SDK route 再引入 Gateway provider config scan、第二个
    `RouteHttpClient` 或 selected provider 之外的发送路径；错误不得被 warning-only 或 silent fallback 吞掉。
11. B-011 upstream 错误 body 或 URI 即使回显 selected runtime provider 的 API key，也不得把原始 key 或其
    URL-encoded 形式返回客户端、写入日志或嵌入 `ProviderError`；脱敏必须发生在敏感 body 离开 provider 前。

## 验收标准

- [ ] 普通与 SSE route 都由 selected runtime provider 直接执行，route 不再读取 `state.config().providers()`。
- [ ] native Gemini 与三个受限命名的 OpenAI-compatible 兼容实例均有正向测试，其他实例有拒绝测试。
- [ ] 路由器构造后篡改 Gateway provider endpoint/key 的测试证明实际请求仍命中 runtime snapshot。
- [ ] fallback、budget、health、lease 与 spend 身份测试覆盖成功、上游失败、预算拒绝和取消/读取失败。
- [ ] upstream error 回显 raw/URL-encoded API key 的回归测试证明客户端错误与日志候选文本均已脱敏。
- [ ] source guard、格式、编译、strict Clippy、全量测试、scope/overlap 与 PR gate 全部通过。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-005；无匹配 runtime deployment 明确失败。 |
| 错误与失败路径 | covered: B-005, B-007, B-008, B-010, B-011；不允许配置反查、凭据泄漏或静默降级。 |
| 授权/权限 | covered: B-002, B-004；认证与 endpoint policy 固定在 selected provider，兼容实例为闭集。 |
| 并发/竞态 | covered: B-001, B-002, B-008；snapshot 不受配置热替换影响，stream lease 恰好释放一次。 |
| 重试/幂等 | covered: B-006, B-007；每次尝试记录本次 selected deployment，不重建第二执行器。 |
| 非法状态转换 | covered: B-002, B-006；配置变化不能改变已选择执行器或记账身份。 |
| 兼容/迁移 | covered: B-003, B-004；wire format 与受限命名兼容路径保持。 |
| 降级/回退 | covered: B-005, B-007, B-010；仅统一路由器 fallback，禁止 route 自行降级。 |
| 证据与审计完整性 | covered: B-006, B-010, B-011；身份矩阵、snapshot mutation、脱敏与 source guard 均为必需证据。 |
| 取消/中断 | covered: B-008；stream 取消释放同一 lease、按已观察数据结算且健康 neutral；upstream 读取失败记健康失败。 |

## 发布说明

Gemini SDK-compatible route 现在直接复用统一路由器选中的 runtime provider。用户可见的 path、JSON/SSE
协议保持不变；依赖未声明 Gemini native 兼容能力的任意 OpenAI-compatible provider 将不再被该 route 误选。
