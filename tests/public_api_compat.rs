#![cfg(feature = "providers-extended")]

#[cfg(feature = "storage")]
use litellm_rs::server::routes::ai::{
    create_file, delete_file, get_file, get_file_content, list_files,
};
#[cfg(feature = "storage")]
use litellm_rs::server::routes::auth::{
    AuthSystem, Claims, JwtHandler, LoginRequest, RefreshTokenRequest, configure_routes, login,
    refresh_token,
};
#[cfg(feature = "storage")]
use litellm_rs::storage::StorageLayer;
#[cfg(feature = "storage")]
use litellm_rs::storage::files::{FileMetadata, FileStorage, LocalStorage, S3Storage};

#[cfg(feature = "storage")]
#[allow(dead_code)]
async fn gh1130_external_auth_and_storage_signatures(
    req: actix_web::HttpRequest,
    state: actix_web::web::Data<litellm_rs::server::state::AppState>,
    storage: &StorageLayer,
) -> actix_web::Result<()> {
    let auth: &AuthSystem = state.auth.as_ref();
    let jwt: &JwtHandler = auth.jwt();
    let subject = uuid::Uuid::new_v4();
    let claims = Claims {
        sub: subject,
        iat: 1,
        exp: 2,
        iss: "litellm-rs".to_string(),
        aud: "api".to_string(),
        jti: "compat-jti".to_string(),
        role: "user".to_string(),
        permissions: vec!["read".to_string()],
        team_id: None,
        session_id: None,
        token_type: serde_json::from_value(serde_json::json!("access"))
            .expect("public TokenType must deserialize from its stable wire value"),
    };
    let access_token: String = jwt
        .create_access_token(
            subject,
            claims.role.clone(),
            claims.permissions.clone(),
            None,
            None,
        )
        .await?;
    let token_pair = jwt
        .create_token_pair(subject, "user".to_string(), vec![], None, None)
        .await?;
    let verified: Claims = jwt.verify_access_token(&access_token).await?;
    let _: String = token_pair.access_token;
    let _: uuid::Uuid = verified.sub;
    let _: (litellm_rs::core::models::user::types::User, String) =
        auth.login("compat-user", "compat-password").await?;

    let _: actix_web::HttpResponse = login(
        req,
        state.clone(),
        actix_web::web::Json(LoginRequest {
            username: "compat".to_string(),
            password: "secret".to_string(),
        }),
    )
    .await?;
    let _: actix_web::HttpResponse = refresh_token(
        state,
        actix_web::web::Json(RefreshTokenRequest {
            refresh_token: "refresh".to_string(),
        }),
    )
    .await?;
    let _: String = storage.store_file("compat.txt", b"compat").await?;
    let _: Vec<u8> = storage.get_file("file-id").await?;
    Ok(())
}

#[cfg(feature = "storage")]
#[test]
fn gh1130_public_files_auth_and_jwt_shapes_remain_source_compatible() {
    let now = chrono::Utc::now();
    let _metadata = FileMetadata {
        id: "file-public-compat".to_string(),
        filename: "compat.jsonl".to_string(),
        content_type: "application/json".to_string(),
        size: 2,
        created_at: now,
        purpose: Some("batch".to_string()),
        checksum: "checksum".to_string(),
    };
    let _login = LoginRequest {
        username: "compat".to_string(),
        password: "secret".to_string(),
    };
    let _refresh = RefreshTokenRequest {
        refresh_token: "refresh".to_string(),
    };

    let _ = create_file;
    let _ = list_files;
    let _ = get_file;
    let _ = delete_file;
    let _ = get_file_content;
    let _ = configure_routes;
    let _ = FileStorage::store;
    let _ = FileStorage::store_with_purpose;
    let _ = FileStorage::metadata;
    let _ = LocalStorage::store;
    let _ = LocalStorage::store_with_purpose;
    let _ = LocalStorage::metadata;
    let _ = S3Storage::store;
    let _ = S3Storage::store_with_purpose;
    let _ = S3Storage::metadata;
}
