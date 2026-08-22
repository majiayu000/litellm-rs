---
name: auth-architecture
description: LiteLLM-RS Authentication Architecture. Covers JWT + API Key + RBAC multi-method auth, rate limiting with DashMap, middleware pipeline, and secure credential management. Use when adding auth methods, debugging JWT/API-key validation, implementing RBAC permission checks, or tuning rate limiting and auth configuration.
---

# Authentication Architecture Guide

## Overview

LiteLLM-RS implements a multi-layered authentication system supporting JWT tokens, API keys, and Role-Based Access Control (RBAC) with lock-free rate limiting.

### Authentication Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Request                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Auth Middleware                               │
│  1. Extract credentials (JWT/API Key)                           │
│  2. Validate credentials                                         │
│  3. Load user context                                           │
│  4. Check rate limits                                           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   RBAC Middleware                                │
│  1. Check required permissions                                   │
│  2. Validate resource access                                     │
│  3. Log access attempt                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                     Handler                                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## API Key Authentication

### Key Generation

```rust
use rand::Rng;

pub fn generate_api_key() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const KEY_LENGTH: usize = 64;

    let mut rng = rand::thread_rng();
    let key: String = (0..KEY_LENGTH)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    format!("sk-{}", key)  // Prefix for easy identification
}
```

### Key Storage

```rust
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand_core::OsRng;

pub struct ApiKeyManager {
    storage: Arc<dyn KeyStorage>,
    hasher: Argon2<'static>,
}

impl ApiKeyManager {
    pub fn new(storage: Arc<dyn KeyStorage>) -> Self {
        Self {
            storage,
            hasher: Argon2::default(),
        }
    }

    pub async fn create_key(&self, user_id: &str, name: &str) -> Result<ApiKey, AuthError> {
        let raw_key = generate_api_key();
        let salt = SaltString::generate(&mut OsRng);
        let hash = self.hasher
            .hash_password(raw_key.as_bytes(), &salt)
            .map_err(|e| AuthError::Internal(e.to_string()))?
            .to_string();

        let key = ApiKey {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: user_id.to_string(),
            name: name.to_string(),
            key_hash: hash,
            prefix: raw_key[..10].to_string(),  // Store prefix for lookup
            created_at: chrono::Utc::now(),
            expires_at: None,
            last_used_at: None,
            permissions: vec![],
        };

        self.storage.store_key(&key).await?;

        // Return the raw key only once - user must save it
        Ok(ApiKey {
            key_hash: raw_key,  // Return raw key instead of hash
            ..key
        })
    }

    pub async fn validate_key(&self, raw_key: &str) -> Result<ApiKey, AuthError> {
        let prefix = &raw_key[..10.min(raw_key.len())];
        let key = self.storage
            .get_key_by_prefix(prefix)
            .await?
            .ok_or(AuthError::InvalidCredentials)?;

        // Verify the hash
        let parsed_hash = argon2::PasswordHash::new(&key.key_hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        self.hasher
            .verify_password(raw_key.as_bytes(), &parsed_hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        // Check expiration
        if let Some(expires_at) = key.expires_at {
            if expires_at < chrono::Utc::now() {
                return Err(AuthError::ExpiredCredentials);
            }
        }

        // Update last used timestamp
        self.storage.update_last_used(&key.id).await?;

        Ok(key)
    }
}
```

### API Key Middleware

```rust
use actix_web::{HttpRequest, HttpMessage, dev::ServiceRequest};

pub async fn api_key_middleware(
    req: ServiceRequest,
    key_manager: Arc<ApiKeyManager>,
) -> Result<ServiceRequest, AuthError> {
    // Extract API key from header
    let api_key = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingCredentials)?;

    // Validate key
    let key = key_manager.validate_key(api_key).await?;

    // Store user context in request extensions
    req.extensions_mut().insert(AuthContext {
        user_id: key.user_id.clone(),
        permissions: key.permissions.clone(),
        auth_method: AuthMethod::ApiKey,
    });

    Ok(req)
}
```

---

## JWT Authentication

### Token Structure

```rust
use serde::{Deserialize, Serialize};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,          // User ID
    pub exp: usize,           // Expiration timestamp
    pub iat: usize,           // Issued at timestamp
    pub iss: String,          // Issuer
    pub aud: String,          // Audience
    pub roles: Vec<String>,   // User roles
    pub permissions: Vec<String>,  // Direct permissions
}

pub struct JwtManager {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    issuer: String,
    audience: String,
    token_expiry: Duration,
}

impl JwtManager {
    pub fn new(secret: &[u8], issuer: String, audience: String) -> Self {
        Self {
            encoding_key: EncodingKey::from_secret(secret),
            decoding_key: DecodingKey::from_secret(secret),
            issuer,
            audience,
            token_expiry: Duration::from_secs(3600),  // 1 hour default
        }
    }

    pub fn create_token(&self, user_id: &str, roles: Vec<String>, permissions: Vec<String>) -> Result<String, AuthError> {
        let now = chrono::Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            exp: (now + chrono::Duration::from_std(self.token_expiry).unwrap()).timestamp() as usize,
            iat: now.timestamp() as usize,
            iss: self.issuer.clone(),
            aud: self.audience.clone(),
            roles,
            permissions,
        };

        encode(&Header::new(Algorithm::HS256), &claims, &self.encoding_key)
            .map_err(|e| AuthError::TokenCreation(e.to_string()))
    }

    pub fn validate_token(&self, token: &str) -> Result<Claims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_issuer(&[&self.issuer]);
        validation.set_audience(&[&self.audience]);

        decode::<Claims>(token, &self.decoding_key, &validation)
            .map(|data| data.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::ExpiredCredentials,
                jsonwebtoken::errors::ErrorKind::InvalidToken => AuthError::InvalidCredentials,
                _ => AuthError::TokenValidation(e.to_string()),
            })
    }
}
```

### JWT Middleware

```rust
pub async fn jwt_middleware(
    req: ServiceRequest,
    jwt_manager: Arc<JwtManager>,
) -> Result<ServiceRequest, AuthError> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(AuthError::MissingCredentials)?;

    let claims = jwt_manager.validate_token(token)?;

    req.extensions_mut().insert(AuthContext {
        user_id: claims.sub,
        permissions: claims.permissions,
        auth_method: AuthMethod::Jwt,
    });

    Ok(req)
}
```

---

## References
- [reference/rbac.md](reference/rbac.md) — RBAC permission model, roles, and route-level permission guards
- [reference/rate-limiting.md](reference/rate-limiting.md) — lock-free DashMap sliding-window limiter and rate-limit middleware
- [reference/middleware-pipeline.md](reference/middleware-pipeline.md) — combined auth middleware tying API key, JWT, RBAC, and rate limiting together
- [reference/configuration.md](reference/configuration.md) — auth, JWT, API key, rate-limiting, and RBAC YAML configuration
- [reference/security-best-practices.md](reference/security-best-practices.md) — secret generation, constant-time comparison, key rotation, audit logging
- [reference/error-types.md](reference/error-types.md) — the AuthError enum variants used across the auth stack
