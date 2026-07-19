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
- Creating or deploying a separate frontend repository or frontend build
  service.

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
- [ ] Route registration, response headers, static content/API contracts, and
      unsafe browser primitives have deterministic automated coverage;
      authentication-state, empty/error, concurrency, and accessibility
      behavior follow a repeatable manual verification checklist.

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

## Rollout Notes

The dashboard is an additive built-in surface. It reuses existing authorization
and management APIs and requires no data migration. A code rollback removes the
dashboard route and embedded assets; existing API behavior and stored data are
unchanged.
