## Contents

- Stream Processing Pipeline
- HTTP Response Streaming

## Stream Processing Pipeline

```rust
use futures::{Stream, StreamExt};
use std::pin::Pin;

pub struct StreamProcessor<T: SSETransformer> {
    parser: UnifiedSSEParser,
    transformer: T,
}

impl<T: SSETransformer> StreamProcessor<T> {
    pub fn new(transformer: T) -> Self {
        Self {
            parser: UnifiedSSEParser::new(),
            transformer,
        }
    }

    /// Process a byte stream and produce ChatChunks
    pub fn process_stream(
        mut self,
        input: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    ) -> Pin<Box<dyn Stream<Item = Result<ChatChunk, StreamError>> + Send>> {
        let stream = input.flat_map(move |result| {
            match result {
                Ok(bytes) => {
                    let events = self.parser.feed(&bytes);
                    let chunks: Vec<Result<ChatChunk, StreamError>> = events
                        .into_iter()
                        .filter_map(|event| {
                            // Check for errors first
                            if let Some(error) = self.transformer.handle_error(&event) {
                                return Some(Err(error));
                            }

                            // Check if done
                            if self.transformer.is_done(&event) {
                                return None;
                            }

                            // Transform event
                            match self.transformer.transform(&event) {
                                Ok(Some(chunk)) => Some(Ok(chunk)),
                                Ok(None) => None,
                                Err(e) => Some(Err(e)),
                            }
                        })
                        .collect();

                    futures::stream::iter(chunks)
                }
                Err(e) => {
                    futures::stream::iter(vec![Err(StreamError::Interrupted {
                        provider: self.transformer.provider_name(),
                        message: e.to_string(),
                    })])
                }
            }
        });

        Box::pin(stream)
    }
}
```

---

## HTTP Response Streaming

```rust
use actix_web::{HttpResponse, web};
use futures::StreamExt;

pub async fn stream_chat_completion(
    request: ChatRequest,
    provider: Arc<dyn LLMProvider>,
) -> HttpResponse {
    // Get streaming response from provider
    let stream = match provider.chat_completion_stream(request, context).await {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .json(json!({"error": e.to_string()}));
        }
    };

    // Transform to SSE format
    let sse_stream = stream.map(|result| {
        match result {
            Ok(chunk) => {
                let json = serde_json::to_string(&chunk).unwrap_or_default();
                Ok::<_, std::io::Error>(bytes::Bytes::from(format!("data: {}\n\n", json)))
            }
            Err(e) => {
                let error_json = json!({"error": e.to_string()});
                Ok(bytes::Bytes::from(format!("data: {}\n\n", error_json)))
            }
        }
    });

    // Add [DONE] marker at the end
    let final_stream = sse_stream.chain(futures::stream::once(async {
        Ok::<_, std::io::Error>(bytes::Bytes::from("data: [DONE]\n\n"))
    }));

    HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(final_stream)
}
```
