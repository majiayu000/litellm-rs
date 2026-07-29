# Tech Spec

## Linked Issue

GH-1130 / #1130

## Product Spec

见 [`product.md`](./product.md)（B-001 ～ B-019）。

## Codebase Context

以下锚点均在本 amendment 基线
`ae26d9d54254124b0942d3b7765ef67ea6fdc9e6` 上核对。

| Area | Verified anchors | Current behavior | Why relevant |
| --- | --- | --- | --- |
| JWT claims / public issuer | `src/auth/jwt/types.rs:34-59`; `src/auth/jwt/handler.rs:25-58,92-133`; `src/auth/jwt/tokens.rs:10-155`; `src/auth/jwt/utils.rs:58-76` | public `Claims` 无 provenance version；public issuer 直接接受 raw `Option<Uuid>` team；specialized tokens 共用 `Claims` | 保持 public `Claims` shape，用 internal access envelope 承载 marker；specialized token 文件无需修改 |
| AuthSystem login / JWT read | `src/auth/user_management.rs:29-69`; `src/auth/system.rs:85-153`; `src/auth/tests.rs:260-316` | internal login 与认证读取都使用 `user.team_ids.first()`；数据库 `Err` 已通过 `?` 保留 | 共享 current-active membership validator 与 `VerifiedActiveTeam` 的唯一创建点 |
| HTTP login / refresh | `src/server/routes/auth/models.rs:16-21,49-53`; `src/server/routes/auth/login.rs:69-185`; `src/server/routes/auth/token.rs:10-82`; `src/server/routes/auth/mod.rs:16-45` | public request DTO 没有 `team_id`；routes 直接注册 public handlers；refresh 重载 user 后未检查 active，且 RBAC `Err` 被 `unwrap_or_default` 吞成空权限 | private wire DTO/route adapter 接收 selection，固定 primary-auth-before-team 顺序和 active-user 401，public DTO/handler signature 保持 |
| Middleware status boundary | `src/server/middleware/auth.rs:211-295,333-369` | `AuthSystem::authenticate` 的 `Err` 已是 generic 500；但 API-key endpoint-policy parse `Err` 被公开成 403 与内部细节 | 保留认证 401/5xx 分流，收紧 policy/check error 为 generic 5xx |
| Canonical team state | `src/core/teams/repository.rs:14-40`; `src/core/models/team/member.rs:45-76`; `src/server/state.rs:31-48,84-86` | repository 已提供 `get`/`get_member`，team/member 都有 active 状态；AppState 已持有同一数据库上的 manager | 复用既有 canonical repository；这些文件无需修改，不新增平行 membership 存储 |
| API-key policy / identity | `src/server/routes/ai/context.rs:86-185,214-246`; `src/core/types/context.rs:61-80,140-159` | bool permission helpers用 `matches!` 吞 runtime-policy parse error；context 的 user 字段是 String，但 authenticated User/API key IDs 本身是 UUID | Files 使用 result-returning checked helper，只从 typed principal 构造 UUID scope；`RequestContext` 结构无需改 |
| Files routes | `src/server/routes/ai/files.rs:60-171`; `src/server/routes/ai/mod.rs:43,155-166` | 五个 public handlers 无 request proof 且直接注册；upload 调用 legacy store；读删只按裸 `file_id` | 保留 public signatures 但 auth-enabled fail closed；HTTP routes 改注册 private proof-aware adapters |
| Metadata / dispatch | `src/storage/files/types.rs:14-32`; `src/storage/files/storage.rs:41-101`; `src/storage/mod.rs:326-334` | public `FileMetadata` 无 owner；`metadata` 与三个 public store 入口不提供 owner | 保持 public struct literal 与方法签名，owner 只进入 internal persisted envelope/additive API |
| Local backend | `src/storage/files/local.rs:34-80,137-225,278-310` | content 先写入 final 可枚举路径，随后才写 `.meta`；两步间失败会留下 content orphan；list 的 `Some(limit)` 是总结果上限 | 增加 presence-aware owner envelope 与 private staged publication，final content 只在完整 metadata 后可见 |
| S3 backend | `src/storage/files/s3.rs:104-150,217-254,256-312,314-353` | owner 未写 metadata；metadata HEAD 把所有错误映射 Internal；list 只取第一页并把 `max_keys` 当完整 limit | 严格 owner metadata、精确 HEAD 404、完整 continuation 与跨页总 limit |
| Existing test surfaces | `src/storage/files/tests.rs:1-78`; `tests/files_routes.rs:1-80`; `tests/integration/auth_middleware_tests_parts/mod.rs:1-202`; `tests/integration/auth_middleware_tests_parts/rejection_rate_limit.rs:1-225`; `tests/public_api_compat.rs:1-64` | 已有 storage CRUD、Files route、middleware 500 与 public API compile harness | 扩充 security/status/source-boundary/compatibility 矩阵，不访问真实 AWS |

