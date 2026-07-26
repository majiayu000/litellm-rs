# Product Spec

## Linked Issue

GH-1131 / #1131

complexity: low

## 用户问题

示例配置和校验注释声称 response cache 没有接入请求路径，但运行时已经在
非流式 chat 与 embeddings 中读取和写入缓存。操作员可能把启用缓存误认为
无效果，并在不知情时改变响应来源。

## 目标

- 让示例配置与代码注释准确描述当前 response cache 接线。
- 明确 chat 与 embeddings 的适用范围和差异。
- 明确 budgeted key、`store: true`、context bypass、`ttl: 0` 等关键行为。
- 不改变任何运行时缓存行为。

## 非目标

- 修改 cache key、隔离、淘汰、TTL 或存储后端。
- 接入 semantic cache 或新增缓存端点。
- 为现有静默 bypass 增加日志；该行为只在本 Issue 中如实记录。
- 改变流式请求、预算或 `store` 语义。

## Behavior Invariants

1. B-001 文档必须说明 `cache.enabled: true` 且 `ttl > 0` 时，非流式 chat 与 embeddings 会进行确定性 response cache lookup/store。
2. B-002 文档不得再把已接线的 response cache 描述为“未实现”或“无效果”。
3. B-003 文档必须说明 chat 在 per-key budget、`store: true` 或 context bypass 条件下同时跳过 lookup 与 store。
4. B-004 文档必须说明 embeddings 没有与 chat 相同的 budget bypass，避免暗示两个端点行为一致。
5. B-005 文档必须说明 `enabled: true` 与 `ttl: 0` 不会启用缓存，并会产生显式配置错误日志。
6. B-006 未接线的 semantic cache 仍必须被准确标记，不能因修正文案而宣称其已可用。
7. B-007 所有修改仅限文档和注释，运行时行为及默认值保持不变。

## 验收标准

- [ ] `config/gateway.yaml.example` 准确描述启用条件、端点范围和 bypass。
- [ ] `GatewayConfig::validate` 附近注释不再声称 response cache 未接线。
- [ ] 文案与 AppState、chat、embeddings、response_cache 和 cache config 的当前实现逐项核对。
- [ ] 示例 YAML 仍可解析并通过配置验证。
- [ ] `cargo fmt --check`、`cargo check` 和相关 config 测试通过。

## 边界情况

- 所有 key 都带 budget 的部署中，chat 命中率为零，但 embeddings 仍可能命中。
- `cache.enabled` 与 `semantic_cache.enabled` 代表不同成熟度。
- `ttl: 0` 与 `enabled: false` 都不会构造有效缓存，但原因和日志不同。

## 发布说明

仅修正文档；升级不会改变缓存运行时行为。操作员应重新检查已有
`cache.enabled: true` 配置是否符合预期。
