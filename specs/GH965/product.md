# Product Spec

## Linked Issue

GH-965 / #965

complexity: large

## 用户问题

同一个库目前存在三条用户可达、但运行时所有权不同的执行路径：HTTP gateway 由
`UnifiedRouter` 管理 provider deployment，高层 `completion()` 由全局
`DefaultRouter` 和独立 `ProviderRegistry` 执行，SDK 则持有自己的 provider 配置、统计、
选择器和 HTTP client。provider 修复、路由策略、重试以及错误映射因此可能只在某一个入口
生效；“支持矩阵一致”也不能证明请求实际经过同一个 provider instance 和 runtime state。

## 目标

- 建立唯一的 provider construction、selection 和 execution runtime contract。
- 让 HTTP、SDK 与 `completion()` 只负责各自的配置/DTO/错误外观适配，不再拥有第二套
  provider 实例、选择状态或 HTTP sender。
- 让三个公共入口在绑定同一 runtime generation 时共享 provider identity、路由状态、重试/
  fallback 与错误分类语义。
- 删除或降级 `DefaultRouter`、`Router` trait、`ProviderRegistry` 和 SDK 本地 load balancer 中
  重复的运行时所有权，同时保留经维护者批准的兼容入口。
- 用跨入口 conformance tests 和源码架构 guard 阻止执行路径再次分叉。

## 非目标

- 不重开 #725 已完成的 provider metadata、factory constructibility 或 catalog dispatch 工作。
- 不重写 #728 已完成的 route-surface support matrix，也不借本 issue 新增 provider 支持。
- 不修改 #966/#1026 的 Gemini SDK-compatible route、wire protocol、endpoint policy 或 selected
  provider identity；GH-965 只消费其合并后的 runtime contract。
- 不修改 #968 的 SSRF/endpoint policy，不允许 adapter 绕过 provider-owned secure client。
- 不处理 #519 的 type-tree、pricing 或 trait-split 其余路线图，也不以本 issue 承接 #727 的通用
  大文件拆分。
- 不扩大维护者已批准的 `HD-001` 至 `HD-004`：生命周期、request context、兼容窗口和错误映射按本
  packet 的 resolved contract 实施，不借实现便利改变选择。

## Behavior Invariants

1. B-001 每个可执行 provider instance 必须由一个 canonical construction path 创建，并注册到一个
   canonical runtime generation；任何公共入口不得把 catalog/support-matrix 声明本身当作可执行
   instance。
2. B-002 当 HTTP、SDK 与 `completion()` 绑定同一 runtime generation 并提交语义等价的 model/
   capability 请求时，它们必须得到同一 selected deployment/provider identity；入口特有的 DTO 或
   wire 格式不得改变 selection 结果。
3. B-003 一旦 canonical runtime 选定 deployment，adapter 不得重新扫描另一份 config、重新构造
   provider、再次选择 provider 或使用第二个 sender；认证、endpoint、headers、timeout 与网络策略
   必须来自同一个 selected runtime provider snapshot。
4. B-004 空 provider 集合、缺失 model、未知 provider/model、非法 provider 配置和无法构造的
   provider 必须在所有入口稳定失败；不得以环境变量、默认 provider、旧 registry 或普通 HTTP client
   静默补位。`headers`/`timeout` 只有通过 runtime policy 验证后才进入 request context；0.6.0 中已弃用的
   `api_key`/`api_base` 只能解析到当前 generation 内 policy-approved canonical deployment，不能构造、注册
   或缓存请求专属 provider/client，无法唯一匹配时必须稳定失败。
5. B-005 provider alias、model alias 与 #728 support matrix 在三个入口必须解析为相同 canonical
   identity/surface；明确 unsupported 的组合返回 unsupported，不能退化为 model-not-found、选择其他
   provider 或 generic passthrough。
6. B-006 `ProviderError` 是 provider/runtime 失败的唯一 typed source；其每个 variant 必须穷尽映射到闭集
   canonical class，区分 invalid configuration/request、authentication、unsupported、model/deployment not
   found、rate limit/budget、timeout/network、upstream unavailable、parsing/internal 和 cancellation。
   HTTP/SDK/`completion()` 可改变外层类型或 status code，但不得从字符串重分类、改变 retryability，或在
   message/log/response 中泄漏 credential、authorization/cookie header、signed URL query 和 provider body 中
   已识别的 secret。
7. B-007 health、cooldown、active lease、RPM/TPM、budget/spend、success/failure 与 latency state 必须
   由 selected canonical deployment 更新且每次 attempt 至多结算一次；adapter 本地统计不得参与下一次
   selection 或与 runtime state 产生第二真值源。
8. B-008 retry 与 fallback 必须由 canonical runtime 决定，并在每次 attempt 上记录实际 selected
   deployment；adapter 不得自行重试、改变 fallback 顺序，或在 canonical runtime 判定不可重试后再发请求。
9. B-009 并发配置替换或 provider reload 时，一个请求只能观察一个 immutable runtime generation；
   已选请求继续使用原 snapshot，新请求在新 generation 原子发布后使用新 snapshot，不得混用旧 key 与新
   endpoint、旧 selection 与新 state。
10. B-010 streaming 在首个可见输出前的失败遵循 B-008；首个输出后的失败、调用方取消与正常结束必须
    释放同一 selected lease，并分别记录 failure、neutral cancellation 或 success，禁止 adapter 发起隐藏
    retry 或重复结算。
