# Tech Spec

## Linked Issue

GH-1067 / #1067

## Product Spec

See `specs/GH1067/product.md` (invariants P1-P10).

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| HTTP assembly | `src/server/http.rs`, `src/server/routes/mod.rs` | Actix configures health, auth, key, team, budget, admin-cache, AI, and pricing routes; no UI route exists | Register one additive dashboard route module |
| Public-route boundary | `src/server/middleware/helpers.rs`, `src/server/middleware/auth.rs` | Only exact paths in `PUBLIC_ROUTES` bypass authentication; all other requests pass through configured auth | The inert shell/assets must load before login without broadening protected API access |
| Security headers | `src/server/middleware/security.rs` | Global middleware sets nosniff, frame denial, HSTS, and referrer policy; it does not set a content security policy | Dashboard responses need a route-specific restrictive CSP |
| Login | `src/server/routes/auth/login.rs`, `src/server/routes/auth/models.rs` | `POST /auth/login` returns `ApiResponse<LoginResponse>` with JWTs and user role | Reuse the existing contract and accept only an admin response |
| Key management | `src/server/routes/keys/{mod.rs,handlers.rs,types.rs}` | `/v1/keys` supports admin list/create and per-key revoke; `KeyInfo` already includes masked identity and usage totals | Drive the minimum key workflow without a new backend API |
| Team management | `src/server/routes/teams.rs` | `/v1/teams` supports admin list/create/delete and per-team usage | Drive the minimum team and spend workflow without a new backend API |
| Frontend assets | `src/server/routes/admin_dashboard/{index.html,app.css,app.js}` | Compile-time embedded HTML/CSS/JavaScript now exists; Rust/source assertions do not execute the browser state machine | Add an isolated test-only executable DOM harness without a runtime frontend toolchain |

## Proposed Design

### Embedded route module

Add `src/server/routes/admin_dashboard.rs` with three compile-time embedded
constants and handlers:

- `GET /admin/dashboard` → `text/html`
- `GET /admin/dashboard/app.css` → `text/css`
- `GET /admin/dashboard/app.js` → `text/javascript`

The source assets live under `src/server/routes/admin_dashboard/` and are
included with `include_str!`. Responses set `Cache-Control: no-store` and a CSP
equivalent to:

```text
default-src 'none'; script-src 'self'; style-src 'self';
connect-src 'self'; img-src 'self' data:; font-src 'self';
base-uri 'none'; form-action 'self'; frame-ancestors 'none'
```

The route module is registered from the existing route assembly. No filesystem
path, directory traversal surface, runtime frontend bundle, runtime package
manager, deployed frontend toolchain, or runtime dependency is introduced. An
isolated, locked, test-only Node/jsdom harness is allowed and is never shipped
with or executed by the gateway runtime.

### Authentication boundary

Add only those three exact asset paths to `is_public_route`. Prefixes, suffixes,
and child paths remain protected. The anonymous responses are immutable source
assets and contain no server data.

The JavaScript calls `POST /auth/login`, unwraps the existing `ApiResponse`,
checks `data.user.role` case-insensitively for `admin`, and only then retains
`data.access_token` in a module-scoped state object. The refresh token is not
retained because this minimum dashboard does not implement token refresh.
Authenticated requests use `Authorization: Bearer <token>`. Sign-out clears all
state and rendered protected data. No browser storage or cookies are read or
written.

### Dashboard state and rendering

Use semantic HTML with login, key, team, and spend panels. JavaScript uses only
DOM construction plus `textContent`; it does not use `innerHTML`, `eval`, or
dynamic script/style injection.

The state object holds the access token, authenticated admin ID, current view,
key/team page numbers, the latest successful key/team payloads, a set of active
`AbortController` values, a monotonically increasing session generation, and
per-mutation busy flags. Every authenticated request, including each mutation,
captures the token plus generation and registers its controller. Before any
state or DOM commit, the response must still match both values. Sign-out aborts
every active controller, increments the generation, clears protected state, and
therefore prevents late list, usage, mutation, or one-time raw-key responses
from repopulating the page.

Existing contracts are used as follows:

- keys: `GET /v1/keys?page=<n>&limit=20`, `POST /v1/keys`,
  `DELETE /v1/keys/{id}`;
- teams: `GET /v1/teams?page=<n>&limit=20`, `POST /v1/teams`,
  `DELETE /v1/teams/{id}`;
- spend: key `usage_stats` from the successful key-list payload and
  `GET /v1/teams/{id}/usage` for teams visible on the current page.

The spend view is explicitly page-scoped. It renders per-key and per-team
`cost_today`, `total_cost`, requests, and tokens. It does not sum key and team
totals together, which could double-count the same traffic. Numeric fields are
formatted only after a finite-number check; missing, non-finite, rejected, or
unavailable values render blank with a row-level error.

