use super::*;
use crate::core::providers::base::sse::{UnifiedSSEStream, create_provider_sse_stream};
use bytes::Bytes;
use futures::{Stream, StreamExt, stream};
use serde_json::json;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

fn event(choices: Value, usage: Value) -> String {
    format!(
        "data: {}\n\n",
        json!({"id":"stream", "model":"test", "created":1,
            "choices":choices, "usage":usage})
    )
}

fn choice(index: u32, finish: Option<&str>) -> Value {
    json!({"index":index, "delta":{"content":"你好"}, "finish_reason":finish})
}

fn usage() -> Value {
    json!({"prompt_tokens":3,"completion_tokens":2,"total_tokens":5})
}

fn assert_incomplete(error: ProviderError, index: Option<u64>) {
    match error {
        ProviderError::Streaming {
            provider,
            stream_type,
            position,
            last_chunk,
            message,
        } => {
            assert_eq!(provider, "test");
            assert_eq!(stream_type, "chat.completion");
            assert_eq!(position, index);
            assert!(last_chunk.is_none());
            assert!(message.contains("stream ended"), "{message}");
        }
        error => panic!("unexpected terminal error: {error}"),
    }
}

#[tokio::test]
async fn incomplete_http_body_eof_emits_a_terminal_error() {
    let body = event(json!([choice(0, None)]), Value::Null);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut request = [0; 4096];
        assert!(socket.read(&mut request).await.unwrap() > 0);
        // A complete HTTP response carrying an incomplete LLM protocol.
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let response = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("http://{address}"))
        .send()
        .await
        .unwrap();
    let mut output = create_provider_sse_stream(response, "test");
    let first = output.next().await.unwrap().unwrap();
    assert_eq!(first.choices[0].delta.content.as_deref(), Some("你好"));
    assert_incomplete(output.next().await.unwrap().unwrap_err(), Some(0));
    assert!(output.next().await.is_none());
    server.await.unwrap();
}

#[tokio::test]
async fn done_does_not_hide_an_unfinished_choice_or_drop_observed_usage() {
    // The completed choice is deliberately listed first, and the unfinished
    // choice uses a nonzero index. All frames arrive in one network read.
    let body = format!(
        "{}{}data: [DONE]\n\n",
        event(
            json!([choice(0, Some("stop")), choice(7, None)]),
            Value::Null
        ),
        event(json!([]), usage())
    );
    let source = stream::iter([Ok::<_, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    assert_eq!(output.next().await.unwrap().unwrap().choices.len(), 2);
    assert_eq!(
        output
            .next()
            .await
            .unwrap()
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        5
    );
    assert_incomplete(output.next().await.unwrap().unwrap_err(), Some(7));
    assert!(output.next().await.is_none());
}

struct OpenBody {
    bytes: Option<Bytes>,
    dropped: Arc<AtomicBool>,
}

impl Stream for OpenBody {
    type Item = Result<Bytes, reqwest::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.bytes.take() {
            Some(bytes) => Poll::Ready(Some(Ok(bytes))),
            None => Poll::Pending,
        }
    }
}

impl Drop for OpenBody {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}

#[tokio::test]
async fn done_releases_an_open_body_after_multi_choice_completion_and_usage() {
    let body = format!(
        "{}{}{}{}data: [DONE]\n\ndata: invalid trailing data\n\n",
        event(json!([choice(0, None), choice(2, None)]), Value::Null),
        event(json!([choice(2, Some("tool_calls"))]), Value::Null),
        event(json!([choice(0, Some("stop"))]), Value::Null),
        event(json!([]), usage())
    );
    let dropped = Arc::new(AtomicBool::new(false));
    let source = OpenBody {
        bytes: Some(Bytes::from(body)),
        dropped: Arc::clone(&dropped),
    };
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    let chunks = tokio::time::timeout(Duration::from_secs(1), async {
        let mut chunks = Vec::new();
        while let Some(chunk) = output.next().await {
            chunks.push(chunk.unwrap());
        }
        chunks
    })
    .await
    .expect("DONE must finish without waiting for HTTP EOF");
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[3].usage.as_ref().unwrap().total_tokens, 5);
    assert!(
        dropped.load(Ordering::SeqCst),
        "body is dropped before the wrapper"
    );
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn completed_choices_allow_eof_and_preserve_unterminated_usage_frame() {
    let body = format!(
        "{}{}",
        event(json!([choice(0, Some("length"))]), Value::Null),
        event(json!([]), usage()).trim_end()
    );
    let source = stream::iter([Ok::<_, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    assert!(
        output.next().await.unwrap().unwrap().choices[0]
            .finish_reason
            .is_some()
    );
    assert_eq!(
        output
            .next()
            .await
            .unwrap()
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        5
    );
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn unfinished_eof_preserves_usage_from_last_buffered_event_before_error() {
    let body = format!(
        "{}{}",
        event(json!([choice(3, None)]), Value::Null),
        event(json!([]), usage()).trim_end()
    );
    let source = stream::iter([Ok::<_, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    assert!(output.next().await.unwrap().is_ok());
    assert_eq!(
        output
            .next()
            .await
            .unwrap()
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        5
    );
    assert_incomplete(output.next().await.unwrap().unwrap_err(), Some(3));
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn every_byte_can_split_utf8_terminal_frames_and_done() {
    let body = format!(
        "{}data: [DONE]\r\n\r\n",
        event(json!([choice(0, Some("stop"))]), usage())
    );
    let source = stream::iter(
        body.into_bytes()
            .into_iter()
            .map(|byte| Ok::<_, reqwest::Error>(Bytes::from(vec![byte]))),
    );
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    let chunk = output.next().await.unwrap().unwrap();
    assert_eq!(chunk.choices[0].delta.content.as_deref(), Some("你好"));
    assert_eq!(chunk.usage.unwrap().total_tokens, 5);
    assert!(output.next().await.is_none());
}

#[tokio::test]
async fn empty_usage_only_and_bare_done_streams_are_not_successes() {
    for body in [
        String::new(),
        event(json!([]), usage()),
        "data: [DONE]\n\n".into(),
    ] {
        let source = stream::iter([Ok::<_, reqwest::Error>(Bytes::from(body))]);
        let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
        let mut error = None;
        while let Some(result) = output.next().await {
            if let Err(failure) = result {
                assert!(error.replace(failure).is_none(), "only one terminal error");
            }
        }
        assert_incomplete(error.expect("missing completion must fail"), None);
    }
}

#[tokio::test]
async fn data_after_a_finished_choice_is_a_protocol_error() {
    let body = format!(
        "{}{}",
        event(json!([choice(0, Some("stop"))]), usage()),
        event(json!([choice(0, None)]), Value::Null)
    );
    let source = stream::iter([Ok::<_, reqwest::Error>(Bytes::from(body))]);
    let mut output = UnifiedSSEStream::new(source, OpenAICompatibleTransformer::new("test"));
    assert_eq!(
        output
            .next()
            .await
            .unwrap()
            .unwrap()
            .usage
            .unwrap()
            .total_tokens,
        5
    );
    let error = output.next().await.unwrap().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("after its terminal finish_reason")
    );
    assert!(output.next().await.is_none());
}

#[test]
fn cloned_transformers_start_a_fresh_stream_lifecycle() {
    let original = OpenAICompatibleTransformer::new("test");
    let data = json!({"choices":[choice(0, Some("stop"))]}).to_string();
    original.transform_stream_chunk(&data).unwrap();
    original.finish_stream().unwrap();
    assert_incomplete(original.clone().finish_stream().unwrap_err(), None);
}
