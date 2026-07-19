# Product Spec

## Linked Issue

GH-1067 / #1067

## User Problem

The gateway exposes APIs for API-key and team administration plus usage
statistics, but operators have no browser interface for routine management.
They must assemble requests manually, cannot see key/team spend in one place,
and can easily miss loading, empty, or permission failures.

## Goals

- Provide a built-in browser dashboard for administrators without requiring a
  separate frontend deployment.
- Reuse the gateway's existing login, key, team, and usage contracts.
- Support the minimum daily workflow: inspect and create keys, revoke keys,
  inspect and create teams, delete teams, and view key/team usage and spend.
- Make authentication, permission, loading, empty, validation, and API failure
  states explicit.

## Non-Goals

- Adding new key, team, budget, spend, user, or authentication APIs.
- Adding a general analytics warehouse, time-series charts, exports, or billing
  reconciliation.
- Adding team membership or role editing, key rotation, key editing, budget
  editing, registration, password reset, OAuth, or SSO controls.
- Persisting browser credentials, access tokens, refresh tokens, or newly
  created raw API keys.
- 不增加运行时前端 bundle、运行时包管理器或随部署交付的前端工具链。允许使用
  隔离、版本锁定、仅测试用途的 `node:test` + jsdom harness，前提是它既不随
  gateway 运行时发布，也不在 gateway 运行时中执行。

## Behavior Invariants

1. `GET /admin/dashboard` returns the dashboard shell without requiring
   credentials. The shell contains no gateway data, credentials, or
   environment-derived configuration; every management or usage request still
   passes through the existing authenticated API.
2. An operator signs in through the existing username/password login contract.
   The dashboard accepts only an authenticated admin response before loading
   management data. Invalid credentials, non-admin users, expired tokens, and
   unavailable authentication are displayed as explicit errors and reveal no
   protected data.
3. Access and refresh tokens exist only in page memory, are not written to
   cookies or browser storage, and are discarded on sign-out, reload, or tab
   close. The raw value of a newly created API key is shown once, can be copied
   deliberately, and is not retained after its one-time notice is dismissed.
4. The key view lists the existing key records with status, scope, creation or
   expiration information when available, and explicit pagination. An admin can
   create a key owned by their user or a selected team only after providing at
   least one allowed model pattern and one allowed endpoint pattern; the
   dashboard never creates an unowned, admin, or unrestricted-wildcard key. An
   active key can be revoked after confirmation.
5. The team view lists existing teams with status and available metadata. An
   admin can create a team and delete an existing team after confirmation.
6. The spend view derives its values only from successful existing key and team
   usage responses. Explicit numeric zero is displayed as zero; unavailable or
   failed data remains blank and is accompanied by an error indicator rather
   than being guessed or silently treated as zero.
7. Initial loads, refreshes, and mutations have visible progress. Empty key,
   team, and usage collections have distinct empty states. A failed request
   preserves already displayed valid data where safe and shows the failed
   operation and server message.
8. Concurrent refresh or navigation responses cannot overwrite newer page
   state. Controls that would duplicate an in-flight mutation are disabled
   until that mutation completes.
9. The dashboard is usable by keyboard, exposes programmatic labels for inputs
   and controls, maintains visible focus, announces status/error changes, and
   does not rely on color alone for meaning.
10. Existing API routes, authentication behavior, JSON shapes, CLI clients, and
    deployments that never open the dashboard remain unchanged.

以下追加的 `B` 不变量定义可执行 DOM 自动化证据，同时不改变 P1-P10：

- `B1` — refresh、navigation 或 mutation 响应乱序完成时，只有最新认证 session
  generation 且符合操作顺序的响应可以更新受保护页面状态或 DOM。
- `B2` — 每个请求 controller 在 success、failure 或 cancellation 后都必须从
  active-request tracking 移除；sign-out 会 abort 所有仍活跃的 controller，且不
  留下 stale controller。
- `B3` — 新建 raw API key 只出现在一次性 notice 中，只能经明确操作复制，并在
  dismissal 或下一次 authentication-state transition 时不可恢复地清除。
- `B4` — usage 请求部分失败时，成功 row 保持可见，真实数值 zero 仍显示为 zero；
  只有 unavailable/failed value 留空并带明确 row-level error。
- `B5` — revoke-key 与 delete-team 仅在 affirmative confirmation 后发出请求；
  取消确认不会发出破坏性请求，也不会产生 optimistic destructive DOM change。
- `B6` — sign-out 后，前一 generation 的延迟 login、list、usage、mutation 与
  raw-key 响应都不能恢复 credential、protected data、status 或一次性 secret。

## Acceptance Criteria

- [ ] `/admin/dashboard` serves a self-contained dashboard shell with no
      external frontend runtime or asset host.
- [ ] Admin login loads key, team, and spend views by calling only existing
      gateway APIs; invalid or non-admin login never loads protected data.
- [ ] Key list/create/revoke and team list/create/delete workflows complete
      against the existing APIs and refresh the affected view.
- [ ] Key and team usage/spend render explicit zero separately from missing or
      failed data.
- [ ] Tokens and one-time raw keys are absent from browser storage and are
      cleared from page state at their documented lifecycle boundaries.
- [ ] 可执行 DOM harness 仅使用隔离且锁定的测试专用 Node/jsdom 环境，对真实
      embedded JavaScript source 确定性执行 `B1`-`B6`。
- [ ] Route registration, response headers, static content/API contracts, and
      unsafe browser primitives have deterministic automated coverage;
      keyboard flow、narrow-layout behavior 与 real-browser rendering 仍保留在
      可重复的 manual verification checklist 中。

## Edge Cases

- Authentication may be disabled, partially configured, or temporarily
  unavailable; the shell remains renderable and the login error remains
  explicit.
- A key or team can disappear between list and mutation requests; the dashboard
  reports the server result and refreshes instead of fabricating success.
- Usage requests can partially fail across a collection. Successful values
  remain visible, while failed entries stay blank and are identified.
- Long names and identifiers wrap or truncate accessibly without changing the
  submitted value.
- Network responses arriving after sign-out are ignored and cannot repopulate
  protected state.
- 可执行 DOM automation 不作为 keyboard ergonomics、narrow-layout readability
  或 real-browser visual rendering 的证据；这些检查继续保持 manual。

## Rollout Notes

The dashboard is an additive built-in surface. It reuses existing authorization
and management APIs and requires no data migration. A code rollback removes the
dashboard route and embedded assets; existing API behavior and stored data are
unchanged.
