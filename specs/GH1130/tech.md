# Tech Spec

## Linked Issue

GH-1130 / #1130

## Product Spec

见 `product.md`（B-001 ～ B-010）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Request identity | `src/server/routes/ai/context.rs`, `src/core/types/context.rs` | middleware 暴露 user、team、API key 与 admin 权限 | owner 必须来自可信 extension/context |
| Files routes | `src/server/routes/ai/files.rs` | handler 只按裸 `file_id` 操作 | 所有对象级 bypass |
| Metadata | `src/storage/files/types.rs` | `FileMetadata` 没有 owner | 持久化根因 |
| Storage dispatch | `src/storage/files/storage.rs` | store API 不接收 owner | Local/S3 必须一致 |
| Local backend | `src/storage/files/local.rs` | `.meta` JSON 持久化基础 metadata | 可向后兼容 `owner: None` |
| S3 backend | `src/storage/files/s3.rs` | 使用 object metadata，list/head 获取 metadata | owner 需稳定编码并 round-trip |
| Route tests | `tests/files_routes.rs`, `src/storage/files/tests.rs` | 只覆盖基础 CRUD | 需要租户矩阵与 legacy |

## 设计方案

1. 定义单值 `FileOwnerScope`，推荐 serde tagged enum：
   `Team(Uuid)`、`User(String)`、`ApiKey(Uuid)`。`FileMetadata.owner` 为
   `Option<FileOwnerScope>` 并带 `#[serde(default)]`，从而旧 Local metadata
   反序列化为 `None`。禁止保存三个 optional ID 并使用 OR 匹配。
2. 在 files route 的 crate-private helper 中从可信 request extensions/context 解析
   `FileCaller { auth_enforced, is_admin, effective_scope }`。有效 scope 严格按
   team -> non-empty user -> API key 优先；auth 开启且普通调用者没有 scope 时返回
   显式 auth/permission error。admin 由现有 RBAC helper 判定，不读取客户端 header。
3. 扩展 `FileStorage::{store,store_with_purpose}` 及 Local/S3 对应入口接收 owner。
   Files route 在写 content 前完成 caller 解析。Local 把 enum 写入 `.meta`；S3 使用
   一个版本化 JSON metadata key（避免三个 key 产生组合歧义），读取时严格解析。
4. 建立唯一 `can_access_file(caller, metadata)`：auth disabled 允许现有单租户行为；
   admin 允许全部；普通调用者仅当单一 effective scope 与 owner 完全相等时允许；
   `owner: None` 仅 admin。该 helper 供 list/get/content/delete 共同使用。
5. list 读取 metadata 后先过滤再构造公开 `FileObject`；任何 metadata 解析错误显式
   返回 error，禁止跳过损坏项并给出不完整/未过滤结果。公开对象不序列化 owner。
6. get/content/delete 对非 owner 使用与 missing file 相同的 `NotFound` 状态、错误
   type/code 和不含 file/owner 细节的正文。日志只记录 request ID 与拒绝类别，不记录
   owner 值。delete 必须先取 metadata 并授权，再调用 backend delete。
7. S3 的 object key 继续不可由调用者指定，owner metadata 仅由服务端写入。测试使用
   backend fixture/mock，不访问真实 AWS。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | effective scope resolver + enum | team/user/key 优先级矩阵 |
| B-002 | list filter | 多 tenant + legacy + limit 结果 |
| B-003 | uniform concealment response | foreign 与 random missing 的 status/body 等价 |
| B-004/B-005 | caller resolver/RBAC | admin 全访问；无 scope fail-closed |
| B-006/B-007 | Local/S3 serialization | round-trip、legacy、restart-style reopen |
| B-008 | FileObject/error mapping | JSON/log capture 无 owner |
| B-009 | auth-disabled branch | anonymous CRUD 兼容测试 |
| B-010 | route call order | mock backend 证明 unauthorized 不调用 get/delete |

## 数据流

auth middleware 将可信 user/API key 与 `RequestContext` 放入 request extensions。
Files handler 解析一个有效 caller scope；upload 把该 scope 随 metadata 交给 storage。
后续 list 或 object 操作先读取 metadata，使用唯一授权 helper 判定，再返回公开对象、
content 或执行 delete。Local/S3 都从持久化 metadata 恢复相同 enum；legacy `None`
只能通过 admin 路径。

## 备选方案

- 保存 user/team/key 三个字段并任一相等即允许：拒绝，会让同一 user 跨 team 绕过。
- 只绑定 API key：隔离最强但会破坏同 team/user 合法共享，且忽略已有租户层次。
- foreign 返回 `403`：拒绝，与 missing `404` 可区分并泄露对象存在性。
- 自动把 legacy 文件归给首个访问者：拒绝，存在可抢占所有权风险。
- 只在 route 内存映射 owner：拒绝，重启和多实例会丢失授权事实。

## 风险

- Security: owner scope、admin 判定和 concealment 是 auth 敏感面，必须人工 review。
- Compatibility: 历史文件普通用户将不可见；这是明确的 fail-closed 迁移。
- Performance: list 可能为每个 S3 object 做 metadata 读取；先保证正确性，优化需另开 Issue。
- Maintenance: 所有新 Files handler 必须复用 caller/ownership helper。

## 测试计划

- [ ] Unit tests: enum serde、scope priority、ownership、uniform not-found。
- [ ] Storage tests: Local/S3 owner round-trip、legacy、损坏 metadata。
- [ ] Route tests: tenant/admin/auth-disabled list/get/content/delete 矩阵与 backend call count。
- [ ] Security tests: cross-team same-user、日志/JSON owner 泄露、unauthorized delete no-op。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

代码可回滚，但新 metadata 的额外 owner 字段必须由旧反序列化器是否拒绝未知字段来决定；
回滚演练先验证兼容。不得通过忽略 owner 恢复全局访问。若必须回滚服务版本，应先暂停
Files API 或保持对象授权补丁，避免重新暴露跨租户读取。