## Planned Changes

以下是完整实现边界，恰好 24 个 repository-relative path。实现若必须修改表外文件，
先停止并提交新的 spec amendment；不得把未申报文件塞入实现 PR。

```specrail-planned-changes
{
  "issue": 1130,
  "complete": true,
  "paths": [
    "src/auth/jwt/types.rs",
    "src/auth/jwt/handler.rs",
    "src/auth/jwt/tests.rs",
    "src/auth/user_management.rs",
    "src/auth/system.rs",
    "src/auth/tests.rs",
    "src/server/routes/auth/models.rs",
    "src/server/routes/auth/login.rs",
    "src/server/routes/auth/token.rs",
    "src/server/routes/auth/mod.rs",
    "src/server/routes/ai/context.rs",
    "src/server/middleware/auth.rs",
    "src/server/routes/ai/files.rs",
    "src/server/routes/ai/mod.rs",
    "src/storage/files/mod.rs",
    "src/storage/files/types.rs",
    "src/storage/files/storage.rs",
    "src/storage/files/local.rs",
    "src/storage/files/s3.rs",
    "src/storage/files/tests.rs",
    "tests/files_routes.rs",
    "tests/integration/auth_middleware_tests_parts/mod.rs",
    "tests/integration/auth_middleware_tests_parts/rejection_rate_limit.rs",
    "tests/public_api_compat.rs"
  ],
  "spec_refs": [
    "B-001",
    "B-002",
    "B-003",
    "B-004",
    "B-005",
    "B-006",
    "B-007",
    "B-008",
    "B-009",
    "B-010",
    "B-011",
    "B-012",
    "B-013",
    "B-014",
    "B-015",
    "B-016",
    "B-017",
    "B-018",
    "B-019"
  ]
}
```

## 设计方案

### 1. 单值 UUID owner

在 `src/storage/files/types.rs` 定义 crate-private adjacent-tag enum，wire 必须精确为：

```text
#[serde(tag = "scope", content = "id", rename_all = "snake_case")]
FileOwnerScope = Team(Uuid) | User(Uuid) | ApiKey(Uuid)

{"scope":"team","id":"<uuid>"}
{"scope":"user","id":"<uuid>"}
{"scope":"api_key","id":"<uuid>"}
```

public `FileMetadata` 的字段集合保持原样。owner wire 使用 crate-private、
presence-aware state，只有 JSON key 缺失才产生 `Absent`：

```text
OwnerField = Absent | Present(FileOwnerScope)

StoredFileMetadata {
  #[serde(flatten)] public: FileMetadata,
  #[serde(
    default,
    skip_serializing_if = "OwnerField::is_absent",
    serialize_with = "serialize_owner_field",
    deserialize_with = "deserialize_owner_field"
  )]
  owner: OwnerField
}
```

field-level `default` 只处理 key absence；key 存在时 custom decoder 必须把 valid
adjacent-tag object 解成 `Present`，并对 explicit `null`、malformed object、unknown
scope 或 invalid UUID 返回 error，不能产生 `Absent`。serializer 对 `Absent` 省略整个
key，对 `Present` 只写 exact adjacent-tag object。Local 新 `.meta` 仍把既有 public
fields 放在顶层；旧 bare `FileMetadata` 解码为 `Absent`，旧 reader 也可忽略新 owner
字段。internal API 可把 `Absent` 投影为 legacy `owner: None`，但损坏值不得进入该投影。
S3 internal metadata result 同样返回 `{ public, owner }`，public `metadata()` 只投影
`public`。不保存三个 optional ID，不做 OR matching，不从任意 String 制造 User
owner。Files caller 只使用已认证 `User::id()`、`ApiKey.user_id`、`ApiKey.team_id`
与 `ApiKey.metadata.id` 的 UUID。

### 2. Provenance 三态与唯一 typed issuer

public `Claims` 字段集合保持不变。`src/auth/jwt/types.rs` 新增 crate-private、
presence-aware marker 与 access envelope：

