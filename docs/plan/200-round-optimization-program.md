# litellm-rs 200 轮持续优化计划

## 0. 目标

这不是一个“凑满 200 个小改动”的清单，而是一个面向 `litellm-rs` 的长期优化程序。

核心原则：

- 以 `core-first` 为主线，而不是先做 gateway-first 扩张。
- 以证据驱动，而不是以“看起来更优雅”驱动。
- 以边界收敛优先，而不是先做大规模物理拆 crate。
- 每轮只允许一个主假设和一个主收益指标。
- 连续 2-3 轮无证据收益时，切换优化 lane。

## 1. 新鲜验证证据

2026-04-10 本轮分析的直接证据：

- `cargo test --quiet` 通过
  - 7871 passed
  - 144 passed, 12 ignored
  - 95 passed, 33 ignored
  - doctests passed
- `cargo test --benches --no-run` 通过
  - `benches/e2e_benchmarks.rs`
  - `benches/performance_benchmarks.rs`
  均成功构建
- 代码规模现状：
  - `src/**/*.rs` 总计约 324,739 行
  - `src/core/providers/**/*.rs` 418 个文件
  - `src/sdk/**/*.rs` 18 个文件
  - `src/server/**/*.rs` 49 个文件
  - `src/core/router/**/*.rs` 22 个文件
- 明显的大文件热点：
  - `src/core/cost/calculator.rs` 1595 行
  - `src/core/providers/gemini/models.rs` 1553 行
  - `src/core/providers/vertex_ai/client.rs` 1455 行
  - `src/core/providers/base/sse.rs` 1250 行
  - `src/sdk/types.rs` 1163 行
- 结构性热点：
  - `src/sdk/router.rs` 仍是 placeholder
  - `src/sdk/client/completions.rs` 仍直接发 OpenAI/Anthropic 请求
  - 默认依赖树仍明显重于 `--no-default-features --features lite`

## 2. 迭代门禁

- 性能轮：
  - 先基线
  - 再改动
  - 再复测
  - 指标不升就丢弃该轮
- 重构轮：
  - 先补保护测试
  - 再收边界
  - 没有减少耦合、重复、公共面复杂度，不算通过
- 安全轮：
  - 必须包含负路径验证
- 构建轮：
  - 必须比较 default 和 `lite` 路径
- 大拆分轮：
  - 必须晚于测试和 benchmark 保护
- 每 10 轮：
  - 重排 backlog
  - 复核收益最高的 3 条 lane

## 3. 主题主线

- E1: Router 与 AI 执行热路径
- E2: `sdk -> core` 回归薄封装
- E3: `completion` / `UnifiedRouter` 合一
- E4: 类型合同收敛
- E5: Provider 一致性与共享执行骨架
- E6: Streaming / serialization / cache / observability 热点
- E7: Server / auth / storage / security 负路径与生命周期
- E8: Feature / build / packaging / docs 收敛
- E9: 大文件拆分与代码质量治理
- E10: 稳态化发布与持续回归

## 4. 200 轮路线

### Rounds 001-010: 建立控制面

- Round 001: 冻结默认回归命令与 benchmark 采样命令。
- Round 002: 记录 router 基线指标并归档当前热路径。
- Round 003: 为 AI route 执行骨架增加独立测量入口。
- Round 004: 盘点 `select_deployment` 调用链上的重复扫描。
- Round 005: 只优化 alias resolution 的一个热点分支。
- Round 006: 盘点 fallback 读路径的 clone 与锁成本。
- Round 007: 只优化 fallback 读路径的数据访问方式。
- Round 008: 盘点 capability lookup 的跨层重复逻辑。
- Round 009: 固化 capability-aware selection 的基准场景。
- Round 010: 回看前 9 轮收益，确认下一组热路径顺序。

### Rounds 011-020: Router 热路径第一波

- Round 011: 审计 `model_index` 与 alias 组合访问模式。
- Round 012: 降低一次 `resolve_model_name` 的多余分配。
- Round 013: 审计 `get_healthy_deployments` 的临时分配。
- Round 014: 只优化 healthy deployment 组装路径。
- Round 015: 审计 round-robin counter 的访问与初始化开销。
- Round 016: 优化一个 round-robin 读写热点。
- Round 017: 审计 record success/failure 的共享状态路径。
- Round 018: 优化一个 router metrics 原子计数使用点。
- Round 019: 为 router execute path 增加回归断言。
- Round 020: 回收未证明收益的 router 优化候选。

### Rounds 021-030: `sdk` 并回 `core` 第一波

