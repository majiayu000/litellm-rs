//! Codex-specific types, separated by wire and domain responsibility.
pub mod wire {
    use crate::core::models::openai::responses_api::{ResponseInputItem, ResponseTool};
    use serde::de::{DeserializeOwned, Error as DeError};
    use serde::ser::Error as SerError;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde_json::{Map, Value};
    /// Codex protocol revision used by the GH-1107 compatibility fixtures.
    pub const CODEX_PROTOCOL_BASELINE: &str = "6e5a2d6b8d148a5554fdceb6f399ca45bd1c78d9";
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexInternalChatMessageMetadataPassthrough {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub turn_id: Option<String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexFunctionCall {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        pub call_id: String,
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub namespace: Option<String>,
        pub arguments: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub internal_chat_message_metadata_passthrough:
            Option<CodexInternalChatMessageMetadataPassthrough>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexFunctionCallOutput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        pub call_id: String,
        pub output: CodexToolOutput,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub internal_chat_message_metadata_passthrough:
            Option<CodexInternalChatMessageMetadataPassthrough>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexCustomToolCall {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        pub call_id: String,
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub namespace: Option<String>,
        pub input: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub internal_chat_message_metadata_passthrough:
            Option<CodexInternalChatMessageMetadataPassthrough>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexCustomToolCallOutput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        pub call_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        pub output: CodexToolOutput,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub internal_chat_message_metadata_passthrough:
            Option<CodexInternalChatMessageMetadataPassthrough>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum CodexToolOutput {
        Text(String),
        ContentItems(Vec<CodexToolOutputContent>),
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    pub enum CodexToolOutputContent {
        InputText {
            text: String,
        },
        InputImage {
            image_url: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            detail: Option<String>,
        },
        InputAudio {
            audio_url: String,
        },
        EncryptedContent {
            encrypted_content: String,
        },
    }
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct CodexCustomTool {
        pub name: String,
        pub description: String,
        pub format: Value,
    }
    /// A fail-closed wire item that retains only approved diagnostic metadata.
    #[derive(Debug, Clone)]
    pub struct CodexUnsupportedWire {
        pub wire_type: String,
        metadata: Map<String, Value>,
    }
    impl CodexUnsupportedWire {
        fn new(wire_type: String, payload: Map<String, Value>) -> Self {
            const ALLOWLIST: [&str; 5] = ["id", "call_id", "name", "namespace", "status"];
            let metadata = payload
                .into_iter()
                .filter(|(key, _)| ALLOWLIST.contains(&key.as_str()))
                .collect();
            Self {
                wire_type,
                metadata,
            }
        }
        fn tagged_payload(&self) -> Value {
            let mut payload = self.metadata.clone();
            payload.insert("type".into(), Value::String(self.wire_type.clone()));
            Value::Object(payload)
        }
    }
    impl ResponseInputItem {
        pub fn feature_name(&self) -> &str {
            match self {
                Self::Message(_) => "message",
                Self::FunctionCall(_) => "function_call",
                Self::FunctionCallOutput(_) => "function_call_output",
                Self::CustomToolCall(_) => "custom_tool_call",
                Self::CustomToolCallOutput(_) => "custom_tool_call_output",
                Self::Unsupported(value) | Self::Unknown(value) => &value.wire_type,
            }
        }
    }
    impl Serialize for ResponseInputItem {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let value = match self {
                Self::Message(value) => tag("message", value),
                Self::FunctionCall(value) => tag("function_call", value),
                Self::FunctionCallOutput(value) => tag("function_call_output", value),
                Self::CustomToolCall(value) => tag("custom_tool_call", value),
                Self::CustomToolCallOutput(value) => tag("custom_tool_call_output", value),
                Self::Unsupported(value) | Self::Unknown(value) => Ok(value.tagged_payload()),
            }
            .map_err(S::Error::custom)?;
            value.serialize(serializer)
        }
    }
    impl<'de> Deserialize<'de> for ResponseInputItem {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let (wire_type, payload) = deserialize_tagged(deserializer)?;
            let decoded = match wire_type.as_str() {
                "message" => decode(payload).map(Self::Message),
                "function_call" => decode(payload).map(Self::FunctionCall),
                "function_call_output" => decode(payload).map(Self::FunctionCallOutput),
                "custom_tool_call" => decode(payload).map(Self::CustomToolCall),
                "custom_tool_call_output" => decode(payload).map(Self::CustomToolCallOutput),
                item if is_known_unsupported_item(item) => Ok(Self::Unsupported(
                    CodexUnsupportedWire::new(wire_type, payload),
                )),
                _ => Ok(Self::Unknown(CodexUnsupportedWire::new(wire_type, payload))),
            };
            decoded.map_err(D::Error::custom)
        }
    }
    impl Serialize for ResponseTool {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let value = match self {
                Self::WebSearch(value) => tag("web_search", value),
                Self::WebSearchPreview(value) => tag("web_search_preview", value),
                Self::FileSearch(value) => tag("file_search", value),
                Self::CodeInterpreter(value) => tag("code_interpreter", value),
                Self::ComputerUsePreview(value) => tag("computer_use_preview", value),
                Self::Mcp(value) => tag("mcp", value),
                Self::Function(value) => tag("function", value),
                Self::CodexFunction(value) => tag("function", value),
                Self::Custom(value) => tag("custom", value),
                Self::Unsupported(value) | Self::Unknown(value) => Ok(value.tagged_payload()),
            }
            .map_err(S::Error::custom)?;
            value.serialize(serializer)
        }
    }
    impl<'de> Deserialize<'de> for ResponseTool {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let (wire_type, payload) = deserialize_tagged(deserializer)?;
            let decoded = match wire_type.as_str() {
                "web_search" => decode(payload).map(Self::WebSearch),
                "web_search_preview" => decode(payload).map(Self::WebSearchPreview),
                "file_search" => decode(payload).map(Self::FileSearch),
                "code_interpreter" => decode(payload).map(Self::CodeInterpreter),
                "computer_use_preview" => decode(payload).map(Self::ComputerUsePreview),
                "mcp" => decode(payload).map(Self::Mcp),
                "function" if payload.contains_key("function") => {
                    decode(payload).map(Self::Function)
                }
                "function" => decode(payload).map(Self::CodexFunction),
                "custom" => decode(payload).map(Self::Custom),
                "image_generation" | "namespace" | "tool_search" => Ok(Self::Unsupported(
                    CodexUnsupportedWire::new(wire_type, payload),
                )),
                _ => Ok(Self::Unknown(CodexUnsupportedWire::new(wire_type, payload))),
            };
            decoded.map_err(D::Error::custom)
        }
    }
    fn is_known_unsupported_item(item: &str) -> bool {
        matches!(
            item,
            "additional_tools"
                | "local_shell_call"
                | "mcp_tool_call_output"
                | "tool_search_call"
                | "tool_search_output"
                | "web_search_call"
                | "image_generation_call"
                | "compaction"
                | "compaction_trigger"
                | "context_compaction"
        )
    }
    fn tag<T: Serialize>(wire_type: &str, value: &T) -> Result<Value, serde_json::Error> {
        let Value::Object(mut payload) = serde_json::to_value(value)? else {
            unreachable!("Codex wire payload structs serialize as objects");
        };
        payload.insert("type".into(), Value::String(wire_type.into()));
        Ok(Value::Object(payload))
    }
    fn deserialize_tagged<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<(String, Map<String, Value>), D::Error> {
        let Value::Object(mut payload) = Value::deserialize(deserializer)? else {
            return Err(D::Error::custom("Codex item must be an object"));
        };
        let wire_type = payload
            .remove("type")
            .and_then(|value| value.as_str().map(str::to_owned))
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| D::Error::custom("Codex item type must be a non-empty string"))?;
        Ok((wire_type, payload))
    }

    fn decode<T: DeserializeOwned>(payload: Map<String, Value>) -> Result<T, serde_json::Error> {
        serde_json::from_value(Value::Object(payload))
    }
}

