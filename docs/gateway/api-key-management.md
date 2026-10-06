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