```text
TeamScopeMarker = Absent | Present(u8)

AccessTokenClaims {
  #[serde(flatten)] public: Claims,
  #[serde(
    default,
    skip_serializing_if = "TeamScopeMarker::is_absent",
    serialize_with = "serialize_team_scope_marker",
    deserialize_with = "deserialize_team_scope_marker"
  )]
  team_scope_version: TeamScopeMarker
}
```

与 owner 一样，field-level `default` 只在 key absence 时产生 `Absent`；key 存在时
custom decoder 只接受 integer `u8` 并产生 `Present`。explicit `null`、string、
fractional/out-of-range number 或其他 malformed value 是 JWT decode/auth error。
新 access-token issuer 永远写 `Present(1)`；`Absent` 只来自 legacy decode。

只有 access-token encode/decode 使用该 envelope；refresh/password-reset/email/
invitation 继续直接使用 public `Claims`，因此 `jwt/tokens.rs` 与 `jwt/utils.rs` 不在
manifest。public `verify_access_token` 的签名不变并只返回其中的 `Claims`；新增
crate-internal `verify_access_token_with_provenance` 供 `AuthSystem` 使用完整 envelope。
access-token 状态机固定为：

| Marker / team claim | Authentication result |
| --- | --- |
| key `Absent` / any `team_id` | legacy；忽略 team，active user 成功后只建立 User scope |
| `Present(1)` / `None` | active user 的 User scope |
| `Present(1)` / `Some(team_id)` | current team 与 membership 都 active 时建立 Team scope；missing/inactive/stale 为 401 |
| `Present(v != 1)` / any `team_id` | 401；在 scope fallback 前拒绝整个 bearer token |
| key present with `null`/non-integer/malformed value | decode/auth 401；不得产生 `Absent` 或进入任何 scope fallback |

在 `src/auth/system.rs` 定义
`VerifiedActiveTeam { user_id: Uuid, team_id: Uuid }`：类型可供 crate-internal typed
issuer 接受，但两个字段与 constructor 都私有于 `auth::system` module。唯一 constructor
由共享 validator 在 canonical `TeamRepository::get` 与 `get_member` 均成功、
`Team::is_active()` 与 `TeamMember::is_active()` 都为真、member/user UUID exact match
后调用。repository `Err` 原样传播；
成功读取但 team/member 不存在或不 active 由调用边界分类：

- primary auth 与 active-user gate 已成功的 login/refresh 显式 selection：400，且在任何
  token encode 前结束；
- version 1 bearer token 的复验：认证失败 401；
- repository/反序列化/连接错误：generic 5xx。

`JwtHandler::{create_access_token,create_token_pair}` 的 public 参数和返回类型不改：
raw `team_id: None` 签发 version 1 User token；raw `team_id: Some` 在 encode 前返回
显式错误。新增 crate-internal typed issuer，只接受 `&VerifiedActiveTeam`，并在
encode 前强制 `proof.user_id == token subject`；不相等时显式拒绝，不能把 user A 的
proof 配给 user B。校验通过后才签发 version 1 Team token。生产 login/refresh
只能走 typed path；没有第二个 raw-team encode helper。

`AuthSystem::login(&str, &str)` 保留原签名并成为 no-selection User-scope 入口；
新增 additive `login_with_active_team`，它必须先完成与 public login 相同的 password/
active-user gate，再与 HTTP login/refresh 共用同一个 membership validator。所有
`team_ids.first()` 签发/读取路径删除。

### 3. HTTP selection 与错误分类

public `LoginRequest`、`RefreshTokenRequest` 字段集合和既有 public login/refresh
handler signatures 保持不变。新增 private wire DTO：

```text
LoginWireRequest {
  #[serde(flatten)] public: LoginRequest,
  team_id: Option<Uuid>
}
RefreshWireRequest {
  #[serde(flatten)] public: RefreshTokenRequest,
  team_id: Option<Uuid>
}
```

`src/server/routes/auth/mod.rs` 的 HTTP registration 只指向 private wire-aware
adapters；existing public handlers 继续作为 no-selection wrappers，调用同一个
internal implementation。private wire DTO 的 JSON/UUID syntactic parse 可以先发生；
parse 成功后，team selection 的 semantic validation 必须遵循固定顺序：

1. login 先完成既有 user/password verification 与 active-user gate；wrong password
   或 user gate failure 立即返回既有 generic auth response。
