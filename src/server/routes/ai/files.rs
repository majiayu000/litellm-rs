//! OpenAI-compatible local Files API routes.

use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use serde::Serialize;
use tracing::error;
use uuid::Uuid;

use crate::core::models::ApiKey;
use crate::core::models::user::types::{User, UserRole};
use crate::core::types::context::{RequestContext, SharedRequestContext};
use crate::server::state::AppState;
use crate::storage::files::{FileMetadata, FileOwnerScope, StoredFileMetadata};
use crate::utils::error::gateway_error::GatewayError;

use super::{context::api_key_has_admin_permission_checked, openai_errors};

const DEFAULT_FILENAME: &str = "upload";
const FILE_READ_ERROR_MESSAGE: &str = "Error reading file";
const FIELD_READ_ERROR_MESSAGE: &str = "Error reading multipart field";
const MAX_FILE_UPLOAD_BYTES: usize = 512 * 1024 * 1024;
const MAX_BATCH_FILE_UPLOAD_BYTES: usize = 200 * 1024 * 1024;
const MAX_TEXT_FIELD_BYTES: usize = 8 * 1024;
const MAX_DISCARDED_FIELD_BYTES: usize = 1024 * 1024;
const SUPPORTED_UPLOAD_PURPOSES: &[&str] = &[
    "assistants",
    "batch",
    "evals",
    "fine-tune",
    "user_data",
    "vision",
];

#[derive(Debug, Serialize)]
struct OpenAiFileObject {
    id: String,
    object: &'static str,
    bytes: u64,
    created_at: i64,
    filename: String,
    purpose: String,
}

#[derive(Debug, Serialize)]
struct OpenAiFileList {
    object: &'static str,
    data: Vec<OpenAiFileObject>,
}

#[derive(Debug, Serialize)]
struct OpenAiFileDelete {
    id: String,
    object: &'static str,
    deleted: bool,
}

struct ParsedFileUpload {
    filename: String,
    content: Vec<u8>,
    purpose: String,
}

#[derive(Debug, Clone)]
struct FileCaller {
    auth_enforced: bool,
    is_admin: bool,
    effective_scope: Option<FileOwnerScope>,
}

/// Upload a file to the configured gateway-local file storage backend.
pub async fn create_file(
    state: web::Data<AppState>,
    payload: Multipart,
) -> ActixResult<HttpResponse> {
    create_file_internal(state, payload, None).await
}

pub(super) async fn create_file_http(
    request: HttpRequest,
    state: web::Data<AppState>,
    payload: Multipart,
) -> ActixResult<HttpResponse> {
    create_file_internal(state, payload, Some(&request)).await
}

