use serde_json::Value;
use std::sync::{Arc, Mutex};

use super::SSETransformer;
use crate::core::providers::shared::strict_direct_gemini_usage_metadata;
#[cfg(any(feature = "providers-extended", test))]
use crate::core::providers::shared::strict_vertex_usage_metadata;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::message::MessageRole;
use crate::core::types::responses::{ChatChunk, ChatDelta, ChatStreamChoice, FinishReason, Usage};

#[derive(Debug, Clone, Copy)]
enum GeminiUsagePolicy {
    Direct,
    #[cfg(any(feature = "providers-extended", test))]
    Vertex,
}

#[derive(Debug, Default)]
enum GeminiStreamUsage {
    #[default]
    Missing,
    Valid(Usage),
    Invalid,
    Finalized,
}

/// Gemini SSE Transformer
///
/// Handles Gemini's streaming format with candidates/parts structure.
#[derive(Debug, Clone)]
pub struct GeminiTransformer {
    model: String,
    chunk_id: String,
    usage_policy: Option<GeminiUsagePolicy>,
    stream_usage: Arc<Mutex<GeminiStreamUsage>>,
}

impl GeminiTransformer {
    pub fn new(model: impl Into<String>) -> Self {
        Self::with_usage_policy(model, Some(GeminiUsagePolicy::Direct))
    }

    #[cfg(any(feature = "providers-extended", test))]
    pub(crate) fn new_vertex(model: impl Into<String>) -> Self {
        Self::with_usage_policy(model, Some(GeminiUsagePolicy::Vertex))
    }

    #[cfg(feature = "providers-extended")]
    pub(crate) fn new_without_usage_policy(model: impl Into<String>) -> Self {
        Self::with_usage_policy(model, None)
    }