2. refresh 先验证 refresh token，再重载 user 并要求 `user.is_active()`；missing 或
   inactive user 统一 generic 401。
3. 只有上述 primary verification 与 user gate 成功后，才可调用 shared team/member
   validator；随后才读取 RBAC 并 encode token。

因此 wrong password 或 invalid refresh 搭配 valid/foreign `team_id` 时，primary auth
结果优先，team/member repository call count 与 token encode count 都为 0。inactive/
missing refresh user 在有/无 `team_id` 时都在 team/RBAC/encode 前返回 401。team
repository error 也只能在 primary auth 成功后对外成为 generic 5xx，不能覆盖错误
credential/token 的结果。其余 wire behavior 为：

- 字段缺失保持兼容，签发 version 1 User token；
- JSON/UUID malformed 由 request boundary 返回 400，不查询 team repository；
- primary auth 成功后的 well-formed foreign/missing/inactive/suspended selection
  统一 generic 400；
- primary auth 成功后的 user/team/membership/RBAC storage `Err` 返回 generic 5xx；
- 任一失败都不得生成 access 或 refresh token。

refresh 删除 `unwrap_or_default`；RBAC `Err` 不能变成空 permissions。错误顺序测试必须
同时断言 wrong-password/invalid-refresh 两类请求各自搭配 valid/foreign selection，
以及 inactive/missing refresh user 搭配 team Some/None 的 call counts。

认证 middleware 已将 `AuthSystem::authenticate` 的 `Err` 映射 generic 500，保留该
边界。它对 `api_key_allows_endpoint` 的 parse/check `Err` 改为同样的 generic 500，
不再返回含内部 policy 细节的 403。普通 deny 仍为 403，无效 credential/token 仍为
401。

### 4. API-key attenuation 与 caller resolver

在 `src/server/routes/ai/context.rs` 增加 result-returning crate-private checked
helper，复用现有 direct permissions 与 runtime-policy parser：

- API key 存在时，只有该 key 的 direct `*`/`system.admin` 或成功解析的 runtime
  `is_admin`/等价 custom permission 可授予 Files admin；
- restricted、empty、runtime payload absent 都是 non-admin，且不得回退 key owner
  的 Admin/SuperAdmin role；
- malformed runtime policy 是 `Err`，由 middleware/Files route 变成 generic 5xx，
  不能伪装 non-admin 后继续 fallback；
- 无 API key 的 JWT/session 才按 canonical user role/RBAC 判定。

`FileCaller { auth_enforced, is_admin, effective_scope }` 的 principal 分支互斥：

1. request extensions 存在 `ApiKey` 时，不读取任何 JWT/context residual team。
   先验证 context 的 API-key/user identity 与 typed key exact match；然后只在该 key
   内按 `team_id -> user_id -> metadata.id` 取一个 UUID scope。
2. 不存在 `ApiKey` 时，要求 typed authenticated `User` 与 context user UUID exact
   match；只有 `AuthSystem` 复验后写入的 JWT team 才优先于 `User::id()`。
3. typed principal/context mismatch、缺失的必需 identity 或 invalid UUID 都是
   consistency/storage error，返回 generic 5xx；不得尝试下一种 principal/scope。

本 Issue 把已认证且 active 的 API key 上持久化 `team_id` 视为 key 自身的 trusted
team association，不把它当 key owner 的 active-team selection，也不查询 owner
membership。team 删除与 key association 撤销/同步属于独立 lifecycle Issue；该风险
不允许通过读取 residual JWT team 来“修复”。

### 5. Additive storage API

以下 public 方法和 public `FileMetadata` struct shape 保留，并继续代表显式
non-HTTP legacy/admin/migration caller，写 `owner: None`：

- `FileStorage::{store,store_with_purpose,metadata}`
- `LocalStorage::{store,store_with_purpose,metadata}`
- `S3Storage::{store,store_with_purpose,metadata}`
- `StorageLayer::{store_file,get_file}`（该文件不改）

新增 owner 非 optional 的 crate-internal
`store_owned_with_purpose(..., owner: FileOwnerScope)`，以及对应 Local/S3 dispatch
；另新增 crate-internal `metadata_with_owner` 返回 `StoredFileMetadata`。
auth-enabled Files HTTP upload 在读取
multipart content 后、backend write 前解析 caller，并且只能调用 owned 方法；scope
失败不得调用 legacy store。auth-disabled + allow-anonymous route 才走兼容入口。

