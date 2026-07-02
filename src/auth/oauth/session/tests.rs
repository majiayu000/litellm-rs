use super::*;
use crate::auth::oauth::types::{OAuthState, UserInfo};
use chrono::Utc;
use uuid::Uuid;

fn create_test_user_info() -> UserInfo {
    UserInfo::new("123", "test@example.com", "google").with_name("Test User")
}

fn create_test_session() -> OAuthSession {
    OAuthSession::new(
        create_test_user_info(),
        "access_token_123".to_string(),
        3600,
        7200,
    )
}

#[test]
fn test_session_creation() {
    let session = create_test_session();

    assert!(!session.session_id.is_empty());
    assert_eq!(session.user_info.email, "test@example.com");
    assert_eq!(session.access_token, "access_token_123");
    assert!(!session.is_expired());
    assert!(!session.is_token_expired());
}

#[test]
fn test_session_builder() {
    let session = OAuthSession::new(
        create_test_user_info(),
        "access_token".to_string(),
        3600,
        7200,
    )
    .with_refresh_token("refresh_token")
    .with_id_token("id_token")
    .with_client_info(
        Some("127.0.0.1".to_string()),
        Some("Mozilla/5.0".to_string()),
    )
    .with_internal_user_id(Uuid::new_v4())
    .with_role("admin");

    assert_eq!(session.refresh_token, Some("refresh_token".to_string()));
    assert_eq!(session.id_token, Some("id_token".to_string()));
    assert!(session.ip_address.is_some());
    assert!(session.user_agent.is_some());
    assert!(session.internal_user_id.is_some());
    assert_eq!(session.role, Some("admin".to_string()));
}

#[test]
fn test_session_expiration() {
    let mut session = create_test_session();
    session.expires_at = Utc::now() - chrono::Duration::seconds(1);
    assert!(session.is_expired());
}

#[test]
fn test_token_expiration() {
    let mut session = create_test_session();
    session.token_expires_at = Utc::now() - chrono::Duration::seconds(1);
    assert!(session.is_token_expired());
}

#[test]
fn test_session_touch() {
    let mut session = create_test_session();
    let original = session.last_accessed_at;
    std::thread::sleep(std::time::Duration::from_millis(10));
    session.touch();
    assert!(session.last_accessed_at > original);
}

#[test]
fn test_session_extend() {
    let mut session = create_test_session();
    let original = session.expires_at;
    session.extend(3600);
    assert!(session.expires_at > original);
}

#[test]
fn test_session_update_token() {
    let mut session = create_test_session();
    session.update_token("new_access_token".to_string(), 7200);
    assert_eq!(session.access_token, "new_access_token");
}

#[tokio::test]
async fn test_in_memory_session_store() {
    let store = InMemorySessionStore::new();
    let session = create_test_session();
    let session_id = session.session_id.clone();

    // Set session
    store.set(session).await.unwrap();

    // Get session
    let retrieved = store.get(&session_id).await.unwrap();
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().session_id, session_id);

    // Delete session
    store.delete(&session_id).await.unwrap();
    let deleted = store.get(&session_id).await.unwrap();
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_in_memory_state_store() {
    let store = InMemorySessionStore::new();
    let state = OAuthState::new("google");
    let state_id = state.state.clone();

    // Set state
    store.set_state(state).await.unwrap();

    // Get and delete state
    let retrieved = store.get_and_delete_state(&state_id).await.unwrap();
    assert!(retrieved.is_some());

    // Should be deleted after retrieval
    let again = store.get_and_delete_state(&state_id).await.unwrap();
    assert!(again.is_none());
}

#[tokio::test]
async fn test_in_memory_user_sessions() {
    let store = InMemorySessionStore::new();

    let mut session1 = create_test_session();
    session1.user_info.email = "user@example.com".to_string();

    let mut session2 = create_test_session();
    session2.user_info.email = "user@example.com".to_string();

    store.set(session1).await.unwrap();
    store.set(session2).await.unwrap();

    let user_sessions = store.get_user_sessions("user@example.com").await.unwrap();
    assert_eq!(user_sessions.len(), 2);

    let deleted = store
        .delete_user_sessions("user@example.com")
        .await
        .unwrap();
    assert_eq!(deleted, 2);

    let after_delete = store.get_user_sessions("user@example.com").await.unwrap();
    assert!(after_delete.is_empty());
}

#[tokio::test]
async fn test_in_memory_cleanup_expired() {
    let store = InMemorySessionStore::new();

    let mut expired_session = create_test_session();
    expired_session.expires_at = Utc::now() - chrono::Duration::seconds(1);
    store.set(expired_session.clone()).await.unwrap();

    let valid_session = create_test_session();
    store.set(valid_session.clone()).await.unwrap();

    let cleaned = store.cleanup_expired().await.unwrap();
    assert_eq!(cleaned, 1);

    // Expired session should be gone
    let retrieved = store.get(&expired_session.session_id).await.unwrap();
    assert!(retrieved.is_none());

    // Valid session should still exist
    let still_valid = store.get(&valid_session.session_id).await.unwrap();
    assert!(still_valid.is_some());
}

#[tokio::test]
async fn test_in_memory_expired_state_cleanup() {
    let store = InMemorySessionStore::new();

    let mut expired_state = OAuthState::new("google");
    expired_state.created_at = Utc::now() - chrono::Duration::seconds(700);
    store.set_state(expired_state.clone()).await.unwrap();

    // Expired state should return None
    let retrieved = store
        .get_and_delete_state(&expired_state.state)
        .await
        .unwrap();
    assert!(retrieved.is_none());
}

#[test]
fn test_session_serialization() {
    let session = create_test_session();
    let json = serde_json::to_string(&session).unwrap();
    let parsed: OAuthSession = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.session_id, session.session_id);
    assert_eq!(parsed.user_info.email, session.user_info.email);
}

#[test]
fn test_session_error_display() {
    assert_eq!(SessionError::NotFound.to_string(), "Session not found");
    assert_eq!(SessionError::Expired.to_string(), "Session expired");
    assert!(
        SessionError::Storage("test".to_string())
            .to_string()
            .contains("test")
    );
}
