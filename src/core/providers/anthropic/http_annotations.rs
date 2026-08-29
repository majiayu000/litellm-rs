use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use serde_json::Value;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::core::providers::unified_provider::ProviderError;

#[derive(Debug)]
pub(crate) struct HttpAnnotation {
    pub(crate) choice_index: u32,
    pub(crate) value: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct HttpAnnotationSender(UnboundedSender<HttpAnnotation>);

pub(crate) struct HttpAnnotationReceiver {
    request_id: String,
    receiver: UnboundedReceiver<HttpAnnotation>,
}

static CHANNELS: LazyLock<Mutex<HashMap<String, HttpAnnotationSender>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn channels() -> std::sync::MutexGuard<'static, HashMap<String, HttpAnnotationSender>> {
    match CHANNELS.lock() {
        Ok(channels) => channels,
        Err(poisoned) => poisoned.into_inner(),
    }
}

pub(crate) fn register_http_annotation_channel(
    request_id: &str,
) -> Result<HttpAnnotationReceiver, ProviderError> {
    if request_id.is_empty() {
        return Err(ProviderError::configuration(
            "router",
            "Anthropic HTTP annotation channel requires a request ID",
        ));
    }

    let (sender, receiver) = mpsc::unbounded_channel();
    let mut registered = channels();
    if registered.contains_key(request_id) {
        return Err(ProviderError::configuration(
            "router",
            "Duplicate Anthropic HTTP annotation channel request ID",
        ));
    }
    registered.insert(request_id.to_string(), HttpAnnotationSender(sender));
    drop(registered);

    Ok(HttpAnnotationReceiver {
        request_id: request_id.to_string(),
        receiver,
    })
}

pub(crate) fn http_annotation_sender(request_id: &str) -> Option<HttpAnnotationSender> {
    channels().get(request_id).cloned()
}

impl HttpAnnotationSender {
    pub(crate) fn send(&self, choice_index: u32, value: Value) -> Result<(), ProviderError> {
        self.0
            .send(HttpAnnotation {
                choice_index,
                value,
            })
            .map_err(|_| {
                ProviderError::streaming_error(
                    "anthropic",
                    "chat.citations",
                    None,
                    None,
                    "Anthropic HTTP annotation receiver closed before citation delivery",
                )
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

impl Drop for HttpAnnotationReceiver {
    fn drop(&mut self) {
        channels().remove(&self.request_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_annotations_keep_order_and_choice_identity() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut receiver = register_http_annotation_channel(&request_id).unwrap();
        let sender = http_annotation_sender(&request_id).unwrap();

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
    fn receiver_drop_deregisters_and_closes_the_private_channel() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let receiver = register_http_annotation_channel(&request_id).unwrap();
        let sender = http_annotation_sender(&request_id).unwrap();

        drop(receiver);

        assert!(http_annotation_sender(&request_id).is_none());
        assert!(
            sender
                .send(0, serde_json::json!({"never":"delivered"}))
                .is_err()
        );
    }

    #[test]
    fn duplicate_live_request_id_is_rejected_without_replacing_the_receiver() {
        let request_id = uuid::Uuid::new_v4().to_string();
        let mut receiver = register_http_annotation_channel(&request_id).unwrap();
        assert!(register_http_annotation_channel(&request_id).is_err());

        http_annotation_sender(&request_id)
            .unwrap()
            .send(0, serde_json::json!({"kept": true}))
            .unwrap();
        assert_eq!(
            receiver.take_for_choice(0).unwrap(),
            serde_json::json!({"kept": true})
        );
    }
}
