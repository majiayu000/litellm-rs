//! Additive OpenAI-compatible HTTP DTOs for typed provider continuations.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _, ser::Error as _};
use serde_json::Value;

use super::{
    messages::MessageRole,
    requests::ChatCompletionRequest,
    responses::ChatCompletionResponse,
    responses_api::{
        ResponseInput, ResponseInputItem, ResponseOutputItem, ResponsesApiRequest,
        ResponsesApiResponse,
    },
};
use crate::core::types::anthropic_continuation::ChatMessageExtensions;
use crate::core::types::codex::domain::{CodexTurn, CodexTurnError, CodexTurnItem};

/// A Chat Completions request with one typed extension carrier per message.
///
/// The legacy request remains unchanged. This wrapper is opt-in and serializes
/// empty carriers to the exact same semantic JSON as the legacy DTO.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ChatCompletionRequestWithExtensions {
    request: ChatCompletionRequest,
    message_extensions: Vec<ChatMessageExtensions>,
}

impl ChatCompletionRequestWithExtensions {
    pub(crate) fn from_parts(
        request: ChatCompletionRequest,
        message_extensions: Vec<ChatMessageExtensions>,
    ) -> Result<Self, String> {
        ensure_len(
            "chat message extensions",
            request.messages.len(),
            message_extensions.len(),
        )?;
        Ok(Self {
            request,
            message_extensions,
        })
    }

    pub(crate) fn legacy(&self) -> &ChatCompletionRequest {
        &self.request
    }

    pub(crate) fn message_extensions(&self) -> &[ChatMessageExtensions] {
        &self.message_extensions
    }

    pub(crate) fn has_continuation(&self) -> bool {
        self.message_extensions
            .iter()
            .any(|extension| !extension.is_empty())
    }

    pub(crate) fn into_parts(self) -> (ChatCompletionRequest, Vec<ChatMessageExtensions>) {
        (self.request, self.message_extensions)
    }
}

impl Serialize for ChatCompletionRequestWithExtensions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serde_json::to_value(&self.request).map_err(S::Error::custom)?;
        insert_nested_extensions(&mut value, &["messages"], None, &self.message_extensions)
            .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChatCompletionRequestWithExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(deserializer)?;
        let message_extensions =
            take_nested_extensions(&mut value, &["messages"], None).map_err(D::Error::custom)?;
        let request =
            serde_json::from_value::<ChatCompletionRequest>(value).map_err(D::Error::custom)?;
        Self::from_parts(request, message_extensions).map_err(D::Error::custom)
    }
}

/// A Chat Completions response with one typed extension carrier per choice.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ChatCompletionResponseWithExtensions {
    response: ChatCompletionResponse,
    choice_extensions: Vec<ChatMessageExtensions>,
}

impl ChatCompletionResponseWithExtensions {
    pub(crate) fn from_parts(
        response: ChatCompletionResponse,
        choice_extensions: Vec<ChatMessageExtensions>,
    ) -> Result<Self, String> {
        ensure_len(
            "chat choice extensions",
            response.choices.len(),
            choice_extensions.len(),
        )?;
        Ok(Self {
            response,
            choice_extensions,
        })
    }

    #[cfg(test)]
    pub(crate) fn has_continuation(&self) -> bool {
        self.choice_extensions
            .iter()
            .any(|extension| !extension.is_empty())
    }

    pub(crate) fn into_parts(self) -> (ChatCompletionResponse, Vec<ChatMessageExtensions>) {
        (self.response, self.choice_extensions)
    }
}

impl Serialize for ChatCompletionResponseWithExtensions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serde_json::to_value(&self.response).map_err(S::Error::custom)?;
        insert_nested_extensions(
            &mut value,
            &["choices"],
            Some("message"),
            &self.choice_extensions,
        )
        .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChatCompletionResponseWithExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(deserializer)?;
        let choice_extensions = take_nested_extensions(&mut value, &["choices"], Some("message"))
            .map_err(D::Error::custom)?;
        let response =
            serde_json::from_value::<ChatCompletionResponse>(value).map_err(D::Error::custom)?;
        Self::from_parts(response, choice_extensions).map_err(D::Error::custom)
    }
}

/// A Responses API request with typed extensions attached to message items.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ResponsesApiRequestWithExtensions {
    request: ResponsesApiRequest,
    input_extensions: Vec<Option<ChatMessageExtensions>>,
}