Local legacy 与 owned store 共享一个 crate-private、same-filesystem staged writer；
public signatures 和 legacy `Absent` wire 均不改变。写入协议固定为：

1. 先在 base path 下、且位于现有 two-character shard/list traversal 之外的 private
   staging location 生成 content 与完整 `StoredFileMetadata`；temp names 不能被
   `list` 当作 file ID。
2. content 与 metadata 都 write-complete 并 close 后，才进入 publish。先把 staged
   metadata 原子 rename 到 final `<file_id>.meta`，再把 staged content 原子 rename
   到 final `<file_id>`；两次 rename 位于同一 filesystem，final content rename 是
   唯一 list-visible commit point。
3. metadata serialize/write/close/rename 任一步失败时绝不 publish final content；
   同步错误对本次 staging 与已发布 sidecar 做 best-effort cleanup 后返回 error。
   content commit 失败同样返回 error并清理可清理的 sidecar。
4. 进程在 commit 前中断只可能留下 private staging 或 meta-only sidecar；reopen/list
   必须忽略（并可安全清理）两者。由于 final content 永不先于 final metadata 出现，
   metadata failure 或中断不能产生可枚举的 ownerless content、也不能永久 poison list。

owned path 的 envelope 必须是 `OwnerField::Present(valid scope)`；legacy public path
才可写 `OwnerField::Absent` 并省略 key。读取时 owner key explicit `null` 或 malformed
由 presence-aware decoder 返回 generic 5xx，不得进入 legacy/admin-visible flow。
Local public/internal metadata 与对象读取以 final content 存在作为 committed
precondition：已知 ID 指向 meta-only sidecar 时按未提交记录忽略/NotFound，不得公开
sidecar 内容；final content 存在但 metadata 缺失或损坏时仍显式失败，不能降级为
legacy。
这些 Local 规则不改变 S3 wire 或 publish behavior。S3 metadata key 固定为
`litellm-owner`，value 固定为：

```json
{"version":1,"scope":"team|user|api_key","id":"<uuid>"}
```

S3 只有整个 key 缺失才表示 legacy `owner: None`；value malformed、unknown version、
unknown scope、missing/extra required field 或 invalid UUID 都是 5xx。public metadata
result 不含 owner，不得用三个独立 metadata key 拼接 owner。

### 6. 完整 listing 与统一授权

建立唯一 `can_access_file(caller, metadata)`：

- auth disabled：保持现有单租户行为；
- admin：允许所有 owner 与 legacy `None`；
- 普通 caller：只有 effective scope 与 metadata 的单一 owner 完全相等才允许；
- legacy `None`：auth-enabled 时仅 admin。

list 必须先得到完整候选集、逐个读取并验证 metadata，再构造公开
`OpenAiFileObject`。任一 metadata failure 使整个请求 5xx，不 skip、不返回部分 data。
Files route 继续调用 `list(None, None)`，因为公开 API 没有 limit/count。

S3 `ListObjectsV2` 对每一页保持同一个 prefix，并循环 continuation token 到
`is_truncated=false`：

- `Some(0)` 不发 S3 请求并返回空结果；
- `None` 返回所有 candidates；每页 `max_keys=1000`；
- `Some(limit > 0)` 是跨页总上限；每页
  `max_keys=min(1000, remaining)`，达到后停止；
- truncated page 必须提供 fresh、non-empty next token；缺失、空值或重复 token
  显式失败；
- 任意 later-page error 使整个 list 失败，不得返回已经收集的 prefix/部分结果。

Local 的 existing `Some(limit)` 总上限语义保持不变，并修正为 `Some(0)` 直接返回空
结果、零目录扫描。所有 limit 模式都只遍历 final content namespace，忽略 private
staging 与 `.meta` sidecar；meta-only interruption residue 不能成为 candidate。

### 7. Concealment 与精确 S3 NotFound

get/content/delete 都先取 metadata 并授权；delete 只有授权成功后才调用 backend
delete。foreign owner 与真实 missing 使用同一 canonical OpenAI-compatible 404
status、type、code 和不含 file/owner 细节的正文。

S3 metadata/exists 共用一个 HeadObject error mapper。只有 SDK modeled
`is_not_found()` 或 raw service response status 404 生成 canonical
`GatewayError::NotFound`；不得读取或假设 HEAD body、`NoSuchKey` code 或 exception
字符串。transport、credential、timeout、403、throttling 与 5xx 保持显式 error，
最终为 generic 5xx。日志只记录 request ID 与拒绝类别，不记录 owner 或持久化 policy。

