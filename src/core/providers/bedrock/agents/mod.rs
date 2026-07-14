//! Bedrock Agents Module
//!
//! Handles agent invocation, session management, and tool calling

use crate::core::providers::unified_provider::ProviderError;
use base64::Engine as _;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::streaming::{BedrockStream, EventStreamMessage};

/// Agent invocation request
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInvocationRequest {
    #[serde(skip_serializing)]
    pub agent_id: String,
    #[serde(skip_serializing)]
    pub agent_alias_id: String,
    #[serde(skip_serializing)]
    pub session_id: String,
    pub input_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_state: Option<SessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_trace: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_session: Option<bool>,
}

/// Session state
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub session_attributes: Option<Value>,
    pub prompt_session_attributes: Option<Value>,
}

/// Agent invocation response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInvocationResponse {
    pub completion: AgentCompletion,
    pub session_id: String,
    pub session_state: Option<SessionState>,
    pub trace: Option<AgentTrace>,
}

/// Agent completion
#[derive(Debug, Deserialize)]
pub struct AgentCompletion {
    pub text: String,
}

/// Complete InvokeAgent event-stream result.
#[derive(Debug)]
pub struct AgentInvocationResult {
    pub completion: AgentCompletion,
    pub session_id: String,
    pub memory_id: Option<String>,
    pub session_state: Option<SessionState>,
    pub trace: Option<AgentTrace>,
    pub attributions: Vec<Value>,
    pub return_control: Option<Value>,
    pub files: Vec<Value>,
}

impl AgentInvocationResult {
    fn into_legacy_response(self) -> Result<AgentInvocationResponse, ProviderError> {
        if self.completion.text.is_empty()
            && (self.return_control.is_some() || !self.files.is_empty())
        {
            return Err(ProviderError::response_parsing(
                "bedrock",
                "agent response has no text completion; use AgentClient::invoke_detailed",
            ));
        }
        Ok(AgentInvocationResponse {
            completion: self.completion,
            session_id: self.session_id,
            session_state: self.session_state,
            trace: self.trace,
        })
    }
}

/// Agent trace
#[derive(Debug, Deserialize)]
pub struct AgentTrace {
    pub traces: Vec<TraceEntry>,
}

/// Trace entry
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceEntry {
    pub trace_id: String,
    pub trace_type: String,
    pub trace_data: Value,
}

#[derive(Default)]
struct AgentResponseAccumulator {
    buffer: Vec<u8>,
    text_bytes: Vec<u8>,
    attributions: Vec<Value>,
    traces: Vec<TraceEntry>,
    return_control: Option<Value>,
    files: Vec<Value>,
    seen_event: bool,
}

impl AgentResponseAccumulator {
    fn trace_id(value: &Value) -> Option<&str> {
        value
            .get("traceId")
            .and_then(Value::as_str)
            .or_else(|| match value {
                Value::Object(values) => values.values().find_map(Self::trace_id),
                Value::Array(values) => values.iter().find_map(Self::trace_id),
                _ => None,
            })
    }

    fn push_bytes(&mut self, chunk: &[u8]) -> Result<(), ProviderError> {
        self.buffer.extend_from_slice(chunk);
        while let Some(message) = BedrockStream::take_event_message(&mut self.buffer) {
            self.consume(message?)?;
        }
        Ok(())
    }

