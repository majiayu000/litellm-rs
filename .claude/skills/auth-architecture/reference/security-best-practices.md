## Security Best Practices

### 1. Secure Secret Generation

```rust
use rand::RngCore;

pub fn generate_jwt_secret() -> String {
    let mut secret = [0u8; 64];
    rand::thread_rng().fill_bytes(&mut secret);
    base64::encode(secret)
}
```

### 2. Constant-Time Comparison

```rust
use subtle::ConstantTimeEq;

fn secure_compare(a: &[u8], b: &[u8]) -> bool {
    a.ct_eq(b).into()
}
```

### 3. Key Rotation

```rust
impl ApiKeyManager {
    pub async fn rotate_key(&self, key_id: &str) -> Result<ApiKey, AuthError> {
        // Create new key
        let old_key = self.storage.get_key(key_id).await?
            .ok_or(AuthError::KeyNotFound)?;

        let new_key = self.create_key(&old_key.user_id, &format!("{} (rotated)", old_key.name)).await?;

        // Mark old key for delayed deletion
        self.storage.mark_for_deletion(key_id, Duration::from_secs(86400)).await?;

        Ok(new_key)
    }
}
```

### 4. Audit Logging

```rust
pub struct AuthAuditLog {
    logger: Arc<dyn AuditLogger>,
}

impl AuthAuditLog {
    pub fn log_auth_success(&self, context: &AuthContext, endpoint: &str) {
        self.logger.log(AuditEvent {
            event_type: "auth_success",
            user_id: &context.user_id,
            auth_method: &context.auth_method.to_string(),
            endpoint,
            timestamp: chrono::Utc::now(),
            success: true,
            error: None,
        });
    }

    pub fn log_auth_failure(&self, error: &AuthError, endpoint: &str) {
        self.logger.log(AuditEvent {
            event_type: "auth_failure",
            user_id: "unknown",
            auth_method: "unknown",
            endpoint,
            timestamp: chrono::Utc::now(),
            success: false,
            error: Some(error.to_string()),
        });
    }
}
```
