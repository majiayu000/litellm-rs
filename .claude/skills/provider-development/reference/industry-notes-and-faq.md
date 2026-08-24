## 当前架构说明

本项目使用闭集 `Provider` 枚举派发和统一 `ProviderError`：

- Router deployment 保存 `src/core/providers/mod.rs` 中的 `Provider` 枚举；
  `enum_dispatch` 将调用转发给具体 provider。
- `LLMProvider` 是各具体实现共享的接口，不是路由层的 trait-object 插件边界。
- Tier 1 OpenAI 兼容端点通过 catalog 复用 `Provider::OpenAILike`；Tier 2
  provider 需要增加模块、枚举/dispatch 和 factory wiring。
- 仓库没有支持具体派发延迟、二进制体积或编译耗时对比的 benchmark，文档不应
  给出这类数字。

---

## 常见问题

### Q: 如何处理 provider 特有的错误？

A: 优先选择语义匹配的 `ProviderError` 工厂方法，例如 `authentication`、
`rate_limit`、`invalid_request` 或 `not_supported`。只有没有更具体分类时才使用
`ProviderError::api_error(provider, status, message)` 或 `Other`；不要创建新的
provider 私有错误枚举。

### Q: 需要添加新的错误变体怎么办？

A: 枚举定义在 `src/core/providers/unified_provider_error.rs`，工厂方法和辅助行为
主要在 `unified_provider_methods.rs`。新增变体还要检查 HTTP 映射、失败分类、
脱敏、格式化和穷举 match；先搜索 `ProviderError::` 的现有处理点，不能假定只改
一个文件即可。
