//! AWS Event Stream parsing for Bedrock streaming responses.

use super::BedrockStream;
use crate::core::providers::bedrock::error::BedrockErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use bytes::Bytes;
use serde_json::Value;

/// AWS Event Stream message
#[derive(Debug)]
pub struct EventStreamMessage {
    pub headers: Vec<EventStreamHeader>,
    pub payload: Bytes,
}

/// Event stream header
#[derive(Debug)]
pub struct EventStreamHeader {
    pub name: String,
    pub value: HeaderValue,
}

/// Header value types
#[derive(Debug)]
pub enum HeaderValue {
    String(String),
    ByteArray(Vec<u8>),
    Boolean(bool),
    Byte(i8),
    Short(i16),
    Integer(i32),
    Long(i64),
    UUID(String),
    Timestamp(i64),
}

impl BedrockStream {
    /// Parse event stream message from bytes
    pub(crate) fn parse_event_message(data: &[u8]) -> Result<EventStreamMessage, ProviderError> {
        let message = aws_smithy_eventstream::frame::read_message_from(data).map_err(|error| {
            ProviderError::response_parsing(
                "bedrock",
                format!("invalid AWS event stream frame: {error}"),
            )
        })?;
        let headers = message
            .headers()
            .iter()
            .map(|header| {
                let value = header.value().as_string().map_err(|_| {
                    ProviderError::response_parsing(
                        "bedrock",
                        "unsupported non-string AWS event stream header",
                    )
                })?;
                Ok(EventStreamHeader {
                    name: header.name().as_str().to_string(),
                    value: HeaderValue::String(value.as_str().to_string()),
                })
            })
            .collect::<Result<Vec<_>, ProviderError>>()?;
        Ok(EventStreamMessage {
            headers,
            payload: message.payload().clone(),
        })
    }

    pub(crate) fn take_event_message(
        buffer: &mut Vec<u8>,
    ) -> Option<Result<EventStreamMessage, ProviderError>> {
        if buffer.len() < 12 {
            return None;
        }
        let total_length =
            u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;
        let headers_length =
            u32::from_be_bytes([buffer[4], buffer[5], buffer[6], buffer[7]]) as usize;
        let prelude_crc = u32::from_be_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]);
        if crc32fast::hash(&buffer[..8]) != prelude_crc {
            buffer.clear();
            return Some(Err(ProviderError::response_parsing(
                "bedrock",
                "invalid AWS event stream prelude checksum",
            )));
        }
        if total_length < 16 || headers_length > total_length - 16 || headers_length == 1 {
            buffer.clear();
            return Some(Err(ProviderError::response_parsing(
                "bedrock",
                "invalid AWS event stream frame length",
            )));
        }
        if buffer.len() < total_length {
            return None;
        }
        let message = Self::parse_event_message(&buffer[..total_length]);
        buffer.drain(..total_length);
        Some(message)
    }

    pub(crate) fn header_value<'a>(message: &'a EventStreamMessage, name: &str) -> Option<&'a str> {
        message.headers.iter().find_map(|header| {
            (header.name == name)
                .then_some(&header.value)
                .and_then(|value| match value {
                    HeaderValue::String(value) => Some(value.as_str()),
                    _ => None,
                })
        })
    }

    fn stream_exception_from_payload(value: &Value) -> Option<(String, String)> {
        let object = value.as_object()?;

        for (code, detail) in object {
            if code.ends_with("Exception") || code.ends_with("exception") {
                let message = detail
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| detail.as_str())
                    .unwrap_or("");
                return Some((code.clone(), message.to_string()));
            }
        }

        None
    }

    fn stream_error(code: &str, message: &str) -> ProviderError {
        if let Some(error) = BedrockErrorMapper::map_service_error(code, message) {
            return error;
        }
        let details = if message.is_empty() {
            format!("Bedrock stream error: {code}")
        } else {
            format!("Bedrock stream error {code}: {message}")
        };

        ProviderError::api_error("bedrock", 500, details)
    }

    pub(crate) fn check_stream_error(message: &EventStreamMessage) -> Result<(), ProviderError> {
        let message_type = Self::header_value(message, ":message-type");
        let exception_type = Self::header_value(message, ":exception-type");

        if matches!(message_type, Some("exception" | "error")) || exception_type.is_some() {
            let payload = serde_json::from_slice::<Value>(&message.payload).ok();
            let payload_message = payload
                .as_ref()
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("");
            let code = exception_type.unwrap_or("streamException");
            return Err(Self::stream_error(code, payload_message));
        }

        if let Ok(payload) = serde_json::from_slice::<Value>(&message.payload)
            && let Some((code, message)) = Self::stream_exception_from_payload(&payload)
        {
            return Err(Self::stream_error(&code, &message));
        }

        Ok(())
    }
}

