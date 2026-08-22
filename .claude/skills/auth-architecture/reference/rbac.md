## Role-Based Access Control (RBAC)

### Permission Model

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub permissions: Vec<Permission>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Permission {
    // Chat permissions
    ChatCompletion,
    ChatCompletionStream,

    // Embedding permissions
    Embeddings,

    // Model permissions
    ListModels,
    GetModelInfo,

    // Admin permissions
    ManageUsers,
    ManageApiKeys,
    ViewMetrics,
    ConfigureProviders,

    // Provider-specific
    UseProvider(String),
    UseModel(String),
}

pub struct RbacManager {
    roles: DashMap<String, Role>,
    user_roles: DashMap<String, Vec<String>>,
}

impl RbacManager {
    pub fn has_permission(&self, user_id: &str, permission: &Permission) -> bool {
        let user_role_ids = match self.user_roles.get(user_id) {
            Some(roles) => roles.clone(),
            None => return false,
        };

        for role_id in user_role_ids {
            if let Some(role) = self.roles.get(&role_id) {
                if role.permissions.contains(permission) {
                    return true;
                }
            }
        }

        false
    }

    pub fn check_permission(&self, user_id: &str, permission: &Permission) -> Result<(), AuthError> {
        if self.has_permission(user_id, permission) {
            Ok(())
        } else {
            Err(AuthError::InsufficientPermissions)
        }
    }
}
```

### RBAC Middleware

```rust
pub fn require_permission(permission: Permission) -> impl Fn(ServiceRequest) -> Result<ServiceRequest, AuthError> {
    move |req: ServiceRequest| {
        let auth_context = req
            .extensions()
            .get::<AuthContext>()
            .ok_or(AuthError::MissingContext)?
            .clone();

        if !auth_context.permissions.contains(&permission.to_string()) {
            return Err(AuthError::InsufficientPermissions);
        }

        Ok(req)
    }
}

// Usage in route configuration
app.route(
    "/chat/completions",
    web::post()
        .wrap(require_permission(Permission::ChatCompletion))
        .to(chat_completion_handler)
)
```