- Round 021: 盘点 `sdk/client/completions.rs` 与 `core` 的重复能力。
- Round 022: 定义 `sdk` completion path 迁移边界。
- Round 023: 将一个非流式 completion 路径改为走 `core`。
- Round 024: 用回归测试锁定迁移前后行为一致性。
- Round 025: 删除一段 `sdk` 中重复的 OpenAI 请求拼装。
- Round 026: 删除一段 `sdk` 中重复的 Anthropic 请求拼装。
- Round 027: 为迁移后的 `sdk` path 补错误映射测试。
- Round 028: 盘点 `sdk` 里仍直接访问 provider 配置的点。
- Round 029: 收缩一个 `sdk` 对 provider 细节的直接依赖。
- Round 030: 复核 `sdk` 是否开始像 `core` 的薄封装。

### Rounds 031-040: `sdk` 并回 `core` 第二波

- Round 031: 盘点 `sdk` streaming 路径与 `core` streaming 的差异。
- Round 032: 迁移一个 streaming 入口到统一 core 能力。
- Round 033: 为 streaming 迁移补取消与结束语义测试。
- Round 034: 盘点 `sdk/router.rs` 应保留的最小职责。
- Round 035: 将 `sdk/router.rs` 从 placeholder 变成最薄适配层。
- Round 036: 删除 `sdk` 中一个无必要路由抽象。
- Round 037: 迁移一个 provider 选择逻辑到 `UnifiedRouter`。
- Round 038: 为 `sdk` 选择逻辑补基准或诊断采样。
- Round 039: 对齐 `sdk` 与 `core` 的错误返回形状。
- Round 040: 复核 `sdk` 剩余平行实现清单。

### Rounds 041-050: `completion` 与 `UnifiedRouter` 合一

- Round 041: 盘点 `core::completion` 与 `core::router` 的双入口问题。
- Round 042: 识别 `DefaultRouter` 中必须保留的兼容职责。
- Round 043: 将一个 completion 调度点改为委托 `UnifiedRouter`。
- Round 044: 为该调度改动补路由回归测试。
- Round 045: 删除一层重复的 provider registry 访问。
- Round 046: 收敛一个 completion/router 错误转换分叉。
- Round 047: 将一个 completion helper 下沉到 router/core。
- Round 048: 给 completion 主入口补热路径 benchmark。
- Round 049: 删除一段只因双入口并存而存在的代码。
- Round 050: 复核“仍保留两套路由”的剩余原因。

### Rounds 051-060: 类型合同收敛第一波

- Round 051: 盘点 `sdk/types.rs` 与 `core/types` 的重复类型。
- Round 052: 先收敛一组 chat message 类型。
- Round 053: 先收敛一组 tool/function calling 类型。
- Round 054: 先收敛一组 chat response 类型。
- Round 055: 给收敛后的类型加序列化回归测试。
- Round 056: 盘点 provider transform 自有类型的必要性。
- Round 057: 合并一个可直接复用的 transform 类型定义。
- Round 058: 删除一段仅做镜像搬运的 schema 代码。
- Round 059: 审核 public re-export，缩小 public surface。
- Round 060: 复核外部 API 是否已更清晰而非更宽。

### Rounds 061-070: Provider 一致性第一波

- Round 061: 盘点 provider execute 前置检查的重复骨架。
- Round 062: 抽出一段共享 precheck skeleton。
- Round 063: 抽出一段共享 execute_with_retry skeleton。
- Round 064: 为共享骨架补回归测试与性能对比。
- Round 065: 盘点 header 构建重复逻辑。
- Round 066: 收敛一个通用 header builder。
- Round 067: 盘点 provider HTTP error mapping 重复。
- Round 068: 用共享 mapper 替换一组重复状态码映射。
- Round 069: 盘点 provider response 解析的双重 serde 路径。
- Round 070: 删除一条高频双重 JSON 反序列化路径。

### Rounds 071-080: Provider 一致性第二波

- Round 071: 盘点连接池与 HTTP client 复用现状。
- Round 072: 统一一个 provider 的流式 client 复用方式。
- Round 073: 统一一个 provider 的非流式 client 复用方式。
- Round 074: 审计 provider capability 声明与实际实现是否一致。
- Round 075: 修一组 capability 标记与路由行为不一致点。
- Round 076: 盘点 catalog provider 的可测试性缺口。
- Round 077: 为 catalog provider 增加完整性校验测试。
- Round 078: 清理一个 provider 特有但低价值的兼容分叉。
- Round 079: 盘点 provider registry 与 factory 的职责重复。
- Round 080: 删除一层无收益的 provider 装配逻辑。

### Rounds 081-090: Streaming 生命周期与 SSE