legacy owner-less metadata 只有 `purpose` 仍是当前公开 File object 支持的值时才可被
list/get 表示。missing/invalid purpose 保持现有显式 error，并使整个 list/get 失败；
不得默认成 `assistants`、skip 或返回部分成功。

### 8. Proof-aware Files route adapters

`src/server/routes/ai/files.rs` 保留五个 public handler signatures，并新增 private
`HttpRequest`-aware adapters 与共享 internal implementation：

- `src/server/routes/ai/mod.rs` 的五个 HTTP routes 只注册 private adapters；
- adapters 解析 request extensions/context 后调用 shared implementation；
- proofless public wrappers 传入 `None` proof：auth enabled 时在任何 storage call 前
  fail closed；只有 auth disabled + allow-anonymous 才进入 legacy behavior。

source-boundary test 读取 route registration，断言不再存在 `.to(create_file)`、
`.to(list_files)`、`.to(get_file)`、`.to(delete_file)` 或
`.to(get_file_content)`，并断言五个 private adapter 都已注册。route test 同时用
call-count 证明 auth-enabled direct wrapper 不触碰 storage。

### 9. 文件级实现边界

24 个 manifest path 的职责划分如下：

- JWT/auth/selection：10 个 auth/JWT/route path；
- API-key policy status：`src/server/routes/ai/context.rs` 与
  `src/server/middleware/auth.rs`；
- Files authorization/registration：`src/server/routes/ai/{files,mod}.rs`；
- owner/storage：6 个 `src/storage/files/**` path；
- integration/status/public-compat fixtures：4 个 `tests/**` path。

`src/core/types/context.rs`、`src/core/teams/**`、`src/server/state.rs` 与 database
repository 不修改：当前 typed UUID accessors、canonical TeamRepository 和 AppState
manager 已足够。若实现证明此判断错误，必须先回到 spec review，而不是越界修改。

## Product-to-Test Mapping

| Product invariant | Implementation area | Deterministic verification |
| --- | --- | --- |
| B-001 | FileCaller + owner enum + typed team context | `cargo test --all-features --test files_routes gh1130_scope_priority -- --nocapture` |
| B-002 | S3 continuation + Files filter | `cargo test --features s3 --lib storage::files::s3::tests -- --nocapture`，包含 `Some(0)`、>1000、stable prefix、repeat/missing/empty token、later-page error 与 `Some(limit)` |
| B-003 | canonical concealment mapper | `cargo test --all-features --test files_routes gh1130_foreign_matches_missing -- --nocapture` |
| B-004 | checked API-key admin parser | `cargo test --lib server::routes::ai::context::tests -- --nocapture`，覆盖 direct/runtime/empty/restricted + admin owner |
| B-005 | FileCaller / upload pre-write gate | `cargo test --all-features --test files_routes gh1130_missing_scope -- --nocapture` |
| B-006 | Local staged publication + Local/S3 envelope + HeadObject mapper | `cargo test --lib storage::files::tests -- --nocapture`，含 metadata publish failure、meta-before-content interruption、reopen/list/direct lookup；`cargo test --features s3 --lib storage::files::s3::tests -- --nocapture` |
| B-007 | presence-aware legacy owner decoder | `cargo test --lib storage::files::tests gh1130_owner_presence -- --nocapture`，覆盖 key missing/null/malformed/valid |
| B-008 | public object/error/log capture | `cargo test --all-features --test files_routes gh1130_owner_redaction -- --nocapture` |
| B-009 | auth-disabled route + legacy store | `cargo test --all-features --test files_routes gh1130_auth_disabled -- --nocapture` |
| B-010 | route call order / failure propagation | `cargo test --all-features --test files_routes gh1130_backend_call_order -- --nocapture` |
| B-011 | presence-aware Claims state table + authenticate_jwt | `cargo test --lib auth::jwt::tests -- --nocapture`; `cargo test --lib auth::tests -- --nocapture`，含 signature-valid explicit-null/non-integer marker |
| B-012 | primary-auth-before-selection + active refresh boundary | inline login/token tests plus `cargo test --all-features --test lib integration::auth_middleware_tests -- --nocapture`，覆盖 wrong password/invalid refresh × valid/foreign team 及 inactive/missing refresh user × team Some/None 的 call counts |
| B-013 | auth ordering + policy/storage/corruption 401/5xx matrix | `cargo test --all-features --test lib integration::auth_middleware_tests -- --nocapture`; `cargo test --lib storage::files::tests -- --nocapture` |
| B-014 | unchanged public method/handler signatures + unchanged public struct fields | `cargo test --all-features --test public_api_compat -- --nocapture`; `cargo check --all-features` |
| B-015 | system-private constructor + subject-bound typed issuer | `cargo test --lib auth::jwt::tests gh1130_verified_team -- --nocapture`; negative proof-subject mismatch；source review confirms no raw-team encode path |
| B-016 | UUID owner envelope + owned-store-only atomic publication | `cargo test --lib storage::files::tests gh1130_owned_store -- --nocapture`; missing/null/malformed/valid owner round-trip；publish interruption；Files mock asserts legacy store call count zero |
| B-017 | mutually exclusive caller resolver | `cargo test --all-features --test files_routes gh1130_exclusive_principal_scope -- --nocapture`，覆盖 key team/user/id、JWT team/user 与 mismatch |
| B-018 | legacy purpose compatibility | `cargo test --all-features --test files_routes gh1130_legacy_purpose -- --nocapture`，valid visible、missing/invalid whole-request failure |
| B-019 | private route adapters + proofless wrapper gate | source-boundary/compile fixture in `tests/public_api_compat.rs`; route call-count tests in `tests/files_routes.rs` |