Create-key submits only declared `CreateKeyRequest` fields used by the form:
`name`, optional `description`, exactly one ownership field, and explicit
`permissions`. Selecting a team sends `team_id`; otherwise the authenticated
admin's ID is sent as `user_id`, so the dashboard never creates an unowned key.
The form requires non-empty comma-separated `allowed_models` and
`allowed_endpoints`, rejects the unrestricted `*` value, and sends
`is_admin=false` plus no custom management permissions. The raw key response is
rendered into a one-time notice with an explicit copy button and is removed from
page state when dismissed or on the next authentication-state transition.
Create-team submits only `name`, optional `display_name`, and optional
`description`. Destructive operations use an explicit confirmation dialog and
disable the originating control until the request settles.

## 可执行 DOM 验证增补（B1-B6）

`tests/admin_dashboard/admin_dashboard_dom.test.mjs` 必须读取并执行真实的
`src/server/routes/admin_dashboard/app.js`，而不是复制或重写一份 dashboard
状态机。harness 使用精确版本 Node `24.14.0`、精确版本 jsdom `29.1.1`、
Node 内建 `node:test` 与 `node:assert/strict`。依赖只允许安装在隔离的测试目录：

```bash
(cd tests/admin_dashboard && npm ci --ignore-scripts)
node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs
```

`package.json` 与已提交的 `package-lock.json` 必须锁定 jsdom `29.1.1`；
`npm ci --ignore-scripts` 禁止 dependency lifecycle scripts。harness 可控地 mock
`fetch`、`confirm`、clipboard、delayed promises 与 `AbortController`，并为
`B1`-`B6` 分别提供确定性断言：乱序 generation/operation guard、controller
全路径 cleanup、raw-key 一次性生命周期、partial usage failure 与真实 zero、
affirmative confirmation gate，以及 sign-out 后所有旧 generation 响应失效。

本次后续实现的完整文件清单是：

```yaml
implementation_manifest:
  complete: true
  planned_paths:
    - tests/admin_dashboard/package.json
    - tests/admin_dashboard/package-lock.json
    - tests/admin_dashboard/admin_dashboard_dom.test.mjs
    - scripts/verify-gh1067.sh
    - .github/workflows/admin-dashboard-verification.yml
    - .gitignore
```

`src/server/routes/admin_dashboard/app.js` 明确不在本清单内。如果可执行测试首先
暴露真实实现缺陷，实施者必须停止，先用新的 spec-only amendment 更新清单与
相关不变量，再修改应用代码；不得借本清单静默扩展实现范围。

### CI 与证据契约

`.github/workflows/admin-dashboard-verification.yml` 在 pull request 的 exact
head SHA 上使用 `actions/checkout@v4`，使用 `actions/setup-node@v4` 安装
Node `24.14.0`，并运行 `bash scripts/verify-gh1067.sh`。workflow 必须以
`github.event.pull_request.head.sha`（非可能变化的 merge ref）作为 PR checkout
目标，并使用 `actions/upload-artifact@v4` 上传验证证据。

本地脚本以 `git rev-parse HEAD` 得到 `HEAD_SHA`，只在
`artifacts/logs/gh1067/<HEAD_SHA>/` 写入：

- `manifest.json`
- `admin_dashboard_dom.log`
- `checksums.sha256`
- `_SUCCESS`（仅全部命令成功后生成）

`.gitignore` 必须忽略 `/artifacts/logs/gh1067/`，并以
`!tests/admin_dashboard/package-lock.json` 覆盖仓库现有的全局 lockfile 忽略
规则；`node_modules/` 继续保持忽略。本地 SHA-scoped manifest、log、checksum
与 `_SUCCESS` 永不提交。远端证据是 GitHub Actions run URL 与该 run 上传的
artifact 名称/URL。

