---
name: provider-development
description: LiteLLM-RS Provider 开发与架构指南。用于添加新 provider（Tier 1 catalog 条目或 Tier 2 代码实现）、统一错误处理与架构选型对比分析。适用于新增 provider 实现、把旧错误枚举迁移到统一 ProviderError、或做架构方案评估时使用。
---

# LiteLLM-RS Provider 开发指南

## 架构概述

本项目采用**统一错误 + Trait Object**架构，这是经过对比分析后的最佳选择。

### 当前架构层次

```
┌────────────────────────────────────────────────────────┐
│                    网关层 (Gateway)                     │
│  LiteLLMError = GatewayError（18 个变体）               │
│  - 别名定义: core/types/errors/litellm.rs              │
│  - 枚举定义: utils/error/gateway_error/types.rs        │
│  - 处理路由、配置、认证等网关级错误                      │
└────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────┐
│                   Provider 层                          │
│  ProviderError（经 unified_provider 模块导出，         │
│  定义于 unified_provider_error.rs）                     │
│  - 统一 provider 错误，24 个变体                        │
│  - 每个变体包含 provider: &'static str 字段            │
│  - 丰富的工厂方法和上下文信息                           │
└────────────────────────────────────────────────────────┘
                          ↓
┌────────────────────────────────────────────────────────┐
│              各 Provider 实现（两层结构）                │
│  - Tier 1: registry/catalog.rs 目录条目（def_chat 等），│
│    经 OpenAILikeProvider 路由，无专属代码               │
│  - Tier 2: 代码型 provider 目录，                       │
│    type Error = ProviderError                          │
└────────────────────────────────────────────────────────┘
```

Provider 数量随版本演进，不在此硬编码。枚举方法：

```bash
grep -c 'def_chat(\|def_local_chat(' src/core/providers/registry/catalog.rs  # Tier 1 catalog 条目数
ls -d src/core/providers/*/                                                  # Tier 2 代码型目录
```

---

## 架构方案对比

### 方案一：Enum Dispatch（编译时多态）

```rust
#[enum_dispatch]
pub enum Provider {
    OpenAI(OpenAIProvider),
    Anthropic(AnthropicProvider),
    // ... 数十个变体
}
```

| 指标 | 评估 |
|------|------|
| 方法调用延迟 | ~480ns（最快） |
| 二进制体积 | 大（每个变体单态化一份代码） |
| 编译时间 | 慢 |
| 运行时扩展 | 不支持 |
| 适用场景 | 封闭类型集、高频调用 |

**结论**：不适合大量 provider 场景，二进制膨胀严重

### 方案二：Trait Object + 统一错误（动态派发）✅ 当前采用

```rust
// src/core/traits/provider/llm_provider/trait_definition.rs
pub trait LLMProvider: Send + Sync + Debug + 'static {
    // 无关联 Error 类型：方法签名直接返回统一错误 ProviderError
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError>;

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>>;
}
```

| 指标 | 评估 |
|------|------|
| 方法调用延迟 | ~5,900ns |
| 二进制体积 | 小（~10MB） |
| 编译时间 | 快（~2分钟） |
| 运行时扩展 | 支持 |
| 适用场景 | 多后端系统、API 网关 |

**结论**：最适合本项目，5μs 延迟在 10ms+ 网络延迟中可忽略（0.05%）

### 方案三：混合模式（分层派发）

```rust
// 热路径：高频 provider 用 enum dispatch
enum FastProvider { OpenAI, Anthropic, Azure, ... }
// 冷路径：其余走 trait object
type ExtendedProvider = Box<dyn LLMProvider>;
```

| 指标 | 评估 |
|------|------|
| 加权延迟 | ~1,500ns（80/20 分布） |
| 复杂度 | 高（两条代码路径） |
| 适用场景 | 极致性能要求 |

**结论**：备选方案，增加复杂度但性能更好

### 方案四：完全单态化

```rust
pub async fn completion<P: LLMProvider>(provider: &P, ...) { ... }
```

| 指标 | 评估 |
|------|------|
| 方法调用延迟 | ~0ns（内联） |
| 二进制体积 | 巨大（泛型代码逐 provider 拷贝） |
| 编译时间 | 非常慢 |

**结论**：不实用，provider 数量增长会导致二进制爆炸

### 方案五：SNAFU 堆栈错误（GreptimeDB 模式）

```rust
#[derive(Debug, Snafu)]
pub enum ProviderError {
    #[snafu(display("Auth failed: {source}"))]
    Authentication { source: Box<dyn Error>, backtrace: Backtrace },
}
```

