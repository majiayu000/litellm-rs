# API key management authorization

When authentication is enabled, `/v1/keys` management routes require both
credential authority and the existing user/team ownership rules.

| Route | Explicit API-key permission |
| --- | --- |
| `POST /v1/keys` | `api_keys.write` |
| `PUT /v1/keys/{id}` | `api_keys.write` |
| `POST /v1/keys/{id}/rotate` | `api_keys.write` |
| `DELETE /v1/keys/{id}` | `api_keys.delete` |
| `GET /v1/keys`, `GET /v1/keys/{id}`, `GET /v1/keys/{id}/usage` | `api_keys.read` or `keys.list_all` |

These grants can be stored in the authentication key's `permissions` or the
virtual key's `permissions.custom_permissions`. Explicit admin keys (`is_admin`,
`*`, or `system.admin`) satisfy the credential check. The route still applies
its existing ownership, target scope, and management-grant restrictions.

An inference key does not acquire these permissions from its owner's admin role.
An empty operation list, model-only limits, token limits, or `use:api` do not
authorize credential management. Changing the key's name, budget, expiry or
policy and rotating/revoking it all require the corresponding management grant.
Denial happens before looking up a target key, so an unauthorized caller cannot
distinguish an existing target from a missing one through these routes.

Authenticated user/session callers keep their existing ability to manage keys
within their authorized scope. They can issue a separate automation credential
with explicit management permissions. Treat that credential as authority to
manage keys within its scope; it is not an attenuated child inference credential.
API-key clients that previously relied on owner-role inheritance must switch to
an appropriately authorized session or an explicitly granted management key.

`POST /v1/keys/verify` keeps its existing self-verification and ownership rules;
it does not require a management grant to verify the key being presented.
Unknown/foreign-key probing requires an explicit `keys.list_all` or admin grant
on an API key, rather than an inherited owner role. Authentication-disabled
development mode is unchanged.

## Delegating management authority

`api_keys.write` permits ordinary key creation, updates, and rotation within the
caller's existing ownership scope. It does not permit a restricted automation
key to inherit global authority from its administrator owner.

Creating a key with management permissions, upgrading a key to those permissions,
or rotating an existing management key requires both an administrator owner and
global admin authority on the presented API key (`is_admin`, `*`, or
`system.admin`). An operation alias such as `api.system.admin` and `use:api`
are not global admin grants. Management permissions include `keys.list_all`, `users.manage`, `config.manage`,
`teams.manage`, and `analytics.admin`, as well as global admin grants. Rotation
uses this same rule because it returns a new secret carrying the target's
permissions. Authenticated administrator sessions retain their existing grant
ability; a global API-key grant does not remove owner restrictions.

This boundary concerns management/admin grants. It does not introduce general
child-key attenuation for model allowlists, budgets, token limits, or target
scope, nor change those existing ownership and policy rules.
