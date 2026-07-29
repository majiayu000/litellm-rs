//! Gemini Streaming Module
//!
//! Uses the unified SSE parser with GeminiTransformer for Gemini's candidates/parts format.

use std::pin::Pin;

use bytes::Bytes;
use futures::Stream;
use pin_project_lite::pin_project;

use crate::core::providers::base::sse::{GeminiTransformer, UnifiedSSEStream};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::ChatChunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GeminiUsagePolicy {
    Direct,
    Vertex,
}

impl GeminiUsagePolicy {
    pub(crate) fn from_vertex_ai(use_vertex_ai: bool) -> Self {
        if use_vertex_ai {
            Self::Vertex
        } else {
            Self::Direct
        }
    }
}

/// Gemini SSE stream type
pub type GeminiSSEStream = UnifiedSSEStream<
    Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    GeminiTransformer,
>;

pin_project! {
    /// Handle streaming response
    pub struct GeminiStream {
        #[pin]
        inner: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>,
    }
}

impl GeminiStream {
    /// Create from response
    pub fn from_response(response: reqwest::Response, model: String) -> Self {
        let transformer = match response.extensions().get::<GeminiUsagePolicy>().copied() {
            Some(GeminiUsagePolicy::Direct) => GeminiTransformer::new(model),
            Some(GeminiUsagePolicy::Vertex) => GeminiTransformer::new_vertex(model),
            None => {
                tracing::error!(
                    "Gemini stream response is missing its explicit usage policy; \
                     usage metadata will be rejected"
                );
                GeminiTransformer::new_without_usage_policy(model)
            }
        };
        let stream = UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer);
        Self {
            inner: Box::pin(stream),
        }
    }
}