/// SP1107-T2 builds the canonical model before SP1107-T3 wires it into routing.
#[allow(dead_code)]
pub(crate) mod domain {
    use super::wire::{CODEX_PROTOCOL_BASELINE, CodexToolOutput, CodexToolOutputContent};
    use crate::core::models::openai::responses_api::{
        ResponseInput, ResponseInputContent, ResponseInputContentPart, ResponseInputItem,
        ResponseInputMessage, ResponseTool, ResponsesApiRequest,
    };
    use std::collections::HashMap;
    use thiserror::Error;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum CodexCallKind {
        Function,
        Custom,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum CodexCallState {
        Declared,
        OutputReceived,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct CodexCallRecord {
        pub(crate) call_id: String,
        pub(crate) kind: CodexCallKind,
        pub(crate) name: String,
        pub(crate) namespace: Option<String>,
        pub(crate) state: CodexCallState,
        pub(crate) declaration_index: usize,
        pub(crate) output_index: Option<usize>,
    }

    #[derive(Debug, Clone, Default)]
    pub(crate) struct CodexCallLedger {
        pub(crate) calls: Vec<CodexCallRecord>,
        call_indices: HashMap<String, usize>,
    }

    impl CodexCallLedger {
        fn declare(
            &mut self,
            call_id: &str,
            kind: CodexCallKind,
            name: &str,
            namespace: Option<&str>,
            item_index: usize,
        ) -> Result<(), CodexTurnError> {
            validate_call_id(call_id, item_index)?;
            if name.trim().is_empty() {
                return Err(CodexTurnError::InvalidCallName { item_index });
            }
            if self.call_indices.contains_key(call_id) {
                return Err(CodexTurnError::DuplicateCallId(item_index));
            }

            let call_index = self.calls.len();
            self.calls.push(CodexCallRecord {
                call_id: call_id.to_string(),
                kind,
                name: name.to_string(),
                namespace: namespace.map(str::to_string),
                state: CodexCallState::Declared,
                declaration_index: item_index,
                output_index: None,
            });
            self.call_indices.insert(call_id.to_string(), call_index);
            Ok(())
        }

        fn receive_output(
            &mut self,
            call_id: &str,
            kind: CodexCallKind,
            name: Option<&str>,
            item_index: usize,
        ) -> Result<(), CodexTurnError> {
            validate_call_id(call_id, item_index)?;
            let Some(&call_index) = self.call_indices.get(call_id) else {
                return Err(CodexTurnError::UnknownCallId(item_index));
            };
            let call = &mut self.calls[call_index];
            if call.state == CodexCallState::OutputReceived {
                return Err(CodexTurnError::DuplicateCallOutput(item_index));
            }
            if call.kind != kind {
                return Err(CodexTurnError::CallKindMismatch(
                    call.kind, kind, item_index,
                ));
            }
            if name.is_some_and(|name| name.trim().is_empty() || name != call.name) {
                return Err(CodexTurnError::InvalidCallName { item_index });
            }
            call.state = CodexCallState::OutputReceived;
            call.output_index = Some(item_index);
            Ok(())
        }
    }

    #[derive(Debug, Clone)]
    pub(crate) enum CodexTurnItem {
        Text(String),
        Item(ResponseInputItem),
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct CodexExecutionRequirements {
        pub(crate) streaming: bool,
        pub(crate) function_tools: bool,
        pub(crate) custom_tools: bool,
        pub(crate) call_outputs: bool,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct CodexTurn {
        pub(crate) protocol_version: &'static str,
        pub(crate) items: Vec<CodexTurnItem>,
        pub(crate) tools: Vec<ResponseTool>,
        pub(crate) store: Option<bool>,
        pub(crate) background: bool,
        pub(crate) requirements: CodexExecutionRequirements,
        pub(crate) ledger: CodexCallLedger,
    }

    impl TryFrom<&ResponsesApiRequest> for CodexTurn {
        type Error = CodexTurnError;

        fn try_from(request: &ResponsesApiRequest) -> Result<Self, Self::Error> {
            if request.additional_tools.is_some() {
                return Err(unsupported("additional_tools"));
            }

            let mut ledger = CodexCallLedger::default();
            let mut requirements = CodexExecutionRequirements {
                streaming: request.stream.unwrap_or(false),
                ..Default::default()
            };
            let items = match &request.input {
                ResponseInput::Text(text) => {
                    if text.trim().is_empty() {
                        return Err(CodexTurnError::EmptyInput);
                    }
                    vec![CodexTurnItem::Text(text.clone())]
                }
                ResponseInput::Items(items) => {
                    if items.is_empty() {
                        return Err(CodexTurnError::EmptyInput);
                    }
                    let mut turn_items = Vec::with_capacity(items.len());
                    for (item_index, item) in items.iter().enumerate() {
                        turn_items.push(normalize_item(
                            item,
                            item_index,
                            &mut ledger,
                            &mut requirements,
                        )?);
                    }
                    turn_items
                }
            };

            let tools = request.tools.clone().unwrap_or_default();
            for (tool_index, tool) in tools.iter().enumerate() {
                let (name, deferred, custom) = match tool {
                    ResponseTool::Function(tool) => {
                        (&tool.function.name, tool.function.defer_loading, false)
                    }
                    ResponseTool::CodexFunction(tool) => (&tool.name, tool.defer_loading, false),
                    ResponseTool::Custom(tool) => (&tool.name, None, true),
                    tool => return Err(unsupported(tool.feature_name())),
                };
                if name.trim().is_empty() {
                    return Err(CodexTurnError::EmptyToolName { tool_index });
                }
                if deferred == Some(true) {
                    return Err(unsupported("defer_loading"));
                }
                requirements.custom_tools |= custom;
                requirements.function_tools |= !custom;
            }

            Ok(Self {
                protocol_version: CODEX_PROTOCOL_BASELINE,
                items,
                tools,
                store: request.store,
                background: request.background.unwrap_or(false),
                requirements,
                ledger,
            })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Error)]
    pub(crate) enum CodexTurnError {
        #[error("Codex input must not be empty")]
        EmptyInput,
        #[error("Codex call_id must not be empty at item {item_index}")]
        EmptyCallId { item_index: usize },
        #[error("invalid Codex call name at item {item_index}")]
        InvalidCallName { item_index: usize },
        #[error("Codex tool name must not be empty at tool {tool_index}")]
        EmptyToolName { tool_index: usize },
        #[error("invalid Codex message role at item {item_index}")]
        InvalidMessageRole { item_index: usize },
        #[error("Codex call payload must not be empty at item {item_index}")]
        EmptyCallPayload { item_index: usize },
        #[error("Codex function arguments must be valid JSON at item {item_index}")]
        InvalidFunctionArguments { item_index: usize },
        #[error("duplicate Codex call_id at item {0}")]
        DuplicateCallId(usize),
        #[error("unknown Codex call_id at item {0}")]
        UnknownCallId(usize),
        #[error("duplicate Codex call output at item {0}")]
        DuplicateCallOutput(usize),
        #[error("Codex call kind mismatch at item {2}: expected {0:?}, got {1:?}")]
        CallKindMismatch(CodexCallKind, CodexCallKind, usize),
        #[error("unsupported Codex feature '{0}'")]
        UnsupportedFeature(String),
    }

    fn normalize_item(
        item: &ResponseInputItem,
        item_index: usize,
        ledger: &mut CodexCallLedger,
        requirements: &mut CodexExecutionRequirements,
    ) -> Result<CodexTurnItem, CodexTurnError> {
        use CodexCallKind::{Custom, Function};
        match item {
            ResponseInputItem::Message(message) => validate_message(message, item_index)?,
            ResponseInputItem::FunctionCall(call) => {
                if serde_json::from_str::<serde_json::Value>(&call.arguments).is_err() {
                    return Err(CodexTurnError::InvalidFunctionArguments { item_index });
                }
                ledger.declare(
                    &call.call_id,
                    Function,
                    &call.name,
                    call.namespace.as_deref(),
                    item_index,
                )?;
                requirements.function_tools = true;
            }
            ResponseInputItem::FunctionCallOutput(output) => {
                validate_tool_output(&output.output, item_index)?;
                ledger.receive_output(&output.call_id, Function, None, item_index)?;
                requirements.function_tools = true;
                requirements.call_outputs = true;
            }
            ResponseInputItem::CustomToolCall(call) => {
                if call.input.trim().is_empty() {
                    return Err(CodexTurnError::EmptyCallPayload { item_index });
                }
                ledger.declare(
                    &call.call_id,
                    Custom,
                    &call.name,
                    call.namespace.as_deref(),
                    item_index,
                )?;
                requirements.custom_tools = true;
            }
            ResponseInputItem::CustomToolCallOutput(output) => {
                validate_tool_output(&output.output, item_index)?;
                ledger.receive_output(
                    &output.call_id,
                    Custom,
                    output.name.as_deref(),
                    item_index,
                )?;
                requirements.custom_tools = true;
                requirements.call_outputs = true;
            }
            item => {
                return Err(unsupported(item.feature_name()));
            }
        }
        Ok(CodexTurnItem::Item(item.clone()))
    }

    fn validate_call_id(call_id: &str, item_index: usize) -> Result<(), CodexTurnError> {
        if call_id.trim().is_empty() {
            return Err(CodexTurnError::EmptyCallId { item_index });
        }
        Ok(())
    }

    fn unsupported(feature: &str) -> CodexTurnError {
        CodexTurnError::UnsupportedFeature(feature.into())
    }

    fn validate_message(
        message: &ResponseInputMessage,
        item_index: usize,
    ) -> Result<(), CodexTurnError> {
        if !matches!(
            message.role.as_str(),
            "user" | "assistant" | "system" | "developer"
        ) {
            return Err(CodexTurnError::InvalidMessageRole { item_index });
        }
        let empty = match &message.content {
            ResponseInputContent::Text(text) => text.trim().is_empty(),
            ResponseInputContent::Parts(parts) => {
                if parts
                    .iter()
                    .any(|part| matches!(part, ResponseInputContentPart::InputAudio { .. }))
                {
                    return Err(unsupported("input_audio"));
                }
                parts.is_empty()
                    || parts.iter().any(|part| match part {
                        ResponseInputContentPart::InputText { text }
                        | ResponseInputContentPart::OutputText { text } => text.trim().is_empty(),
                        ResponseInputContentPart::InputImage { image_url, .. } => image_url
                            .as_ref()
                            .is_none_or(|image_url| image_url.trim().is_empty()),
                        _ => false,
                    })
            }
        };
        if empty {
            return Err(CodexTurnError::EmptyCallPayload { item_index });
        }
        Ok(())
    }

    fn validate_tool_output(
        output: &CodexToolOutput,
        item_index: usize,
    ) -> Result<(), CodexTurnError> {
        let empty = match output {
            CodexToolOutput::Text(text) => text.trim().is_empty(),
            CodexToolOutput::ContentItems(items) => {
                items.is_empty()
                    || items.iter().any(|item| match item {
                        CodexToolOutputContent::InputText { text } => text.trim().is_empty(),
                        CodexToolOutputContent::InputImage { image_url, .. } => {
                            image_url.trim().is_empty()
                        }
                        CodexToolOutputContent::InputAudio { audio_url } => {
                            audio_url.trim().is_empty()
                        }
                        CodexToolOutputContent::EncryptedContent { encrypted_content } => {
                            encrypted_content.trim().is_empty()
                        }
                    })
            }
        };
        if empty {
            return Err(CodexTurnError::EmptyCallPayload { item_index });
        }
        Ok(())
    }
}