    fn consume(&mut self, message: EventStreamMessage) -> Result<(), ProviderError> {
        BedrockStream::check_stream_error(&message)?;
        let event_type = BedrockStream::header_value(&message, ":event-type").ok_or_else(|| {
            ProviderError::response_parsing("bedrock", "agent event type missing")
        })?;
        let payload = serde_json::from_slice::<Value>(&message.payload)
            .map_err(|error| ProviderError::response_parsing("bedrock", error.to_string()))?;
        let event = &payload;
        self.seen_event = true;

        match event_type {
            "chunk" => {
                let encoded = event.get("bytes").and_then(Value::as_str).ok_or_else(|| {
                    ProviderError::response_parsing("bedrock", "agent chunk bytes missing")
                })?;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(encoded)
                    .map_err(|error| {
                        ProviderError::response_parsing("bedrock", error.to_string())
                    })?;
                self.text_bytes.extend_from_slice(&bytes);
                if let Some(attribution) = event.get("attribution") {
                    self.attributions.push(attribution.clone());
                }
            }
            "trace" => {
                let trace_type = event
                    .get("trace")
                    .unwrap_or(event)
                    .as_object()
                    .and_then(|trace| trace.keys().next())
                    .map_or_else(|| "trace".to_string(), Clone::clone);
                let trace_id = Self::trace_id(event).unwrap_or_default().to_string();
                self.traces.push(TraceEntry {
                    trace_id,
                    trace_type,
                    trace_data: event.clone(),
                });
            }
            "returnControl" => {
                if self.return_control.replace(event.clone()).is_some() {
                    return Err(ProviderError::response_parsing(
                        "bedrock",
                        "duplicate agent returnControl event",
                    ));
                }
            }
            "files" => {
                let files = event
                    .as_array()
                    .or_else(|| event.get("files").and_then(Value::as_array))
                    .ok_or_else(|| {
                        ProviderError::response_parsing("bedrock", "agent files payload missing")
                    })?;
                self.files.extend(files.iter().cloned());
            }
            other => {
                return Err(ProviderError::response_parsing(
                    "bedrock",
                    format!("unsupported agent event type '{other}'"),
                ));
            }
        }
        Ok(())
    }

    fn finish(
        self,
        session_id: String,
        memory_id: Option<String>,
    ) -> Result<AgentInvocationResult, ProviderError> {
        if !self.buffer.is_empty() {
            return Err(ProviderError::response_parsing(
                "bedrock",
                "incomplete agent event stream frame",
            ));
        }
        if !self.seen_event
            || (self.text_bytes.is_empty()
                && self.return_control.is_none()
                && self.files.is_empty())
        {
            return Err(ProviderError::response_parsing(
                "bedrock",
                "agent response contained no completion or returnControl event",
            ));
        }
        let text = String::from_utf8(self.text_bytes)
            .map_err(|error| ProviderError::response_parsing("bedrock", error.to_string()))?;
        Ok(AgentInvocationResult {
            completion: AgentCompletion { text },
            session_id,
            memory_id,
            session_state: None,
            trace: (!self.traces.is_empty()).then_some(AgentTrace {
                traces: self.traces,
            }),
            attributions: self.attributions,
            return_control: self.return_control,
            files: self.files,
        })
    }
}

/// Agent client for managing agent interactions
pub struct AgentClient<'a> {
    client: &'a crate::core::providers::bedrock::client::BedrockClient,
}

impl<'a> AgentClient<'a> {
    /// Create a new agent client
    pub fn new(client: &'a crate::core::providers::bedrock::client::BedrockClient) -> Self {
        Self { client }
    }

    /// Invoke an agent using the legacy response shape.
    ///
    /// Successful memory, attribution, return-control, and file metadata is omitted when text is
    /// available. Use [`Self::invoke_detailed`] to preserve those fields.
    pub async fn invoke(
        &self,
        agent_id: &str,
        agent_alias_id: &str,
        session_id: &str,
        input_text: &str,
        enable_trace: bool,
    ) -> Result<AgentInvocationResponse, ProviderError> {
        self.invoke_detailed(
            agent_id,
            agent_alias_id,
            session_id,
            input_text,
            enable_trace,
        )
        .await?
        .into_legacy_response()
    }

