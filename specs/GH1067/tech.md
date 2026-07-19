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
| Frontend assets | none | No HTML, CSS, JavaScript, build pipeline, or static file service exists | Add small compile-time embedded assets instead of a second toolchain |

## Proposed Design

### Embedded route module

Add `src/server/routes/admin_dashboard.rs` with three compile-time embedded
constants and handlers:

- `GET /admin/dashboard` → `text/html`
- `GET /admin/dashboard/app.css` → `text/css`
- `GET /admin/dashboard/app.js` → `application/javascript`

The source assets live under `src/server/routes/admin_dashboard/` and are
included with `include_str!`. Responses set `Cache-Control: no-store` and a CSP
equivalent to:

```text
default-src 'none'; script-src 'self'; style-src 'self';
connect-src 'self'; img-src 'self' data:; font-src 'self';
base-uri 'none'; form-action 'self'; frame-ancestors 'none'
```

The route module is registered from the existing route assembly. No filesystem
path, directory traversal surface, frontend package manager, runtime asset
directory, or new dependency is introduced.

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

The state object holds the access token, current view, key/team page numbers,
the latest successful key/team payloads, an `AbortController`, a monotonically
increasing request generation, and per-mutation busy flags. Each refresh aborts
the older refresh and validates the generation before committing results.
Sign-out increments the generation so late responses are ignored.

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
`name`, optional `description`, and optional `team_id`. The raw key response is
rendered into a one-time notice with an explicit copy button and is removed from
page state when dismissed or on the next authentication-state transition.
Create-team submits only `name`, optional `display_name`, and optional
`description`. Destructive operations use an explicit confirmation dialog and
disable the originating control until the request settles.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1, P10 | dashboard route registration and exact public paths | Actix handler tests plus `is_public_route` positive and near-match negative tests |
| P2, P3 | login/state code | asset contract tests for existing login shape, admin-role check, memory-only state, and forbidden storage/sink APIs |
| P4 | key panel and request builder | asset contract tests for declared key endpoints/fields; focused existing key route tests |
| P5 | team panel and request builder | asset contract tests for declared team endpoints/fields; focused existing team route tests |
| P6 | finite-value formatter and spend renderers | source-level unit contract assertions plus manual browser verification with zero/missing/failed fixtures |
| P7, P8 | request generation, abort, status region, busy flags | source-level contract assertions and manual slow/failing-request verification |
| P9 | semantic HTML and CSS | asset assertions for labels, landmarks, live region, focus styling, and no color-only status |
| Security | CSP, no-store, safe DOM sinks | response-header tests and negative asset assertions for storage, `innerHTML`, `eval`, and external URLs |

## Data Flow

Anonymous browser request → embedded shell/assets only → login form posts to
existing `/auth/login` → successful admin response yields an in-memory access
token → same-origin authenticated key/team requests → existing server-side
authorization and persistence → JSON response → safe DOM rendering.

No dashboard request bypasses the existing key/team authorization checks. No
new persistence, schema, migration, generated file, external service, or
cross-origin call is added.

## Alternatives Considered

- Separate SPA repository and deployment: rejected because the issue requests a
  minimum built-in surface and provides no cross-repository ownership or
  deployment contract.
- Add React/Vite or another build tool: rejected because it adds dependency,
  release, and generated-asset workflows disproportionate to the bounded
  feature.
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
  paths/fields and route tests make drift visible, while the scope avoids a
  second dependency ecosystem.

## Test Plan

- [ ] Unit tests: dashboard content types, CSP/no-store headers, exact public
      route matching, asset safety/contract/accessibility assertions.
- [ ] Focused tests: dashboard module, middleware public/admin route helpers,
      existing key handler tests, and existing team route tests.
- [ ] Manual verification: admin login; key list/create/revoke; team
      list/create/delete; zero/missing/failed spend; keyboard navigation; reload
      clears auth and one-time key.
- [ ] Deterministic verification: `cargo fmt --check`, `cargo check`,
      `cargo clippy --all-targets -- -D warnings`, full `cargo test`, SpecRail
      workflow/spec checks, scope guard, and overlap guard.

## Rollback Plan

Remove the dashboard route registration, route module, exact public asset
entries, and embedded asset sources together. No stored data, configuration,
API contract, or migration requires rollback.
