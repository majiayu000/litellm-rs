use crate::auth::oauth::types::UserInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Session data stored for authenticated users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthSession {
    /// Session ID
    pub session_id: String,

    /// User information from OAuth provider
    pub user_info: UserInfo,

    /// Access token from the OAuth provider
    pub access_token: String,

    /// Refresh token (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,

    /// ID token (for OIDC)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,

    /// When the access token expires
    pub token_expires_at: DateTime<Utc>,

    /// When the session was created
    pub created_at: DateTime<Utc>,

    /// When the session was last accessed
    pub last_accessed_at: DateTime<Utc>,

    /// Session expiration time
    pub expires_at: DateTime<Utc>,

    /// IP address of the client
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,

    /// User agent of the client
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,

    /// Internal user ID (after user creation/lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_user_id: Option<Uuid>,

    /// Assigned role
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

impl OAuthSession {
    /// Create a new OAuth session
    pub fn new(
        user_info: UserInfo,
        access_token: String,
        token_expires_in: u64,
        session_ttl: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            session_id: Uuid::new_v4().to_string(),
            user_info,
            access_token,
            refresh_token: None,
            id_token: None,
            token_expires_at: now + chrono::Duration::seconds(token_expires_in as i64),
            created_at: now,
            last_accessed_at: now,
            expires_at: now + chrono::Duration::seconds(session_ttl as i64),
            ip_address: None,
            user_agent: None,
            internal_user_id: None,
            role: None,
        }
    }

    /// Set the refresh token
    pub fn with_refresh_token(mut self, token: impl Into<String>) -> Self {
        self.refresh_token = Some(token.into());
        self
    }

    /// Set the ID token
    pub fn with_id_token(mut self, token: impl Into<String>) -> Self {
        self.id_token = Some(token.into());
        self
    }

    /// Set client metadata
    pub fn with_client_info(
        mut self,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Self {
        self.ip_address = ip_address;
        self.user_agent = user_agent;
        self
    }

    /// Set the internal user ID
    pub fn with_internal_user_id(mut self, user_id: Uuid) -> Self {
        self.internal_user_id = Some(user_id);
        self
    }

    /// Set the role
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Check if the session has expired
    pub fn is_expired(&self) -> bool {
        Utc::now() > self.expires_at
    }

    /// Check if the access token has expired
    pub fn is_token_expired(&self) -> bool {
        Utc::now() > self.token_expires_at
    }

    /// Update the last accessed timestamp
    pub fn touch(&mut self) {
        self.last_accessed_at = Utc::now();
    }

    /// Extend the session expiration
    pub fn extend(&mut self, additional_seconds: u64) {
        self.expires_at += chrono::Duration::seconds(additional_seconds as i64);
    }

    /// Update the access token
    pub fn update_token(&mut self, access_token: String, expires_in: u64) {
        self.access_token = access_token;
        self.token_expires_at = Utc::now() + chrono::Duration::seconds(expires_in as i64);
    }
}
