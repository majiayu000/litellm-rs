use super::OAuthSession;
use crate::auth::oauth::types::OAuthState;
use async_trait::async_trait;

/// Session store trait for managing OAuth sessions
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Store a session
    async fn set(&self, session: OAuthSession) -> Result<(), SessionError>;

    /// Retrieve a session by ID
    async fn get(&self, session_id: &str) -> Result<Option<OAuthSession>, SessionError>;

    /// Delete a session
    async fn delete(&self, session_id: &str) -> Result<(), SessionError>;

    /// Update a session
    async fn update(&self, session: OAuthSession) -> Result<(), SessionError>;

    /// Store an OAuth state for CSRF protection
    async fn set_state(&self, state: OAuthState) -> Result<(), SessionError>;

    /// Retrieve and remove an OAuth state
    async fn get_and_delete_state(
        &self,
        state_id: &str,
    ) -> Result<Option<OAuthState>, SessionError>;

    /// Get all sessions for a user
    async fn get_user_sessions(&self, user_email: &str) -> Result<Vec<OAuthSession>, SessionError>;

    /// Delete all sessions for a user
    async fn delete_user_sessions(&self, user_email: &str) -> Result<usize, SessionError>;

    /// Clean up expired sessions
    async fn cleanup_expired(&self) -> Result<usize, SessionError>;
}

/// Session store errors
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("Session not found")]
    NotFound,

    #[error("Session expired")]
    Expired,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Connection error: {0}")]
    Connection(String),
}