impl ResponsesApiRequestWithExtensions {
    pub(crate) fn from_parts(
        request: ResponsesApiRequest,
        input_extensions: Vec<Option<ChatMessageExtensions>>,
    ) -> Result<Self, String> {
        let expected = response_input_len(&request);
        ensure_len(
            "Responses input extensions",
            expected,
            input_extensions.len(),
        )?;
        Ok(Self {
            request,
            input_extensions,
        })
    }

    pub(crate) fn legacy(&self) -> &ResponsesApiRequest {
        &self.request
    }

    pub(crate) fn has_continuation(&self) -> bool {
        self.input_extensions
            .iter()
            .flatten()
            .any(|extension| !extension.is_empty())
    }

    pub(crate) fn into_parts(self) -> (ResponsesApiRequest, Vec<Option<ChatMessageExtensions>>) {
        (self.request, self.input_extensions)
    }
}

impl Serialize for ResponsesApiRequestWithExtensions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serde_json::to_value(&self.request).map_err(S::Error::custom)?;
        insert_item_extensions(&mut value, "input", &self.input_extensions)
            .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ResponsesApiRequestWithExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(deserializer)?;
        let input_extensions =
            take_item_extensions(&mut value, "input").map_err(D::Error::custom)?;
        let request =
            serde_json::from_value::<ResponsesApiRequest>(value).map_err(D::Error::custom)?;
        Self::from_parts(request, input_extensions).map_err(D::Error::custom)
    }
}

/// A Responses API response with typed extensions attached to message items.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ResponsesApiResponseWithExtensions {
    response: ResponsesApiResponse,
    output_extensions: Vec<Option<ChatMessageExtensions>>,
}

impl ResponsesApiResponseWithExtensions {
    pub(crate) fn from_parts(
        response: ResponsesApiResponse,
        output_extensions: Vec<Option<ChatMessageExtensions>>,
    ) -> Result<Self, String> {
        ensure_len(
            "Responses output extensions",
            response.output.len(),
            output_extensions.len(),
        )?;
        Ok(Self {
            response,
            output_extensions,
        })
    }

    #[cfg(test)]
    pub(crate) fn has_continuation(&self) -> bool {
        self.output_extensions
            .iter()
            .flatten()
            .any(|extension| !extension.is_empty())
    }
}

impl Serialize for ResponsesApiResponseWithExtensions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut value = serde_json::to_value(&self.response).map_err(S::Error::custom)?;
        insert_item_extensions(&mut value, "output", &self.output_extensions)
            .map_err(S::Error::custom)?;
        value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ResponsesApiResponseWithExtensions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut value = Value::deserialize(deserializer)?;
        let output_extensions =
            take_item_extensions(&mut value, "output").map_err(D::Error::custom)?;
        let response =
            serde_json::from_value::<ResponsesApiResponse>(value).map_err(D::Error::custom)?;
        Self::from_parts(response, output_extensions).map_err(D::Error::custom)
    }
}

pub(crate) fn attach_responses_choice_extensions(
    response: &mut ResponsesApiResponse,
    choice_extensions: Vec<ChatMessageExtensions>,
    empty_message: ResponseOutputItem,
) -> Result<Vec<Option<ChatMessageExtensions>>, String> {
    if choice_extensions.len() > 1 {
        return Err("Anthropic continuation requires exactly one response choice".to_string());
    }
    let mut output_extensions = vec![None; response.output.len()];
    if let Some(extension) = choice_extensions.into_iter().next()
        && !extension.is_empty()
    {
        let index = response
            .output
            .iter()
            .position(|item| matches!(item, ResponseOutputItem::Message(_)))
            .unwrap_or_else(|| {
                response.output.insert(0, empty_message);
                output_extensions.insert(0, None);
                0
            });
        output_extensions[index] = Some(extension);
    }
    Ok(output_extensions)
}

