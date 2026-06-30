use crate::core::streaming::types::Event;
use bytes::Bytes;
use serde_json::json;
use tokio::sync::mpsc;

pub(super) async fn send_stream_error(
    tx: &mpsc::Sender<Bytes>,
    message: &str,
    error_type: &str,
    code: &str,
) {
    let error_json = json!({
        "error": {
            "message": message,
            "type": error_type,
            "code": code,
        }
    });
    let mut bytes = Event::default()
        .data(&error_json.to_string())
        .to_bytes()
        .to_vec();
    bytes.extend_from_slice(&Event::default().data("[DONE]").to_bytes());
    let _ = tx.send(Bytes::from(bytes)).await;
}