    /// Invoke an agent and preserve every successful event-stream result field.
    pub async fn invoke_detailed(
        &self,
        agent_id: &str,
        agent_alias_id: &str,
        session_id: &str,
        input_text: &str,
        enable_trace: bool,
    ) -> Result<AgentInvocationResult, ProviderError> {
        let request = AgentInvocationRequest {
            agent_id: agent_id.to_string(),
            agent_alias_id: agent_alias_id.to_string(),
            session_id: session_id.to_string(),
            input_text: input_text.to_string(),
            session_state: None,
            enable_trace: Some(enable_trace),
            end_session: None,
        };

        let url = format!(
            "agents/{}/agentAliases/{}/sessions/{}/text",
            agent_id, agent_alias_id, session_id
        );

        let response = self
            .client
            .send_request("", &url, &serde_json::to_value(request)?)
            .await?;
        let response_session_id = match response.headers().get("x-amz-bedrock-agent-session-id") {
            Some(value) => value
                .to_str()
                .map(str::to_string)
                .map_err(|error| ProviderError::response_parsing("bedrock", error.to_string()))?,
            None => session_id.to_string(),
        };
        let memory_id = response
            .headers()
            .get("x-amz-bedrock-agent-memory-id")
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_string)
                    .map_err(|error| ProviderError::response_parsing("bedrock", error.to_string()))
            })
            .transpose()?;
        let mut stream = response.bytes_stream();
        let mut accumulator = AgentResponseAccumulator::default();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|error| ProviderError::network("bedrock", error.to_string()))?;
            accumulator.push_bytes(&chunk)?;
        }
        accumulator.finish(response_session_id, memory_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation_request() -> AgentInvocationRequest {
        AgentInvocationRequest {
            agent_id: "agent-1".to_string(),
            agent_alias_id: "alias-1".to_string(),
            session_id: "session-1".to_string(),
            input_text: "hello".to_string(),
            session_state: None,
            enable_trace: Some(true),
            end_session: None,
        }
    }

    fn event_message(event_type: &str, payload: Value) -> EventStreamMessage {
        EventStreamMessage {
            headers: vec![super::super::streaming::EventStreamHeader {
                name: ":event-type".to_string(),
                value: super::super::streaming::HeaderValue::String(event_type.to_string()),
            }],
            payload: bytes::Bytes::from(
                serde_json::to_vec(&payload)
                    .unwrap_or_else(|error| panic!("event should serialize: {error}")),
            ),
        }
    }

    fn exception_message(exception_type: &str, payload: Value) -> EventStreamMessage {
        EventStreamMessage {
            headers: vec![
                super::super::streaming::EventStreamHeader {
                    name: ":message-type".to_string(),
                    value: super::super::streaming::HeaderValue::String("exception".to_string()),
                },
                super::super::streaming::EventStreamHeader {
                    name: ":exception-type".to_string(),
                    value: super::super::streaming::HeaderValue::String(exception_type.to_string()),
                },
            ],
            payload: bytes::Bytes::from(
                serde_json::to_vec(&payload)
                    .unwrap_or_else(|error| panic!("exception should serialize: {error}")),
            ),
        }
    }

    fn finish(value: AgentResponseAccumulator) -> Result<AgentInvocationResult, ProviderError> {
        value.finish("session-1".to_string(), None)
    }

    #[test]
    fn invocation_body_excludes_path_parameters() {
        let body = serde_json::to_value(invocation_request())
            .unwrap_or_else(|error| panic!("request should serialize: {error}"));

        for path_parameter in ["agentId", "agentAliasId", "sessionId"] {
            assert!(
                body.get(path_parameter).is_none(),
                "unexpected {path_parameter}"
            );
        }
        assert_eq!(body.get("inputText"), Some(&serde_json::json!("hello")));
        assert_eq!(body.get("enableTrace"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn legacy_response_struct_literals_remain_constructible() {
        let response = AgentInvocationResponse {
            completion: AgentCompletion {
                text: "hello".into(),
            },
            session_id: "session-1".to_string(),
            session_state: None,
            trace: None,
        };

        assert_eq!(response.completion.text, "hello");
    }

    #[test]
    fn legacy_conversion_keeps_successful_text_when_extended_metadata_is_present() {
        let result = AgentInvocationResult {
            completion: AgentCompletion {
                text: "hello".to_string(),
            },
            session_id: "session-1".to_string(),
            memory_id: Some("memory-1".to_string()),
            session_state: None,
            trace: None,
            attributions: vec![serde_json::json!({"citations": []})],
            return_control: None,
            files: Vec::new(),
        };

        let response = result
            .into_legacy_response()
            .unwrap_or_else(|error| panic!("successful legacy response must not fail: {error}"));
        assert_eq!(response.completion.text, "hello");
        assert_eq!(response.session_id, "session-1");
    }

    #[test]
    fn legacy_conversion_rejects_control_only_successes() {
        for (return_control, files) in [
            (
                Some(serde_json::json!({"invocationId": "invoke-1"})),
                Vec::new(),
            ),
            (None, vec![serde_json::json!({"name": "report.csv"})]),
        ] {
            let result = AgentInvocationResult {
                completion: AgentCompletion {
                    text: String::new(),
                },
                session_id: "session-1".to_string(),
                memory_id: None,
                session_state: None,
                trace: None,
                attributions: Vec::new(),
                return_control,
                files,
            };

            let error = result
                .into_legacy_response()
                .expect_err("control-only response cannot be represented by legacy API");
            assert!(error.to_string().contains("invoke_detailed"), "{error}");
        }
    }

    #[test]
    fn agent_error_events_keep_provider_error_categories() {
        let cases = [
            (
                "throttlingException",
                ProviderError::rate_limit("bedrock", None),
            ),
            (
                "validationException",
                ProviderError::invalid_request("bedrock", "invalid input"),
            ),
            (
                "accessDeniedException",
                ProviderError::api_error("bedrock", 403, "denied"),
            ),
        ];

        for (event_type, expected) in cases {
            let error = AgentResponseAccumulator::default()
                .consume(exception_message(
                    event_type,
                    serde_json::json!({"message": "request failed"}),
                ))
                .expect_err("agent error event must fail");
            assert_eq!(
                std::mem::discriminant(&error),
                std::mem::discriminant(&expected),
                "unexpected category for {event_type}: {error}"
            );
        }
    }

    #[test]
    fn event_chunk_is_not_a_top_level_completion_response() {
        let mut accumulator = AgentResponseAccumulator::default();
        accumulator
            .consume(event_message(
                "chunk",
                serde_json::json!({
                    "bytes": "aGVsbG8=",
                    "attribution": {"citations": []}
                }),
            ))
            .unwrap_or_else(|error| panic!("event chunk should parse: {error}"));
        let response =
            finish(accumulator).unwrap_or_else(|error| panic!("response should finish: {error}"));

        assert_eq!(response.completion.text, "hello");
        assert_eq!(response.attributions.len(), 1);
    }

    #[test]
    fn trace_and_return_control_events_are_preserved() {
        let mut accumulator = AgentResponseAccumulator::default();
        accumulator
            .consume(event_message(
                "trace",
                serde_json::json!({
                    "agentId": "agent-1",
                    "trace": {"orchestrationTrace": {"rationale": {"traceId": "trace-1"}}}
                }),
            ))
            .unwrap_or_else(|error| panic!("trace should parse: {error}"));
        accumulator
            .consume(event_message(
                "returnControl",
                serde_json::json!({"invocationId": "invoke-1", "invocationInputs": []}),
            ))
            .unwrap_or_else(|error| panic!("returnControl should parse: {error}"));

        let response =
            finish(accumulator).unwrap_or_else(|error| panic!("response should finish: {error}"));
        let trace = response
            .trace
            .as_ref()
            .and_then(|trace| trace.traces.first())
            .unwrap_or_else(|| panic!("trace event should be preserved"));
        assert_eq!(trace.trace_id, "trace-1");
        assert_eq!(
            trace.trace_data.get("agentId"),
            Some(&serde_json::json!("agent-1"))
        );
        assert_eq!(
            response
                .return_control
                .as_ref()
                .and_then(|value| value.get("invocationId")),
            Some(&serde_json::json!("invoke-1"))
        );
    }

    #[test]
    fn generated_files_are_preserved_as_success_events() {
        let mut accumulator = AgentResponseAccumulator::default();
        accumulator
            .consume(event_message(
                "files",
                serde_json::json!({
                    "files": [{"name": "report.csv", "type": "text/csv", "bytes": "YQ=="}]
                }),
            ))
            .unwrap_or_else(|error| panic!("files event should parse: {error}"));

        let response = finish(accumulator)
            .unwrap_or_else(|error| panic!("files response should finish: {error}"));
        assert_eq!(response.files.len(), 1);
    }

    fn event_frame(event_type: &str, payload: Value) -> Vec<u8> {
        let payload = serde_json::to_vec(&payload)
            .unwrap_or_else(|error| panic!("event should serialize: {error}"));
        let mut headers = Vec::new();
        headers.push(":event-type".len() as u8);
        headers.extend_from_slice(b":event-type");
        headers.push(7);
        headers.extend_from_slice(&(event_type.len() as u16).to_be_bytes());
        headers.extend_from_slice(event_type.as_bytes());
        let total_length = 16 + headers.len() + payload.len();
        let mut frame = Vec::new();
        frame.extend_from_slice(&(total_length as u32).to_be_bytes());
        frame.extend_from_slice(&(headers.len() as u32).to_be_bytes());
        frame.extend_from_slice(&crc32fast::hash(&frame).to_be_bytes());
        frame.extend_from_slice(&headers);
        frame.extend_from_slice(&payload);
        frame.extend_from_slice(&crc32fast::hash(&frame).to_be_bytes());
        frame
    }

    #[test]
    fn split_frames_are_buffered_and_completion_chunks_are_concatenated() {
        let mut frames = event_frame("chunk", serde_json::json!({"bytes": "aGVs"}));
        frames.extend(event_frame("chunk", serde_json::json!({"bytes": "bG8="})));
        frames.extend(event_frame("chunk", serde_json::json!({"bytes": "8J8="})));
        frames.extend(event_frame("chunk", serde_json::json!({"bytes": "mIA="})));
        let split = frames.len() / 2;
        let mut accumulator = AgentResponseAccumulator::default();
        accumulator
            .push_bytes(&frames[..split])
            .unwrap_or_else(|error| panic!("first network chunk should buffer: {error}"));
        accumulator
            .push_bytes(&frames[split..])
            .unwrap_or_else(|error| panic!("second network chunk should parse: {error}"));

        let response =
            finish(accumulator).unwrap_or_else(|error| panic!("response should finish: {error}"));
        assert_eq!(response.completion.text, "hello😀");
    }

    #[test]
    fn incomplete_event_frame_fails_closed() {
        let frame = event_frame("chunk", serde_json::json!({"bytes": "aGVsbG8="}));
        let mut accumulator = AgentResponseAccumulator::default();
        accumulator
            .push_bytes(&frame[..frame.len() - 1])
            .unwrap_or_else(|error| panic!("partial frame should buffer: {error}"));

        let error = finish(accumulator).expect_err("partial frame must fail");
        assert!(error.to_string().contains("incomplete"), "{error}");
    }

    #[test]
    fn malformed_or_unknown_agent_events_fail_closed() {
        let invalid_json = EventStreamMessage {
            headers: event_message("chunk", serde_json::json!({})).headers,
            payload: bytes::Bytes::from_static(b"not-json"),
        };
        let missing_type = EventStreamMessage {
            headers: Vec::new(),
            payload: bytes::Bytes::from_static(b"{}"),
        };
        let cases = [
            (missing_type, "type missing"),
            (invalid_json, "expected ident"),
            (
                event_message("chunk", serde_json::json!({})),
                "bytes missing",
            ),
            (
                event_message("chunk", serde_json::json!({"bytes": "%%%"})),
                "Invalid symbol",
            ),
            (
                event_message("futureEvent", serde_json::json!({})),
                "unsupported",
            ),
        ];

        for (message, expected) in cases {
            let error = AgentResponseAccumulator::default()
                .consume(message)
                .expect_err("malformed event must fail");
            assert!(error.to_string().contains(expected), "{error}");
        }
        let mut invalid_utf8 = AgentResponseAccumulator::default();
        invalid_utf8
            .consume(event_message("chunk", serde_json::json!({"bytes": "/w=="})))
            .unwrap_or_else(|error| panic!("raw bytes should buffer: {error}"));
        let error = finish(invalid_utf8).expect_err("invalid combined UTF-8 must fail");
        assert!(error.to_string().contains("invalid utf-8"), "{error}");
    }

    #[test]
    fn empty_stream_and_duplicate_return_control_fail_closed() {
        let empty =
            finish(AgentResponseAccumulator::default()).expect_err("empty stream must fail");
        assert!(empty.to_string().contains("no completion"), "{empty}");

        let mut accumulator = AgentResponseAccumulator::default();
        let control = serde_json::json!({"invocationId": "invoke-1"});
        accumulator
            .consume(event_message("returnControl", control.clone()))
            .unwrap_or_else(|error| panic!("first returnControl should parse: {error}"));
        let duplicate = accumulator
            .consume(event_message("returnControl", control))
            .expect_err("duplicate returnControl must fail");
        assert!(duplicate.to_string().contains("duplicate"), "{duplicate}");
    }
}
