# Feature Spec: 架构级重构 — Type 统一 / God Module 拆分 / Error 系统改造

## 概述

消除 litellm-rs 的三大架构债务：12+ 重复类型定义、2352 行 God Module、27 个 error type 的 lossy 转换链。目标是统一核心类型、拆分超大文件、让 error 全链路保留结构化数据直到 HTTP 响应。

## 执行顺序

```
Phase 1: God Module 拆分 (低风险，不改逻辑)
    ↓
Phase 2: Type 统一 (中风险，大量 rename/re-export)
    ↓
Phase 3: Error 系统改造 (高风险，改动转换链)
```

每个 Phase 独立可提交，不依赖后续 Phase。

---

## Phase 1: God Module 拆分

### FR-01: providers/mod.rs 按职责拆为 3 文件

**当前**: `src/core/providers/mod.rs` — 2352 行，包含 Provider enum 定义 + factory 逻辑 + 所有 provider variant 注册 + trait impl

**目标结构**:
```
src/core/providers/
├── mod.rs           (~200 行) — pub use re-exports + module declarations
├── provider_enum.rs (~400 行) — Provider enum 定义 + Display/Debug/From impls
├── factory.rs       (~600 行) — create_provider(), from_config_async() 等工厂函数
└── registry.rs      (已存在)  — PROVIDER_CATALOG 数据驱动注册
```

**约束**:
- 公开 API 不变：外部通过 `use crate::core::providers::Provider` 访问，路径不变
- mod.rs 只做 re-export，不含业务逻辑
- factory.rs 和 provider_enum.rs 之间通过 `pub(crate)` 共享内部类型

### FR-02: router/strategy_impl.rs 拆分

**当前**: `src/core/router/strategy_impl.rs` — 1183 行

**目标**: 按路由策略拆分
```
src/core/router/
├── strategy_impl.rs  (~200 行) — trait 定义 + 策略分发
├── strategies/
│   ├── round_robin.rs
│   ├── least_latency.rs
│   ├── cost_optimized.rs
│   ├── weighted.rs
│   ├── fallback.rs
│   ├── priority.rs
│   └── random.rs
```

### 验证
- `cargo check --all-features` 通过
- `cargo test --all-features` 全部通过
- 无公开 API 变更 (外部 import 路径不变)

---

## Phase 2: Type 统一

### FR-03: 消除核心层重复类型

**合并计划**:

| 保留 (canonical) | 删除/替换 | 文件 |
|------------------|-----------|------|
| `core::types::responses::Usage` | `core::providers::transform::types::Usage` | transform/types.rs → re-export |
| `core::types::responses::ChatResponse` | `sdk::types::ChatResponse`, `transform::types::ChatResponse` | sdk/ + transform/ → re-export |
| `core::types::chat::ChatRequest` | `transform::types::TransformChatRequest`, `sdk::types::SdkChatRequest` | 同上 |
| `core::types::model::ModelInfo` | `core::models::ModelInfo` | core/models/ → re-export |

**规则**:
- canonical 类型在 `core::types::` 下定义
- 其他模块通过 `pub use core::types::X` 引用
- 字段不同的类型用 `#[serde(default)]` 兼容，不强制合并

### FR-04: Provider 特有类型 → serde 适配层

**策略**: 每个 provider 保留 `transform.rs` 做 serde 映射

```rust
// src/core/providers/anthropic/transform.rs
use crate::core::types::{ChatRequest, ChatResponse};

/// Anthropic API 的 JSON 格式 (与核心类型不同)
#[derive(Serialize)]
struct AnthropicApiRequest { /* Anthropic 特有字段布局 */ }

impl From<&ChatRequest> for AnthropicApiRequest { ... }

#[derive(Deserialize)]
struct AnthropicApiResponse { /* Anthropic 特有字段布局 */ }

impl TryFrom<AnthropicApiResponse> for ChatResponse { ... }
```

**影响范围**: openai, anthropic, gemini, bedrock, azure, vertex_ai, v0 (7 个 provider 有独立类型)

### FR-05: ModelInfo 统一

**当前**: 12+ 个 ModelInfo 定义

**方案**:
- `core::types::model::ModelInfo` 为 canonical (含完整字段)
- Provider 特有的轻量 ModelInfo (如 `AzureModelInfo { deployment_name }`) 保留但改名为 `XxxDeploymentInfo` 或类似，避免命名冲突
- `services::pricing::ModelInfo` 改名为 `PricingModelInfo` 或合并到 canonical + pricing 字段

### 验证
- `cargo check --all-features` 通过
- `cargo test --all-features` 全部通过
- grep 确认每个类型只有一个 `pub struct` 定义

---

## Phase 3: Error 系统改造

### FR-06: 引入 ErrorContext 结构

```rust
// src/utils/error/context.rs
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    pub provider: Option<String>,
    pub status: Option<u16>,
    pub retry_after: Option<u64>,
    pub rate_limits: Option<RateLimits>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct RateLimits {
    pub rpm_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
    pub current_usage: Option<u64>,
}
```

### FR-07: 三层 Error 架构

