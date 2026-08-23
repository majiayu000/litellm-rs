## Role-Based Access Control (RBAC)

### Types

`Permission` and `Role` are plain structs; permissions are compared as strings
(`src/auth/rbac/types.rs`):

```rust
pub struct Permission {
    pub name: String,          // e.g. "users.read", "api.chat", "system.admin"
    pub description: String,
    pub resource: String,      // "users" | "teams" | "api" | "api_keys" | ...
    pub action: String,        // "read" | "write" | "delete" | "chat" | ...
    pub is_system: bool,
}

pub struct Role {
    pub name: String,
    pub description: String,
    pub permissions: HashSet<String>,
    pub parent_roles: HashSet<String>,   // inheritance sources
    pub is_system: bool,
}

pub struct PermissionCheck {           // result of check_permission_detailed
    pub granted: bool,
    pub granted_by_roles: Vec<String>,
    pub denial_reason: Option<String>,
}
```

### RbacSystem

`RbacSystem::new(config: &RbacConfig)` seeds 14 system permissions and 6 system
roles into plain `HashMap`s (not concurrent maps; the system is cloned behind
`Arc` in `AuthSystem`). Public API (`src/auth/rbac/{system,permissions,roles}.rs`):

- `get_user_permissions(user) -> Vec<String>` — resolves the user's role name to
  a role and expands it.
- `check_permissions(user_perms, required) -> bool` — all required present;
  `"*"` or `"system.admin"` in user perms grants everything.
- `check_any_permission(user_perms, required) -> bool` — same wildcards, any match.
- `check_permission_detailed(user, permission) -> PermissionCheck`.
- `check_resource_permission(user_perms, resource, action)` — builds
  `"{resource}.{action}"` and delegates.
- `is_admin(user)` — role is listed in `RbacConfig::admin_roles`
  (default `["admin", "superuser"]`).
- `get_role` / `add_role` / `get_permission` / `add_permission`
  (adding an `is_system: true` permission is rejected).
- `list_roles` / `list_permissions`.

Role inheritance is recursive via `get_role_permissions`
(`src/auth/rbac/helpers.rs`): a role's effective set is its own permissions plus
those of every resolvable parent. Missing parents are silently skipped; there is
no explicit cycle guard, so avoid self-referencing custom roles when calling
`add_role`. Default roles ship with empty `parent_roles`; inheritance activates
only for roles added at runtime.

Default roles: `super_admin` (all 14 permissions), `admin`, `manager`, `user`
(`api.chat`, `api.embeddings`, `api_keys.read`), `viewer` (read-only), `api_user`
(`api.chat`, `api.embeddings`, `api.images`).

Note the two parallel permission vocabularies: RBAC role permissions use dotted
`resource.action` names, while legacy per-role grants in
`AuthSystem::get_user_permissions` use colon forms (`read:all`, `use:api`).
Route enforcement accepts both — `permission_matches_operation` maps `api.chat`
to operation `chat` and `use:api` to any non-management operation.

### Enforcement points

There is no standalone `require_permission` route wrapper. Authorization runs
inside `AuthMiddleware` after authentication:

1. `api_key_allows_endpoint` — key-level endpoint pattern restrictions.
2. `operation_for_path(path)` + `check_permission(user, api_key, operation)` —
   the admin-vs-user two-role model described in middleware-pipeline.md.
3. Handler-level helpers for key runtime policy:
   `enforce_api_key_model_and_token_limits` (allowed_models /
   max_tokens_per_request from the key's `__core_keys` metadata payload).

For programmatic checks outside HTTP, use `AuthSystem::authorize(user,
permissions) -> AuthzResult`, which wraps `get_user_permissions` +
`check_permissions`.