pub(crate) fn map_responses_input_extensions(
    request: &ResponsesApiRequest,
    chat: &ChatCompletionRequest,
    input: Vec<Option<ChatMessageExtensions>>,
) -> Result<Vec<ChatMessageExtensions>, String> {
    let mut mapped = Vec::with_capacity(chat.messages.len());
    let mut chat_index = 0;
    if request.instructions.is_some() {
        mapped.push(ChatMessageExtensions::new());
        chat_index += 1;
    }
    match &request.input {
        ResponseInput::Text(_) => {
            mapped.push(ChatMessageExtensions::new());
            chat_index += 1;
        }
        ResponseInput::Items(items) => {
            ensure_len("Responses input extensions", items.len(), input.len())?;
            for (item, extension) in items.iter().zip(input) {
                match item {
                    ResponseInputItem::Message(_) => {
                        if chat.messages.get(chat_index).is_none() {
                            return Err("Responses message mapping drifted".to_string());
                        }
                        mapped.push(extension.unwrap_or_default());
                        chat_index += 1;
                    }
                    ResponseInputItem::FunctionCall(call) => {
                        map_tool_call(&mut mapped, chat, &mut chat_index, &call.call_id)?;
                    }
                    ResponseInputItem::CustomToolCall(call) => {
                        map_tool_call(&mut mapped, chat, &mut chat_index, &call.call_id)?;
                    }
                    ResponseInputItem::FunctionCallOutput(output) => {
                        map_tool_output(&mut mapped, chat, &mut chat_index, &output.call_id)?;
                    }
                    ResponseInputItem::CustomToolCallOutput(output) => {
                        map_tool_output(&mut mapped, chat, &mut chat_index, &output.call_id)?;
                    }
                    item => {
                        return Err(format!(
                            "unsupported Responses continuation item: {}",
                            item.feature_name()
                        ));
                    }
                }
            }
        }
    }
    ensure_len(
        "Responses continuation mapping",
        chat.messages.len(),
        mapped.len(),
    )?;
    ensure_len(
        "Responses continuation cursor",
        chat.messages.len(),
        chat_index,
    )?;
    Ok(mapped)
}

fn map_tool_call(
    mapped: &mut Vec<ChatMessageExtensions>,
    chat: &ChatCompletionRequest,
    chat_index: &mut usize,
    call_id: &str,
) -> Result<(), String> {
    let contains_call = |index: usize| {
        chat.messages
            .get(index)
            .and_then(|message| message.tool_calls.as_ref())
            .is_some_and(|calls| calls.iter().any(|call| call.id == call_id))
    };
    if *chat_index > 0 && contains_call(*chat_index - 1) {
        return Ok(());
    }
    if !contains_call(*chat_index) {
        return Err("Responses tool-call mapping drifted".to_string());
    }
    mapped.push(ChatMessageExtensions::new());
    *chat_index += 1;
    Ok(())
}

fn map_tool_output(
    mapped: &mut Vec<ChatMessageExtensions>,
    chat: &ChatCompletionRequest,
    chat_index: &mut usize,
    call_id: &str,
) -> Result<(), String> {
    let message = chat
        .messages
        .get(*chat_index)
        .ok_or_else(|| "Responses tool-output mapping drifted".to_string())?;
    if message.role != MessageRole::Tool || message.tool_call_id.as_deref() != Some(call_id) {
        return Err("Responses tool-output mapping drifted".to_string());
    }
    mapped.push(ChatMessageExtensions::new());
    *chat_index += 1;
    Ok(())
}

fn ensure_len(label: &str, expected: usize, actual: usize) -> Result<(), String> {
    if expected == actual {
        Ok(())
    } else {
        Err(format!(
            "{label} length mismatch: expected {expected}, got {actual}"
        ))
    }
}

fn response_input_len(request: &ResponsesApiRequest) -> usize {
    match &request.input {
        super::responses_api::ResponseInput::Text(_) => 0,
        super::responses_api::ResponseInput::Items(items) => items.len(),
    }
}

pub(crate) fn build_responses_continuation_turn(
    request: &ResponsesApiRequest,
    extensions: &[Option<ChatMessageExtensions>],
) -> Result<CodexTurn, CodexTurnError> {
    let mut validated = request.clone();
    if let super::responses_api::ResponseInput::Items(items) = &mut validated.input {
        for (item, extension) in items.iter_mut().zip(extensions) {
            let super::responses_api::ResponseInputItem::Message(message) = item else {
                continue;
            };
            let empty = match &message.content {
                super::responses_api::ResponseInputContent::Text(text) => text.trim().is_empty(),
                super::responses_api::ResponseInputContent::Parts(parts) => parts.is_empty(),
            };
            if extension.as_ref().is_some_and(|item| !item.is_empty())
                && message.role == "assistant"
                && empty
            {
                message.content = super::responses_api::ResponseInputContent::Text(
                    "typed continuation".to_string(),
                );
            }
        }
    }
    let mut turn = CodexTurn::try_from(&validated)?;
    if let super::responses_api::ResponseInput::Items(items) = &request.input {
        for ((turn_item, original), extension) in turn.items.iter_mut().zip(items).zip(extensions) {
            if extension.as_ref().is_some_and(|item| !item.is_empty()) {
                *turn_item = CodexTurnItem::Item(original.clone());
            }
        }
    }
    Ok(turn)
}