| 指标 | 评估 |
|------|------|
| 错误上下文 | 丰富（堆栈、回溯） |
| 性能开销 | 堆分配 + 回溯收集 |
| 适用场景 | 复杂系统调试 |

**结论**：可选增强，但当前 `ProviderError` 已足够

---

## 性能对比总结

| 方案 | 调用延迟 | 二进制体积 | 编译时间 | 扩展性 | 推荐度 |
|------|----------|------------|----------|--------|--------|
| Enum Dispatch | ~480ns | 大 | 慢 | 无 | ⭐⭐ |
| **Trait Object** | ~5,900ns | **小** | **快** | **有** | ⭐⭐⭐⭐⭐ |
| 混合模式 | ~1,500ns | 中 | 中 | 有 | ⭐⭐⭐⭐ |
| 完全单态化 | ~0ns | 巨大 | 极慢 | 编译时 | ⭐ |
| SNAFU | ~5,900ns | 小 | 快 | 有 | ⭐⭐⭐⭐ |

### 为什么 5μs 差异不重要

```
典型 LLM API 请求延迟分解：
├── 网络往返：         50-200ms
├── Provider API 处理： 500-5000ms
├── 序列化/反序列化：   0.1-1ms
├── 路由决策：          0.01-0.1ms
└── 派发开销：          0.005ms (5μs)  ← 占比 0.001%-0.01%
```

---

## 统一错误类型详解

### ProviderError 变体

```rust
pub enum ProviderError {
    // 认证与授权
    Authentication { provider, message },

    // 限流与配额
    RateLimit { provider, message, retry_after, rpm_limit, tpm_limit, current_usage },
    QuotaExceeded { provider, message },

    // 模型与请求
    ModelNotFound { provider, model },
    InvalidRequest { provider, message },

    // 网络与可用性
    Network { provider, message },
    Timeout { provider, message },
    ProviderUnavailable { provider, message },

    // 功能支持
    NotSupported { provider, feature },
    NotImplemented { provider, feature },
    FeatureDisabled { provider, feature },

    // 内容与长度
    ContextLengthExceeded { provider, max, actual },
    TokenLimitExceeded { provider, message },
    ContentFiltered { provider, reason, policy_violations, potentially_retryable },

    // 配置与序列化
    Configuration { provider, message },
    Serialization { provider, message },

    // 高级错误
    ApiError { provider, status, message },
    DeploymentError { provider, deployment, message },
    ResponseParsing { provider, message },
    RoutingError { provider, attempted_providers, message },
    TransformationError { provider, from_format, to_format, message },
    Streaming { provider, stream_type, position, last_chunk, message },
    Cancelled { provider, operation_type, cancellation_reason },

    Other { provider, message },
}
```

### 工厂方法使用

```rust
// 基础工厂方法
ProviderError::authentication("openai", "Invalid API key")
ProviderError::rate_limit("anthropic", Some(60))
ProviderError::model_not_found("groq", "llama-invalid")
ProviderError::network("azure", "Connection timeout")

// 增强工厂方法
ProviderError::rate_limit_with_limits("openai", Some(60), Some(100), Some(40000), None)
ProviderError::context_length_exceeded("claude", 100000, 150000)
ProviderError::content_filtered("openai", "Violence detected", Some(vec!["violence"]), Some(false))
ProviderError::streaming_error("fireworks", "chat", Some(42), None, "Connection reset")
```

---

## 添加新 Provider

### 先判断 Tier

- **Tier 1（OpenAI 兼容、无需定制逻辑）**：只需在 `src/core/providers/registry/catalog.rs` 加一条 `def_chat("name", "Display Name", "https://api.example.com/v1", "NAME_API_KEY")`，工厂自动经 `OpenAILikeProvider` 路由，无需新建目录（本地部署类用 `def_local_chat`）。
- **Tier 2（自定义请求转换、认证签名、非 SSE 流式协议、专属模型元数据等）**：按下文创建代码目录。

### 目录结构

```
src/core/providers/my_provider/
├── mod.rs           # 模块导出
├── config.rs        # ProviderConfig 实现
├── provider.rs      # LLMProvider 实现
├── model_info.rs    # 模型定义和能力
└── streaming.rs     # SSE 流解析（可选）
```

### 配置实现

实现 `crate::core::traits::provider::ProviderConfig`（定义于 `src/core/traits/provider/config.rs`，必需方法：`validate` / `api_key` / `api_base` / `timeout` / `max_retries`）。参考真实实现：`src/core/providers/cloudflare/config.rs`。

