# Product Spec

## Linked Issue

GH-1130 / #1130

complexity: high

## 用户问题

Files API 的存储元数据没有所有者，所有已认证调用者都可以列出、读取、下载或
删除其他租户文件。文件内容可能包含 batch 输入、训练数据或 assistant 文档，
因此对象级授权必须在每个入口一致执行。

## 目标

- 上传时为文件记录一个确定的有效租户范围。
- list、metadata、content 和 delete 全部执行同一对象所有权规则。
- 管理员保留运维访问，历史无 owner 文件仅管理员可见。
- 在 Local 与 S3 后端持久保存相同 owner 语义。
- 防止未授权调用者通过响应差异枚举其他租户文件。

## 非目标

- 改变文件 ID、purpose、内容类型或上传大小限制。
- 增加跨租户共享、ACL、转移所有权或公开链接。
- 回填或猜测历史文件的 owner。
- 改变 auth 关闭时的本地开发兼容行为。
- 为 API key 的 persisted `team_id` 新增 team-deletion/membership lifecycle 联动；
  本 Issue 继续把它视为 key 自身的 trusted association，撤销同步另开 Issue。

## Behavior Invariants

1. B-001 auth 开启时，每个新文件必须绑定一个有效租户范围；优先级为可信 `active_team_id`、其次 `user_id`、最后 `api_key_id`，不得把多个范围作为“任一匹配即可”的并列授权。API key 的 team 来自该 key 自身；JWT 的 team 只能来自 `/auth/login` 或 `/auth/refresh` 可选 `team_id` 经服务端验证属于该 user 后签发的 active-team claim。新 access token 必须带版本化 provenance marker；旧 token 缺 marker 时即使含 `team_id` 也忽略该 team 并回退 user scope。签发和认证读取两端都禁止用 `user.team_ids.first()` 猜测。
2. B-002 非管理员只能列出与当前有效租户范围完全匹配的文件；其他租户和历史无 owner 文件不得出现在返回数组中。当前 Files list 没有公共 pagination/limit/count 参数，必须遍历后端完整分页并返回全部已授权文件，不得发明隐式 public limit。
3. B-003 metadata、content 和 delete 对非 owner 的结果必须与不存在文件不可区分，不得泄露目标文件是否存在、owner 或其他元数据。
4. B-004 管理员可以列出和操作所有文件，包括历史无 owner 文件。API key 请求只有 key 自身的直接或运行时权限明确包含 admin 能力时才是管理员；admin user 持有的受限、空权限或无 admin 能力 key 都不得继承用户管理员权限。无 API key 的 JWT/session 才可按用户 admin role/RBAC 判定。
5. B-005 auth 开启但无法解析任何有效 owner 范围时，上传和对象访问必须 fail-closed。
6. B-006 Local 与 S3 后端必须持久化、读取和列举一致的 owner 信息；进程重启后授权结果不变。S3 HeadObject 只有 HTTP 404 进入 canonical storage NotFound；HEAD 错误没有可依赖的响应正文或精确 exception，403、transport、timeout 与 5xx 等其他 S3 错误保持显式失败。
7. B-007 旧元数据缺少 owner 时仍可反序列化，但只对管理员可见；不得自动归属给第一个访问者。
8. B-008 owner 信息是内部授权元数据，不得出现在 OpenAI-compatible File 响应或错误正文中。
9. B-009 auth 明确关闭并允许匿名访问时，Files API 保持现有单租户行为，不伪造 owner。
10. B-010 所有权检查必须发生在返回内容或执行删除之前；失败不得静默降级为无过滤访问。
11. B-011 access token 的 team provenance 是闭集状态：marker 缺失时是 legacy
    token，必须忽略其中任何 `team_id` 并使用 User scope；marker 为 version 1 且
    `team_id` 缺失时使用 User scope；marker 为 version 1 且 `team_id` 存在时，只有
    当前 active membership 复验成功才使用 Team scope；任何其他 marker 值都使整个
    bearer token 无效。未知 version、stale team 或无效 membership 均不得回退到
    User、API-key 或其他 team。
12. B-012 `/auth/login` 与 `/auth/refresh` 的 optional `team_id` 只接受 UUID
    格式且必须对应该 user 的当前 active membership。字段缺失时签发 version 1
    User-scope token；malformed、foreign、missing、inactive 或 suspended selection
    均返回稳定 400，且不得签发 access 或 refresh token。
13. B-013 已签发 version 1 team token 的 membership 缺失或不再 active 时返回稳定
    401；user/team/membership repository、RBAC、API-key policy 或 file metadata 的
    读取、解析、转换错误返回 generic 5xx。上述错误不得被转成“无 membership”、空权限、
    User scope、legacy store、跳过坏 metadata 或部分成功 list。