fn nested_array_mut<'a>(value: &'a mut Value, path: &[&str]) -> Result<&'a mut Vec<Value>, String> {
    let mut current = value;
    for segment in path {
        current = current
            .get_mut(*segment)
            .ok_or_else(|| format!("missing {segment} field"))?;
    }
    current
        .as_array_mut()
        .ok_or_else(|| format!("{} must be an array", path.join(".")))
}

fn message_object_mut<'a>(
    item: &'a mut Value,
    nested_message: Option<&str>,
) -> Result<&'a mut serde_json::Map<String, Value>, String> {
    let message = if let Some(field) = nested_message {
        item.get_mut(field)
            .ok_or_else(|| format!("missing {field} field"))?
    } else {
        item
    };
    message
        .as_object_mut()
        .ok_or_else(|| "message must be an object".to_string())
}

fn take_nested_extensions(
    value: &mut Value,
    path: &[&str],
    nested_message: Option<&str>,
) -> Result<Vec<ChatMessageExtensions>, String> {
    nested_array_mut(value, path)?
        .iter_mut()
        .map(|item| {
            let message = message_object_mut(item, nested_message)?;
            message
                .remove("extensions")
                .map(serde_json::from_value)
                .transpose()
                .map_err(|error| error.to_string())
                .map(Option::unwrap_or_default)
        })
        .collect()
}