所有计划新增的 GH1130 test 名使用 `gh1130_` 前缀；focused command 若没有匹配测试，
不构成完成证据，最终还必须运行对应完整 module/target。

## 数据流

1. private login/refresh wire adapters 只完成 optional UUID syntactic parse；public
   request DTO/handlers 保持 no-selection，parse error 在 request boundary 400。
2. login 完成 password/active-user gate；refresh 完成 token verification、user reload
   与 active-user gate。任何 primary auth failure 在 team/RBAC/encode 前结束。
3. primary auth 成功后，共享 validator 才查询 canonical team 与 membership；
   no-selection 签 version 1 User token，valid selection 产生 `VerifiedActiveTeam` 并走
   typed Team issuer，selection rejection 在 encode 前结束。
4. middleware 用 presence-aware marker 验证 JWT：key absent 的 legacy 忽略 team；
   version 1 team 复验 membership；explicit null/malformed/unknown version 与 stale
   membership 为 401，repository `Err` 为 generic 5xx。
5. middleware 插入 typed User/API key 与只含可信 team 的 RequestContext；policy parse
   error 在 route 前 generic 5xx。
6. private Files route adapter 提供 request proof；caller resolver 先选互斥 principal，
   再解析一个 UUID scope。proofless public wrapper 在 auth-enabled 时 fail closed。
7. Local 用 presence-aware envelope 写 hidden staging，final `.meta` 先 publish、final
   content 后 publish；final content 是 list 与 direct lookup 的 commit precondition。
   S3 保持 exact metadata wire。list 只遍历 committed candidates 并逐个调用 internal
   metadata-with-owner，public `FileMetadata` 不变。
8. 唯一授权 helper 在公开对象/content/delete 前判断；foreign 与 missing 统一 404，
   storage/policy failure 统一 generic 5xx。

## 备选方案

- 保存 user/team/key 三个 optional 字段并任一相等即允许：拒绝，会产生 same-user
  cross-team bypass。
- 继续让 raw `Option<Uuid>` issuer 签 team token：拒绝，无法证明 provenance。
- 用 defaulted `Option` 表示 owner/marker presence：拒绝，会把 explicit `null` 与 key
  absence 合并成 legacy fail-open。
- 在 password/refresh verification 前查询 team selection：拒绝，会形成 membership/
  repository-state oracle；inactive refresh user 也不得续签。
- Local 先写 final content 再写 `.meta`：拒绝，中断会留下可枚举 orphan 并 poison
  fail-closed list；使用 metadata-first、content-last staged publication。
- 修改既有 public storage/JWT/handler signature 或 public DTO/metadata/claims fields：
  拒绝，会造成不必要 Rust source break；使用 internal envelope/wire adapter。
- unknown version 当 legacy 或 stale membership 回退 User：拒绝，会把未来/损坏状态
  当可信降级。
- foreign 返回 403：拒绝，会成为文件存在性 oracle。
- 只取 S3 第一页或 metadata 错误时 skip：拒绝，会返回不完整且可能泄露的成功结果。
- 自动归属 legacy 文件：拒绝，存在 owner 抢占。

