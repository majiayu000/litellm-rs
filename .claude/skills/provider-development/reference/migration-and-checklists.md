## Contents

- 迁移现有 Provider
- 检查清单

## 迁移现有 Provider

### 迁移步骤

1. **删除 error.rs 文件**
2. **修改 provider.rs**：

```rust
// 修改前（XxxError 为 provider 专属错误枚举）
use super::error::XxxError;

impl LLMProvider for XxxProvider {
    type Error = XxxError;
    // ...
}

// 创建错误
Err(XxxError::AuthenticationError("Invalid key".into()))
Err(XxxError::NetworkError(e.to_string()))
```

```rust
// 修改后
use crate::core::providers::unified_provider::ProviderError;

impl LLMProvider for XxxProvider {
    async fn chat_completion(/* ... */) -> Result<ChatResponse, ProviderError> {
        // ...
    }
}

// 创建错误
Err(ProviderError::authentication("xxx", "Invalid key"))
Err(ProviderError::network("xxx", e.to_string()))
```

### 错误映射对照表

| 旧模式 | 新模式 |
|--------|--------|
| `XxxError::AuthenticationError(msg)` | `ProviderError::authentication("xxx", msg)` |
| `XxxError::RateLimitError { retry_after }` | `ProviderError::rate_limit("xxx", retry_after)` |
| `XxxError::ModelNotFoundError(model)` | `ProviderError::model_not_found("xxx", model)` |
| `XxxError::InvalidRequestError(msg)` | `ProviderError::invalid_request("xxx", msg)` |
| `XxxError::NetworkError(msg)` | `ProviderError::network("xxx", msg)` |
| `XxxError::TimeoutError(msg)` | `ProviderError::timeout("xxx", msg)` |
| `XxxError::ApiError(status, msg)` | `ProviderError::api_error("xxx", status, msg)` |
| `XxxError::NotSupported(feature)` | `ProviderError::not_supported("xxx", feature)` |

### 迁移边界

当前 `LLMProvider` 没有关联 `Error` 类型，方法签名直接返回
`ProviderError`。不要在新实现中写 `type Error = ProviderError`。也不要机械删除
所有 `error.rs`：现存文件可能保留公开类型别名，或承载基于 `ProviderError`
的 HTTP 映射逻辑；先搜索调用方并保持公开 API。

---

## 检查清单

### 新 Provider 检查清单

```markdown
## 配置
- [ ] 实现 ProviderConfig trait
- [ ] 包含 validate() 方法
- [ ] 支持环境变量
- [ ] 有合理的默认值

## Provider 实现
- [ ] 使用 ProviderError（不是自定义错误类型）
- [ ] 实现所有必需的 LLMProvider 方法
- [ ] HTTP 错误正确映射
- [ ] 支持流式传输（如适用）

## 模型信息
- [ ] 定义支持的模型
- [ ] 包含定价信息
- [ ] 正确指定能力

## 质量
- [ ] 无 unwrap() 调用
- [ ] 错误消息清晰
- [ ] 所有错误包含 provider 名称
- [ ] 单元测试覆盖错误映射

## 注册
- [ ] Tier 1：添加 catalog 条目和 `providers/mod.rs` 注释
- [ ] Tier 2：添加模块、`Provider` 枚举/dispatch 和 factory 分支
```

### 迁移检查清单

```markdown
## 错误迁移
- [ ] 搜索 `error.rs` 调用方；仅在无别名/映射/API 用途时删除
- [ ] 删除旧的关联 `type Error` 声明；方法直接返回 ProviderError
- [ ] 替换所有 XxxError::Variant 为 ProviderError::factory()
- [ ] 删除 ProviderErrorTrait impl
- [ ] 删除 From<XxxError> impl
- [ ] 更新 ErrorMapper（如需要）

## 测试
- [ ] 所有测试通过
- [ ] 错误类型编译正确
- [ ] HTTP 错误映射正常工作
```

---
