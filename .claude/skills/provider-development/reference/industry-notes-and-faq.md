## 行业参考

### 类似系统的架构选择

| 项目 | 架构 | 理由 |
|------|------|------|
| **SQLx** | Trait Object + 统一错误 | 多数据库支持 |
| **SeaORM** | Trait Object + 统一错误 | 数据库抽象 |
| **Diesel** | 泛型单态化 | 编译时类型安全 |
| **GreptimeDB** | SNAFU 堆栈错误 | 复杂系统调试 |
| **本项目** | Trait Object + 统一错误 | 66+ provider、API 网关 |

### 性能基准来源

- [enum_dispatch crate](https://docs.rs/enum_dispatch) - 12x 性能提升数据
- [Rust Error Handling Guide 2025](https://markaicode.com/rust-error-handling-2025-guide/)
- [thiserror vs anyhow vs snafu](https://dev.to/leapcell/rust-error-handling-compared-anyhow-vs-thiserror-vs-snafu-2003)

---

## 常见问题

### Q: 为什么不用 enum dispatch？
A: 66 个 provider 会导致：
- 二进制体积 ~50MB（vs ~10MB）
- 编译时间 ~10 分钟（vs ~2 分钟）
- 每次添加 provider 需要重新编译整个枚举

### Q: 5μs 的性能损失重要吗？
A: 不重要。在典型 LLM 请求中（500ms-5000ms），5μs 占比 0.001%-0.01%，完全可忽略。

### Q: 如何处理 provider 特有的错误？
A: 使用 `ProviderError::Other { provider, message }` 或 `ProviderError::api_error(provider, status, message)`，在 message 中包含详细信息。

### Q: 需要添加新的错误变体怎么办？
A: 修改 `unified_provider.rs` 中的 `ProviderError` 枚举，添加新变体和工厂方法。这只需要改一个文件，而不是 50+ 文件。