14. B-014 下列既有 public Rust method call、handler signature 与 struct literal
    保持 source-compatible：Files storage 的
    `store`、`store_with_purpose`、`StorageLayer::store_file`，JWT 的
    `create_access_token`、`create_token_pair`，以及 `AuthSystem::login` 的参数和
    返回类型不变；public `FileMetadata`、`Claims`、`LoginRequest` 与
    `RefreshTokenRequest` 字段集合不变；既有 public login/refresh handler 签名不变。
    既有五个 public Files handler 的参数/返回类型也不变。新
    owner/provenance/team-selection/request-proof 能力必须是 additive internal API，
    不得要求外部 Rust caller 构造内部授权证据。
15. B-015 只有服务端完成 current active membership 校验后产生的 typed
    `VerifiedActiveTeam` 才能授权或签发 version 1 team-scoped access token。既有
    raw `team_id: Some(...)` JWT 入口必须在 encode 前显式拒绝，不能制造可信
    provenance；no-team 旧签名仍签发 version 1 User-scope token。
16. B-016 internal persisted envelope 中的 owner 是且仅是一个 canonical UUID scope
    `{Team(Uuid), User(Uuid), ApiKey(Uuid)}`。auth-enabled Files upload 必须走 owner
    非 optional 的 additive owned-store；无效/缺失 UUID 不得变成字符串 owner 或
    退回 legacy `owner: None`。既有非 HTTP storage 调用继续产生 legacy
    `owner: None`；public `FileMetadata` 只呈现既有非授权字段，auth-disabled anonymous
    route 也保持 B-009 行为。
17. B-017 caller scope 的认证来源必须互斥。存在 API key 时只能按该 key 的
    `team_id -> user_id -> key UUID` 选择一个 scope，并忽略任何残留 JWT/context team；
    不存在 API key 时才按复验通过的 JWT Team -> authenticated User UUID 选择。typed
    principal 与 request context 的 user/key identity 不一致或可信 UUID 损坏时返回
    generic 5xx，不得尝试下一种身份。
18. B-018 “返回全部已授权文件”只覆盖 owner 合法且现有公开 File object 必需字段可表示
    的记录。历史 metadata 的 `purpose` 缺失或无效时，list/get 必须保持显式错误并使
    整个请求失败；不得猜测 purpose、静默跳过该记录或把成功数组伪装成完整结果。
19. B-019 auth-enabled HTTP Files routes 必须只注册可读取 request proof 的 internal
    adapter。为保持 Rust signature 而保留的 proofless public Files wrappers 在 auth
    开启时必须 fail-closed，不得访问 storage；只有 auth disabled +
    allow-anonymous 时才可复用 legacy 单租户行为。任何 route 都不得继续指向 proofless
    wrapper。

## 验收标准

- [ ] 两个 API key、两个 user、两个 team 的矩阵测试覆盖 list/get/content/delete；login/refresh 的可选 `team_id` 必须验证 membership，新 token 带 provenance marker；multi-team 且无选择时签发 `team_id: None`，旧 token 缺 marker 时忽略其 guessed team，Files 均使用 User scope。
- [ ] JWT 矩阵分别覆盖 legacy marker 缺失、version 1 team/no-team、unknown version
  team/no-team；unknown version 与 stale membership 均为 401 且没有身份 fallback。
- [ ] login/refresh 的 malformed、foreign、missing、inactive、suspended team
  selection 均为同一公开 400 且没有任何新 token；membership/team repository
  故障为 generic 5xx。
- [ ] team-scoped 文件不能仅因相同 `user_id` 从另一个 team 访问；有效范围优先级有测试。
- [ ] 非 owner 与随机不存在 ID 的状态码和公开错误形状一致。
- [ ] 管理员可以访问所有 owner 与 legacy 文件，普通调用者看不到 legacy 文件；直接 `*`/`system.admin`、runtime `is_admin`、受限 key + admin owner、空权限 key + admin owner、无 key admin JWT 各有测试。
- [ ] malformed API-key runtime policy 与 RBAC/storage conversion error 返回 generic
  5xx，不得回退 admin owner role 或普通 allow；错误正文不包含持久化 policy 细节。
- [ ] 无身份的已启用 auth 请求在上传和对象操作上 fail-closed。
- [ ] 既有 public storage/JWT/login 方法、public login/refresh handlers，以及
  `FileMetadata`、`Claims`、`LoginRequest`、`RefreshTokenRequest` struct literals
  与五个 public Files handler signatures 的 compile fixture 保持通过；raw
  `team_id: Some` JWT 调用不生成 token，typed verified 路径可生成 team token；
  auth-enabled HTTP upload 只调用 owner 非 optional 的 owned-store，并只用 internal
  metadata-with-owner 结果授权。
- [ ] Local 与 S3 元数据 round-trip、旧元数据兼容和重启后读取有测试；S3 HeadObject HTTP 404 与 foreign 的公开 404 等价，非 404 S3 故障仍显式失败。
- [ ] S3 超过 1000 个对象时以稳定 prefix 遍历 continuation token 到结束，返回全部已授权文件且不泄露其他 owner；`Some(0)` 零请求且为空，单页大小为 `min(1000, remaining)`；truncated page 的 token 缺失、为空或重复，以及任意 later-page error 都使整个请求失败；既有 internal `Some(limit)` 是跨页总上限；不存在公共 limit/count 断言。
- [ ] OpenAI File JSON 与错误日志不泄露 owner。
- [ ] API-key 与 JWT 同时留下 context 字段时 scope 来源互斥；API key 的 team/user/key
  fallback 矩阵和无 key JWT team/user 矩阵逐项覆盖，identity mismatch/invalid UUID
  为 5xx 且不 fallback。
