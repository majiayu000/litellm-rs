# Tech Spec

## Linked Issue

GH-1130 / #1130

## Product Spec

见 `product.md`（B-001 ～ B-010）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Request identity | `src/auth/{user_management,system}.rs`, `src/auth/jwt/{types,handler}.rs`, `src/server/routes/auth/{models,login,token}.rs`, `src/server/routes/ai/context.rs`, `src/core/types/context.rs` | 一个内部 login 路径签发 first-team，JWT 认证读取也重新选择 first-team；HTTP login/refresh 没有 active-team 输入；旧 token 无 provenance；空权限 key 会回退 admin owner | 增加服务端验证的 login/refresh team selection 与版本化 claim，旧 guessed claim 安全降级；Files admin 必须把任何 API key 当 attenuation boundary |
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
   `FileCaller { auth_enforced, is_admin, effective_scope }`：
   - API key 请求的 active team 只能是已验证 `ApiKey.team_id`。
   - `LoginRequest` 与 `RefreshTokenRequest` 增加 optional snake_case `team_id`。
     login/refresh handler 读取数据库中的 current user memberships；缺失时签发
     user-scope token，存在时只在 exact membership match 后签发，否则稳定拒绝且
     不生成 token。`AuthSystem::login` 同步接受 optional selection 并复用同一
     validator，禁止 `user.team_ids.first()`。
   - access-token `Claims` 增加 `#[serde(default)] team_scope_version: Option<u8>`；
     本 tranche 唯一接受值为 `Some(1)`。只有通过上述 validator 的 typed
     `VerifiedActiveTeam` 才能同时写入 `team_id` 与 version 1；无选择的新 token
     写 `team_id: None`/version 1。旧 token 因字段缺失解码为 `None`：即使其中有
     历史 guessed `team_id`，认证也忽略该 team，但 token 仍作为 user-scope 身份。
     version 1 的 team 仍须在认证时再次验证 current membership；失败时拒绝 token，
     不得静默切换其他 team。不得新增或信任客户端自报 header。
   - API key 存在时，admin 只能由该 key 的直接 `*`/`system.admin` 或 runtime
     `is_admin`/等价 admin permission 授予。把 `context.rs` 现有 direct/runtime
     admin parser 暴露为唯一 crate-private helper 供 Files caller 复用；只要 API key
     存在，受限、空 permissions、runtime payload absent 都不得回退到 key owner 的
     用户角色。无 API key 的 JWT/session 才按 canonical user admin role/RBAC 判定。
   有效 scope 严格按 trusted team -> non-empty user -> API key 优先；auth 开启且
   普通调用者没有 scope 时返回显式 auth/permission error。
3. 扩展 `FileStorage::{store,store_with_purpose}` 及 Local/S3 对应入口接收 owner。
   Files route 在写 content 前完成 caller 解析。Local 把 enum 写入 `.meta`；S3 使用
   一个版本化 JSON metadata key（避免三个 key 产生组合歧义），读取时严格解析。
4. 建立唯一 `can_access_file(caller, metadata)`：auth disabled 允许现有单租户行为；
   admin 允许全部；普通调用者仅当单一 effective scope 与 owner 完全相等时允许；
   `owner: None` 仅 admin。该 helper 供 list/get/content/delete 共同使用。
5. list 读取 metadata 后先过滤再构造公开 `FileObject`；任何 metadata 解析错误显式
   返回 error，禁止跳过损坏项并给出不完整/未过滤结果。当前 route 没有 query
   pagination/limit/count，公开响应只有 `object` 与 `data`，因此返回全部已授权文件，
   不新增 public limit。Local/S3 storage listing 必须返回完整候选集：S3 循环
   `ListObjectsV2` continuation token 直到 `is_truncated=false`，检测 token
   缺失/重复并显式失败；Files route 保持调用 `list(None, None)`，不得把内部
   `max_keys` 当公开截断。公开对象不序列化 owner。
6. get/content/delete 对非 owner 使用与 missing file 相同的 canonical `NotFound`
   状态、错误 type/code 和不含 file/owner 细节的正文。S3 `HeadObject` mapper 使用
   Rust SDK `HeadObjectError::is_not_found()` 判定服务 HTTP 404；HEAD 错误响应没有
   可依赖的 body/精确 exception，不得解析 `NoSuchKey` 字符串。transport、
   credential、timeout、403、5xx 与其他 service errors 保持显式 5xx，禁止
   blanket 映射。route 对 foreign 与 storage NotFound 使用同一公开 404 mapper。
   日志只记录 request ID 与拒绝类别，不记录 owner 值。delete 必须先取 metadata
   并授权，再调用 backend delete。
7. S3 的 object key 继续不可由调用者指定，owner metadata 仅由服务端写入。测试使用
   backend fixture/mock，不访问真实 AWS。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | token selection/provenance + effective scope resolver + enum | login/refresh valid/foreign/no team、version 1、legacy no-marker、API-key team、multi-team no-claim、user/key fallback |
