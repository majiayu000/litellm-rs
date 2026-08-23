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
    type Error = ProviderError;  // 无关联类型，方法签名直接用 ProviderError
    // ...
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

### 迁移影响

| 指标 | 迁移前 | 迁移后 | 节省 |
|------|--------|--------|------|
| 错误文件数 | 50 | 1 | **49 文件** |
| 错误代码行数 | ~10,000 | ~740 | **~9,260 行** |
| ProviderErrorTrait 实现 | 50 | 0 | **50 实现** |
| From<XxxError> 实现 | 50 | 0 | **50 实现** |
| 编译时间 | ~5 分钟 | ~2 分钟 | **~60%** |

注：上表为统一错误迁移完成时的累计统计。当前残留的 `src/core/providers/*/error.rs` 均不再是独立枚举——它们是 `pub type XxxError = ProviderError;` 别名（如 ollama、cohere、gemini）或基于 `ProviderError` 的 HTTP 错误映射器（如 bedrock、azure）。

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
- [ ] 添加到 providers/mod.rs
- [ ] 添加到 provider registry
```

### 迁移检查清单

```markdown
## 错误迁移
- [ ] 删除 error.rs 文件
- [ ] 更新 type Error = ProviderError
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