- Round 081: 盘点 `base/sse.rs` 的 flush/cancel/backpressure 风险。
- Round 082: 修复 stream end 时的 buffer flush 语义。
- Round 083: 为 flush 修复补 provider 级回归测试。
- Round 084: 审计 SSE parser 中的 CRLF 兼容性。
- Round 085: 修一条 CRLF/partial event 解析边界。
- Round 086: 审计立即唤醒造成的 busy loop 风险。
- Round 087: 收敛一次无收益的自唤醒行为。
- Round 088: 审计 `tokio::spawn` 后的 client disconnect 生命周期。
- Round 089: 修复一个 streaming task 的取消传播缺口。
- Round 090: 为 streaming 取消与背压增加回归场景。

### Rounds 091-100: 序列化、缓存、观测热点

- Round 091: 盘点 request parsing 与 response serialization 基准场景。
- Round 092: 优化一个 request parsing 分配热点。
- Round 093: 优化一个 response serialization 分配热点。
- Round 094: 盘点 cache value 深拷贝热点。
- Round 095: 用共享所有权替换一处高频深拷贝。
- Round 096: 盘点 observability key 构建重复分配。
- Round 097: 优化一个 metrics key 组装热点。
- Round 098: 盘点 Prometheus/metrics 锁风暴问题。
- Round 099: 合并一组低价值锁分片或读写路径。
- Round 100: 回看这些热点是否真实改善 benchmark。

### Rounds 101-110: Server AI 路由变薄

- Round 101: 盘点 `server/routes/ai/*` 直接抓 core 内部的点。
- Round 102: 将一个 AI route 改为只依赖薄 service 接口。
- Round 103: 给该 route 补 HTTP 层回归测试。
- Round 104: 迁移一个 provider selection 细节出 route 层。
- Round 105: 迁移一个 request conversion 细节出 route 层。
- Round 106: 盘点 `AppState` 过宽依赖面。
- Round 107: 从 `AppState` 抽离一个 AI route 不该直持的依赖。
- Round 108: 收窄一组 route 对 pricing/storage/auth 的直接访问。
- Round 109: 对齐 route 错误返回与 core/provider 错误边界。
- Round 110: 复核 server 是否更像 adapter 而非业务核心。

### Rounds 111-120: Auth / security / storage 负路径

- Round 111: 补登录成功与失败的基础测试。
- Round 112: 补注册重复用户名与非法输入测试。
- Round 113: 补 token refresh 的有效、过期、非法测试。
- Round 114: 补 auth middleware 的缺失 header 与非法 token 测试。
- Round 115: 审计 `X-Forwarded-For` 可信代理逻辑。
- Round 116: 修一条 rate limiting 的代理信任漏洞。
- Round 117: 审计 password reset token 的事务边界。
- Round 118: 修一条 token 使用竞态。
- Round 119: 盘点 storage 层最缺的 mock/test seam。
- Round 120: 抽出一个最小 storage trait 或 mock seam。

### Rounds 121-130: Feature、构建、打包

- Round 121: 比较 default 与 `lite` 的依赖树和构建意图。
- Round 122: 重新定义 default feature 的最小目标用户。
- Round 123: 缩小一个默认 feature 负担。
- Round 124: 给 default/lite/full 三条路径补构建验证矩阵。
- Round 125: 盘点 `storage -> gateway` 的不必要耦合。
- Round 126: 剥离一段 storage-only 不该依赖 server 的代码。
- Round 127: 盘点 docs.rs feature 集合是否过宽。
- Round 128: 收紧 docs.rs feature 组合。
- Round 129: 盘点示例与 README 对重量级路径的偏置。
- Round 130: 将文档叙事改为 library-first、gateway-second。

### Rounds 131-140: Provider 分层与目录策略

- Round 131: 盘点 Tier-1 catalog 化进度与剩余候选。
- Round 132: 将一组适合 catalog 的 provider 迁到数据驱动路径。
- Round 133: 为迁移后的 catalog provider 增加完整性测试。
- Round 134: 清理一个仅因历史兼容而保留的 provider 分叉。
- Round 135: 盘点 `core/providers/mod.rs` 过宽职责。
- Round 136: 拆分一个 providers 顶层职责块。
- Round 137: 盘点 factory 与 registry 的重复职责。
- Round 138: 合并一组低价值的 provider 装配入口。
- Round 139: 盘点 provider shared utilities 的散落位置。
- Round 140: 收敛一个共享 utility 到更稳定的边界。

### Rounds 141-150: 大文件拆分第一波

- Round 141: 拆分 `src/core/cost/calculator.rs` 的一个高内聚子模块。
- Round 142: 为 cost 拆分补回归测试与基准比较。
- Round 143: 拆分 `src/sdk/types.rs` 的一个 schema 子模块。
- Round 144: 拆分 `src/core/providers/base/sse.rs` 的 parser/stream 子模块。
- Round 145: 为 SSE 拆分补行为等价验证。
- Round 146: 拆分 `src/server/routes/keys/handlers.rs` 的 handler/service 层。
- Round 147: 拆分 `src/core/providers/gemini/models.rs` 的静态模型数据。
- Round 148: 拆分 `src/core/providers/vertex_ai/client.rs` 的请求构造与发送层。
- Round 149: 审计拆分后 public API 是否变窄。
- Round 150: 删除拆分后遗留的过渡兼容层。

