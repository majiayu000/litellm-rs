use super::{OAuthSession, SessionError, SessionStore};
use crate::auth::oauth::types::OAuthState;
use async_trait::async_trait;
use chrono::Utc;
use dashmap::DashMap;
use std::sync::Arc;
use std::time::Duration;

/// In-memory session store implementation
#[derive(Clone)]
pub struct InMemorySessionStore {
    sessions: Arc<DashMap<String, OAuthSession>>,
    states: Arc<DashMap<String, OAuthState>>,
    cleanup_interval: Duration,
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySessionStore {
    /// Create a new in-memory session store
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            states: Arc::new(DashMap::new()),
            cleanup_interval: Duration::from_secs(300), // 5 minutes
        }
    }

    /// Create with custom cleanup interval
    pub fn with_cleanup_interval(mut self, interval: Duration) -> Self {
        self.cleanup_interval = interval;
        self
    }

    /// Start background cleanup task
    pub fn start_cleanup_task(self: Arc<Self>) {
        let store = self.clone();
        let interval = self.cleanup_interval;

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                if let Err(e) = store.cleanup_expired().await {
                    tracing::warn!("Session cleanup error: {}", e);
                }
            }
        });
    }
}

impl std::fmt::Debug for InMemorySessionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InMemorySessionStore")
            .field("session_count", &self.sessions.len())
            .field("state_count", &self.states.len())
            .finish()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn set(&self, session: OAuthSession) -> Result<(), SessionError> {
        self.sessions.insert(session.session_id.clone(), session);
        Ok(())
    }

    async fn get(&self, session_id: &str) -> Result<Option<OAuthSession>, SessionError> {
        match self.sessions.get(session_id) {
            Some(entry) => {
                let session = entry.value().clone();
                if session.is_expired() {
                    drop(entry);
                    self.sessions.remove(session_id);
                    Ok(None)
                } else {
                    Ok(Some(session))
                }
            }
            None => Ok(None),
        }
    }

    async fn delete(&self, session_id: &str) -> Result<(), SessionError> {
        self.sessions.remove(session_id);
        Ok(())
    }

    async fn update(&self, session: OAuthSession) -> Result<(), SessionError> {
        self.sessions.insert(session.session_id.clone(), session);
        Ok(())
    }

    async fn set_state(&self, state: OAuthState) -> Result<(), SessionError> {
        self.states.insert(state.state.clone(), state);
        Ok(())
    }

    async fn get_and_delete_state(
        &self,
        state_id: &str,
    ) -> Result<Option<OAuthState>, SessionError> {
        match self.states.remove(state_id) {
            Some((_, state)) => {
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
        let sessions: Vec<OAuthSession> = self
            .sessions
            .iter()
            .filter(|entry| {
                entry.value().user_info.email == user_email && !entry.value().is_expired()
            })
            .map(|entry| entry.value().clone())
            .collect();
        Ok(sessions)
    }

    async fn delete_user_sessions(&self, user_email: &str) -> Result<usize, SessionError> {
        let to_delete: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.value().user_info.email == user_email)
            .map(|entry| entry.key().clone())
            .collect();

        let count = to_delete.len();
        for session_id in to_delete {
            self.sessions.remove(&session_id);
        }
        Ok(count)
    }

    async fn cleanup_expired(&self) -> Result<usize, SessionError> {
        let now = Utc::now();

        // Clean up expired sessions
        let expired_sessions: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.value().expires_at < now)
            .map(|entry| entry.key().clone())
            .collect();

        let session_count = expired_sessions.len();
        for session_id in expired_sessions {
            self.sessions.remove(&session_id);
        }

        // Clean up expired states
        let expired_states: Vec<String> = self
            .states
            .iter()
            .filter(|entry| entry.value().is_expired())
            .map(|entry| entry.key().clone())
            .collect();

        for state_id in expired_states {
            self.states.remove(&state_id);
        }

        Ok(session_count)
    }
}