```rust
// config.rs
use crate::core::traits::provider::ProviderConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyProviderConfig {
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_timeout() -> u64 { 60 }
fn default_max_retries() -> u32 { 3 }

impl Default for MyProviderConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("MY_PROVIDER_API_KEY").ok(),
            api_base: None,
            timeout: default_timeout(),
            max_retries: default_max_retries(),
        }
    }
}

impl ProviderConfig for MyProviderConfig {
    fn validate(&self) -> Result<(), String> {
        self.validate_standard("my_provider")
    }

    fn api_key(&self) -> Option<&str> { self.api_key.as_deref() }
    fn api_base(&self) -> Option<&str> { self.api_base.as_deref() }
    fn timeout(&self) -> std::time::Duration { std::time::Duration::from_secs(self.timeout) }
    fn max_retries(&self) -> u32 { self.max_retries }
}
```

### Provider 实现（使用统一错误）

`LLMProvider` trait（`src/core/traits/provider/llm_provider/trait_definition.rs`）没有关联类型：方法签名直接使用 `ProviderError`，错误映射通过 `get_error_mapper()` 提供；trait 方法是原生 `async fn`，实现时无需 `#[async_trait]` 宏。

```rust
// provider.rs
use crate::core::providers::base::{GlobalPoolManager, HttpMethod, header};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::{LLMProvider, ProviderConfig};
use crate::core::types::{
    chat::ChatRequest, context::RequestContext, model::{ModelInfo, ProviderCapability},
    responses::ChatResponse,
};

const PROVIDER_NAME: &str = "my_provider";

const MY_CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ChatCompletion];

#[derive(Debug, Clone)]
pub struct MyProvider {
    config: MyProviderConfig,
    pool_manager: Arc<GlobalPoolManager>,
    models: Vec<ModelInfo>,
}

impl MyProvider {
    pub async fn new(config: MyProviderConfig) -> Result<Self, ProviderError> {
        config.validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?
        );

        Ok(Self { config, pool_manager, models: get_models() })
    }
}

impl LLMProvider for MyProvider {
    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        MY_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        let api_key = self.config.api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key required"))?;

        let headers = vec![
            header("Authorization", format!("Bearer {}", api_key)),
            header("Content-Type", "application/json".to_string()),
        ];

        let response = self.pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !response.status().is_success() {
            return Err(self.map_http_error(response.status().as_u16(), &body));
        }

        // ...
    }
}

impl MyProvider {
    fn map_http_error(&self, status: u16, body: &str) -> ProviderError {
        use crate::core::providers::shared::parse_retry_after_from_body;
        match status {
            401 => ProviderError::authentication(PROVIDER_NAME, "Invalid API key"),
            404 => ProviderError::model_not_found(PROVIDER_NAME, body),
            429 => ProviderError::rate_limit(PROVIDER_NAME, parse_retry_after_from_body(body)),
            400 => ProviderError::invalid_request(PROVIDER_NAME, body),
            500..=599 => ProviderError::provider_unavailable(PROVIDER_NAME, body),
            _ => ProviderError::api_error(PROVIDER_NAME, status, body),
        }
    }
}
```

### 模型信息

```rust
// model_info.rs
use crate::core::types::model::{ModelInfo, ProviderCapability};

pub fn get_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "my-model-large".to_string(),
            name: "My Model Large".to_string(),
            provider: "my_provider".to_string(),
            max_context_length: 128000,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: false,
            input_cost_per_1k_tokens: Some(0.01),
            output_cost_per_1k_tokens: Some(0.03),
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::ToolCalling,
            ],
            ..Default::default()
        },
    ]
}
```

### 注册 Provider

```rust
// src/core/providers/my_provider/mod.rs
mod config;
mod model_info;
mod provider;

pub use config::MyProviderConfig;
pub use provider::MyProvider;
pub use model_info::get_models;
```

Tier 2 provider 接入闭合枚举（无法运行时注册，需以下 crate 内改动，参考 `cloudflare` 的接线方式）：

1. `src/core/providers/mod.rs`：`pub mod my_provider;` + `Provider` 枚举加变体 `MyProvider(my_provider::MyProvider)` + `name()` 映射分支
2. `src/core/providers/provider_type.rs`：`ProviderType` 枚举加变体并支持 `From<&str>` 解析
3. `src/core/providers/factory/registry.rs`：`ProviderType::MyProvider => ...` 工厂分支

---

## References

- [reference/migration-and-checklists.md](reference/migration-and-checklists.md) — 迁移现有 provider 到统一错误的步骤、错误映射对照表与迁移检查清单
- [reference/industry-notes-and-faq.md](reference/industry-notes-and-faq.md) — 行业架构选择参考、性能基准来源与常见问题解答