```
Layer 1: 领域 Error (保留独立 enum)
├── ProviderError  (24 variants)  — provider 层专用
├── AuthError      — 认证/授权
├── McpError       — MCP 协议
├── A2AError       — A2A 协议
├── CacheError     — 缓存层
└── ConfigError    — 配置

Layer 2: GatewayError (统一入口，携带 ErrorContext)
├── Provider { kind: ProviderErrorKind, context: ErrorContext }
├── Auth { kind: AuthErrorKind, context: ErrorContext }
├── Protocol { kind: ProtocolErrorKind, context: ErrorContext }
├── Internal { message: String, context: ErrorContext }
└── ...

Layer 3: HTTP Response
├── StatusCode ← 从 kind 映射
├── Headers   ← 从 context 提取 (Retry-After, X-RateLimit-*)
└── Body      ← JSON { error: { type, message, code, metadata } }
```

### FR-08: 修复 lossy From 实现

**当前**:
```rust
// conversions.rs — 丢失 retry_after, rpm_limit 等
impl From<ProviderError> for GatewayError {
    fn from(e: ProviderError) -> Self {
        match e {
            ProviderError::RateLimit { message, .. } => GatewayError::RateLimit(message),
            // ^^ retry_after, rpm_limit, tpm_limit 全部丢失
        }
    }
}
```

**目标**:
```rust
impl From<ProviderError> for GatewayError {
    fn from(e: ProviderError) -> Self {
        match e {
            ProviderError::RateLimit { message, retry_after, rpm_limit, tpm_limit, .. } => {
                GatewayError::Provider {
                    kind: ProviderErrorKind::RateLimit,
                    context: ErrorContext {
                        retry_after,
                        rate_limits: Some(RateLimits { rpm_limit, tpm_limit, .. }),
                        ..Default::default()
                    },
                }
            }
        }
    }
}
```

### FR-09: HTTP Response 携带结构化数据

```rust
// response.rs
fn to_http_response(err: &GatewayError) -> HttpResponse {
    let status = err.status_code();
    let mut response = HttpResponse::build(status);

    // 从 ErrorContext 提取 headers
    if let Some(ctx) = err.context() {
        if let Some(retry_after) = ctx.retry_after {
            response.insert_header(("Retry-After", retry_after.to_string()));
        }
        if let Some(ref limits) = ctx.rate_limits {
            if let Some(rpm) = limits.rpm_limit {
                response.insert_header(("X-RateLimit-Limit-Requests", rpm.to_string()));
            }
        }
    }

    response.json(ErrorBody {
        error: ErrorDetail {
            r#type: err.error_type(),
            message: err.message(),
            code: err.error_code(),
        },
    })
}
```

### 验证
- `cargo check --all-features` 通过
- `cargo test --all-features` 全部通过
- 确认 RateLimit error 的 retry_after 从 provider 到 HTTP header 全链路保留
- 确认 HTTP 429 响应包含 Retry-After header

---

## 非功能需求

- NFR-01: 每个 Phase 独立可提交，不破坏中间状态的编译
- NFR-02: 公开 API (HTTP endpoints) 保持向后兼容，新增 header 不破坏现有客户端
- NFR-03: 单文件不超过 800 行 (God Module 拆分后)
- NFR-04: 每个 Phase 完成后运行 `cargo clippy --all-features -- -D warnings`

## 边界情况

- EC-01: SDK 类型 (`sdk::types`) 删除后，依赖 SDK 的外部代码需要迁移到 `core::types`
- EC-02: `core::providers::transform::types` 中的类型被多个 provider 引用，删除时需要逐个更新 import
- EC-03: Error 改造中，38 个 GatewayError variant 可能缩减到 ~10 个 (Provider/Auth/Protocol/Internal/...)，所有 match 分支需要更新
- EC-04: 部分 provider 的 transform 逻辑与核心类型字段不完全对齐，需要用 `Option<>` 或 `#[serde(default)]` 兼容

## 验收标准

- [ ] AC-01: `src/core/providers/mod.rs` < 300 行
- [ ] AC-02: `src/core/router/strategy_impl.rs` < 300 行
- [ ] AC-03: `Usage` struct 只有 1 个 canonical 定义 (其他模块 re-export)
- [ ] AC-04: `ChatResponse` struct 只有 1 个 canonical 定义
- [ ] AC-05: `ModelInfo` 无同名不同义的 struct
- [ ] AC-06: GatewayError 携带 ErrorContext，From 实现零信息丢失
- [ ] AC-07: HTTP 429 响应包含 `Retry-After` header
- [ ] AC-08: 所有测试通过 (10276 单元 + 137 集成 + 100 doc)
- [ ] AC-09: `cargo clippy --all-features -- -D warnings` 零警告

## 风险与缓解

| 风险 | 影响 | 缓解 |
|------|------|------|
| Phase 2 类型合并导致 serde 不兼容 | JSON 格式变化 | 每个 provider 保留 transform.rs 适配层 |
| Phase 3 Error variant 缩减导致 match 遗漏 | 编译错误 | Rust exhaustive match 编译器保证 |
| 跨 Phase 依赖导致中间状态不可编译 | 开发中断 | 每个 Phase 独立可提交，不依赖后续 |
| Provider transform.rs 逻辑复杂度低估 | 超时 | 先做 3 个代表性 provider，再批量推广 |

---

> **执行建议**: 在新会话中读取此 SPEC → `/vibeguard:preflight` 生成约束集 → 按 Phase 顺序实施。每个 Phase 完成后 review + test + commit。