impl Stream for GeminiStream {
    type Item = Result<ChatChunk, ProviderError>;

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.project();
        this.inner.poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess;
    use crate::core::providers::base::sse::GeminiTransformer;
    use crate::core::providers::base::sse::UnifiedSSEParser;
    use crate::core::providers::gemini::{GeminiClient, GeminiConfig};
    use crate::core::types::chat::{ChatMessage, ChatRequest};
    use crate::core::types::message::{MessageContent, MessageRole};
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn serve_sse_once(body: &str) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let body = body.to_string();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0_u8; 4096];
            assert!(socket.read(&mut request).await.unwrap() > 0);
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\
                 content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        (format!("http://{address}"), server)
    }

    async fn response_at(path: &str, body: &str) -> reqwest::Response {
        let (base_url, server) = serve_sse_once(body).await;
        let response = reqwest::Client::builder()
            .no_proxy()
            .build()
            .unwrap()
            .get(format!("{base_url}{path}"))
            .send()
            .await
            .unwrap();
        server.await.unwrap();
        response
    }

    fn stream_request() -> ChatRequest {
        ChatRequest {
            model: "gemini-test".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn usage_event(total_tokens: u32) -> String {
        format!(
            "data: {{\"usageMetadata\":{{\"promptTokenCount\":10,\
             \"toolUsePromptTokenCount\":2,\"candidatesTokenCount\":3,\
             \"thoughtsTokenCount\":4,\"totalTokenCount\":{total_tokens}}}}}\n\n"
        )
    }

    async fn response_stream_chunks(policy: GeminiUsagePolicy, events: &[&str]) -> Vec<ChatChunk> {
        let body = events
            .iter()
            .map(|event| format!("data: {event}\n\n"))
            .collect::<String>();
        let mut response = response_at("/stream", &body).await;
        response.extensions_mut().insert(policy);
        GeminiStream::from_response(response, "gemini-test".to_string())
            .map(|chunk| chunk.unwrap())
            .collect()
            .await
    }

    #[tokio::test]
    async fn direct_and_vertex_public_streams_publish_only_final_valid_usage() {
        let valid = r#"{"candidates":[],"usageMetadata":{"promptTokenCount":1,"candidatesTokenCount":2,"totalTokenCount":3}}"#;
        let invalid =
            r#"{"candidates":[],"usageMetadata":{"promptTokenCount":4,"totalTokenCount":4}}"#;
        let missing = r#"{"candidates":[{"content":{"parts":[{"text":"ok"}]}}]}"#;
        let recovered = r#"{"candidates":[],"usageMetadata":{"promptTokenCount":4,"candidatesTokenCount":2,"totalTokenCount":6}}"#;
        for policy in [GeminiUsagePolicy::Direct, GeminiUsagePolicy::Vertex] {
            let invalid_final = response_stream_chunks(policy, &[valid, invalid]).await;
            assert_eq!(invalid_final.len(), 1);
            assert!(invalid_final[0].usage.is_none());

            let missing_final = response_stream_chunks(policy, &[valid, missing]).await;
            assert_eq!(missing_final.len(), 2);
            assert!(missing_final[0].usage.is_none());
            assert_eq!(
                missing_final[1]
                    .usage
                    .as_ref()
                    .map(|usage| usage.total_tokens),
                Some(3)
            );

            let recovered_final = response_stream_chunks(policy, &[invalid, recovered]).await;
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
                assert!(!json.contains("\"prompt_tokens\":0"));
            }
        }
    }

    #[tokio::test]
    async fn client_stream_response_carries_configured_usage_policy() {
        for use_vertex_ai in [false, true] {
            let total_tokens = if use_vertex_ai { 19 } else { 17 };
            let (base_url, server) = serve_sse_once(&usage_event(total_tokens)).await;
            let mut config = if use_vertex_ai {
                GeminiConfig::new_vertex_ai("project", "location")
            } else {
                GeminiConfig::new_google_ai("test-key-12345678901234567890")
            };
            config.base_url = format!("{base_url}/custom-base");
            config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
            let client = GeminiClient::new(config).unwrap();
            let response = client.chat_stream(stream_request()).await.unwrap();
            server.await.unwrap();
            assert_eq!(
                response.extensions().get::<GeminiUsagePolicy>().copied(),
                Some(GeminiUsagePolicy::from_vertex_ai(use_vertex_ai))
            );
            let mut stream = GeminiStream::from_response(response, "gemini-test".to_string());
            let usage = stream.next().await.unwrap().unwrap().usage.unwrap();
            assert_eq!(
                (
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens
                ),
                (12, 7, 19)
            );
        }
    }

    #[tokio::test]
    async fn from_response_uses_policy_not_arbitrary_final_url() {
        for (policy, total_tokens) in [
            (GeminiUsagePolicy::Direct, 17),
            (GeminiUsagePolicy::Vertex, 19),
        ] {
            let mut response = response_at(
                "/redirected/final/opaque-stream",
                &usage_event(total_tokens),
            )
            .await;
            assert_eq!(response.url().path(), "/redirected/final/opaque-stream");
            response.extensions_mut().insert(policy);
            let mut stream = GeminiStream::from_response(response, "gemini-test".to_string());
            assert_eq!(
                stream
                    .next()
                    .await
                    .unwrap()
                    .unwrap()
                    .usage
                    .unwrap()
                    .total_tokens,
                19
            );
        }
    }

    #[tokio::test]
    async fn missing_policy_preserves_content_and_rejects_usage() {
        let body = concat!(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"ok\"}]}}],",
            "\"usageMetadata\":{\"promptTokenCount\":2,\"candidatesTokenCount\":1,",
            "\"totalTokenCount\":3}}\n\n",
            "data: {\"usageMetadata\":{\"promptTokenCount\":2,\"candidatesTokenCount\":1,",
            "\"totalTokenCount\":3}}\n\n"
        );
        let response = response_at("/redirected/final/no-policy", body).await;
        let mut stream = GeminiStream::from_response(response, "gemini-test".to_string());
        let chunk = stream.next().await.unwrap().unwrap();
        assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("ok"));
        assert!(chunk.usage.is_none());
        let terminal = stream.next().await.unwrap().unwrap();
        assert!(terminal.choices.is_empty());
        assert!(terminal.usage.is_none());
        assert!(
            !serde_json::to_string(&terminal)
                .unwrap()
                .contains("__litellm")
        );
        assert!(stream.next().await.is_none());
    }

    #[test]
    fn test_sse_parsing() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}]}, "finishReason": null}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_done_parsing() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = b"data: [DONE]\n\n";
        let result = parser.process_bytes(data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_chunk_with_finish_reason_stop() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Done."}]}, "finishReason": "STOP"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::Stop)
        );
    }

    #[test]
    fn test_chunk_with_finish_reason_max_tokens() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "..."}]}, "finishReason": "MAX_TOKENS"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::Length)
        );
    }

    #[test]
    fn test_chunk_with_finish_reason_safety() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "..."}]}, "finishReason": "SAFETY"}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].finish_reason,
            Some(crate::core::types::responses::FinishReason::ContentFilter)
        );
    }

    #[test]
    fn test_chunk_with_usage() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}]}}], "usageMetadata": {"promptTokenCount": 10, "toolUsePromptTokenCount": 2, "candidatesTokenCount": 5, "thoughtsTokenCount": 3, "cachedContentTokenCount": 4, "totalTokenCount": 18}}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert!(result[0].usage.is_some());
        let usage = result[0].usage.as_ref().unwrap();
        assert_eq!(usage.prompt_tokens, 12);
        assert_eq!(usage.completion_tokens, 8);
        assert_eq!(usage.total_tokens, 20);
        assert_eq!(usage.thinking_tokens(), Some(3));
        assert!(usage.completion_tokens_details.is_none());
        assert_eq!(
            usage
                .prompt_tokens_details
                .as_ref()
                .and_then(|details| details.cache_read_tokens),
            Some(4)
        );
    }

    #[test]
    fn test_usage_only_chunk_fails_closed_on_partial_metadata() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);
        let data = br#"data: {"usageMetadata":{"promptTokenCount":10,"totalTokenCount":10}}

"#;
        assert!(parser.process_bytes(data).unwrap().is_empty());
        let huge = br#"data: {"usageMetadata":{"promptTokenCount":18446744073709551615,"candidatesTokenCount":0,"totalTokenCount":18446744073709551615}}

"#;
        assert_eq!(
            parser.process_bytes(huge).unwrap()[0]
                .usage
                .as_ref()
                .unwrap()
                .total_tokens,
            u32::MAX
        );
    }

    #[test]
    fn test_multiple_parts() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Hello"}, {"text": " world"}]}}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Hello world"
        );
    }

    #[test]
    fn test_multiple_candidates() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": [{"content": {"parts": [{"text": "Response 1"}]}}, {"content": {"parts": [{"text": "Response 2"}]}}]}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert_eq!(result[0].choices.len(), 2);
        assert_eq!(
            result[0].choices[0].delta.content.as_ref().unwrap(),
            "Response 1"
        );
        assert_eq!(
            result[0].choices[1].delta.content.as_ref().unwrap(),
            "Response 2"
        );
    }

    #[test]
    fn test_empty_candidates() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"candidates": []}

"#;
        let result = parser.process_bytes(data).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_error_response() {
        let transformer = GeminiTransformer::new("gemini-pro");
        let mut parser = UnifiedSSEParser::new(transformer);

        let data = br#"data: {"error": {"code": 400, "message": "Bad request"}}

"#;
        let result = parser.process_bytes(data);
        assert!(result.is_err());
    }
}