#[cfg(test)]
mod strict_frame_tests {
    use super::*;

    fn exception_message(exception_type: &str) -> EventStreamMessage {
        EventStreamMessage {
            headers: vec![
                EventStreamHeader {
                    name: ":message-type".to_string(),
                    value: HeaderValue::String("exception".to_string()),
                },
                EventStreamHeader {
                    name: ":exception-type".to_string(),
                    value: HeaderValue::String(exception_type.to_string()),
                },
            ],
            payload: Bytes::from_static(br#"{"message":"request failed"}"#),
        }
    }

    fn frame(headers: &[u8], payload: &[u8]) -> Vec<u8> {
        let total_length = 16 + headers.len() + payload.len();
        let mut data = Vec::new();
        data.extend_from_slice(&(total_length as u32).to_be_bytes());
        data.extend_from_slice(&(headers.len() as u32).to_be_bytes());
        data.extend_from_slice(&crc32fast::hash(&data).to_be_bytes());
        data.extend_from_slice(headers);
        data.extend_from_slice(payload);
        data.extend_from_slice(&crc32fast::hash(&data).to_be_bytes());
        data
    }

    #[test]
    fn rejects_bad_checksums() {
        let mut bad_prelude = frame(&[], b"{}");
        bad_prelude[8..12].copy_from_slice(&1_u32.to_be_bytes());
        assert!(BedrockStream::parse_event_message(&bad_prelude).is_err());

        let mut bad_message = frame(&[], b"{}");
        let end = bad_message.len();
        bad_message[end - 4..].copy_from_slice(&1_u32.to_be_bytes());
        assert!(BedrockStream::parse_event_message(&bad_message).is_err());
    }

    #[test]
    fn rejects_truncated_header() {
        let truncated_header = [4_u8, b'a'];
        assert!(BedrockStream::parse_event_message(&frame(&truncated_header, &[])).is_err());
    }

    #[test]
    fn maps_modeled_service_exceptions_to_structured_provider_errors() {
        let cases = [
            ("validationException", "invalid_request", None, false),
            ("accessDeniedException", "authentication", None, false),
            ("throttlingException", "rate_limit", None, true),
            ("serviceQuotaExceededException", "rate_limit", None, true),
            ("resourceNotFoundException", "api_error", Some(404), false),
            ("modelNotReadyException", "api_error", Some(424), true),
            ("badGatewayException", "network", None, true),
            ("conflictException", "api_error", Some(409), false),
            ("dependencyFailedException", "api_error", Some(424), true),
            ("internalServerException", "api_error", Some(500), true),
        ];

        for (exception_type, expected_category, expected_status, expected_retryable) in cases {
            let error = BedrockStream::check_stream_error(&exception_message(exception_type))
                .expect_err("modeled exception must fail");
            assert_eq!(error.is_retryable(), expected_retryable, "{exception_type}");
            let (category, status) = match error {
                ProviderError::InvalidRequest { .. } => ("invalid_request", None),
                ProviderError::Authentication { .. } => ("authentication", None),
                ProviderError::RateLimit { .. } => ("rate_limit", None),
                ProviderError::ModelNotFound { .. } => ("model_not_found", None),
                ProviderError::Network { .. } => ("network", None),
                ProviderError::ApiError { status, .. } => ("api_error", Some(status)),
                other => panic!("unexpected category for {exception_type}: {other}"),
            };
            assert_eq!(category, expected_category, "{exception_type}");
            assert_eq!(status, expected_status, "{exception_type}");
        }
    }
}
