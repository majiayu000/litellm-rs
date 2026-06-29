//! OpenAI-compatible local Files API routes.

use actix_multipart::Multipart;
use actix_web::{HttpResponse, Result as ActixResult, web};
use futures::StreamExt;
use serde::Serialize;
use tracing::error;

use crate::server::state::AppState;
use crate::storage::files::FileMetadata;
use crate::utils::error::gateway_error::GatewayError;

use super::openai_errors;

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

/// Upload a file to the configured gateway-local file storage backend.
pub async fn create_file(
    state: web::Data<AppState>,
    payload: Multipart,
) -> ActixResult<HttpResponse> {
    let upload = match parse_upload(payload).await {
        Ok(upload) => upload,
        Err(response) => return Ok(response),
    };

    let file_id = match state
        .storage
        .files
        .store_with_purpose(&upload.filename, &upload.content, Some(&upload.purpose))
        .await
    {
        Ok(file_id) => file_id,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    let metadata = match state.storage.files.metadata(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    match file_object(&metadata) {
        Ok(object) => Ok(HttpResponse::Ok().json(object)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

/// List files known by the configured file storage backend.
pub async fn list_files(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    let file_ids = match state.storage.files.list(None, None).await {
        Ok(file_ids) => file_ids,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    let mut data = Vec::with_capacity(file_ids.len());
    for file_id in file_ids {
        let metadata = match state.storage.files.metadata(&file_id).await {
            Ok(metadata) => metadata,
            Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
        };
        let object = match file_object(&metadata) {
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
    let metadata = match state.storage.files.metadata(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    match file_object(&metadata) {
        Ok(object) => Ok(HttpResponse::Ok().json(object)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

/// Delete one file.
pub async fn delete_file(
    state: web::Data<AppState>,
    file_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let file_id = file_id.into_inner();
    if let Err(error) = state.storage.files.metadata(&file_id).await {
        return Ok(openai_errors::gateway_error_response(&error));
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
    let file_id = file_id.into_inner();
    let metadata = match state.storage.files.metadata(&file_id).await {
        Ok(metadata) => metadata,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };
    let content = match state.storage.files.get(&file_id).await {
        Ok(content) => content,
        Err(error) => return Ok(openai_errors::gateway_error_response(&error)),
    };

    Ok(HttpResponse::Ok()
        .content_type(metadata.content_type)
        .body(content))
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
}
