//! Shared Gemini/Vertex function-call request and response helpers.
#![cfg_attr(
    not(any(feature = "providers-extended", feature = "providers-extra")),
    allow(dead_code)
)]

use std::collections::{HashMap, HashSet};

use serde_json::{Value, json};

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    content::ContentPart,
    message::{MessageContent, MessageRole},
    responses::FinishReason,
    tools::{FunctionCall, ToolCall, ToolChoice},
};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum GoogleToolPart {
    FunctionCall {
        id: String,
        name: String,
        args: Value,
    },
    FunctionResponse {
        name: String,
        response: Value,
    },
}

impl GoogleToolPart {
    pub(crate) fn to_wire_value(&self) -> Value {
        match self {
            Self::FunctionCall { id, name, args } => json!({
                "functionCall": {
                    "id": id,
                    "name": name,
                    "args": args
                }
            }),
            Self::FunctionResponse { name, response } => json!({
                "functionResponse": {
                    "name": name,
                    "response": response
                }
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct LedgerCall {
    name: String,
    consumed: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct GoogleToolPlanner {
    provider: &'static str,
    calls: HashMap<String, LedgerCall>,
}

impl GoogleToolPlanner {
    pub(crate) fn new(provider: &'static str) -> Self {
        Self {
            provider,
            calls: HashMap::new(),
        }
    }

    pub(crate) fn top_level_result(
        &mut self,
        message: &ChatMessage,
    ) -> Result<Option<GoogleToolPart>, ProviderError> {
        let Some(tool_call_id) = message.tool_call_id.as_deref() else {
            return Ok(None);
        };
        if !matches!(message.role, MessageRole::Tool | MessageRole::Function) {
            return Err(invalid(
                self.provider,
                "tool_call_id requires tool or function role",
            ));
        }
        if content_has_tool_part(&message.content) {
            return Err(invalid(
                self.provider,
                "tool result has ambiguous top-level and content-part representation",
            ));
        }
        let response = normalize_tool_role_result(self.provider, &message.content)?;
        self.consume_result(tool_call_id, response).map(Some)
    }

    pub(crate) fn top_level_calls(
        &mut self,
        calls: &[ToolCall],
    ) -> Result<Vec<GoogleToolPart>, ProviderError> {
        calls
            .iter()
            .map(|call| {
                let args = parse_arguments_object(
                    self.provider,
                    &call.function.arguments,
                    "tool_calls.function.arguments",
                )?;
                self.register_call(&call.id, &call.function.name, args)
            })
            .collect()
    }

    pub(crate) fn legacy_function_call(
        &mut self,
        message_index: usize,
        function_call: &FunctionCall,
    ) -> Result<GoogleToolPart, ProviderError> {
        let id = format!("call_m{message_index}_legacy_function");
        let args = parse_arguments_object(
            self.provider,
            &function_call.arguments,
            "function_call.arguments",
        )?;
        self.register_call(&id, &function_call.name, args)
    }

    pub(crate) fn content_tool_use(
        &mut self,
        id: &str,
        name: &str,
        input: &Value,
    ) -> Result<GoogleToolPart, ProviderError> {
        if !input.is_object() {
            return Err(invalid(
                self.provider,
                "tool_use.input must be a JSON object",
            ));
        }
        self.register_call(id, name, input.clone())
    }

    pub(crate) fn content_tool_result(
        &mut self,
        tool_use_id: &str,
        content: &Value,
        is_error: Option<bool>,
    ) -> Result<GoogleToolPart, ProviderError> {
        let response = normalize_tool_result_content(content, is_error);
        self.consume_result(tool_use_id, response)
    }

    fn register_call(
        &mut self,
        id: &str,
        name: &str,
        args: Value,
    ) -> Result<GoogleToolPart, ProviderError> {
        if id.trim().is_empty() {
            return Err(invalid(self.provider, "tool call id must be non-empty"));
        }
        if name.trim().is_empty() {
            return Err(invalid(self.provider, "tool name must be non-empty"));
        }
        if self.calls.contains_key(id) {
            return Err(invalid(self.provider, "duplicate tool call id"));
        }
        self.calls.insert(
            id.to_string(),
            LedgerCall {
                name: name.to_string(),
                consumed: false,
            },
        );
        Ok(GoogleToolPart::FunctionCall {
            id: id.to_string(),
            name: name.to_string(),
            args,
        })
    }

    fn consume_result(
        &mut self,
        tool_use_id: &str,
        response: Value,
    ) -> Result<GoogleToolPart, ProviderError> {
        if tool_use_id.trim().is_empty() {
            return Err(invalid(self.provider, "tool result id must be non-empty"));
        }
        let Some(call) = self.calls.get_mut(tool_use_id) else {
            return Err(invalid(
                self.provider,
                "tool result references unknown call id",
            ));
        };
        if call.consumed {
            return Err(invalid(
                self.provider,
                "tool result consumes call id more than once",
            ));
        }
        call.consumed = true;
        Ok(GoogleToolPart::FunctionResponse {
            name: call.name.clone(),
            response,
        })
    }
}

pub(crate) fn request_requires_tool_capability(request: &ChatRequest) -> bool {
    request.tools.is_some()
        || request.tool_choice.is_some()
        || request.parallel_tool_calls.is_some()
        || request.functions.is_some()
        || request.function_call.is_some()
        || request
            .messages
            .iter()
            .any(message_requires_tool_capability)
}

fn message_requires_tool_capability(message: &ChatMessage) -> bool {
    message
        .tool_calls
        .as_ref()
        .is_some_and(|calls| !calls.is_empty())
        || message.tool_call_id.is_some()
        || message.function_call.is_some()
        || content_has_tool_part(&message.content)
}

pub(crate) fn content_has_tool_use(content: &Option<MessageContent>) -> bool {
    matches!(
        content,
        Some(MessageContent::Parts(parts))
            if parts.iter().any(|part| matches!(part, ContentPart::ToolUse { .. }))
    )
}

pub(crate) fn content_has_tool_result(content: &Option<MessageContent>) -> bool {
    matches!(
        content,
        Some(MessageContent::Parts(parts))
            if parts.iter().any(|part| matches!(part, ContentPart::ToolResult { .. }))
    )
}

pub(crate) fn content_has_tool_part(content: &Option<MessageContent>) -> bool {
    content_has_tool_use(content) || content_has_tool_result(content)
}

pub(crate) fn build_tool_declarations(
    provider: &'static str,
    request: &ChatRequest,
) -> Result<(Option<Value>, Vec<String>), ProviderError> {
    if request.tools.is_some() && request.functions.is_some() {
        return Err(invalid(
            provider,
            "legacy functions cannot be combined with modern tools",
        ));
    }

    if let Some(tools) = &request.tools {
        let mut names = HashSet::new();
        let mut declarations = Vec::with_capacity(tools.len());
        for tool in tools {
            let name = validate_name(provider, &tool.function.name, "tool function name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(provider, "duplicate tool declaration name"));
            }
            let parameters = validate_parameters(provider, tool.function.parameters.as_ref())?;
            declarations.push(json!({
                "name": name,
                "description": tool.function.description.clone().unwrap_or_default(),
                "parameters": parameters
            }));
        }
        return Ok((
            Some(json!([{
                "functionDeclarations": declarations
            }])),
            names.into_iter().collect(),
        ));
    }

    if let Some(functions) = &request.functions {
        let mut names = HashSet::new();
        let mut declarations = Vec::with_capacity(functions.len());
        for function in functions {
            let object = function.as_object().ok_or_else(|| {
                invalid(
                    provider,
                    "legacy function declaration must be a JSON object",
                )
            })?;
            let name_value = object
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name = validate_name(provider, name_value, "legacy function name")?;
            if !names.insert(name.clone()) {
                return Err(invalid(
                    provider,
                    "duplicate legacy function declaration name",
                ));
            }
            let parameters = validate_parameters(provider, object.get("parameters"))?;
            declarations.push(json!({
                "name": name,
                "description": object
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                "parameters": parameters
            }));
        }
        return Ok((
            Some(json!([{
                "functionDeclarations": declarations
            }])),
            names.into_iter().collect(),
        ));
    }

    Ok((None, Vec::new()))
}

pub(crate) fn build_tool_config(
    provider: &'static str,
    request: &ChatRequest,
    declaration_names: &[String],
) -> Result<Option<Value>, ProviderError> {
    if matches!(request.parallel_tool_calls, Some(false)) {
        return Err(invalid(
            provider,
            "parallel_tool_calls=false is not supported by Google tool calling",
        ));
    }
    if request.tool_choice.is_some() && request.function_call.is_some() {
        return Err(invalid(
            provider,
            "legacy function_call cannot be combined with tool_choice",
        ));
    }

    let Some(config) = tool_choice_config(provider, request, declaration_names)? else {
        return Ok(None);
    };
    Ok(Some(json!({
        "functionCallingConfig": config
    })))
}

fn tool_choice_config(
    provider: &'static str,
    request: &ChatRequest,
    declaration_names: &[String],
) -> Result<Option<Value>, ProviderError> {
    if let Some(choice) = &request.tool_choice {
        return match choice {
            ToolChoice::String(value) if value == "auto" => Ok(Some(json!({"mode": "AUTO"}))),
            ToolChoice::String(value) if value == "none" => Ok(Some(json!({"mode": "NONE"}))),
            ToolChoice::String(_) => Err(invalid(provider, "unsupported tool_choice string")),
            ToolChoice::Specific {
                choice_type,
                function,
            } => {
                if choice_type != "function" {
                    return Err(invalid(provider, "tool_choice type must be function"));
                }
                let Some(function) = function else {
                    return Err(invalid(provider, "tool_choice function is required"));
                };
                forced_tool_config(provider, declaration_names, &function.name)
            }
        };
    }

    if let Some(choice) = &request.function_call {
        if let Some(value) = choice.as_str() {
            return match value {
                "auto" => Ok(Some(json!({"mode": "AUTO"}))),
                "none" => Ok(Some(json!({"mode": "NONE"}))),
                _ => Err(invalid(provider, "unsupported legacy function_call string")),
            };
        }
        let Some(name) = choice
            .as_object()
            .and_then(|object| object.get("name"))
            .and_then(Value::as_str)
        else {
            return Err(invalid(provider, "malformed legacy function_call"));
        };
        return forced_tool_config(provider, declaration_names, name);
    }

    Ok(None)
}

fn forced_tool_config(
    provider: &'static str,
    declaration_names: &[String],
    name: &str,
) -> Result<Option<Value>, ProviderError> {
    let name = validate_name(provider, name, "forced tool name")?;
    if !declaration_names.iter().any(|declared| declared == &name) {
        return Err(invalid(provider, "forced tool_choice name is not declared"));
    }
    Ok(Some(json!({
        "mode": "ANY",
        "allowedFunctionNames": [name]
    })))
}

pub(crate) fn candidate_index(
    provider: &'static str,
    candidate: &Value,
    fallback: usize,
) -> Result<u32, ProviderError> {
    let Some(index) = candidate.get("index") else {
        return Ok(fallback as u32);
    };
    let Some(index) = index.as_u64() else {
        return Err(ProviderError::response_parsing(
            provider,
            "candidate index must be a non-negative integer",
        ));
    };
    u32::try_from(index)
        .map_err(|_| ProviderError::response_parsing(provider, "candidate index is too large"))
}

pub(crate) fn parse_function_call_parts(
    provider: &'static str,
    parts: &[Value],
    candidate_index: u32,
) -> Result<Vec<ToolCall>, ProviderError> {
    let mut calls = Vec::new();
    for (part_index, part) in parts.iter().enumerate() {
        let Some(function_call) = part.get("functionCall") else {
            continue;
        };
        let name = function_call
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if name.trim().is_empty() {
            return Err(ProviderError::response_parsing(
                provider,
                "functionCall name must be non-empty",
            ));
        }
        let args = function_call.get("args").ok_or_else(|| {
            ProviderError::response_parsing(provider, "functionCall args are required")
        })?;
        if !args.is_object() {
            return Err(ProviderError::response_parsing(
                provider,
                "functionCall args must be a JSON object",
            ));
        }
        let id = function_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("call_{candidate_index}_{part_index}"));
        calls.push(ToolCall {
            id,
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        });
    }
    Ok(calls)
}

pub(crate) fn finish_reason(
    provider: &'static str,
    reason: Option<&str>,
    has_tool_calls: bool,
) -> Result<FinishReason, ProviderError> {
    if has_tool_calls {
        return Ok(FinishReason::ToolCalls);
    }
    match reason {
        Some("STOP") | None => Ok(FinishReason::Stop),
        Some("MAX_TOKENS") => Ok(FinishReason::Length),
        Some("SAFETY") | Some("RECITATION") => Ok(FinishReason::ContentFilter),
        Some(_) => Err(ProviderError::response_parsing(
            provider,
            "unknown Gemini finishReason",
        )),
    }
}

fn normalize_tool_role_result(
    provider: &'static str,
    content: &Option<MessageContent>,
) -> Result<Value, ProviderError> {
    match content {
        Some(MessageContent::Text(text)) => Ok(json!({ "result": text })),
        Some(MessageContent::Parts(parts)) => {
            let mut values = Vec::with_capacity(parts.len());
            for part in parts {
                if matches!(
                    part,
                    ContentPart::ToolUse { .. } | ContentPart::ToolResult { .. }
                ) {
                    return Err(invalid(
                        provider,
                        "tool role content cannot embed tool-use or tool-result parts",
                    ));
                }
                values.push(
                    serde_json::to_value(part).map_err(|error| {
                        ProviderError::serialization(provider, error.to_string())
                    })?,
                );
            }
            Ok(json!({ "result": values }))
        }
        None => Err(invalid(provider, "tool result content is required")),
    }
}

fn normalize_tool_result_content(content: &Value, is_error: Option<bool>) -> Value {
    if is_error == Some(true) {
        return json!({
            "result": content,
            "is_error": true
        });
    }
    if content.is_object() {
        return content.clone();
    }
    json!({ "result": content })
}

fn parse_arguments_object(
    provider: &'static str,
    arguments: &str,
    field: &'static str,
) -> Result<Value, ProviderError> {
    let value: Value = serde_json::from_str(arguments)
        .map_err(|_| invalid(provider, format!("{field} must be valid JSON")))?;
    if !value.is_object() {
        return Err(invalid(provider, format!("{field} must be a JSON object")));
    }
    Ok(value)
}

fn validate_name(
    provider: &'static str,
    name: &str,
    field: &'static str,
) -> Result<String, ProviderError> {
    if name.trim().is_empty() {
        return Err(invalid(provider, format!("{field} must be non-empty")));
    }
    Ok(name.to_string())
}

fn validate_parameters(
    provider: &'static str,
    parameters: Option<&Value>,
) -> Result<Value, ProviderError> {
    match parameters {
        Some(parameters) if parameters.is_object() => Ok(parameters.clone()),
        Some(_) => Err(invalid(
            provider,
            "tool declaration parameters must be a JSON object",
        )),
        None => Ok(json!({})),
    }
}

fn invalid(provider: &'static str, message: impl Into<String>) -> ProviderError {
    ProviderError::invalid_request(provider, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::tools::{FunctionDefinition, Tool, ToolType};

    fn tool_call(id: &str, name: &str, args: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: name.to_string(),
                arguments: args.to_string(),
            },
        }
    }

    #[test]
    fn round_trip_top_level_call_and_result() {
        let mut planner = GoogleToolPlanner::new("gemini");
        let calls = planner
            .top_level_calls(&[tool_call("call_1", "weather", r#"{"city":"Paris"}"#)])
            .unwrap();
        assert_eq!(
            calls[0].to_wire_value(),
            json!({"functionCall":{"id":"call_1","name":"weather","args":{"city":"Paris"}}})
        );
        let result = planner
            .top_level_result(&ChatMessage {
                role: MessageRole::Tool,
                tool_call_id: Some("call_1".to_string()),
                content: Some(MessageContent::Text("sunny".to_string())),
                ..Default::default()
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            result.to_wire_value(),
            json!({"functionResponse":{"name":"weather","response":{"result":"sunny"}}})
        );
    }

    #[test]
    fn rejects_unknown_or_reused_result_id() {
        let mut planner = GoogleToolPlanner::new("gemini");
        let unknown = planner.content_tool_result("missing", &json!("x"), None);
        assert!(matches!(unknown, Err(ProviderError::InvalidRequest { .. })));
        planner
            .top_level_calls(&[tool_call("call_1", "weather", "{}")])
            .unwrap();
        planner
            .content_tool_result("call_1", &json!({"ok": true}), None)
            .unwrap();
        let duplicate = planner.content_tool_result("call_1", &json!({"ok": true}), None);
        assert!(matches!(
            duplicate,
            Err(ProviderError::InvalidRequest { .. })
        ));
    }

    #[test]
    fn maps_declarations_and_forced_choice() {
        let request = ChatRequest {
            tools: Some(vec![Tool {
                tool_type: ToolType::Function,
                function: FunctionDefinition {
                    name: "weather".to_string(),
                    description: Some("Get weather".to_string()),
                    parameters: Some(json!({"type":"object"})),
                },
            }]),
            tool_choice: Some(ToolChoice::Specific {
                choice_type: "function".to_string(),
                function: Some(crate::core::types::tools::FunctionChoice {
                    name: "weather".to_string(),
                }),
            }),
            ..Default::default()
        };
        let (tools, names) = build_tool_declarations("gemini", &request).unwrap();
        assert_eq!(
            tools.unwrap()[0]["functionDeclarations"][0]["name"],
            "weather"
        );
        let config = build_tool_config("gemini", &request, &names)
            .unwrap()
            .unwrap();
        assert_eq!(
            config,
            json!({"functionCallingConfig":{"mode":"ANY","allowedFunctionNames":["weather"]}})
        );
    }

    #[test]
    fn strict_response_parser_rejects_bad_function_call() {
        let err = parse_function_call_parts(
            "gemini",
            &[json!({"functionCall":{"name":"","args":{}}})],
            0,
        )
        .unwrap_err();
        assert!(matches!(err, ProviderError::ResponseParsing { .. }));
        let err = parse_function_call_parts(
            "gemini",
            &[json!({"functionCall":{"name":"weather","args":[]}})],
            0,
        )
        .unwrap_err();
        assert!(matches!(err, ProviderError::ResponseParsing { .. }));
    }
}
