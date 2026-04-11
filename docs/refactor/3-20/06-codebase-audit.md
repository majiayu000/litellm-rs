# Codebase Architecture Audit (March 20, 2026)

**规模**: ~302,000 行，629 个 Rust 源文件，78+ provider 目录
**维度**: 10个（Provider、错误处理、核心模块、配置、路由、Server/Auth、存储、测试、性能并发、依赖安全）

## 严重性总览

| 严重程度 | 数量 |
|----------|------|
| Critical | 4 |
| High | 14 |
| Medium | 21 |
| Low | 8 |
| **合计** | **47** |

---

## A. Provider 架构

两层分发：Tier 1（registry/catalog.rs，53 个 OpenAI-compatible provider，纯数据）→ Tier 2（factory.rs，10 个 enum variant 的代码工厂）。

| # | 问题 | 位置 | 严重度 |
|---|------|------|--------|
| A-01 | factory.rs 1227 行，同时承担路由分发、config builder、header helper、settings 合并 5 类职责，严重违反 SRP | `src/core/providers/factory.rs` | High |
| A-02 | ProviderType::from() 未知字符串静默转 Custom，错误只在异步初始化时暴露，无早期验证 | `src/core/providers/unified_provider.rs` | Medium |
| A-03 | LLMProvider trait 关联类型 Config 导致 trait object 不可用，强迫调用方全链泛型化 | `src/core/traits/provider/` | Medium |
| A-04 | 78 个 provider 目录中大量为半迁移状态（git status DU/M），Tier 1/Tier 2 边界不清晰 | `src/core/providers/` | Medium |
| A-05 | apply_tier1_openai_like_overrides() 丢弃未知 settings key 只打 warn，用户无法感知配置被忽略 | `factory.rs` | Low |

**建议**: 拆分 factory.rs → dispatch.rs（纯分发）+ 各 Tier 2 provider 自带 Config::from_value()。

---

## B. 错误处理

两层体系：GatewayError（15 variants）← ProviderError（11+ variants）。全库 error 文件超 40 个，合计超 6400 行。

| # | 问题 | 位置 | 严重度 |
|---|------|------|--------|
| B-01 | 4160 处 .unwrap()（含 inline test），热路径 panic 风险 | 全库 | High |
| B-02 | src/core/a2a/error.rs 602 行超上限；utils/error/utils.rs 1435 行，是 catch-all 大杂烩 | `src/core/a2a/`、`src/utils/error/` | High |
| B-03 | GatewayError 刚从 29 variant 压缩为 15（commit aea82974），变体映射逻辑无任何测试保护 | `src/utils/error/` | High |
| B-04 | From<ProviderError> for GatewayError 转换过程中上下文信息（provider name、request id）部分丢失 | `provider_error_conversions.rs` | Medium |
| B-05 | utils/error/conversions.rs 1295 行、response.rs 886 行，整个 error utils 子系统 5800+ 行过度集中 | `src/utils/error/` | Medium |

---

## C. 核心模块结构

src/core/ 下 37 个子目录，35 个启用，2 个功能完整但被注释禁用。

| # | 问题 | 位置 | 严重度 |
|---|------|------|--------|
| C-01 | user_management 模块完整（8 个文件，含 UserManager/Organization/Team）但被注释禁用，等待 DB 方法实现 | `src/core/mod.rs:43-48` | Critical |
| C-02 | virtual_keys 模块完整（5 个文件，含 VirtualKeyManager/VirtualKey/Permission）但被注释禁用，是 LiteLLM 核心代理功能 | `src/core/mod.rs:43-49` | Critical |
| C-03 | 两个禁用模块不参与 cargo test，代码静默腐烂，后续启用时可能存在大量编译错误 | `src/core/user_management/`、`src/core/virtual_keys/` | High |
| C-04 | pub vs pub(crate) 比例约 83:1（3145 vs 38），模块边界形同虚设，编译器无法拦截架构越界 | `src/core/` 全局 | Medium |
| C-05 | pub use core::models::openai::* wildcard re-export，外部 API 暴露面不可静态审计 | `src/lib.rs` | Medium |
| C-06 | #[allow(dead_code)] 共 58 处，集中在 monitoring、storage/vector、token_counter，掩盖未实现代码 | 32 个文件 | Medium |
| C-07 | providers 被 387 个文件引用，是隐性上帝模块；types 被 300 个文件引用，类型未按领域分散 | `src/core/providers/`、`src/core/types/` | Medium |

### 重要 TODO 清单

| 文件:行 | 内容 |
|---------|------|
| `src/core/mod.rs:47` | 实现 DB 方法并启用 user_management、virtual_keys |
| `src/server/routes/teams.rs:97` | 将 TeamManager 存入 AppState，当前无持久化 |
| `src/server/routes/keys/handlers.rs:740` | 从 AppState 正确获取 KeyManager |
| `src/storage/redis/pubsub.rs:38,49` | 等待 Redis API 兼容性修复（2 处） |
| `src/core/cache/llm_cache.rs:299` | 语义缓存 lookup 未实现 |
| `src/monitoring/background.rs:62,66,74` | 3 处监控后台未实现（类型不匹配、时序存储、健康整合） |
| `src/monitoring/metrics/getters.rs:131-196` | 6 处系统指标未实现（memory/disk/db/redis 连接数、p50 延迟） |
| Vertex AI 模块 | vector stores(8)、batches(5)、model garden(3)、TTS(2) 全部 stub |

---

## D. 配置系统

| # | 问题 | 位置 | 严重度 |
|---|------|------|--------|
| D-01 | ${ENV_VAR} 语法在 YAML 加载时不展开：Config::from_file() 直接 serde_yaml::from_str，无预处理，gateway.yaml.example 中所有 ${OPENAI_API_KEY} 形式写法均不生效 | `src/config/mod.rs:45` | Critical |
| D-02 | 热重载是死代码：OptimizedConfigManager::enable_hot_reload() 无任何调用方，主配置加载链完全不使用它 | `src/utils/config/optimized.rs:160-219` | High |
| D-03 | gateway.rs 1143 行（超限 343 行），混合了常量定义、env 解析、配置 struct、impl 和 600 行测试 | `src/config/models/gateway.rs` | Medium |
| D-04 | storage.rs merge 逻辑通过与硬编码字符串比较判断