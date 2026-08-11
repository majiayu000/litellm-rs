use actix_web::body::{BodySize, MessageBody};
use bytes::Bytes;
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pub(super) enum AuditBodyOutcome {
    Completed,
    Failed(&'static str),
}

pub(super) struct AuditTerminalRecorder {
    callback: Option<Box<dyn FnOnce(AuditBodyOutcome)>>,
}

impl AuditTerminalRecorder {
    pub(super) fn new(callback: impl FnOnce(AuditBodyOutcome) + 'static) -> Self {
        Self {
            callback: Some(Box::new(callback)),
        }
    }

    pub(super) fn record(mut self, outcome: AuditBodyOutcome) {
        if let Some(callback) = self.callback.take() {
            callback(outcome);
        }
    }
}

pin_project! {
    pub struct AuditResponseBody<B> {
        #[pin]
        body: B,
        recorder: Option<AuditTerminalRecorder>,
        detect_sse_errors: bool,
        event_buffer: String,
        emitted_error: bool,
    }

    impl<B> PinnedDrop for AuditResponseBody<B> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(recorder) = this.recorder.take() {
                recorder.record(AuditBodyOutcome::Failed(
                    "stream body dropped before completion",
                ));
            }
        }
    }
}

impl<B> AuditResponseBody<B> {
    pub(super) fn passthrough(body: B) -> Self {
        Self {
            body,
            recorder: None,
            detect_sse_errors: false,
            event_buffer: String::new(),
            emitted_error: false,
        }
    }

    pub(super) fn streaming(
        body: B,
        recorder: AuditTerminalRecorder,
        detect_sse_errors: bool,
    ) -> Self {
        Self {
            body,
            recorder: Some(recorder),
            detect_sse_errors,
            event_buffer: String::new(),
            emitted_error: false,
        }
    }
}

impl<B> MessageBody for AuditResponseBody<B>
where
    B: MessageBody,
{
    type Error = B::Error;

    fn size(&self) -> BodySize {
        self.body.size()
    }

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Bytes, Self::Error>>> {
        let this = self.project();
        match this.body.poll_next(cx) {
            Poll::Ready(Some(Ok(bytes))) => {
                if *this.detect_sse_errors && !*this.emitted_error {
                    this.event_buffer.push_str(&String::from_utf8_lossy(&bytes));
                    *this.emitted_error = consume_sse_errors(this.event_buffer);
                    if this.event_buffer.len() > 65_536 {
                        let mut keep_from = this.event_buffer.len() - 65_536;
                        while !this.event_buffer.is_char_boundary(keep_from) {
                            keep_from += 1;
                        }
                        this.event_buffer.drain(..keep_from);
                    }
                }
                Poll::Ready(Some(Ok(bytes)))
            }
            Poll::Ready(Some(Err(error))) => {
                if let Some(recorder) = this.recorder.take() {
                    recorder.record(AuditBodyOutcome::Failed("response body stream failed"));
                }
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                if *this.detect_sse_errors && !*this.emitted_error {
                    *this.emitted_error = is_error_sse_event(this.event_buffer);
                }
                if let Some(recorder) = this.recorder.take() {
                    let outcome = if *this.emitted_error {
                        AuditBodyOutcome::Failed("stream emitted an error event")
                    } else {
                        AuditBodyOutcome::Completed
                    };
                    recorder.record(outcome);
                }
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

fn consume_sse_errors(buffer: &mut String) -> bool {
    while let Some((position, delimiter_len)) = event_boundary(buffer) {
        let event = buffer.drain(..position + delimiter_len).collect::<String>();
        if is_error_sse_event(&event[..position]) {
            return true;
        }
    }
    false
}

fn event_boundary(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|position| (position, 2));
    let crlf = buffer.find("\r\n\r\n").map(|position| (position, 4));
    match (lf, crlf) {
        (Some(left), Some(right)) => Some(if left.0 <= right.0 { left } else { right }),
        (Some(boundary), None) | (None, Some(boundary)) => Some(boundary),
        (None, None) => None,
    }
}

fn is_error_sse_event(event: &str) -> bool {
    let mut data = String::new();
    for line in event.lines() {
        let line = line.trim_end_matches('\r');
        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.strip_prefix(' ').unwrap_or(value);
        if field == "event" && value == "error" {
            return true;
        }
        if field == "data" {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value);
        }
    }
    serde_json::from_str::<serde_json::Value>(&data)
        .ok()
        .is_some_and(|value| value.get("error").is_some())
}

#[cfg(test)]
mod tests {
    use super::{consume_sse_errors, is_error_sse_event};

    #[test]
    fn recognizes_supported_stream_error_envelopes() {
        let mut json_error = "data: {\"error\":{\"code\":\"timeout\"}}\n\n".to_string();
        assert!(consume_sse_errors(&mut json_error));
        assert!(is_error_sse_event(
            "event: error\ndata: Gemini upstream stream error"
        ));
        let mut content = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":",
            "\"SSE uses event: error for failures\"}}]}\n\n"
        )
        .to_string();
        assert!(!consume_sse_errors(&mut content));
    }

    #[test]
    fn waits_for_complete_events_across_chunks_and_supports_crlf() {
        let mut buffer = "event: err".to_string();
        assert!(!consume_sse_errors(&mut buffer));
        buffer.push_str("or\r\ndata: failed\r\n\r\n");
        assert!(consume_sse_errors(&mut buffer));
    }
}