async fn create_file_internal(
    state: web::Data<AppState>,
    payload: Multipart,
    request: Option<&HttpRequest>,
) -> ActixResult<HttpResponse> {
    let upload = match parse_upload(payload).await {
        Ok(upload) => upload,
        Err(response) => return Ok(response),
    };
    let caller = match resolve_file_caller(&state, request) {
        Ok(caller) => caller,
        Err(error) => return Ok(internal_file_error(&error)),
    };

    let store_result = if caller.auth_enforced {
        let Some(owner) = caller.effective_scope.clone() else {
            return Ok(openai_errors::internal_error("Internal server error"));
        };
        state
            .storage
            .files
            .store_owned_with_purpose(
                &upload.filename,
                &upload.content,
                Some(&upload.purpose),
                owner,
            )
            .await
    } else {
        state
            .storage
            .files
            .store_with_purpose(&upload.filename, &upload.content, Some(&upload.purpose))
            .await
    };
    let file_id = match store_result {
        Ok(file_id) => file_id,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    let metadata = match state.storage.files.metadata_with_owner(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    if !can_access_file(&caller, &metadata) {
        return Ok(concealed_not_found());
    }

    match file_object(&metadata.public) {
        Ok(object) => Ok(HttpResponse::Ok().json(object)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

/// List files known by the configured file storage backend.
pub async fn list_files(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    list_files_internal(state, None).await
}

pub(super) async fn list_files_http(
    request: HttpRequest,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    list_files_internal(state, Some(&request)).await
}

async fn list_files_internal(
    state: web::Data<AppState>,
    request: Option<&HttpRequest>,
) -> ActixResult<HttpResponse> {
    let caller = match resolve_file_caller(&state, request) {
        Ok(caller) => caller,
        Err(error) => return Ok(internal_file_error(&error)),
    };
    let file_ids = match state.storage.files.list(None, None).await {
        Ok(file_ids) => file_ids,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    let mut data = Vec::with_capacity(file_ids.len());
    for file_id in file_ids {
        let metadata = match state.storage.files.metadata_with_owner(&file_id).await {
            Ok(metadata) => metadata,
            Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
        };
        if !can_access_file(&caller, &metadata) {
            continue;
        }
        let object = match file_object(&metadata.public) {
            Ok(object) => object,
            Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
        };
        data.push(object);
    }

    Ok(HttpResponse::Ok().json(OpenAiFileList {
        object: "list",
        data,
    }))
}

/// Retrieve metadata for one file.
pub async fn get_file(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    get_file_internal(state, file_id, None).await
}

pub(super) async fn get_file_http(
    request: HttpRequest,
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    get_file_internal(state, file_id, Some(&request)).await
}

async fn get_file_internal(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
    request: Option<&HttpRequest>,
) -> ActixResult<HttpResponse> {
    let caller = match resolve_file_caller(&state, request) {
        Ok(caller) => caller,
        Err(error) => return Ok(internal_file_error(&error)),
    };
    let metadata = match state.storage.files.metadata_with_owner(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(file_lookup_error(&error)),
    };
    if !can_access_file(&caller, &metadata) {
        return Ok(concealed_not_found());
    }

    match file_object(&metadata.public) {
        Ok(object) => Ok(HttpResponse::Ok().json(object)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

/// Delete one file.
pub async fn delete_file(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    delete_file_internal(state, file_id, None).await
}

pub(super) async fn delete_file_http(
    request: HttpRequest,
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    delete_file_internal(state, file_id, Some(&request)).await
}

async fn delete_file_internal(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
    request: Option<&HttpRequest>,
) -> ActixResult<HttpResponse> {
    let caller = match resolve_file_caller(&state, request) {
        Ok(caller) => caller,
        Err(error) => return Ok(internal_file_error(&error)),
    };
    let file_id = file_id.into_inner();
    let metadata = match state.storage.files.metadata_with_owner(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(file_lookup_error(&error)),
    };
    if !can_access_file(&caller, &metadata) {
        return Ok(concealed_not_found());
    }
    if let Err(error) = state.storage.files.delete(&file_id).await {
        return Ok(openai_errors::gateway_error_response(&error));
    }

    Ok(HttpResponse::Ok().json(OpenAiFileDelete {
        id: file_id,
        object: "file",
        deleted: true,
    }))
}

/// Return raw file content using the stored metadata content type.
pub async fn get_file_content(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    get_file_content_internal(state, file_id, None).await
}

pub(super) async fn get_file_content_http(
    request: HttpRequest,
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    get_file_content_internal(state, file_id, Some(&request)).await
}

async fn get_file_content_internal(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
    request: Option<&HttpRequest>,
) -> ActixResult<HttpResponse> {
    let caller = match resolve_file_caller(&state, request) {
        Ok(caller) => caller,
        Err(error) => return Ok(internal_file_error(&error)),
    };
    let file_id = file_id.into_inner();
    let metadata = match state.storage.files.metadata_with_owner(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(file_lookup_error(&error)),
    };
    if !can_access_file(&caller, &metadata) {
        return Ok(concealed_not_found());
    }
    let content = match state.storage.files.get(&file_id).await {
        Ok(content) => content,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    Ok(HttpResponse::Ok()
        .content_type(metadata.public.content_type)
        .body(content))
}

fn resolve_file_caller(
    state: &AppState,
    request: Option<&HttpRequest>,
) -> Result<FileCaller, GatewayError> {
    let config = state.config.load();
    let auth = config.auth();
    let auth_enforced = auth.enable_api_key || auth.enable_jwt;
    if !auth_enforced {
        if !auth.allow_anonymous {
            return Err(GatewayError::internal(
                "Files route requires authentication configuration",
            ));
        }
        return Ok(FileCaller {
            auth_enforced: false,
            is_admin: false,
            effective_scope: None,
        });
    }

    let request =
        request.ok_or_else(|| GatewayError::internal("Missing authenticated request proof"))?;
    let context = authenticated_context(request)?;
    let extensions = request.extensions();
    authenticated_file_caller(
        &context,
        extensions.get::<User>(),
        extensions.get::<ApiKey>(),
    )
}

fn authenticated_file_caller(
    context: &RequestContext,
    user: Option<&User>,
    api_key: Option<&ApiKey>,
) -> Result<FileCaller, GatewayError> {
    if let Some(api_key) = api_key {
        if context.api_key_id() != Some(api_key.metadata.id) {
            return Err(GatewayError::internal("API-key identity mismatch"));
        }
        let context_user = parse_context_user(context)?;
        if context_user != api_key.user_id {
            return Err(GatewayError::internal("API-key user identity mismatch"));
        }
        let scope = api_key
            .team_id
            .map(FileOwnerScope::Team)
            .or_else(|| api_key.user_id.map(FileOwnerScope::User))
            .unwrap_or(FileOwnerScope::ApiKey(api_key.metadata.id));
        return Ok(FileCaller {
            auth_enforced: true,
            is_admin: api_key_has_admin_permission_checked(api_key)?,
            effective_scope: Some(scope),
        });
    }

    if context.api_key_id().is_some() {
        return Err(GatewayError::internal(
            "Request context has API-key identity without an authenticated key",
        ));
    }
    let user = user.ok_or_else(|| GatewayError::internal("Missing authenticated user identity"))?;
    if parse_context_user(context)? != Some(user.id()) {
        return Err(GatewayError::internal(
            "Authenticated user identity mismatch",
        ));
    }
    let scope = context
        .team_id()
        .map(FileOwnerScope::Team)
        .unwrap_or_else(|| FileOwnerScope::User(user.id()));
    Ok(FileCaller {
        auth_enforced: true,
        is_admin: matches!(user.role, UserRole::Admin | UserRole::SuperAdmin),
        effective_scope: Some(scope),
    })
}

fn authenticated_context(request: &HttpRequest) -> Result<RequestContext, GatewayError> {
    let extensions = request.extensions();
    if let Some(context) = extensions.get::<SharedRequestContext>() {
        return Ok(context.as_ref().clone());
    }
    extensions
        .get::<RequestContext>()
        .cloned()
        .ok_or_else(|| GatewayError::internal("Missing authenticated request context"))
}

fn parse_context_user(context: &RequestContext) -> Result<Option<Uuid>, GatewayError> {
    context
        .user_id
        .as_deref()
        .map(Uuid::parse_str)
        .transpose()
        .map_err(|_| GatewayError::internal("Invalid authenticated user identity"))
}

fn can_access_file(caller: &FileCaller, metadata: &StoredFileMetadata) -> bool {
    if !caller.auth_enforced || caller.is_admin {
        return true;
    }
    caller.effective_scope.as_ref() == metadata.owner()
}

fn concealed_not_found() -> HttpResponse {
    openai_errors::gateway_error_response(&GatewayError::not_found("File not found"))
}

fn file_lookup_error(error: &GatewayError) -> HttpResponse {
    if matches!(error, GatewayError::NotFound(_)) {
        concealed_not_found()
    } else {
        openai_errors::gateway_error_response(error)
    }
}

fn internal_file_error(error: &GatewayError) -> HttpResponse {
    error!("Files authorization state is invalid: {}", error);
    openai_errors::internal_error("Internal server error")
}

async fn parse_upload(mut payload: Multipart) -> Result<ParsedFileUpload, HttpResponse> {
    let mut filename = DEFAULT_FILENAME.to_string();
    let mut content: Option<Vec<u8>> = None;
    let mut purpose: Option<String> = None;

    while let Some(item) = payload.next().await {
        let mut field = item.map_err(|error| {
            error!("Error reading multipart field: {}", error);
            openai_errors::validation_error(format!("Invalid multipart data: {}", error))
        })?;

        let field_name = match field.name() {
            Some(name) => name.to_string(),
            None => {
                drain_field(&mut field).await?;
                continue;
            }
        };

        match field_name.as_str() {
            "file" => {
                if let Some(disposition) = field.content_disposition()
                    && let Some(name) = disposition.get_filename()
                    && !name.is_empty()
                {
                    filename = name.to_string();
                }
                content = Some(
                    read_field_bytes(&mut field, FILE_READ_ERROR_MESSAGE, MAX_FILE_UPLOAD_BYTES)
                        .await?,
                );
            }
            "purpose" => {
                let value = read_text_field(&mut field).await?;
                if !value.trim().is_empty() {
                    purpose = Some(validate_upload_purpose(&value)?);
                }
            }
            _ => drain_field(&mut field).await?,
        }
    }

    let content = match content {
        Some(content) if !content.is_empty() => content,
        _ => return Err(openai_errors::validation_error("No file provided")),
    };
    let purpose = purpose.ok_or_else(|| openai_errors::validation_error("purpose is required"))?;
    validate_purpose_size_limit(&purpose, content.len())?;

    Ok(ParsedFileUpload {
        filename,
        content,
        purpose,
    })
}

async fn read_text_field(field: &mut actix_multipart::Field) -> Result<String, HttpResponse> {
    let data = read_field_bytes(field, FIELD_READ_ERROR_MESSAGE, MAX_TEXT_FIELD_BYTES).await?;
    Ok(String::from_utf8(data)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned()))
}

async fn drain_field(field: &mut actix_multipart::Field) -> Result<(), HttpResponse> {
    let mut total = 0usize;
    while let Some(chunk) = field.next().await {
        let bytes = chunk.map_err(|error| {
            error!("Error draining multipart chunk: {}", error);
            openai_errors::validation_error(FIELD_READ_ERROR_MESSAGE)
        })?;
        total = checked_field_size(
            total,
            bytes.len(),
            MAX_DISCARDED_FIELD_BYTES,
            "Multipart field too large",
        )?;
    }
    Ok(())
}

async fn read_field_bytes(
    field: &mut actix_multipart::Field,
    user_message: &'static str,
    max_bytes: usize,
) -> Result<Vec<u8>, HttpResponse> {
    let mut data = Vec::new();
    while let Some(chunk) = field.next().await {
        let bytes = chunk.map_err(|error| {
            error!("Error reading multipart chunk: {}", error);
            openai_errors::validation_error(user_message)
        })?;
        checked_field_size(
            data.len(),
            bytes.len(),
            max_bytes,
            "File size limit exceeded",
        )?;
        data.extend_from_slice(&bytes);
    }
    Ok(data)
}

fn checked_field_size(
    current: usize,
    additional: usize,
    max_bytes: usize,
    user_message: &'static str,
) -> Result<usize, HttpResponse> {
    let total = current
        .checked_add(additional)
        .ok_or_else(|| openai_errors::validation_error(user_message))?;
    if total > max_bytes {
        return Err(openai_errors::validation_error(user_message));
    }
    Ok(total)
}

fn validate_upload_purpose(value: &str) -> Result<String, HttpResponse> {
    let purpose = value.trim();
    if SUPPORTED_UPLOAD_PURPOSES.contains(&purpose) {
        return Ok(purpose.to_string());
    }

    Err(openai_errors::validation_error(format!(
        "Unsupported file purpose: {purpose}"
    )))
}

fn validate_purpose_size_limit(purpose: &str, bytes: usize) -> Result<(), HttpResponse> {
    let max_bytes = if purpose == "batch" {
        MAX_BATCH_FILE_UPLOAD_BYTES
    } else {
        MAX_FILE_UPLOAD_BYTES
    };
    checked_field_size(0, bytes, max_bytes, "File size limit exceeded").map(|_| ())
}

fn file_object(metadata: &FileMetadata) -> Result<OpenAiFileObject, GatewayError> {
    let purpose = metadata
        .purpose
        .as_deref()
        .filter(|purpose| SUPPORTED_UPLOAD_PURPOSES.contains(purpose))
        .ok_or_else(|| GatewayError::validation("File metadata purpose is missing or invalid"))?;

    Ok(OpenAiFileObject {
        id: metadata.id.clone(),
        object: "file",
        bytes: metadata.size,
        created_at: metadata.created_at.timestamp(),
        filename: metadata.filename.clone(),
        purpose: purpose.to_string(),
    })
}

#[cfg(test)]
mod tests {
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
    }
}