- [ ] legacy owner-less metadata 的 valid purpose 可按管理员规则列出；missing/invalid
  purpose 使 list/get 显式失败，不 skip、不猜测、不返回部分成功。
- [ ] source-boundary test 证明五个 HTTP Files routes 全部注册 internal proof-aware
  adapter；直接调用 proofless public wrapper 时，auth enabled 不触碰 storage，
  auth disabled + allow-anonymous 才保持 legacy 行为。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过；S3 路径至少用 `--all-features`（或等价 `--features s3`）完成 check、Clippy 与相关测试。

## 边界情况

- 同一 user 可能通过不同 team 或不同 API key 调用；team 范围不得被 user 匹配绕过。
- JWT user 可能属于多个 team；没有经过 membership 验证的 active-team claim 时使用
  user scope，不得选择列表第一项。team 缺失但 user 存在时使用 user；两者都缺失时
  才回退到 API key。
- 旧 access token 可能包含历史 guessed `team_id`；缺 provenance marker 时该字段
  必须被忽略，token 可继续作为 user-scope 身份使用。
- access token 可能包含未来或损坏的非 1 marker；即使 `team_id` 缺失也必须整体
  拒绝，不能把 unknown version 当 legacy。
- version 1 token 的 membership 可能在签发后被删除、暂停或对应 team 被停用；
  下一次认证必须 401，读取基础设施失败则必须 5xx。
- admin owner 可能使用受限或空权限 API key；key 是权限衰减边界，不能因 owner
  的用户角色提权。
- API-key runtime policy 可能是 malformed JSON；不能把解析失败当作空 policy。
- API key 的 persisted `team_id` 在本 Issue 中代表 key 自身的 trusted team
  association，不读取或继承 key owner 的 active-team selection；team/key lifecycle
  同步与撤销策略不在本 Issue 扩展。
- 文件可能在升级前创建且没有 owner。
- metadata 存在但内容丢失，或删除时后端失败；S3 missing 使用 canonical NotFound，
  其他后端错误仍需遵循现有显式失败语义。
- list 期间单个 metadata 损坏不得导致未过滤内容泄露。
- S3 一页最多返回有限对象，授权代码不能把第一页或 backend max_keys 当公开 limit。
- 大 bucket 的完整分页与逐对象 metadata 读取可能触发 S3 throttling/`503 Slow Down`；
  当前请求必须显式失败而不是返回未过滤或不完整列表，后续索引/缓存优化不能弱化授权。

## Boundary Checklist

| Category | Verdict |
| --- | --- |
| Empty / missing input | covered: B-005, B-007, B-009, B-011, B-012, B-016, B-018 |
| Error and failure paths | covered: B-003, B-006, B-010, B-013, B-017, B-018 |
| Authorization / permission | covered: B-001, B-003, B-004, B-005, B-011, B-012, B-015, B-016, B-017, B-019 |
| Concurrency / race / ordering | covered: B-010；授权必须先于内容返回或删除，owner 不可由 caller 更新 |
| Retry / repetition / idempotency | covered: B-002, B-006, B-013；S3 continuation 重复或缺失必须失败，不得返回部分结果 |
| Illegal state transitions | covered: B-011, B-012, B-013, B-015 |
| Compatibility / migration | covered: B-007, B-009, B-014, B-016, B-018, B-019；listed public structs/handlers 不改变 shape/signature |
| Degradation / fallback | covered: B-005, B-010, B-011, B-013, B-016, B-017, B-018, B-019 |
| Evidence and audit integrity | N/A：本 Issue 不产生审批或审计 ledger；B-008 仍约束公开响应与日志不得泄露 owner |
| Cancellation / interruption / partial completion | covered: B-002, B-010, B-013；失败请求不得返回部分未过滤 list 或执行未授权 delete |

## 发布说明

升级后新文件带有内部 owner 元数据。历史无 owner 文件仅管理员可访问；如需普通
租户继续使用，必须由单独、经审计的迁移显式归属。升级前签发且缺 active-team
provenance marker 的 access token 仍可认证，但其历史 `team_id` 被忽略并使用
user scope；客户端可通过 login/refresh 的可选 `team_id` 获取经验证的新 team-scope
token。unknown marker 的客户端必须重新登录，不能自动降级。既有 public Rust
method/handler signatures 与 listed public struct shapes 保留，但 raw
`team_id: Some` access-token 签发会被安全拒绝；HTTP 客户端使用 optional `team_id`
wire field，Rust 内部 route adapter 将其转换为既有 public request DTO。auth 关闭的
单租户开发部署保持现有行为。
