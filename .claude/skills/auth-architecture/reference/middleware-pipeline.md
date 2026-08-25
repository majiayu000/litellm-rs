## Middleware Pipeline

### Combined Auth Middleware

```rust
pub struct AuthMiddleware {
    api_key_manager: Arc<ApiKeyManager>,
    jwt_manager: Arc<JwtManager>,
    rbac_manager: Arc<RbacManager>,
    rate_limiter: Arc<RateLimiter>,
}

impl AuthMiddleware {
    pub async fn authenticate(&self, req: &ServiceRequest) -> Result<AuthContext, AuthError> {
        let auth_header = req
            .headers()
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(AuthError::MissingCredentials)?;

        // Determine auth method and validate
        let context = if auth_header.starts_with("Bearer sk-") {
            // API Key
            let key = auth_header.strip_prefix("Bearer ").unwrap();
            let api_key = self.api_key_manager.validate_key(key).await?;
            AuthContext {
                user_id: api_key.user_id,
                permissions: api_key.permissions,
                auth_method: AuthMethod::ApiKey,
            }
        } else if auth_header.starts_with("Bearer ey") {
            // JWT (starts with "ey" after base64 encoding)
            let token = auth_header.strip_prefix("Bearer ").unwrap();
            let claims = self.jwt_manager.validate_token(token)?;
            AuthContext {
                user_id: claims.sub,
                permissions: claims.permissions,
                auth_method: AuthMethod::Jwt,
            }
        } else {
            return Err(AuthError::InvalidCredentials);
        };

        // Check rate limits
        self.rate_limiter.check_rate_limit(&context.user_id)?;

        Ok(context)
    }

    pub fn authorize(&self, context: &AuthContext, required: &Permission) -> Result<(), AuthError> {
        if context.permissions.contains(&required.to_string()) {
            Ok(())
        } else {
            self.rbac_manager.check_permission(&context.user_id, required)
        }
    }
}
```