### Rounds 151-160: 大文件拆分第二波

- Round 151: 盘点 1000+ 行文件中的下一批高收益候选。
- Round 152: 拆分一个 `anthropic` 大文件。
- Round 153: 拆分一个 `azure` 大文件。
- Round 154: 拆分一个 `budget` 大文件。
- Round 155: 为拆分后的模块补 focused tests。
- Round 156: 复核是否只做了“物理切片”而未改善边界。
- Round 157: 删除一个拆分后已不需要的 helper。
- Round 158: 收敛一个新出现的中间层命名混乱点。
- Round 159: 补一轮 clippy/rustdoc/可读性治理。
- Round 160: 对下一波拆分重新排序，避免无效切文件。

### Rounds 161-170: 质量债与防回归

- Round 161: 盘点生产代码中的高风险 `unwrap`。
- Round 162: 替换一组高风险 `unwrap` 为上下文化错误。
- Round 163: 盘点真正不该存在的 `panic!` 路径。
- Round 164: 收敛一个 panic 到 `Result` 或 `Option`。
- Round 165: 盘点 stale TODO/FIXME 与文档债。
- Round 166: 清理一组已过期的 TODO 或转正式 issue。
- Round 167: 给最缺的 critical path 添加 negative tests。
- Round 168: 给 provider registry/catalog 补完整性与边界测试。
- Round 169: 固化一个 benchmark regression threshold。
- Round 170: 回看质量轮是否真正降低了结构风险。

### Rounds 171-180: 发布硬化与持续门禁

- Round 171: 盘点 release 前必须跑的最小验证矩阵。
- Round 172: 固化 default/lite/full 三类 smoke checks。
- Round 173: 固化 router/perf/request parsing 关键 benchmark 采样。
- Round 174: 固化 auth/security/storage 的负路径检查。
- Round 175: 审计文档与实际 feature/行为是否一致。
- Round 176: 修一条 README 与真实构建路径不一致项。
- Round 177: 修一条 examples 与真实 SDK/core 叙事不一致项。
- Round 178: 固化一个 release notes 模板。
- Round 179: 固化一个 benchmark diff 记录模板。
- Round 180: 做一次完整 dry-run 发布检查。

### Rounds 181-190: 稳态收割

- Round 181: 盘点仍残留的低价值兼容层。
- Round 182: 删除一个无收益兼容层。
- Round 183: 盘点仍残留的低价值中间抽象。
- Round 184: 删除一个无收益中间抽象。
- Round 185: 盘点文档体系中的重复分析与失效结论。
- Round 186: 合并一组重复文档或标记过期结论。
- Round 187: 复核 `sdk` 是否已基本变成 core adapter。
- Round 188: 复核 `server` 是否已基本变成 gateway adapter。
- Round 189: 复核 `providers` 是否继续向数据驱动收敛。
- Round 190: 重排最后 10 轮，只保留高价值残项。

### Rounds 191-200: 长期运维节奏

- Round 191: 做一次全量热点重测，确认前 190 轮净收益。
- Round 192: 做一次 public API surface 审计。
- Round 193: 做一次 default/lite 用户体验审计。
- Round 194: 做一次安全与配置正确性复盘。
- Round 195: 做一次 streaming 生命周期复盘。
- Round 196: 做一次 provider catalog 与实现分层复盘。
- Round 197: 下线一条连续无收益的优化 lane。
- Round 198: 提升一条证据最强的新优化 lane。
- Round 199: 固化下一周期的 20 轮高优先 backlog。
- Round 200: 产出阶段总结，重置基线，进入下一周期。

## 5. 执行建议

如果按价值排序，前 20 轮最该盯的不是“全仓都优化一点”，而是这 5 条：

1. `sdk -> core` 回归薄封装
2. `completion` / `UnifiedRouter` 合一
3. 类型合同收敛
4. router / AI 执行热路径持续 benchmark
5. server 变薄后再谈 crate 级物理拆分

## 6. 退出条件

满足以下信号时，可以认为第一周期已完成，而不是必须机械跑满 200 轮：

- `sdk` 基本不再平行实现 provider HTTP 逻辑
- completion 与 router 只剩一套主执行入口
- default 与 `lite` 的叙事、依赖、验证矩阵稳定
- router、streaming、serialization、AI route 有稳定 benchmark gate
- 高价值大文件已拆、低价值兼容层已删、回归风险下降
