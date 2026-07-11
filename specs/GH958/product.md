# Product Spec

## Linked Issue

GH-958 / #958

## 用户问题

API key 自身为 active 且未过期时，实时验证路径会接受其数据库记录，即使该 key 明确绑定的 user 已不存在或不是 active。详细验证路径只拒绝 inactive owner，仍会接受 missing owner。两条入口因此可能为已经失去有效 owner 的凭证授权。

## 目标

- 明确 user-owned API key 的 owner 存在性与 active 状态是认证成功的必要条件。
- 普通验证与详细验证使用同一个 owner 判定契约。
- 保持 `user_id = None` 的 ownerless/global key 现有有效性语义。
- owner 查询基础设施错误继续作为系统错误传播，不伪装成无效凭证。

## 非目标

- 不改变 API-key Redis cache 的读取或失效策略；该工作属于 #959。
- 不改变 user 删除、外键 `ON DELETE SET NULL` 或 owner provenance；该工作属于 #961。
- 不改变 key hash、创建、撤销、过期或权限语义。
- 不新增公开 API 或数据库 migration。

## Behavior Invariants

1. `P1`：`user_id = Some(id)` 的 key 只有在 owner 查询返回 user 且 `user.is_active()` 为 true 时才有效。
2. `P2`：`user_id = Some(id)` 且 owner 不存在时，普通验证返回无效，详细验证返回 `is_valid = false`，且两者都不得更新 `last_used_at`。
3. `P3`：owner 为 inactive、suspended、pending 或 deleted 时按同一规则拒绝；只有 `Active` 状态通过。
4. `P4`：`user_id = None` 的 ownerless/global key 不触发 owner 查询，也不因缺少 owner 被拒绝。
5. `P5`：普通验证和详细验证调用同一 owner predicate；任一入口新增或修改 owner 状态时不能发生语义漂移。
6. `P6`：owner repository 查询返回错误时，两条验证入口传播系统错误；不得降级为 valid、ownerless 或普通 invalid credential。
7. `P7`：普通认证入口对 missing 与 inactive owner 都保持通用 `Invalid API key` 结果，不向客户端披露 owner 存在性。

## 验收标准

- [ ] missing owner 的 user-owned key 被普通与详细验证拒绝。
- [ ] non-active owner 的 key 被普通与详细验证拒绝。
- [ ] ownerless/global key 的成功语义有明确回归测试。
- [ ] 两条验证入口共享一个 owner 判定函数，且 owner 有效前不更新 last-used。
- [ ] 聚焦测试、格式、strict clippy、全量测试、scope/overlap guards 与 SpecRail gates 通过。

## 边界情况

- `UserStatus` 中除 `Active` 外的所有状态均视为 non-active。
- 当前 schema 删除 owner 后可能把 `user_id` 置为 `None`；本 issue 无法恢复已丢失的 owner provenance，不把该情况重新解释为 missing owner。
- owner 查询成功返回 `None` 是 credential lifecycle invalid；查询本身返回 `Err` 是基础设施失败。

## 发布说明

这是 fail-closed 的认证修复。仍绑定 owner 但 owner missing/non-active 的 API key 将不再认证成功；显式 ownerless/global key 保持不变。