    fn with_usage_policy(
        model: impl Into<String>,
        usage_policy: Option<GeminiUsagePolicy>,
    ) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        Self {
            model: model.into(),
            chunk_id: format!("gemini-stream-{}", nanos),
            usage_policy,
            stream_usage: Arc::new(Mutex::new(GeminiStreamUsage::Missing)),
        }
    }

    fn transform_usage_metadata(&self, metadata: &Value) -> Option<Usage> {
        match self.usage_policy {
            Some(GeminiUsagePolicy::Direct) => strict_direct_gemini_usage_metadata(metadata),
            #[cfg(any(feature = "providers-extended", test))]
            Some(GeminiUsagePolicy::Vertex) => strict_vertex_usage_metadata(metadata),
            None => None,
        }
    }

    fn empty_chunk(&self) -> ChatChunk {
        ChatChunk {
            id: self.chunk_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: self.model.clone(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        }
    }

    fn lock_stream_usage(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, GeminiStreamUsage>, ProviderError> {
        self.stream_usage.lock().map_err(|_| {
            ProviderError::response_parsing("gemini", "Gemini stream usage state is poisoned")
        })
    }

    fn observe_stream_usage(&self, parsed: &Value) -> Result<(), ProviderError> {
        let Some(metadata) = parsed.get("usageMetadata") else {
            return Ok(());
        };
        let next = self
            .transform_usage_metadata(metadata)
            .map_or(GeminiStreamUsage::Invalid, GeminiStreamUsage::Valid);
        let mut usage = self.lock_stream_usage()?;
        if !matches!(*usage, GeminiStreamUsage::Finalized) {
            *usage = next;
        }
        Ok(())
    }

    fn invalidate_stream_usage(&self) -> Result<(), ProviderError> {
        let mut usage = self.lock_stream_usage()?;
        if !matches!(*usage, GeminiStreamUsage::Finalized) {
            *usage = GeminiStreamUsage::Invalid;
        }
        Ok(())
    }

    fn transform_stream_data(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let parsed: Value = match serde_json::from_str(data) {
            Ok(parsed) => parsed,
            Err(_) if data.contains("\"usageMetadata\"") => {
                self.invalidate_stream_usage()?;
                return Ok(None);
            }
            Err(_) => return self.transform_chunk(data),
        };
        self.observe_stream_usage(&parsed)?;
        let Some(mut chunk) = self.transform_chunk(data)? else {
            return Ok(None);
        };
        chunk.usage = None;
        if chunk.choices.is_empty() {
            Ok(None)
        } else {
            Ok(Some(chunk))
        }
    }

    fn finish_stream_usage(&self) -> Result<Option<ChatChunk>, ProviderError> {
        let state = {
            let mut usage = self.lock_stream_usage()?;
            std::mem::replace(&mut *usage, GeminiStreamUsage::Finalized)
        };
        match state {
            GeminiStreamUsage::Valid(usage) => {
                let mut chunk = self.empty_chunk();
                chunk.usage = Some(usage);
                Ok(Some(chunk))
            }
            GeminiStreamUsage::Invalid => Ok(Some(self.empty_chunk())),
            GeminiStreamUsage::Missing | GeminiStreamUsage::Finalized => Ok(None),
        }
    }
}

impl SSETransformer for GeminiTransformer {
    fn provider_name(&self) -> &'static str {
        "gemini"
    }

    fn transform_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        let json: Value = serde_json::from_str(data).map_err(|e| {
            ProviderError::response_parsing("gemini", format!("Failed to parse Gemini SSE: {}", e))
        })?;

        // Error response
        if let Some(error) = json.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown Gemini error");
            return Err(ProviderError::api_error(
                "gemini",
                error.get("code").and_then(|c| c.as_u64()).unwrap_or(500) as u16,
                msg.to_string(),
            ));
        }

        let empty_arr = vec![];
        let candidates = json
            .get("candidates")
            .and_then(|c| c.as_array())
            .unwrap_or(&empty_arr);

        if candidates.is_empty() {
            // Usage-only chunk
            let usage = json
                .get("usageMetadata")
                .and_then(|metadata| self.transform_usage_metadata(metadata));
            if usage.is_some() {
                return Ok(Some(ChatChunk {
                    id: self.chunk_id.clone(),
                    object: "chat.completion.chunk".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: self.model.clone(),
                    choices: vec![],
                    usage,
                    system_fingerprint: None,
                }));
            }
            return Ok(None);
        }

        let mut choices = Vec::new();
        for (position, candidate) in candidates.iter().enumerate() {
            // Prefer the upstream candidate index (n>1 sends real indices);
            // fall back to the array position only when absent.
            let index = candidate
                .get("index")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(position as u32);

            let empty_parts = vec![];
            let parts = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .unwrap_or(&empty_parts);

            let mut text_parts = Vec::new();
            for part in parts {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            let delta_content = text_parts.join("");

            let finish_reason = candidate
                .get("finishReason")
                .and_then(|r| r.as_str())
                .map(|r| match r {
                    "STOP" => FinishReason::Stop,
                    "MAX_TOKENS" => FinishReason::Length,
                    "SAFETY" | "RECITATION" => FinishReason::ContentFilter,
                    _ => FinishReason::Stop,
                });

            choices.push(ChatStreamChoice {
                index,
                delta: ChatDelta {
                    role: if !delta_content.is_empty() || finish_reason.is_some() {
                        Some(MessageRole::Assistant)
                    } else {
                        None
                    },
                    content: if delta_content.is_empty() {
                        None
                    } else {
                        Some(delta_content)
                    },
                    thinking: None,
                    function_call: None,
                    tool_calls: None,
                    audio: None,
                },
                finish_reason,
                logprobs: None,
            });
        }

        let usage = json
            .get("usageMetadata")
            .and_then(|metadata| self.transform_usage_metadata(metadata));

        if choices.is_empty() && usage.is_none() {
            return Ok(None);
        }

        Ok(Some(ChatChunk {
            id: self.chunk_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: self.model.clone(),
            choices,
            usage,
            system_fingerprint: None,
        }))
    }

    fn transform_stream_chunk(&self, data: &str) -> Result<Option<ChatChunk>, ProviderError> {
        self.transform_stream_data(data)
    }

    fn finish_stream(&self) -> Result<Option<ChatChunk>, ProviderError> {
        self.finish_stream_usage()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::base::sse::UnifiedSSEStream;
    use bytes::Bytes;
    use futures::StreamExt;

    #[test]
    fn strict_usage_metadata_applies_to_candidate_and_usage_only_chunks() {
        let transformer = GeminiTransformer::new("gemini-test");
        let usage = r#""usageMetadata":{
            "promptTokenCount":10,"toolUsePromptTokenCount":2,
            "candidatesTokenCount":3,"thoughtsTokenCount":4,
            "cachedContentTokenCount":5,"totalTokenCount":17
        }"#;
        for data in [
            format!(r#"{{"candidates":[],{usage}}}"#),
            format!(r#"{{"candidates":[{{"content":{{"parts":[{{"text":"ok"}}]}}}}],{usage}}}"#),
        ] {
            let chunk = transformer.transform_chunk(&data).unwrap().unwrap();
            let usage = chunk.usage.unwrap();
            assert_eq!(
                (
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens
                ),
                (12, 7, 19)
            );
            assert!(usage.completion_tokens_details.is_none());
            assert_eq!(usage.thinking_tokens(), Some(4));
        }
    }

    #[test]
    fn vertex_usage_policy_applies_to_candidate_and_usage_only_chunks() {
        let transformer = GeminiTransformer::new_vertex("gemini-test");
        for candidates in ["[]", r#"[{"content":{"parts":[{"text":"ok"}]}}]"#] {
            let data = format!(
                r#"{{"candidates":{candidates},"usageMetadata":{{
                    "promptTokenCount":10,"toolUsePromptTokenCount":2,
                    "candidatesTokenCount":3,"thoughtsTokenCount":4,
                    "cachedContentTokenCount":5,"totalTokenCount":19
                }}}}"#
            );
            let usage = transformer
                .transform_chunk(&data)
                .unwrap()
                .unwrap()
                .usage
                .unwrap();
            assert_eq!(
                (
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens
                ),
                (12, 7, 19)
            );
        }
        let direct_total = r#"{
            "promptTokenCount":10,"toolUsePromptTokenCount":2,
            "candidatesTokenCount":3,"thoughtsTokenCount":4,"totalTokenCount":17
        }"#;
        let usage_only = format!(r#"{{"candidates":[],"usageMetadata":{direct_total}}}"#);
        assert!(transformer.transform_chunk(&usage_only).unwrap().is_none());
        let candidate = format!(
            r#"{{"candidates":[{{"content":{{"parts":[{{"text":"ok"}}]}}}}],"usageMetadata":{direct_total}}}"#
        );
        assert!(
            transformer
                .transform_chunk(&candidate)
                .unwrap()
                .unwrap()
                .usage
                .is_none()
        );
    }

    #[test]
    fn malformed_usage_never_becomes_zero_usage() {
        let transformer = GeminiTransformer::new("gemini-test");
        for metadata in [
            r#"{"promptTokenCount":2,"candidatesTokenCount":1,"totalTokenCount":4}"#,
            r#"{"promptTokenCount":2,"candidatesTokenCount":null,"totalTokenCount":2}"#,
            r#"{"promptTokenCount":0,"candidatesTokenCount":0,"totalTokenCount":0}"#,
        ] {
            let usage_only = format!(r#"{{"candidates":[],"usageMetadata":{metadata}}}"#);
            assert!(transformer.transform_chunk(&usage_only).unwrap().is_none());
            let with_output = format!(
                r#"{{"candidates":[{{"content":{{"parts":[{{"text":"ok"}}]}}}}],"usageMetadata":{metadata}}}"#
            );
            let chunk = transformer.transform_chunk(&with_output).unwrap().unwrap();
            assert!(chunk.usage.is_none());
        }
        let huge = transformer
            .transform_chunk(
                r#"{"candidates":[],"usageMetadata":{"promptTokenCount":18446744073709551615,"candidatesTokenCount":0,"totalTokenCount":18446744073709551615}}"#,
            )
            .unwrap()
            .unwrap();
        assert_eq!(huge.usage.unwrap().total_tokens, u32::MAX);
    }

    async fn collect_stream(transformer: GeminiTransformer, events: &[&str]) -> Vec<ChatChunk> {
        let body = events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>();
        let source = futures::stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
        UnifiedSSEStream::new(source, transformer)
            .map(|result| result.unwrap())
            .collect()
            .await
    }

    #[tokio::test]
    async fn terminal_stream_usage_is_private_and_stateful() {
        for vertex in [false, true] {
            let transformer = || {
                if vertex {
                    GeminiTransformer::new_vertex("vertex-test")
                } else {
                    GeminiTransformer::new("direct-test")
                }
            };
            let valid = r#"{"candidates":[],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":2,"totalTokenCount":3}}"#;
            let invalid =
                r#"{"candidates":[],"usageMetadata":{"promptTokenCount":4,"totalTokenCount":4}}"#;
            let missing = r#"{"candidates":[{"content":{"parts":[{"text":"still streaming"}]}}]}"#;
            let recovered = r#"{"candidates":[],"usageMetadata":{"promptTokenCount":4,"candidatesTokenCount":2,"totalTokenCount":6}}"#;

            let invalid_final = collect_stream(transformer(), &[valid, invalid]).await;
            assert_eq!(invalid_final.len(), 1);
            assert!(invalid_final[0].choices.is_empty());
            assert!(invalid_final[0].usage.is_none());

            let missing_final = collect_stream(transformer(), &[valid, missing]).await;
            assert_eq!(missing_final.len(), 2);
            assert!(missing_final[0].usage.is_none());
            assert_eq!(
                missing_final[1]
                    .usage
                    .as_ref()
                    .map(|usage| usage.total_tokens),
                Some(3)
            );

            let recovered_final = collect_stream(transformer(), &[valid, invalid, recovered]).await;
            assert_eq!(recovered_final.len(), 1);
            assert_eq!(
                recovered_final[0]
                    .usage
                    .as_ref()
                    .map(|usage| usage.total_tokens),
                Some(6)
            );

            for chunk in invalid_final
                .iter()
                .chain(missing_final.iter())
                .chain(recovered_final.iter())
            {
                let json = serde_json::to_string(chunk).unwrap();
                assert!(!json.contains("__litellm"));
                assert!(!chunk.usage.as_ref().is_some_and(|usage| {
                    usage.prompt_tokens == 0
                        && usage.completion_tokens == 0
                        && usage.total_tokens == 0
                }));
            }
        }
    }

    #[tokio::test]
    async fn stream_flushes_truncated_usage_at_eof_as_invalid() {
        let body = concat!(
            "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":1,",
            "\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\n\n",
            "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4"
        );
        let source = futures::stream::iter([Ok::<Bytes, reqwest::Error>(Bytes::from(body))]);
        let stream = UnifiedSSEStream::new(source, GeminiTransformer::new("gemini-test"));
        let chunks = stream
            .map(|chunk| chunk.unwrap())
            .collect::<Vec<ChatChunk>>()
            .await;
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].choices.is_empty());
        assert!(chunks[0].usage.is_none());
        assert!(
            !serde_json::to_string(&chunks[0])
                .unwrap()
                .contains("__litellm")
        );
    }

    #[tokio::test]
    async fn stream_flushes_truncated_usage_before_read_error() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 1024];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                      transfer-encoding: chunked\r\n\r\n",
                )
                .await
                .unwrap();
            for body in [
                concat!(
                    "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":1,",
                    "\"candidatesTokenCount\":2,\"totalTokenCount\":3}}\n\n"
                ),
                "data: {\"candidates\":[],\"usageMetadata\":{\"promptTokenCount\":4",
            ] {
                socket
                    .write_all(format!("{:x}\r\n{body}\r\n", body.len()).as_bytes())
                    .await
                    .unwrap();
                socket.flush().await.unwrap();
                tokio::task::yield_now().await;
            }
        });
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap();
        let mut stream = UnifiedSSEStream::new(
            Box::pin(response.bytes_stream()),
            GeminiTransformer::new("gemini-test"),
        );
        let mut chunks = Vec::new();
        let mut saw_read_error = false;
        while let Some(result) = stream.next().await {
            match result {
                Ok(chunk) => chunks.push(chunk),
                Err(_) => {
                    saw_read_error = true;
                    break;
                }
            }
        }
        server.await.unwrap();
        assert!(saw_read_error);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].usage.is_none());
        assert!(
            !serde_json::to_string(&chunks[0])
                .unwrap()
                .contains("__litellm")
        );
    }
}
