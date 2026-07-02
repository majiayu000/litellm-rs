use std::collections::HashMap;
use tracing::debug;

use super::span::{AttributeValue, Span, SpanKind, SpanStatus};

pub(super) async fn export_spans(
    client: &reqwest::Client,
    endpoint: &str,
    headers: &HashMap<String, String>,
    service_name: &str,
    spans: Vec<Span>,
) -> Result<(), String> {
    if spans.is_empty() {
        return Ok(());
    }

    // Build OTLP JSON payload
    let payload = build_otlp_payload(service_name, &spans);

    let mut request = client
        .post(format!("{}/v1/traces", endpoint))
        .header("Content-Type", "application/json");

    for (key, value) in headers {
        request = request.header(key, value);
    }

    let response = request
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("HTTP error: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "OTLP export failed with status: {}",
            response.status()
        ));
    }

    debug!("Exported {} spans to OTLP", spans.len());
    Ok(())
}

/// Build OTLP JSON payload
pub(super) fn build_otlp_payload(service_name: &str, spans: &[Span]) -> serde_json::Value {
    let resource_spans = serde_json::json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [{
                    "key": "service.name",
                    "value": { "stringValue": service_name }
                }]
            },
            "scopeSpans": [{
                "scope": {
                    "name": "litellm-rs",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "spans": spans.iter().map(|span| {
                    let mut span_json = serde_json::json!({
                        "traceId": span.trace_id,
                        "spanId": span.span_id,
                        "name": span.name,
                        "kind": match span.kind {
                            SpanKind::Internal => 1,
                            SpanKind::Server => 2,
                            SpanKind::Client => 3,
                            SpanKind::Producer => 4,
                            SpanKind::Consumer => 5,
                        },
                        "startTimeUnixNano": span.start_time_ns.to_string(),
                        "endTimeUnixNano": span.end_time_ns.unwrap_or(span.start_time_ns).to_string(),
                        "status": {
                            "code": match span.status {
                                SpanStatus::Unset => 0,
                                SpanStatus::Ok => 1,
                                SpanStatus::Error => 2,
                            }
                        },
                        "attributes": span.attributes.iter().map(|(k, v)| {
                            serde_json::json!({
                                "key": k,
                                "value": match v {
                                    AttributeValue::String(s) => serde_json::json!({ "stringValue": s }),
                                    AttributeValue::Int(i) => serde_json::json!({ "intValue": i.to_string() }),
                                    AttributeValue::Float(f) => serde_json::json!({ "doubleValue": f }),
                                    AttributeValue::Bool(b) => serde_json::json!({ "boolValue": b }),
                                    _ => serde_json::json!({ "stringValue": "unsupported" }),
                                }
                            })
                        }).collect::<Vec<_>>()
                    });

                    if let Some(ref parent) = span.parent_span_id {
                        span_json["parentSpanId"] = serde_json::json!(parent);
                    }

                    if let Some(ref msg) = span.status_message {
                        span_json["status"]["message"] = serde_json::json!(msg);
                    }

                    span_json
                }).collect::<Vec<_>>()
            }]
        }]
    });

    resource_spans
}
