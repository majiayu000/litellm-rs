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
                    *this.emitted_error = contains_sse_error(this.event_buffer);
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

fn contains_sse_error(buffer: &str) -> bool {
    buffer.contains("event: error") || buffer.contains(r#""error":{"#)
}

#[cfg(test)]
mod tests {
    use super::contains_sse_error;

    #[test]
    fn recognizes_supported_stream_error_envelopes() {
        assert!(contains_sse_error(
            r#"data: {"error":{"code":"timeout"}}\n\n"#
        ));
        assert!(contains_sse_error(
            "event: error\ndata: Gemini upstream stream error\n\n"
        ));
        assert!(!contains_sse_error(
            r#"data: {"choices":[{"delta":{"content":"error"}}]}\n\n"#
        ));
    }
}
