# Product Spec

## Linked Issue

GH-967 / #967

## 用户问题

所有 Tier-1 catalog provider 都由 `OpenAILikeProvider` 执行，但当前 provider instance 返回同一份
静态 capability 列表，其中包含 `ImageEdit`、`ImageVariation` 和 `Moderation`。Tier-1 catalog selector
没有这些 endpoint 的执行路径，route selection 因而可能选择一个随后无法映射到配置的 deployment。
显式 `openai_compatible` selector 则不同：gateway 的 image/moderation server routes 为它提供独立 proxy
执行面。当前单一列表无法表达两种事实。与此同时，现有 provider support matrix 已把通用 catalog
provider 限定为 HTTP chat/stream，形成互相矛盾的能力真值源。

## 目标

- 每个 Tier-1 catalog entry 必须显式选择 capability profile，不能继承隐式全局过度声明。
- catalog factory 创建的 runtime provider 必须携带并返回该 entry 的 capability 列表。
- capability 声明必须是 OpenAI-like 可执行 surface 的子集；不合法声明必须在构造阶段 fail closed。
- route selection 不得选择没有目标执行方法的 Tier-1 provider。
- 显式 `openai_compatible` 必须保留 image edit/variation/moderation proxy 的既有可执行能力。
- conformance tests 必须遍历全部 catalog entry，阻止 capability 与执行面再次漂移。

## 非目标

- 不新增 image edit、image variation、moderation 或其他 endpoint 实现；只保留已有显式 proxy surface。
- 不探测第三方上游或按模型动态推断 capability。
- 不修改 `LLMProvider::capabilities()` 的全局签名，也不重开 #729 的 sub-trait 方案。
- 不重写 `ProviderSurfaceSupport` 或合并 #965 的 router/provider 架构工作。
- 不保证所有第三方模型都支持 tools；本 issue 只约束 gateway 是否存在可执行方法。

## Behavior Invariants

1. 每个 Tier-1 catalog entry 必须通过命名明确的 capability profile 声明其 endpoint 能力；新增 entry
   缺少 profile 时不能编译或不能通过 catalog conformance test。
2. catalog factory 生成的 `OpenAILikeProvider` 必须返回该 `ProviderDefinition` 的 exact capability
   slice；alias 与 canonical selector 必须解析到同一份声明。
3. Tier-1 catalog profile 只包含它实际可执行的方法组：chat completion、chat stream，以及由 chat
   passthrough 执行的 tool/function calling。
4. `ImageEdit`、`ImageVariation`、`Moderation` 不得由默认或 Tier-1 profile 声明；显式
   `openai_compatible` factory 必须选择包含这三项的 proxy profile，因为 server routes 有对应执行路径。
5. 传入包含不可执行 capability 的 instance profile 必须返回 configuration error；不得删除字段、
   回退到默认列表或只记录 warning。
6. router/server 已有 canonical capability predicate 必须消费 instance 声明；请求目标 capability
   不受支持时返回 `UnsupportedCapability`，且不得执行该 deployment。
7. 直接构造的通用 `OpenAILikeProvider` 必须采用保守的可执行 profile，保持 chat/stream/tool/function
   行为，同时不重新引入 image/moderation 过度声明。
8. conformance test 必须遍历每个 catalog definition，证明其声明非空、无重复、属于可执行 surface，
   并证明 factory instance 与 definition 一致。
9. exported completion `DefaultRouter` 的环境变量注册路径也必须传递 definition slice；构造失败必须传播，
   不能静默跳过。

## 验收标准

- [ ] Tier-1 catalog 明确声明各 entry 的 capability profile。
- [ ] catalog 与直接构造的 OpenAI-like instance 都只返回可执行 capability。
- [ ] route selection 对 Tier-1 `ImageEdit`/`ImageVariation`/`Moderation` 返回 unsupported，chat 仍可选。
- [ ] 显式 `openai_compatible` 的既有 image edit/variation/moderation proxy route 保持可用。
- [ ] conformance test 覆盖全部 catalog entry 和非法声明 fail-closed 路径。
- [ ] alias、factory registry 与主 gateway factory 使用同一 capability 数据。
- [ ] 格式、PR scope/overlap guards、strict clippy、全量测试与 SpecRail packet 校验通过。

## 边界情况

- catalog alias 不得复制或覆盖 capability；它只解析到 canonical definition。
- capability slice 中的重复项属于无效 catalog 数据，必须被 conformance gate 拒绝。
- support matrix 的 public surface 可比 provider method surface 更窄，但不能把不存在的方法标为可用。
- custom OpenAI-compatible base URL 不构成新增 capability 的证据。
- capability 是否可执行按 selector/runtime route 判断，不能只检查 `LLMProvider` trait 方法。

## 发布说明

这是 fail-closed 的 provider routing 修复。Tier-1 chat/stream 与 tool/function passthrough 保持可用；
image edit、image variation 和 moderation 不再由 Tier-1 参与 route selection，同时显式
`openai_compatible` 的既有 proxy route 保持可用。