11. B-011 在 staged migration 中，`LLMClient`、`ClientConfig` 与高层 free functions 必须保留为 canonical
    runtime 的无状态 facade；`DefaultRouter`、completion `Router` trait、mutable `ProviderRegistry` runtime
    ownership 及 request-level `api_key`/`api_base` 在 0.6.0 标记 deprecated，并只可委托 canonical runtime，
    在 0.7.0 移除。0.7.0 removal 合并前，版本工作流必须经独立 fixture 证明 0.x breaking policy 产出 0.7.0
    而非 1.0.0；迁移说明、替代 API 或此前置证据缺失时 removal 必须 blocked。
12. B-012 完成证据必须以同一组确定性 fixtures 经 HTTP、SDK 与 `completion()` 验证 provider identity、
    unsupported/error class、retry/fallback、snapshot isolation、stream/cancel 和 exactly-once state；另有
    source guard 证明 production adapter 中不存在第二 provider map、config rescan、local routing state 或
    第二 sender。只通过 support matrix/unit test 不算完成。

## 验收标准

- [x] `HD-001` 至 `HD-004` 已由维护者在 issue comment `4982855807` 批准，并由本 amendment 固化 exact
  API、映射、迁移窗口和 rollback；production implementation 仍需本 amendment review/merge 后开始。
- [ ] 代码中只有一个 provider construction + deployment selection + execution runtime contract。
- [ ] HTTP、SDK 与 `completion()` 的 adapter 不持有第二 provider instance map、路由统计或 sender。
- [ ] 重复 `DefaultRouter`/`Router`/`ProviderRegistry` 状态已删除或变为经批准的无状态兼容 facade。
- [ ] 0.6.0 发布弃用标记与迁移说明；0.7.0 removal 前版本工作流 fixture 证明 breaking 0.x 可显式发布
  0.7.0 且不会伪装成 non-breaking commit。
- [ ] 跨入口 conformance fixtures 覆盖 B-001 至 B-012，且 source guard 对 production 为零命中。
- [ ] 每个 implementation PR 满足最多 10 个非文档文件、500 changed lines，并通过格式、构建、
  strict Clippy、全量测试、scope/overlap、review threads 与 PR gate。

## 边界情况清单

| 类别 | 判定（covered: B-xxx / N/A + 原因） |
| --- | --- |
| 空/缺失输入 | covered: B-004；空 provider、缺失 model 与缺失 override 语义均需显式。 |
| 错误与失败路径 | covered: B-004, B-005, B-006, B-008, B-010；构造、选择、执行、stream 失败均有闭集语义。 |
| 授权/权限 | N/A：本 issue 不改变用户鉴权或私网授权；runtime secret/endpoint policy 继续受 #968 约束。 |
| 并发/竞态 | covered: B-007, B-009, B-010；generation、lease 与结算不得跨 snapshot。 |
| 重试/幂等 | covered: B-007, B-008, B-010；attempt 选择与 state exactly-once。 |
| 非法状态转换 | covered: B-007, B-009, B-010；旧/新 generation 和 stream terminal state 不得混用。 |
| 兼容/迁移 | covered: B-011；0.6.0 deprecation、0.7.0 removal 与 release-policy prerequisite 已锁定。 |
| 降级/回退 | covered: B-003, B-004, B-005, B-008；禁止 config/client fallback 和 adapter retry。 |
| 证据与审计完整性 | covered: B-012；三入口 fixture 与 source guard 缺一即未完成。 |
| 取消/中断 | covered: B-009, B-010；取消释放同一 lease 且不得伪装 success/failure。 |

## Human Decisions

维护者于 issue comment `4982855807` 批准 recommended matrix；以下状态均为 `resolved`，实现者不得重新选择：

| ID | Resolved decision | Observable consequence |
| --- | --- | --- |
| `HD-001` | HTTP 与每个 SDK instance 显式持有 `Arc<UnifiedRouter>`；仅高层 free functions 使用可替换的 process-default binding。 | replacement 原子发布新 immutable generation；已取得 handle 的 in-flight request 完成于旧 generation。未安装 default 时 free functions 返回 typed configuration error，不从 env 隐式初始化。 |
| `HD-002` | `headers`/`timeout` 保留为 validated request-scoped context；`api_key`/`api_base` 在 0.6.0 deprecated，0.7.0 removed。 | legacy pair 只可匹配当前 generation 内 policy-approved canonical deployment；零匹配、多匹配、被 endpoint/header policy 拒绝均 fail closed，且不得创建请求专属 provider/client。 |
| `HD-003` | 永久保留 `LLMClient`、`ClientConfig` 和高层 free functions 为 stateless facade；`DefaultRouter`、completion `Router` trait、mutable `ProviderRegistry` runtime ownership 在 0.6.0 deprecated、0.7.0 removed。 | 0.6.0 facade 也不得 fallback 到 legacy engine；0.7.0 removal 前必须先修订并验证 release workflow 的 0.x breaking policy。 |
| `HD-004` | `ProviderError` 是 canonical typed runtime error；HTTP、SDK、Gateway、retry/redaction/cancellation 使用 tech spec 的 exhaustive mapping。 | adapter 从 typed variant 映射并保留 class/retryability；字符串只能作为已 redacted 的展示文本，不能参与分支。 |

## 发布说明

这是 staged architecture migration。0.6.0 release note 必须列出全部 deprecated symbol/field、替代 API、
request-context 规则和 error mapping；0.7.0 release note 必须列出实际 removal。当前 version-bump workflow
把 0.x breaking commit 计算为 1.0.0；在 workflow 与 deterministic fixture 显式支持批准的 0.7.0 policy 前，
任何 removal tranche 均不得合并，也不得用 non-breaking commit label 隐藏 breaking change。任何阶段都不能
把未迁移入口留在独立 runtime 上作为“临时 fallback”。
