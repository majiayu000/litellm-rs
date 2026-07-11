# Product Spec

## Linked Issue

GH-959 / #959

## 用户问题

API-key 认证当前优先读取 Redis 中的完整 key 快照。撤销或权限更新即使已经成功写入数据库，只要 cache invalidation 失败，旧的 active 快照仍可在 TTL 内继续授权，导致安全撤销结果取决于 best-effort cache 删除。

## 目标

- 让数据库成为 API-key active/revoked 生命周期的单一认证权威来源。
- 保证数据库成功撤销后，任何旧 Redis 快照都不能继续通过普通或详细验证。
- 保留 cache invalidation 作为兼容性清理，但其失败不再影响认证正确性。
- 用真实 cache 删除拒绝场景证明 stale active 快照仍存在时认证会 fail closed。

## 非目标

- 不改变 owner 存在性或状态判定；该契约已由 #958 处理。
- 不改变认证基础设施错误的公开响应；该工作属于 #960。
- 不改变 user 删除、API-key 外键或 owner provenance；该工作属于 #961。
- 不改变 key hash、创建、权限、过期、last-used 或公开 API。
- 不新增数据库 migration 或新的 cache backend。

## Behavior Invariants

1. `P1`：`verify_key` 与 `verify_key_detailed` 每次都从数据库读取当前 API-key 记录；Redis 快照不参与认证决定。
2. `P2`：数据库中的 `is_active = false` 一旦可见，两条验证入口都必须拒绝该 key，即使 Redis 仍保存同一 key 的旧 active 快照。
3. `P3`：cache 删除失败可记录诊断信息并允许数据库撤销成功，但不能通过 warning、TTL 或后台清理延迟授权失效。
4. `P4`：数据库查找错误必须继续传播为系统错误；不得回退到 Redis、旧快照或普通 invalid credential。
5. `P5`：数据库返回的 active、expiry、owner 与权限数据保持现有验证语义；本变更只调整数据权威边界。
6. `P6`：遗留 Redis API-key 快照可以继续被 best-effort invalidation 清理，但新认证读取不得填充或消费该快照。

## 验收标准

- [ ] 两条 API-key 验证入口只使用数据库中的当前 key 记录。
- [ ] 回归测试真实制造 cache `DEL` 权限失败，并确认数据库撤销成功、旧 active 快照仍存在、两条验证入口均拒绝。
- [ ] cache warning 与 TTL 不再构成撤销安全保证。
- [ ] owner、expiry、last-used 与详细 invalid reason 的现有行为保持不变。
- [ ] 聚焦测试、格式、strict clippy、全量测试、scope/overlap guards 与 SpecRail gates 通过。

## 边界情况

- Redis 不可用或返回损坏数据时，认证不读取 Redis，因此结果只取决于数据库查询。
- 旧部署留下的 `api_key:hash:*` 快照不需要迁移；它们不再位于授权路径。
- 数据库撤销失败时仍返回错误，不能仅凭 cache 删除成功声称 key 已撤销。

## 发布说明

这是 fail-closed 的认证安全修复。API key 撤销以数据库状态即时生效，不再受 Redis stale snapshot 或五分钟 TTL 影响；代价是每次 key 验证都执行权威数据库读取。