fn insert_nested_extensions(
    value: &mut Value,
    path: &[&str],
    nested_message: Option<&str>,
    extensions: &[ChatMessageExtensions],
) -> Result<(), String> {
    let items = nested_array_mut(value, path)?;
    ensure_len("nested extensions", items.len(), extensions.len())?;
    for (item, extension) in items.iter_mut().zip(extensions) {
        if !extension.is_empty() {
            message_object_mut(item, nested_message)?.insert(
                "extensions".to_string(),
                serde_json::to_value(extension).map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(())
}

fn take_item_extensions(
    value: &mut Value,
    field: &str,
) -> Result<Vec<Option<ChatMessageExtensions>>, String> {
    let Some(items) = value.get_mut(field).and_then(Value::as_array_mut) else {
        return Ok(Vec::new());
    };
    items
        .iter_mut()
        .map(|item| {
            let object = item
                .as_object_mut()
                .ok_or_else(|| format!("{field} item must be an object"))?;
            let extension = object
                .remove("extensions")
                .map(serde_json::from_value::<ChatMessageExtensions>)
                .transpose()
                .map_err(|error| error.to_string())?;
            if extension.is_some() && object.get("type").and_then(Value::as_str) != Some("message")
            {
                return Err(format!(
                    "{field} extensions are only valid on message items"
                ));
            }
            Ok(extension)
        })
        .collect()
}

fn insert_item_extensions(
    value: &mut Value,
    field: &str,
    extensions: &[Option<ChatMessageExtensions>],
) -> Result<(), String> {
    let Some(items) = value.get_mut(field).and_then(Value::as_array_mut) else {
        return ensure_len(&format!("{field} extensions"), 0, extensions.len());
    };
    ensure_len(
        &format!("{field} extensions"),
        items.len(),
        extensions.len(),
    )?;
    for (item, extension) in items.iter_mut().zip(extensions) {
        if let Some(extension) = extension
            && !extension.is_empty()
        {
            let object = item
                .as_object_mut()
                .ok_or_else(|| format!("{field} item must be an object"))?;
            if object.get("type").and_then(Value::as_str) != Some("message") {
                return Err(format!(
                    "{field} extensions are only valid on message items"
                ));
            }
            object.insert(
                "extensions".to_string(),
                serde_json::to_value(extension).map_err(|error| error.to_string())?,
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::anthropic_continuation::{
        AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent, ChatMessageExtensions,
    };
    use serde_json::json;

    fn extension() -> ChatMessageExtensions {
        ChatMessageExtensions::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "plan".to_string(),
                signature: AnthropicSignature::try_from("opaque-signature")
                    .expect("fixture signature is non-empty"),
            },
        ]))
    }

    #[test]
    fn chat_http_typed_messages_roundtrip_and_legacy_shape_stays_unchanged() {
        let legacy = json!({
            "model": "claude-opus-5",
            "messages": [{"role": "user", "content": "hello"}]
        });
        let legacy_request: super::ChatCompletionRequest =
            serde_json::from_value(legacy.clone()).expect("legacy request is valid");
        let empty: ChatCompletionRequestWithExtensions =
            serde_json::from_value(legacy.clone()).expect("legacy request remains accepted");
        assert!(!empty.has_continuation());
        assert_eq!(
            serde_json::to_value(empty).expect("serialize wrapper"),
            serde_json::to_value(legacy_request).expect("serialize legacy request")
        );

        let typed = json!({
            "model": "claude-opus-5",
            "messages": [{
                "role": "assistant",
                "content": null,
                "extensions": {"anthropic_thinking": [{
                    "type": "thinking",
                    "thinking": "plan",
                    "signature": "opaque-signature"
                }]}
            }]
        });
        let decoded: ChatCompletionRequestWithExtensions =
            serde_json::from_value(typed.clone()).expect("typed request is valid");
        assert!(decoded.has_continuation());
        let encoded = serde_json::to_value(&decoded).expect("roundtrip");
        assert_eq!(
            encoded["messages"][0]["extensions"],
            typed["messages"][0]["extensions"]
        );
        let replayed: ChatCompletionRequestWithExtensions =
            serde_json::from_value(encoded).expect("replay wrapper");
        assert!(replayed.has_continuation());
    }

    #[test]
    fn chat_http_response_preserves_choice_sidecars() {
        let typed = json!({
            "id": "chatcmpl_1",
            "object": "chat.completion",
            "created": 1,
            "model": "claude-opus-5",
            "system_fingerprint": null,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "extensions": {"anthropic_thinking": [{
                        "type": "thinking",
                        "thinking": "plan",
                        "signature": "opaque-signature"
                    }]}
                },
                "logprobs": null,
                "finish_reason": "tool_calls"
            }],
            "usage": null
        });
        let decoded: ChatCompletionResponseWithExtensions =
            serde_json::from_value(typed.clone()).expect("typed response is valid");
        assert!(decoded.has_continuation());
        let encoded = serde_json::to_value(&decoded).expect("roundtrip");
        assert_eq!(
            encoded["choices"][0]["message"]["extensions"],
            typed["choices"][0]["message"]["extensions"]
        );
        let replayed: ChatCompletionResponseWithExtensions =
            serde_json::from_value(encoded).expect("replay response wrapper");
        assert!(replayed.has_continuation());
    }

    #[test]
    fn responses_http_message_items_preserve_typed_extensions() {
        let request = json!({
            "model": "claude-opus-5",
            "input": [{
                "type": "message",
                "role": "assistant",
                "content": [],
                "extensions": {"anthropic_thinking": [{
                    "type": "thinking",
                    "thinking": "plan",
                    "signature": "opaque-signature"
                }]}
            }]
        });
        let decoded: ResponsesApiRequestWithExtensions =
            serde_json::from_value(request.clone()).expect("typed Responses input is valid");
        assert!(decoded.has_continuation());
        assert_eq!(serde_json::to_value(decoded).expect("roundtrip"), request);

        let response = json!({
            "id": "resp_1",
            "object": "response",
            "created_at": 1,
            "status": "completed",
            "model": "claude-opus-5",
            "output": [{
                "type": "message",
                "id": "msg_1",
                "role": "assistant",
                "status": "completed",
                "content": [],
                "extensions": {"anthropic_thinking": [{
                    "type": "thinking",
                    "thinking": "plan",
                    "signature": "opaque-signature"
                }]}
            }],
            "usage": null,
            "error": null,
            "previous_response_id": null,
            "metadata": null
        });
        let decoded: ResponsesApiResponseWithExtensions =
            serde_json::from_value(response.clone()).expect("typed Responses output is valid");
        assert!(decoded.has_continuation());
        let encoded = serde_json::to_value(&decoded).expect("roundtrip");
        assert_eq!(
            encoded["output"][0]["extensions"],
            response["output"][0]["extensions"]
        );
        let replayed: ResponsesApiResponseWithExtensions =
            serde_json::from_value(encoded).expect("replay Responses wrapper");
        assert!(replayed.has_continuation());
    }

    #[test]
    fn typed_http_boundaries_reject_unknown_provider_extensions() {
        let unknown = json!({
            "model": "claude-opus-5",
            "messages": [{
                "role": "assistant",
                "content": null,
                "extensions": {"future_provider": {"secret": "must-not-drop"}}
            }]
        });
        assert!(serde_json::from_value::<ChatCompletionRequestWithExtensions>(unknown).is_err());

        let _ = extension();
    }
}
