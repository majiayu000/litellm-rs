use super::{OAuthSession, SessionError, SessionStore};
use crate::auth::oauth::types::OAuthState;
use async_trait::async_trait;
use chrono::Utc;

/// Redis session store implementation
pub struct RedisSessionStore {
    client: redis::Client,
    prefix: String,
    session_ttl: u64,
    state_ttl: u64,
}

impl RedisSessionStore {
    /// Create a new Redis session store
    pub fn new(redis_url: &str) -> Result<Self, SessionError> {
        let client =
            redis::Client::open(redis_url).map_err(|e| SessionError::Connection(e.to_string()))?;

        Ok(Self {
            client,
            prefix: "oauth:".to_string(),
            session_ttl: 3600,
            state_ttl: 600,
        })
    }

    /// Set custom prefix for Redis keys
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set session TTL
    pub fn with_session_ttl(mut self, ttl: u64) -> Self {
        self.session_ttl = ttl;
        self
    }

    /// Set state TTL
    pub fn with_state_ttl(mut self, ttl: u64) -> Self {
        self.state_ttl = ttl;
        self
    }

    fn session_key(&self, session_id: &str) -> String {
        format!("{}session:{}", self.prefix, session_id)
    }

    fn state_key(&self, state_id: &str) -> String {
        format!("{}state:{}", self.prefix, state_id)
    }

    fn user_sessions_key(&self, email: &str) -> String {
        format!("{}user_sessions:{}", self.prefix, email)
    }
}

impl std::fmt::Debug for RedisSessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RedisSessionStore")
            .field("prefix", &self.prefix)
            .field("session_ttl", &self.session_ttl)
            .finish()
    }
}

#[async_trait]
impl SessionStore for RedisSessionStore {
    async fn set(&self, session: OAuthSession) -> Result<(), SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let key = self.session_key(&session.session_id);
        let value = serde_json::to_string(&session)
            .map_err(|e| SessionError::Serialization(e.to_string()))?;

        // Calculate TTL from session expiration
        let ttl = (session.expires_at - Utc::now()).num_seconds().max(0) as u64;

        let _: () = conn
            .set_ex(&key, &value, ttl)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        // Add to user's session set
        let user_key = self.user_sessions_key(&session.user_info.email);
        let _: () = conn
            .sadd(&user_key, &session.session_id)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;
        let _: () = conn
            .expire(&user_key, ttl as i64)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn get(&self, session_id: &str) -> Result<Option<OAuthSession>, SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let key = self.session_key(session_id);
        let value: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        match value {
            Some(v) => {
                let session: OAuthSession = serde_json::from_str(&v)
                    .map_err(|e| SessionError::Serialization(e.to_string()))?;

                if session.is_expired() {
                    self.delete(session_id).await?;
                    Ok(None)
                } else {
                    Ok(Some(session))
                }
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        // Get session first to remove from user's set
        if let Some(session) = self.get(session_id).await? {
            let user_key = self.user_sessions_key(&session.user_info.email);
            let _: () = conn
                .srem(&user_key, session_id)
                .await
                .map_err(|e| SessionError::Storage(e.to_string()))?;
        }

        let key = self.session_key(session_id);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn update(&self, session: OAuthSession) -> Result<(), SessionError> {
        self.set(session).await
    }

    async fn set_state(&self, state: OAuthState) -> Result<(), SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let key = self.state_key(&state.state);
        let value = serde_json::to_string(&state)
            .map_err(|e| SessionError::Serialization(e.to_string()))?;

        let _: () = conn
            .set_ex(&key, &value, state.ttl_seconds)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        Ok(())
    }

    async fn get_and_delete_state(
        &self,
        state_id: &str,
    ) -> Result<Option<OAuthState>, SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let key = self.state_key(state_id);

        // Get and delete atomically
        let value: Option<String> = conn
            .get_del(&key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        match value {
            Some(v) => {
                let state: OAuthState = serde_json::from_str(&v)
                    .map_err(|e| SessionError::Serialization(e.to_string()))?;

                if state.is_expired() {
                    Ok(None)
                } else {
                    Ok(Some(state))
                }
            }
            None => Ok(None),
        }
    }

    async fn get_user_sessions(&self, user_email: &str) -> Result<Vec<OAuthSession>, SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let user_key = self.user_sessions_key(user_email);
        let session_ids: Vec<String> = conn
            .smembers(&user_key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        let mut sessions = Vec::new();
        for session_id in session_ids {
            if let Some(session) = self.get(&session_id).await? {
                sessions.push(session);
            }
        }

        Ok(sessions)
    }

    async fn delete_user_sessions(&self, user_email: &str) -> Result<usize, SessionError> {
        use redis::AsyncCommands;

        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SessionError::Connection(e.to_string()))?;

        let user_key = self.user_sessions_key(user_email);
        let session_ids: Vec<String> = conn
            .smembers(&user_key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        let count = session_ids.len();

        for session_id in &session_ids {
            let key = self.session_key(session_id);
            let _: () = conn
                .del(&key)
                .await
                .map_err(|e| SessionError::Storage(e.to_string()))?;
        }

        let _: () = conn
            .del(&user_key)
            .await
            .map_err(|e| SessionError::Storage(e.to_string()))?;

        Ok(count)
    }

    async fn cleanup_expired(&self) -> Result<usize, SessionError> {
        // Redis handles TTL-based expiration automatically
        // This is a no-op for Redis
        Ok(0)
    }
}