## 风险

- Security: auth、owner、API-key attenuation 与 concealment 都是高风险边界；实现 PR
  必须有独立人工 security review，不能由 agent 自批。
- Compatibility: key-absent legacy token 继续 User-scope；explicit-null/malformed/
  unknown marker 必须重新登录。列出的
  public methods/handlers 与 `FileMetadata`/`Claims`/auth request struct literals 保留；
  marker/selection/owner 由 internal envelope/wire 承载。raw team signing 从可用变为
  显式拒绝，这是有意的 runtime 安全收紧。
- Compatibility: 历史 owner-less 文件在 auth-enabled 部署仅 admin 可见；不自动迁移。
- Compatibility: 只有 owner key absent 是历史格式；present null/malformed metadata
  现在显式 5xx，不再被当作 legacy admin-visible record。
- Compatibility: 历史 missing/invalid purpose 不被本 Issue 猜测；对应 list/get
  继续显式失败，可能需要单独迁移。
- Security/Lifecycle: API-key `team_id` 被视为 key-owned trusted association；本
  Issue 不新增 team deletion 与 key revocation 的联动。该 lifecycle gap 需单独跟踪，
  但不得通过 key owner/JWT fallback 扩权。
- Performance/Availability: 完整 S3 listing + per-object HeadObject 可能触发
  throttling/503；本 tranche 必须整体失败。owner index/cache 另开 Issue，且 cache
  失效不能降级为全局放行。
- Availability: Local staged publish 可能因进程中断留下 private staging 或 meta-only
  sidecar；list/reopen 必须忽略它们，cleanup 失败也不能重新暴露为 committed content。
- Maintenance: typed validator、checked policy helper 与 ownership helper 各保持单一
  owner；不得新增平行 auth/storage module。

## 测试计划

- [ ] Unit: marker absent/null/non-integer/malformed/1/unknown exact state table、typed
  issuer、active/inactive membership；owner missing/null/malformed/valid exact decoder；
  Local staged metadata failure/interruption/reopen/list/direct lookup；checked
  admin/policy error 与 call ordering。
- [ ] Auth route: wrong password/invalid refresh × valid/foreign team 均保持 primary
  auth result且 team-repository/encode count 为零；valid refresh 的 inactive/missing
  user × team Some/None 均 generic 401 且 team/RBAC/encode count 为零。
- [ ] Route: two users/teams/keys + admin/legacy/auth-disabled 的
  list/get/content/delete/upload 矩阵；foreign/missing 公开错误完全等价。
- [ ] S3 feature: owner round-trip、strict malformed metadata、HeadObject exact 404、
  403/timeout/5xx、`Some(0)`、>1000 continuation、stable prefix、repeat/missing/empty
  token、later-page failure、cross-page `Some(limit)`。
- [ ] Middleware: explicit-null/non-integer/malformed/unknown marker 与 stale membership
  401；repository/RBAC/policy error generic 5xx 且不泄露内部 detail。
- [ ] Compatibility/source boundary: existing public method/handler 与
  `FileMetadata`/`Claims`/auth request struct-literal compile fixtures；legacy bare/new
  envelope round-trip；raw team issuer 拒绝；private auth/Files adapters 是唯一 route
  registration；proofless Files wrappers 在 auth-enabled 时不调用 storage。
- [ ] Repository: `cargo fmt --check`; `cargo check`; `cargo check --all-features`;
  `cargo clippy --all-targets --all-features -- -D warnings`; focused tests;
  `cargo test`; SpecRail packet/workflow checks；`git diff --check`。

## 回滚方案

代码可整体 revert，但不得恢复未授权跨租户访问。回滚前验证旧 reader 是否接受新增
metadata 字段；若不确定，暂停 auth-enabled Files API 或保留 authorization patch。
已经写入 owner 的 metadata 不自动删除，legacy 文件不自动归属。即使其余代码回滚，
也不得恢复 owner/marker explicit-null → absent 的降级，或 Local final-content-first
publication；必要时保留 fail-closed decoder 与 staged writer。若 JWT provenance 逻辑
需紧急回滚，先撤销 team-scoped login/refresh issuance 并只允许 User scope，不得重新
启用 first-team guessing或 inactive-user refresh。Spec approval、implementation、
security review、merge 与 release 仍是独立 human gates。
