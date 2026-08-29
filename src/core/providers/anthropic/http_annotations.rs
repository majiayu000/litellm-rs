use serde_json::Value;
use tokio::sync::mpsc::{self, Receiver, Sender, error::TrySendError};

use crate::core::providers::unified_provider::ProviderError;

#[derive(Debug)]
pub(crate) struct HttpAnnotation {
    pub(crate) choice_index: u32,
    pub(crate) value: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct HttpAnnotationSender(Sender<HttpAnnotation>);

pub(crate) struct HttpAnnotationReceiver {
    receiver: Receiver<HttpAnnotation>,
}

// Keep this aligned with the shared SSE chunk-buffer ceiling. The synchronous
// transformer cannot await capacity, so a full channel fails the stream closed
// rather than allocating without a bound.
const HTTP_ANNOTATION_CHANNEL_CAPACITY: usize = 10_000;

pub(crate) fn http_annotation_channel() -> (HttpAnnotationSender, HttpAnnotationReceiver) {
    let (sender, receiver) = mpsc::channel(HTTP_ANNOTATION_CHANNEL_CAPACITY);
    (
        HttpAnnotationSender(sender),
        HttpAnnotationReceiver { receiver },
    )
}

impl HttpAnnotationSender {
    pub(crate) fn send(&self, choice_index: u32, value: Value) -> Result<(), ProviderError> {
        self.0
            .try_send(HttpAnnotation {
                choice_index,
                value,
            })
            .map_err(|error| {
                let message = match error {
                    TrySendError::Full(_) => {
                        "Anthropic HTTP annotation channel reached its bounded capacity"
                    }
                    TrySendError::Closed(_) => {
                        "Anthropic HTTP annotation receiver closed before citation delivery"
                    }
                };
                ProviderError::streaming_error("anthropic", "chat.citations", None, None, message)
            })
    }
}

impl HttpAnnotationReceiver {
    pub(crate) fn take_for_choice(&mut self, choice_index: u32) -> Result<Value, ProviderError> {
        let annotation = self.receiver.try_recv().map_err(|error| {
            ProviderError::response_parsing(
                "router",
                format!("Anthropic annotation marker has no matching typed payload: {error}"),
            )
        })?;
        if annotation.choice_index != choice_index {
            return Err(ProviderError::response_parsing(
                "router",
                format!(
                    "Anthropic annotation choice {} does not match marker choice {choice_index}",
                    annotation.choice_index
                ),
            ));
        }
        Ok(annotation.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_annotations_keep_order_and_choice_identity() {
        let (sender, mut receiver) = http_annotation_channel();

        sender.send(0, serde_json::json!({"ordinal": 1})).unwrap();
        sender.send(2, serde_json::json!({"ordinal": 2})).unwrap();

        assert_eq!(
            receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"ordinal": 1})
        );
        assert_eq!(
            receiver.take_for_choice(2).unwrap(),
            serde_json::json!({"ordinal": 2})
        );
    }

    #[test]
    fn receiver_drop_closes_the_request_owned_private_channel() {
        let (sender, receiver) = http_annotation_channel();

        drop(receiver);

        assert!(
            sender
                .send(0, serde_json::json!({"never":"delivered"}))
                .is_err()
        );
    }

    #[test]
    fn duplicate_external_request_ids_do_not_share_private_channels() {
        // HTTP request IDs are observability data only. Each request owns a
        // fresh channel, so identical caller headers cannot collide.
        let (first_sender, mut first_receiver) = http_annotation_channel();
        let (second_sender, mut second_receiver) = http_annotation_channel();

        first_sender
            .send(0, serde_json::json!({"stream": "first"}))
            .unwrap();
        second_sender
            .send(0, serde_json::json!({"stream": "second"}))
            .unwrap();
        assert_eq!(
            first_receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"stream": "first"})
        );
        assert_eq!(
            second_receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"stream": "second"})
        );
    }

    #[test]
    fn bounded_channel_fails_closed_and_recovers_after_drain() {
        let (sender, receiver) = mpsc::channel(1);
        let sender = HttpAnnotationSender(sender);
        let mut receiver = HttpAnnotationReceiver { receiver };

        sender.send(0, serde_json::json!({"ordinal": 1})).unwrap();
        let error = sender
            .send(0, serde_json::json!({"ordinal": 2}))
            .expect_err("a full request-local channel must not allocate without a bound");
        assert!(matches!(
            error,
            ProviderError::Streaming { message, .. } if message.contains("bounded capacity")
        ));
        assert_eq!(
            receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"ordinal": 1})
        );
        sender.send(0, serde_json::json!({"ordinal": 3})).unwrap();
        assert_eq!(
            receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"ordinal": 3})
        );
    }
}