以上 workflow 只能描述为“current-head check evidence/check rollup”。它是否
成为 required 或 blocking check 由可变的 branch-protection 设置决定，本规格
不声称、也不要求它已经是 required/blocking check。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1, P10 | dashboard route registration and exact public paths | Actix handler tests plus `is_public_route` positive and near-match negative tests |
| P2, P3 | login/state code | asset contract tests for existing login shape, admin-role check, all-request generation/abort guards, memory-only state, and forbidden storage/sink APIs |
| P4 | key panel and request builder | asset contract tests for declared key endpoints, exactly-one ownership, non-wildcard model/endpoint permissions, and `is_admin=false`; focused existing key route tests |
| P5 | team panel and request builder | asset contract tests for declared team endpoints/fields; focused existing team route tests |
| P6 | finite-value formatter and spend renderers | deterministic asset contract assertions plus repeatable manual browser verification with explicit zero/missing/failed fixtures |
| P7, P8 | request generation, abort, status region, busy flags | deterministic all-request guard/abort contract assertions plus a repeatable slow/failing-request manual checklist |
| P9 | semantic HTML and CSS | deterministic asset assertions for labels, landmarks, live region, focus styling, and no color-only status plus a keyboard checklist |
| Security | CSP, no-store, safe DOM sinks | response-header tests and negative asset assertions for storage, `innerHTML`, `eval`, and external URLs |
| B1 | executable DOM generation and operation-order guards | delayed and out-of-order `fetch` resolutions prove that only the current session/operation commits |
| B2 | active `AbortController` registry and request finalization | success, failure, cancellation, and sign-out paths leave no stale controller; sign-out aborts all active requests |
| B3 | raw-key notice, copy, dismissal, and auth transition | the real DOM exposes the raw key once, copies only on explicit action, and cannot recover it after dismissal/auth transition |
| B4 | per-row usage rendering and partial-failure handling | mixed successful/failed usage responses retain successful rows, preserve numeric zero, and leave only failed/unavailable values blank with row errors |
| B5 | destructive-operation confirmation gates | `confirm=true` issues the declared request; `confirm=false` issues no request and makes no optimistic destructive DOM change |
| B6 | logout generation invalidation | resolve delayed login/list/usage/mutation/raw-key operations after sign-out and assert no credential, protected DOM/state, status, or secret is restored |

## Data Flow

Anonymous browser request → embedded shell/assets only → login form posts to
existing `/auth/login` → successful admin response yields an in-memory access
token → same-origin authenticated key/team requests → existing server-side
authorization and persistence → JSON response → safe DOM rendering.

No dashboard request bypasses the existing key/team authorization checks. No
new persistence, schema, migration, runtime frontend bundle/toolchain,
external service, or cross-origin call is added. Test-only Node dependencies
stay under `tests/admin_dashboard/` and never enter the deployment data flow.

## Alternatives Considered

- Separate SPA repository and deployment: rejected because the issue requests a
  minimum built-in surface and provides no cross-repository ownership or
  deployment contract.
- Add React/Vite or another runtime/deployment build tool: rejected because it
  adds runtime dependency, release, and generated-asset workflows
  disproportionate to the bounded feature. The isolated, locked,
  test-only `node:test` + jsdom harness is accepted because it executes the
  real embedded source without producing or shipping a frontend bundle.
- Put a token in a URL, browser storage, or embedded HTML: rejected because it
  leaks credentials through history, storage, logs, or source.
- Make management APIs public or proxy them through dashboard-only endpoints:
  rejected because existing API authorization is the correct authority and a
  proxy would duplicate contracts.
- Sum key and team spend into one total: rejected because the scopes can overlap
  and silently double-count.

## Risks

- Security: three exact anonymous asset paths sit under `/admin`; restrictive
  headers, exact-match route tests, inert compile-time content, safe DOM sinks,
  and unchanged API authorization constrain the exposure.
- Compatibility: all API shapes and protected-route behavior remain unchanged;
  only additive `GET` assets are registered.
- Performance: spend loads usage only for teams on the visible page and aborts
  stale refreshes; it does not fan out across the entire database.
- Maintenance: frontend contracts can drift from Rust response types; named API
  paths/fields, route tests, and a locked executable DOM harness make drift
  visible. The Node/jsdom dependency set is exact and test-only, so it does not
  create a second runtime dependency ecosystem.

## Test Plan

- [ ] Unit tests: dashboard content types, CSP/no-store headers, exact public
      route matching, asset safety/contract/accessibility assertions.
- [ ] Focused tests: dashboard module, middleware public/admin route helpers,
      existing key handler tests, and existing team route tests.
- [ ] Executable DOM tests: Node `24.14.0`, jsdom `29.1.1`,
      `(cd tests/admin_dashboard && npm ci --ignore-scripts)`, then
      `node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs` covers
      each of `B1`-`B6` against the real embedded JavaScript source.
- [ ] Evidence workflow: `bash scripts/verify-gh1067.sh` writes the ignored
      SHA-scoped manifest/log/checksum/`_SUCCESS`; exact-head Actions checkout
      uploads it with `actions/upload-artifact@v4`.
- [ ] Manual verification checklist: admin and non-admin login; key
      list/create/revoke; required safe key ownership/model/endpoint scope; team
      list/create/delete; explicit zero/missing/failed spend; delayed
      list/create responses followed by sign-out; reload clears auth and
      one-time key. Keyboard-only navigation, narrow-layout behavior, and
      real-browser visual rendering remain manual.
- [ ] Deterministic verification: `cargo fmt --check`, `cargo check`,
      `cargo clippy --all-targets -- -D warnings`, full `cargo test`, SpecRail
      workflow/spec checks, scope guard, and overlap guard.

## Rollback Plan

Remove the dashboard route registration, route module, exact public asset
entries, and embedded asset sources together. No stored data, configuration,
API contract, or migration requires rollback.