| B-002 | paginated list + filter | 多 tenant + legacy + >1000 S3 objects；返回全部 authorized data，无 public limit/count |
| B-003 | uniform concealment response | foreign 与 Local/S3 random missing 的 status/body 等价 |
| B-004/B-005 | caller resolver/canonical RBAC | key direct/runtime admin、restricted key + admin owner、admin JWT、无 scope |
| B-006/B-007 | Local/S3 serialization + error mapper | round-trip、legacy、restart、S3 404/非 404 |
| B-008 | FileObject/error mapping | JSON/log capture 无 owner |
| B-009 | auth-disabled branch | anonymous CRUD 兼容测试 |
| B-010 | route call order | mock backend 证明 unauthorized 不调用 get/delete |

## 数据流

login/refresh 接收 optional `team_id` 并验证 current membership；token issuer 只把
typed verified selection 与 `team_scope_version=1` 写入 JWT，否则写
`team_id: None`。旧 access token 缺 version 时忽略历史 team claim并保持 user-scope
认证。auth middleware 将可信 user/API key 与 `RequestContext` 放入 request
extensions；version 1 JWT active-team claim 只有在再次验证 membership 后才进入
Files caller，签发/读取两端都不能复用 `team_ids.first()`。Files handler 解析一个
有效 caller scope；upload 把该 scope随 metadata 交给 storage。
后续 list 或 object 操作先读取 metadata，使用唯一授权 helper 判定，再返回公开对象、
content 或执行 delete。S3 list 先遍历全部 continuation pages，再由 route 过滤并返回
完整 authorized set；Local/S3 都从持久化 metadata 恢复相同 enum；legacy `None`
只能通过 admin 路径。

## 备选方案

- 保存 user/team/key 三个字段并任一相等即允许：拒绝，会让同一 user 跨 team 绕过。
- 只绑定 API key：隔离最强但会破坏同 team/user 合法共享，且忽略已有租户层次。
- foreign 返回 `403`：拒绝，与 missing `404` 可区分并泄露对象存在性。
- 自动把 legacy 文件归给首个访问者：拒绝，存在可抢占所有权风险。
- 只在 route 内存映射 owner：拒绝，重启和多实例会丢失授权事实。

## 风险

- Security: owner scope、admin 判定和 concealment 是 auth 敏感面，必须人工 review。
- Security: API key 是 permission attenuation boundary；不得让 admin user 身份抬高
  受限/空权限 key 权限，也不得把任意 team membership 当 active team。旧 token 的
  guessed team 必须因缺 version marker 被忽略。
- Compatibility: 历史文件普通用户将不可见；这是明确的 fail-closed 迁移。旧 access
  token 继续作为 user-scope 身份有效，但不再保留 guessed team scope；需要 team
  scope 的客户端通过 login/refresh 的 optional `team_id` 获取新 token。
- Performance/Availability: 完整的跨租户授权列表要先遍历/过滤全部 S3 candidates，
  可能增加大量分页与逐对象 `HeadObject`，在大 bucket/高并发下可能触发
  throttling 或 `503 Slow Down` 并使整个 list 显式失败。实现应保持 bounded
  concurrency/retry 与 fail-closed，不得为可用性跳过 metadata 或返回部分未过滤
  结果；可扩展性需另开 Issue，以按 owner 建立可信索引或缓存（例如数据库/Redis），
  且索引失效时仍回到安全失败而非全局放行。
- Maintenance: 所有新 Files handler 必须复用 caller/ownership helper。

## 测试计划

- [ ] Unit tests: enum serde、login/refresh team membership、versioned/legacy claims、trusted active-team scope priority、empty/restricted key attenuation、ownership、uniform not-found。
- [ ] Storage tests: Local/S3 owner round-trip、legacy、损坏 metadata、HeadObject SDK `is_not_found()`/非 404、>1000 object pagination，以及 throttling/503 保持显式失败。
- [ ] Route tests: tenant/admin/auth-disabled list/get/content/delete 矩阵、完整 authorized list 与 backend call count。
- [ ] Security tests: login/refresh valid/foreign/no-team selection、legacy no-marker token 回退 User scope、version 1 active-team claim、cross-team same-user、restricted/empty key + admin owner、日志/JSON owner 泄露、unauthorized delete no-op。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`；S3 feature 用 `cargo check --all-features`、`cargo clippy --all-targets --all-features -- -D warnings` 与相关 S3 tests。

## 回滚方案

代码可回滚，但新 metadata 的额外 owner 字段必须由旧反序列化器是否拒绝未知字段来决定；
回滚演练先验证兼容。不得通过忽略 owner 恢复全局访问。若必须回滚服务版本，应先暂停
Files API 或保持对象授权补丁，避免重新暴露跨租户读取。
