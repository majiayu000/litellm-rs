use bytes::Bytes;
use serde::Serialize;
use serde_json::json;
use std::fmt;
use tokio::sync::mpsc;

use crate::core::models::openai::responses_api::{
    ResponseInputTokensDetails, ResponseOutputItem, ResponseOutputTokensDetails,
    ResponseReasoningItem, ResponseStreamEvent, ResponseUsage, ResponsesApiRequest,
    ResponsesApiResponse,
};
use crate::core::providers::ProviderError;
use crate::core::streaming::types::Event;
use crate::core::types::responses::Usage as ChatUsage;

use super::super::stream_output_guardrail::{StreamGuardrailError, StreamOutputGuardrail};

pub(super) fn completed_reasoning_item(
    item_id: String,
    status: &str,
    summary_text: String,
) -> ResponseOutputItem {
    ResponseOutputItem::Reasoning(ResponseReasoningItem {
        id: item_id,
        status: status.to_string(),
        summary: Some(vec![json!({
            "type": "summary_text",
            "text": summary_text,
        })]),
    })
}

pub(super) fn in_progress_reasoning_item(item_id: String) -> ResponseOutputItem {
    ResponseOutputItem::Reasoning(ResponseReasoningItem {
        id: item_id,
        status: "in_progress".to_string(),
        summary: Some(vec![]),
    })
}

pub(super) fn output_items_in_stream_order(
    mut all_output: Vec<(u32, ResponseOutputItem)>,
) -> Vec<ResponseOutputItem> {
    all_output.sort_by_key(|(index, _)| *index);
    all_output.into_iter().map(|(_, item)| item).collect()
}

pub(super) fn response_usage_from_chat_usage(usage: &ChatUsage) -> ResponseUsage {
    ResponseUsage {
        input_tokens: usage.prompt_tokens,
        output_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        input_tokens_details: usage.prompt_tokens_details.as_ref().map(|details| {
            ResponseInputTokensDetails {
                cached_tokens: details.cached_tokens.unwrap_or(0),
            }
        }),
        output_tokens_details: usage.completion_tokens_details.as_ref().map(|details| {
            ResponseOutputTokensDetails {
                reasoning_tokens: details.reasoning_tokens.unwrap_or(0),
            }
        }),
    }
}

pub(super) fn make_shell(
    id: &str,
    created_at: i64,
    model: &str,
    status: &str,
    original: &ResponsesApiRequest,
) -> ResponsesApiResponse {
    ResponsesApiResponse {
        id: id.to_string(),
        object: "response".to_string(),
        created_at,
        status: status.to_string(),
        model: model.to_string(),
        output: vec![],
        usage: None,
        error: None,
        previous_response_id: original.previous_response_id.clone(),
        metadata: None,
    }
}

#[derive(Debug)]
pub(super) enum ResponseStreamEmitError {
    Serialization(serde_json::Error),
    ClientDisconnected,
}

impl fmt::Display for ResponseStreamEmitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "stream serialization failed: {error}"),
            Self::ClientDisconnected => formatter.write_str("client disconnected"),
        }
    }
}

pub(super) async fn emit(
    tx: &mpsc::Sender<Bytes>,
    event: &ResponseStreamEvent,
) -> Result<(), ResponseStreamEmitError> {
    emit_serialized(tx, event).await
}

async fn emit_serialized<T: Serialize + ?Sized>(
    tx: &mpsc::Sender<Bytes>,
    event: &T,
) -> Result<(), ResponseStreamEmitError> {
    send_encoded(tx, encode(event)?).await
}

pub(super) fn encode<T: Serialize + ?Sized>(event: &T) -> Result<Bytes, ResponseStreamEmitError> {
    let json = serde_json::to_string(event).map_err(ResponseStreamEmitError::Serialization)?;
    Ok(Event::default().data(&json).to_bytes())
}

pub(super) async fn send_encoded(
    tx: &mpsc::Sender<Bytes>,
    event: Bytes,
) -> Result<(), ResponseStreamEmitError> {
    tx.send(event)
        .await
        .map_err(|_| ResponseStreamEmitError::ClientDisconnected)
}

pub(super) async fn flush_output_guardrail(
    tx: &mpsc::Sender<Bytes>,
    guardrail: &mut StreamOutputGuardrail,
) -> Result<bool, StreamGuardrailError> {
    let Some(pending) = guardrail.finish_until_closed(tx).await? else {
        return Ok(false);
    };
    for event in pending {
        if send_encoded(tx, event).await.is_err() {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn sse_error(message: &str, error_type: &str, code: &str) -> Bytes {
    let error = json!({"type":"error","error":{"type":error_type,"code":code,"message":message}});
    let error_event = Event::default().data(&error.to_string());
    let done_event = Event::default().data("[DONE]");
    let mut bytes = error_event.to_bytes().to_vec();
    bytes.extend_from_slice(&done_event.to_bytes());
    Bytes::from(bytes)
}

pub(super) async fn send_guardrail_error(
    tx: &mpsc::Sender<Bytes>,
    error: StreamGuardrailError,
) -> bool {
    tx.send(sse_error(error.message(), error.error_type(), error.code()))
        .await
        .is_err()
}

pub(super) fn classify(error: &ProviderError) -> (&'static str, &'static str) {
    match error {
        ProviderError::Authentication { .. } => ("invalid_request_error", "authentication_error"),
        ProviderError::RateLimit { .. } => ("rate_limit_error", "rate_limit_exceeded"),
        ProviderError::Timeout { .. } => ("server_error", "timeout"),
        _ => ("server_error", "internal_error"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serializer;

    struct FailingSerialization;

    impl Serialize for FailingSerialization {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            Err(serde::ser::Error::custom(
                "intentional serialization failure",
            ))
        }
    }

    #[tokio::test]
    async fn emit_serialized_preserves_serialization_failure() {
        let (tx, _rx) = mpsc::channel(1);
        let error = emit_serialized(&tx, &FailingSerialization)
            .await
            .expect_err("serialization should fail");
        assert!(matches!(error, ResponseStreamEmitError::Serialization(_)));
    }

    #[tokio::test]
    async fn emit_serialized_preserves_client_disconnect() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx);
        let error = emit_serialized(&tx, &json!({"type": "test"}))
            .await
            .expect_err("closed receiver should reject delivery");
        assert!(matches!(error, ResponseStreamEmitError::ClientDisconnected));
    }
}
