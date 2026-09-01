use crate::core::guardrails::GuardrailEngine;
use crate::core::models::openai::responses_api::{
    ResponseInput, ResponseInputContent, ResponseInputContentPart, ResponseInputItem,
    ResponsesApiRequest,
};
use crate::core::types::codex::wire::{CodexToolOutput, CodexToolOutputContent};
use crate::utils::error::gateway_error::GatewayError;

pub(crate) fn mask_responses_input_for_storage(
    engine: &GuardrailEngine,
    request: &ResponsesApiRequest,
) -> Result<ResponsesApiRequest, GatewayError> {
    if !engine.input_checks_enabled() {
        return Ok(request.clone());
    }

    let mut projected = request.clone();
    if let Some(user) = projected.user.as_mut() {
        super::mask_text(engine, user)?;
    }
    super::mask_metadata(engine, projected.metadata.as_mut())?;
    match &mut projected.input {
        ResponseInput::Text(text) => {
            super::mask_text(engine, text)?;
        }
        ResponseInput::Items(items) => {
            for item in items {
                match item {
                    ResponseInputItem::Message(message) => {
                        reject_identifiers(
                            engine,
                            [
                                message.id.as_deref(),
                                message.phase.as_deref(),
                                message
                                    .internal_chat_message_metadata_passthrough
                                    .as_ref()
                                    .and_then(|metadata| metadata.turn_id.as_deref()),
                            ],
                        )?;
                        match &mut message.content {
                            ResponseInputContent::Text(text) => {
                                super::mask_text(engine, text)?;
                            }
                            ResponseInputContent::Parts(parts) => {
                                for part in parts {
                                    match part {
                                        ResponseInputContentPart::InputText { text }
                                        | ResponseInputContentPart::OutputText { text } => {
                                            super::mask_text(engine, text)?;
                                        }
                                        ResponseInputContentPart::InputImage {
                                            image_url: Some(image_url),
                                            detail,
                                        } => {
                                            reject_identifiers(engine, [detail.as_deref()])?;
                                            super::mask_text(engine, image_url)?;
                                        }
                                        ResponseInputContentPart::InputAudio { audio_url } => {
                                            super::mask_text(engine, audio_url)?;
                                        }
                                        ResponseInputContentPart::InputImage {
                                            image_url: None,
                                            detail,
                                        } => {
                                            reject_identifiers(engine, [detail.as_deref()])?;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ResponseInputItem::FunctionCall(call) => {
                        reject_identifiers(
                            engine,
                            [
                                call.id.as_deref(),
                                Some(call.call_id.as_str()),
                                Some(call.name.as_str()),
                                call.namespace.as_deref(),
                                call.status.as_deref(),
                                call.internal_chat_message_metadata_passthrough
                                    .as_ref()
                                    .and_then(|metadata| metadata.turn_id.as_deref()),
                            ],
                        )?;
                        reject_unprojectable_mask(engine, &call.arguments)?;
                    }
                    ResponseInputItem::CustomToolCall(call) => {
                        reject_identifiers(
                            engine,
                            [
                                call.id.as_deref(),
                                Some(call.call_id.as_str()),
                                Some(call.name.as_str()),
                                call.namespace.as_deref(),
                                call.status.as_deref(),
                                call.internal_chat_message_metadata_passthrough
                                    .as_ref()
                                    .and_then(|metadata| metadata.turn_id.as_deref()),
                            ],
                        )?;
                        reject_unprojectable_mask(engine, &call.input)?;
                    }
                    ResponseInputItem::FunctionCallOutput(output) => {
                        reject_identifiers(
                            engine,
                            [
                                output.id.as_deref(),
                                Some(output.call_id.as_str()),
                                output
                                    .internal_chat_message_metadata_passthrough
                                    .as_ref()
                                    .and_then(|metadata| metadata.turn_id.as_deref()),
                            ],
                        )?;
                        mask_tool_output(engine, &mut output.output)?;
                    }
                    ResponseInputItem::CustomToolCallOutput(output) => {
                        reject_identifiers(
                            engine,
                            [
                                output.id.as_deref(),
                                Some(output.call_id.as_str()),
                                output.name.as_deref(),
                                output
                                    .internal_chat_message_metadata_passthrough
                                    .as_ref()
                                    .and_then(|metadata| metadata.turn_id.as_deref()),
                            ],
                        )?;
                        mask_tool_output(engine, &mut output.output)?;
                    }
                    ResponseInputItem::Unsupported(_) | ResponseInputItem::Unknown(_) => {}
                }
            }
        }
    }
    Ok(projected)
}

fn mask_tool_output(
    engine: &GuardrailEngine,
    output: &mut CodexToolOutput,
) -> Result<(), GatewayError> {
    match output {
        CodexToolOutput::Text(text) => super::mask_text(engine, text).map(|_| ()),
        CodexToolOutput::ContentItems(items) => {
            for item in items {
                match item {
                    CodexToolOutputContent::InputText { text } => {
                        super::mask_text(engine, text)?;
                    }
                    CodexToolOutputContent::InputImage { image_url, .. } => {
                        super::mask_text(engine, image_url)?;
                    }
                    CodexToolOutputContent::InputAudio { audio_url } => {
                        super::mask_text(engine, audio_url)?;
                    }
                    CodexToolOutputContent::EncryptedContent { encrypted_content } => {
                        super::mask_text(engine, encrypted_content)?;
                    }
                }
            }
            Ok(())
        }
    }
}

fn reject_unprojectable_mask(engine: &GuardrailEngine, content: &str) -> Result<(), GatewayError> {
    if super::mask_content(engine, content, "Responses input")?.is_some() {
        Err(super::projection_error("input"))
    } else {
        Ok(())
    }
}

fn reject_identifiers<'a>(
    engine: &GuardrailEngine,
    identifiers: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<(), GatewayError> {
    for identifier in identifiers.into_iter().flatten() {
        reject_unprojectable_mask(engine, identifier)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::gateway::GatewayConfig;
    use crate::core::guardrails::{GuardrailAction, PIIConfig};
    use crate::core::models::openai::responses_api::ResponseInput;
    use serde_json::json;

    fn engine() -> GuardrailEngine {
        let mut config = GatewayConfig::default().guardrails;
        config.pii = Some(PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[MASKED]".to_string()),
            ..PIIConfig::default()
        });
        GuardrailEngine::new(config).expect("PII policy must compile")
    }

    #[test]
    fn masks_plain_responses_input_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": "Email user@example.com"
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .expect("Responses input should be maskable");

        let ResponseInput::Text(input) = masked.input else {
            panic!("plain input should remain plain input");
        };
        assert_eq!(input, "Email [MASKED]");
    }

    #[test]
    fn masks_structured_text_without_reordering_non_text_parts() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "Email user@example.com"},
                    {"type": "input_image", "image_url": "https://example.com/image.png"}
                ]
            }]
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .expect("Responses input should be maskable");
        let serialized = serde_json::to_value(masked).expect("request should serialize");

        assert_eq!(
            serialized["input"][0]["content"][0]["text"],
            "Email [MASKED]"
        );
        assert_eq!(
            serialized["input"][0]["content"][1]["image_url"],
            "https://example.com/image.png"
        );
    }

    #[test]
    fn rejects_pii_in_unprojectable_function_arguments() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "function_call",
                "call_id": "call-1",
                "name": "lookup",
                "arguments": "{\"email\":\"user@example.com\"}"
            }]
        }))
        .expect("request should deserialize");

        assert!(mask_responses_input_for_storage(&engine(), &request).is_err());
    }

    #[test]
    fn masks_non_text_message_parts_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": "https://example.com/user@example.com"},
                    {"type": "input_audio", "audio_url": "https://example.com/user@example.com"}
                ]
            }]
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .expect("Responses input should be maskable");
        let serialized = serde_json::to_value(masked).expect("request should serialize");

        assert_eq!(
            serialized["input"][0]["content"][0]["image_url"],
            "https://example.com/[MASKED]"
        );
        assert_eq!(
            serialized["input"][0]["content"][1]["audio_url"],
            "https://example.com/[MASKED]"
        );
    }

    #[test]
    fn masks_non_text_tool_outputs_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "function_call_output",
                "call_id": "call-1",
                "output": [
                    {"type": "input_image", "image_url": "https://example.com/user@example.com"},
                    {"type": "input_audio", "audio_url": "https://example.com/user@example.com"},
                    {"type": "encrypted_content", "encrypted_content": "user@example.com"}
                ]
            }]
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .expect("Responses input should be maskable");
        let serialized = serde_json::to_value(masked).expect("request should serialize");

        assert_eq!(
            serialized["input"][0]["output"][0]["image_url"],
            "https://example.com/[MASKED]"
        );
        assert_eq!(
            serialized["input"][0]["output"][1]["audio_url"],
            "https://example.com/[MASKED]"
        );
        assert_eq!(
            serialized["input"][0]["output"][2]["encrypted_content"],
            "[MASKED]"
        );
    }

    #[test]
    fn masks_responses_user_and_metadata_before_echo_or_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": "safe",
            "user": "user@example.com",
            "metadata": {"owner": "second@example.com"}
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .expect("Responses identity fields should be maskable");

        assert_eq!(masked.user.as_deref(), Some("[MASKED]"));
        assert_eq!(
            masked
                .metadata
                .as_ref()
                .and_then(|metadata| metadata.get("owner"))
                .map(String::as_str),
            Some("[MASKED]")
        );
    }

    #[test]
    fn rejects_pii_in_responses_call_identifiers() {
        for input in [
            json!({
                "type": "function_call",
                "call_id": "user@example.com",
                "name": "lookup",
                "arguments": "{}"
            }),
            json!({
                "type": "function_call_output",
                "call_id": "user@example.com",
                "output": "safe"
            }),
            json!({
                "type": "custom_tool_call",
                "call_id": "call-1",
                "name": "user@example.com",
                "input": "safe"
            }),
            json!({
                "type": "custom_tool_call_output",
                "call_id": "call-1",
                "name": "user@example.com",
                "output": "safe"
            }),
        ] {
            let request: ResponsesApiRequest = serde_json::from_value(json!({
                "model": "gpt-4o",
                "input": [input]
            }))
            .expect("request should deserialize");

            assert!(mask_responses_input_for_storage(&engine(), &request).is_err());
        }
    }
}
