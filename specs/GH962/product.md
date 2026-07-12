# Product Spec

## Linked Issue

GH-962 / #962

## 用户问题

网关已经接受 legacy OpenAI `functions` 与 `function_call` 字段，并把它们保存在 canonical
chat request 中，但 OpenAI 与 OpenAI-compatible provider 发往上游的请求会静默丢弃这两个字段。
调用方看到请求被正常接受，却无法让上游执行所声明的 legacy function contract。

## 目标

- OpenAI 与 OpenAI-compatible provider 都完整转发调用方提交的 legacy function 字段。
- 缺失字段保持缺失，不向上游伪造 `null`、默认 function 或现代 tool 配置。
- 修复仅改变此前被静默丢弃的 outbound payload，不改变认证、路由或响应语义。

## 非目标

- 不把 legacy `functions` / `function_call` 自动转换成 `tools` / `tool_choice`。
- 不改变 HTTP 请求 DTO、canonical chat request 类型或 legacy 字段的校验规则。
- 不扩展其他 provider、response delta、provider capability 或 supported-parameter 声明。
- 不承诺所有 OpenAI-compatible 上游都支持 legacy function calling；上游可按其协议返回错误。

## Behavior Invariants

1. `GH962-P1`：调用方提供 `functions` 时，OpenAI outbound JSON 包含同名字段，数组内容与顺序保持不变。
2. `GH962-P2`：调用方提供 `function_call` 时，OpenAI outbound JSON 保留原始字符串或对象结构。
3. `GH962-P3`：OpenAI-compatible outbound JSON 对 `functions` 与 `function_call` 满足与 OpenAI 相同的转发契约。
4. `GH962-P4`：两个字段可同时出现，且不会覆盖或删除现代 `tools` 与 `tool_choice`。
5. `GH962-P5`：调用方未提供 legacy 字段时，outbound JSON 不包含对应 key。
6. `GH962-P6`：typed legacy 字段是 canonical 值；同名 provider extra parameter 不得覆盖显式 typed 值。

## 验收标准

- [ ] OpenAI mock upstream 收到与输入完全一致的 `functions` 与 `function_call`。
- [ ] OpenAI-compatible mock upstream 收到相同字段和值。
- [ ] 测试覆盖字符串形式与对象形式的 `function_call`，并证明两个字段可同时转发。
- [ ] 测试证明字段缺失时不会生成对应 key，且 typed 值不会被同名 extra parameter 覆盖。
- [ ] 聚焦测试、格式、编译、Clippy 与全量测试通过。

## 边界情况

- `functions: []` 是显式空数组，必须保留为 `[]`；它不同于未提供字段。
- `function_call` 可为 legacy 字符串选择器，也可为命名函数对象；两种 JSON 形状都必须保持。
- 上游拒绝 legacy 字段时，沿用现有 provider error mapping，不静默删除字段后重试。

## 发布说明

OpenAI 与 OpenAI-compatible chat 请求现在会按调用方输入转发 legacy `functions` 和
`function_call`；现代 tool calling 与其他 provider 行为不变。
