use super::*;
use crate::core::models::{Metadata, UsageStats};
use actix_web::body::to_bytes;
use chrono::{TimeZone, Utc};
use serde_json::Value;

fn metadata() -> FileMetadata {
    FileMetadata {
        id: "file_test".to_string(),
        filename: "batch.jsonl".to_string(),
        content_type: "application/json".to_string(),
        size: 42,
        created_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        purpose: Some("batch".to_string()),
        checksum: "checksum".to_string(),
    }
}

#[test]
fn file_object_uses_metadata_purpose() {
    let object = serde_json::to_value(file_object(&metadata()).unwrap()).unwrap();

    assert_eq!(object["id"], "file_test");
    assert_eq!(object["object"], "file");
    assert_eq!(object["bytes"], 42);
    assert_eq!(object["created_at"], 1_700_000_000);
    assert_eq!(object["filename"], "batch.jsonl");
    assert_eq!(object["purpose"], "batch");
    assert!(object.get("content_type").is_none());
    assert!(object.get("checksum").is_none());
}

#[test]
fn file_object_rejects_missing_purpose() {
    let metadata = FileMetadata {
        purpose: None,
        ..metadata()
    };

    let error = file_object(&metadata).unwrap_err();

    assert!(error.to_string().contains("purpose"));
}

#[test]
fn validates_supported_upload_purpose() {
    assert_eq!(validate_upload_purpose(" batch ").unwrap(), "batch");

    let response = validate_upload_purpose("invalid-purpose").unwrap_err();
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
}

#[test]
fn enforces_upload_size_limit() {
    assert!(validate_purpose_size_limit("batch", MAX_BATCH_FILE_UPLOAD_BYTES).is_ok());

    let response =
        validate_purpose_size_limit("batch", MAX_BATCH_FILE_UPLOAD_BYTES + 1).unwrap_err();
    assert_eq!(response.status(), actix_web::http::StatusCode::BAD_REQUEST);
}

#[actix_web::test]
async fn validation_error_for_missing_file_uses_openai_shape() {
    let response = openai_errors::validation_error("No file provided");
    let status = response.status();
    let body = to_bytes(response.into_body()).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(status, actix_web::http::StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["message"], "No file provided");
    assert_eq!(json["error"]["type"], "invalid_request_error");
    assert_eq!(json["error"]["code"], "invalid_request");
}

#[test]
fn gh1130_file_access_is_exact_scope_not_any_owner_match() {
    let owner = FileOwnerScope::Team(Uuid::new_v4());
    let stored = StoredFileMetadata::owned(metadata(), owner.clone());
    let same = FileCaller {
        auth_enforced: true,
        is_admin: false,
        effective_scope: Some(owner),
    };
    let foreign = FileCaller {
        auth_enforced: true,
        is_admin: false,
        effective_scope: Some(FileOwnerScope::User(Uuid::new_v4())),
    };
    let admin = FileCaller {
        auth_enforced: true,
        is_admin: true,
        effective_scope: Some(FileOwnerScope::ApiKey(Uuid::new_v4())),
    };
    let anonymous_legacy_mode = FileCaller {
        auth_enforced: false,
        is_admin: false,
        effective_scope: None,
    };

    assert!(can_access_file(&same, &stored));
    assert!(!can_access_file(&foreign, &stored));
    assert!(can_access_file(&admin, &stored));
    assert!(can_access_file(&anonymous_legacy_mode, &stored));

    let legacy = StoredFileMetadata::legacy(metadata());
    assert!(!can_access_file(&same, &legacy));
    assert!(can_access_file(&admin, &legacy));
    assert!(can_access_file(&anonymous_legacy_mode, &legacy));
}

#[actix_web::test]
async fn gh1130_foreign_and_missing_use_identical_public_not_found() {
    let foreign = concealed_not_found();
    let missing = file_lookup_error(&GatewayError::not_found("secret file identifier"));
    assert_eq!(foreign.status(), missing.status());

    let foreign_body = to_bytes(foreign.into_body()).await.unwrap();
    let missing_body = to_bytes(missing.into_body()).await.unwrap();
    let foreign_json: Value = serde_json::from_slice(&foreign_body).unwrap();
    let missing_json: Value = serde_json::from_slice(&missing_body).unwrap();
    assert_eq!(foreign_json, missing_json);
    assert_eq!(
        foreign_json["error"]["message"],
        "Not found: File not found"
    );
    assert!(
        !serde_json::to_string(&foreign_json)
            .unwrap()
            .contains("secret file identifier")
    );
    let internal = storage_file_error(&GatewayError::Storage("secret backend detail".into()));
    assert_eq!(
        internal.status(),
        actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
    );
    let body = to_bytes(internal.into_body()).await.unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["error"]["message"], "Internal server error");
    assert!(!serde_json::to_string(&json).unwrap().contains("secret"));
}

#[test]
fn gh1130_api_key_principal_is_exclusive_and_attenuates_admin_owner() {
    let user_id = Uuid::new_v4();
    let key_team = Uuid::new_v4();
    let residual_jwt_team = Uuid::new_v4();
    let mut admin_user = User::new(
        "admin-owner".to_string(),
        "admin@example.com".to_string(),
        "hash".to_string(),
    );
    admin_user.role = UserRole::Admin;
    let key = ApiKey {
        metadata: Metadata::new(),
        name: "restricted".to_string(),
        key_hash: "hash".to_string(),
        key_prefix: "gw-test".to_string(),
        user_id: Some(user_id),
        team_id: Some(key_team),
        permissions: vec![],
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };
    let context = RequestContext::new()
        .with_user(user_id, Some(residual_jwt_team))
        .with_api_key(key.metadata.id);

    let caller = authenticated_file_caller(&context, Some(&admin_user), Some(&key)).unwrap();
    assert!(!caller.is_admin);
    assert_eq!(caller.effective_scope, Some(FileOwnerScope::Team(key_team)));

    let mismatched = RequestContext::new()
        .with_user(Uuid::new_v4(), None)
        .with_api_key(key.metadata.id);
    assert!(authenticated_file_caller(&mismatched, Some(&admin_user), Some(&key)).is_err());
    for field in ["team_id", "api_key_id"] {
        for value in [Value::Null, serde_json::json!(7), serde_json::json!("bad")] {
            let context = RequestContext::new()
                .with_user(admin_user.id(), None)
                .with_metadata(field, value);
            assert!(authenticated_file_caller(&context, Some(&admin_user), None).is_err());
        }
    }
}
