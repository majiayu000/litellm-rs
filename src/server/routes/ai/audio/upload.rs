use crate::server::routes::ai::openai_errors;
use actix_web::HttpResponse;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use std::fmt::Display;
use tracing::error;

const MAX_AUDIO_FILE_SIZE_BYTES: usize = 25 * 1024 * 1024;
const MAX_AUDIO_FORM_FIELD_SIZE_BYTES: usize = 64 * 1024;
const AUDIO_FILE_TOO_LARGE_MESSAGE: &str = "Audio file too large (max 25MB)";
const AUDIO_FIELD_TOO_LARGE_MESSAGE: &str = "Audio form field too large (max 64KB)";
const AUDIO_FILE_READ_ERROR_MESSAGE: &str = "Error reading file";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AudioUploadError {
    FileTooLarge,
    FieldTooLarge,
    ReadChunk,
}

pub(super) fn upload_error_response(error: AudioUploadError) -> HttpResponse {
    match error {
        AudioUploadError::FileTooLarge => {
            openai_errors::validation_error(AUDIO_FILE_TOO_LARGE_MESSAGE)
        }
        AudioUploadError::FieldTooLarge => {
            openai_errors::validation_error(AUDIO_FIELD_TOO_LARGE_MESSAGE)
        }
        AudioUploadError::ReadChunk => {
            openai_errors::validation_error(AUDIO_FILE_READ_ERROR_MESSAGE)
        }
    }
}

pub(super) fn raw_response_format_error(response_format: Option<&str>) -> Option<HttpResponse> {
    let response_format = response_format?;
    match response_format.to_ascii_lowercase().as_str() {
        "text" | "srt" | "vtt" => Some(openai_errors::validation_error(format!(
            "response_format '{}' requires raw response passthrough, which is not supported yet",
            response_format
        ))),
        _ => None,
    }
}

pub(super) fn parse_optional_f32_field(
    field_name: &str,
    value: &str,
) -> Result<Option<f32>, HttpResponse> {
    if value.is_empty() {
        return Ok(None);
    }

    value.parse::<f32>().map(Some).map_err(|_| {
        openai_errors::validation_error(format!("{field_name} must be a valid number"))
    })
}

fn ensure_field_within_limit(
    current_size: usize,
    chunk_size: usize,
    max_size: usize,
    too_large_error: AudioUploadError,
) -> Result<(), AudioUploadError> {
    match current_size.checked_add(chunk_size) {
        Some(total_size) if total_size <= max_size => Ok(()),
        _ => Err(too_large_error),
    }
}

async fn read_limited_field<S, E>(
    field: &mut S,
    max_size: usize,
    too_large_error: AudioUploadError,
    log_context: &str,
) -> Result<Vec<u8>, AudioUploadError>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let mut data = Vec::new();

    while let Some(chunk) = field.next().await {
        let bytes = chunk.map_err(|e| {
            error!("Error reading {} chunk: {}", log_context, e);
            AudioUploadError::ReadChunk
        })?;

        ensure_field_within_limit(data.len(), bytes.len(), max_size, too_large_error)?;
        data.extend_from_slice(&bytes);
    }

    Ok(data)
}

pub(super) async fn read_audio_file<S, E>(field: &mut S) -> Result<Vec<u8>, AudioUploadError>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    read_limited_field(
        field,
        MAX_AUDIO_FILE_SIZE_BYTES,
        AudioUploadError::FileTooLarge,
        "file",
    )
    .await
}

pub(super) async fn read_text_field<S, E>(field: &mut S) -> Result<String, AudioUploadError>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let data = read_limited_field(
        field,
        MAX_AUDIO_FORM_FIELD_SIZE_BYTES,
        AudioUploadError::FieldTooLarge,
        "multipart field",
    )
    .await?;

    Ok(String::from_utf8(data)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned()))
}

pub(super) async fn drain_field<S, E>(field: &mut S) -> Result<(), AudioUploadError>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
    E: Display,
{
    let mut total_size = 0;

    while let Some(chunk) = field.next().await {
        let bytes = chunk.map_err(|e| {
            error!("Error draining multipart field chunk: {}", e);
            AudioUploadError::ReadChunk
        })?;

        ensure_field_within_limit(
            total_size,
            bytes.len(),
            MAX_AUDIO_FORM_FIELD_SIZE_BYTES,
            AudioUploadError::FieldTooLarge,
        )?;
        total_size += bytes.len();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::fmt;

    #[derive(Debug)]
    struct TestChunkError;

    impl fmt::Display for TestChunkError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("chunk failed")
        }
    }

    #[test]
    fn audio_file_limit_allows_exact_25mb() {
        assert_eq!(
            ensure_field_within_limit(
                MAX_AUDIO_FILE_SIZE_BYTES - 1,
                1,
                MAX_AUDIO_FILE_SIZE_BYTES,
                AudioUploadError::FileTooLarge,
            ),
            Ok(())
        );
    }

    #[test]
    fn audio_file_limit_rejects_chunk_that_exceeds_25mb() {
        assert_eq!(
            ensure_field_within_limit(
                MAX_AUDIO_FILE_SIZE_BYTES,
                1,
                MAX_AUDIO_FILE_SIZE_BYTES,
                AudioUploadError::FileTooLarge,
            ),
            Err(AudioUploadError::FileTooLarge)
        );
    }

    #[actix_web::test]
    async fn text_field_limit_rejects_oversized_chunk() {
        let mut chunks = stream::iter(vec![Ok::<Bytes, TestChunkError>(Bytes::from(vec![
            b'a';
            MAX_AUDIO_FORM_FIELD_SIZE_BYTES
                + 1
        ]))]);

        assert_eq!(
            read_text_field(&mut chunks).await,
            Err(AudioUploadError::FieldTooLarge)
        );
    }

    #[actix_web::test]
    async fn read_audio_file_returns_error_for_chunk_read_failure() {
        let mut chunks = stream::iter(vec![
            Ok::<Bytes, TestChunkError>(Bytes::from_static(b"ok")),
            Err(TestChunkError),
        ]);

        assert_eq!(
            read_audio_file(&mut chunks).await,
            Err(AudioUploadError::ReadChunk)
        );
    }

    #[actix_web::test]
    async fn drain_field_limit_rejects_oversized_chunk() {
        let mut chunks = stream::iter(vec![Ok::<Bytes, TestChunkError>(Bytes::from(vec![
            b'a';
            MAX_AUDIO_FORM_FIELD_SIZE_BYTES
                + 1
        ]))]);

        assert_eq!(
            drain_field(&mut chunks).await,
            Err(AudioUploadError::FieldTooLarge)
        );
    }
}
