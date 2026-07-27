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

## 验收标准

- [ ] 两个 API key、两个 user、两个 team 的矩阵测试覆盖 list/get/content/delete；login/refresh 的可选 `team_id` 必须验证 membership，新 token 带 provenance marker；multi-team 且无选择时签发 `team_id: None`，旧 token 缺 marker 时忽略其 guessed team，Files 均使用 User scope。
- [ ] team-scoped 文件不能仅因相同 `user_id` 从另一个 team 访问；有效范围优先级有测试。
- [ ] 非 owner 与随机不存在 ID 的状态码和公开错误形状一致。
- [ ] 管理员可以访问所有 owner 与 legacy 文件，普通调用者看不到 legacy 文件；直接 `*`/`system.admin`、runtime `is_admin`、受限 key + admin owner、空权限 key + admin owner、无 key admin JWT 各有测试。
- [ ] 无身份的已启用 auth 请求在上传和对象操作上 fail-closed。
- [ ] Local 与 S3 元数据 round-trip、旧元数据兼容和重启后读取有测试；S3 HeadObject HTTP 404 与 foreign 的公开 404 等价，非 404 S3 故障仍显式失败。
- [ ] S3 超过 1000 个对象时遍历 continuation token 到结束，返回全部已授权文件且不泄露其他 owner；不存在公共 limit/count 断言。
- [ ] OpenAI File JSON 与错误日志不泄露 owner。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过；S3 路径至少用 `--all-features`（或等价 `--features s3`）完成 check、Clippy 与相关测试。

## 边界情况

- 同一 user 可能通过不同 team 或不同 API key 调用；team 范围不得被 user 匹配绕过。
- JWT user 可能属于多个 team；没有经过 membership 验证的 active-team claim 时使用
  user scope，不得选择列表第一项。team 缺失但 user 存在时使用 user；两者都缺失时
  才回退到 API key。
- 旧 access token 可能包含历史 guessed `team_id`；缺 provenance marker 时该字段
  必须被忽略，token 可继续作为 user-scope 身份使用。
- admin owner 可能使用受限或空权限 API key；key 是权限衰减边界，不能因 owner
  的用户角色提权。
- 文件可能在升级前创建且没有 owner。
- metadata 存在但内容丢失，或删除时后端失败；S3 missing 使用 canonical NotFound，
  其他后端错误仍需遵循现有显式失败语义。
- list 期间单个 metadata 损坏不得导致未过滤内容泄露。
- S3 一页最多返回有限对象，授权代码不能把第一页或 backend max_keys 当公开 limit。
- 大 bucket 的完整分页与逐对象 metadata 读取可能触发 S3 throttling/`503 Slow Down`；
  当前请求必须显式失败而不是返回未过滤或不完整列表，后续索引/缓存优化不能弱化授权。

## 发布说明

升级后新文件带有内部 owner 元数据。历史无 owner 文件仅管理员可访问；如需普通
租户继续使用，必须由单独、经审计的迁移显式归属。升级前签发且缺 active-team
provenance marker 的 access token 仍可认证，但其历史 `team_id` 被忽略并使用
user scope；客户端可通过 login/refresh 的可选 `team_id` 获取经验证的新 team-scope
token。auth 关闭的单租户开发部署保持现有行为。
