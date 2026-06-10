//! JWT module tests

#[cfg(test)]
use crate::auth::jwt::types::{JwtHandler, TokenType};
use crate::config::models::auth::AuthConfig;
use uuid::Uuid;

async fn create_test_handler() -> JwtHandler {
    let config = AuthConfig {
        jwt_secret: "test_secret_key_for_testing_only".to_string(),
        jwt_expiration: 3600,
        api_key_header: "Authorization".to_string(),
        enable_api_key: true,
        enable_jwt: true,
        api_key_hmac_secret: None,
        allow_anonymous: false,
        rbac: crate::config::models::auth::RbacConfig {
            enabled: true,
            default_role: "user".to_string(),
            admin_roles: vec!["admin".to_string()],
        },
    };

    JwtHandler::new(&config).await.unwrap()
}

#[tokio::test]
async fn test_create_and_verify_access_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let token = handler
        .create_access_token(
            user_id,
            "user".to_string(),
            vec!["read".to_string()],
            None,
            None,
        )
        .await
        .unwrap();

    let claims = handler.verify_access_token(&token).await.unwrap();
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.role, "user");
    assert_eq!(claims.permissions, vec!["read"]);
    assert!(matches!(claims.token_type, TokenType::Access));
}

#[tokio::test]
async fn test_create_token_pair() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let token_pair = handler
        .create_token_pair(
            user_id,
            "user".to_string(),
            vec!["read".to_string()],
            None,
            None,
        )
        .await
        .unwrap();

    assert!(!token_pair.access_token.is_empty());
    assert!(!token_pair.refresh_token.is_empty());
    assert_eq!(token_pair.token_type, "Bearer");
    assert_eq!(token_pair.expires_in, 3600);

    // Verify both tokens
    let access_claims = handler
        .verify_access_token(&token_pair.access_token)
        .await
        .unwrap();
    let refresh_user_id = handler
        .verify_refresh_token(&token_pair.refresh_token)
        .await
        .unwrap();

    assert_eq!(access_claims.sub, user_id);
    assert_eq!(refresh_user_id, user_id);
}

#[tokio::test]
async fn test_password_reset_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let token = handler.create_password_reset_token(user_id).await.unwrap();
    let verified_user_id = handler.verify_password_reset_token(&token).await.unwrap();

    assert_eq!(verified_user_id, user_id);
}

#[tokio::test]
async fn test_email_verification_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let token = handler
        .create_email_verification_token(user_id)
        .await
        .unwrap();
    let verified_user_id = handler
        .verify_email_verification_token(&token)
        .await
        .unwrap();

    assert_eq!(verified_user_id, user_id);
}

#[tokio::test]
async fn test_invitation_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();
    let team_id = Uuid::new_v4();

    let token = handler
        .create_invitation_token(user_id, team_id, "member".to_string())
        .await
        .unwrap();
    let (verified_user_id, verified_team_id, role) =
        handler.verify_invitation_token(&token).await.unwrap();

    assert_eq!(verified_user_id, user_id);
    assert_eq!(verified_team_id, team_id);
    assert_eq!(role, "member");
}

#[test]
fn test_extract_token_from_header() {
    let header = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9";
    let token = JwtHandler::extract_token_from_header(header).unwrap();
    assert_eq!(token, "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");

    let invalid_header = "Basic dXNlcjpwYXNz";
    assert!(JwtHandler::extract_token_from_header(invalid_header).is_none());
}

#[tokio::test]
async fn test_invalid_token_verification() {
    let handler = create_test_handler().await;
    let invalid_token = "invalid.jwt.token";

    let result = handler.verify_access_token(invalid_token).await;
    assert!(result.is_err());
}

/// Verify that verify_access_token() rejects a refresh token at the audience level,
/// preventing token type confusion before any token_type field check.
#[tokio::test]
async fn test_refresh_token_rejected_by_verify_access_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let refresh_token = handler.create_refresh_token(user_id, None).await.unwrap();

    // verify_access_token must reject refresh tokens (audience mismatch: "refresh" vs "api")
    let result = handler.verify_access_token(&refresh_token).await;
    assert!(
        result.is_err(),
        "refresh token must be rejected by verify_access_token"
    );
}

/// Verify that verify_refresh_token() rejects an access token at the audience level.
#[tokio::test]
async fn test_access_token_rejected_by_verify_refresh_token() {
    let handler = create_test_handler().await;
    let user_id = Uuid::new_v4();

    let access_token = handler
        .create_access_token(
            user_id,
            "user".to_string(),
            vec!["read".to_string()],
            None,
            None,
        )
        .await
        .unwrap();

    // verify_refresh_token must reject access tokens (audience mismatch: "api" vs "refresh")
    let result = handler.verify_refresh_token(&access_token).await;
    assert!(
        result.is_err(),
        "access token must be rejected by verify_refresh_token"
    );
}
