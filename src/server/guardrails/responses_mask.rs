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
    match &mut projected.input {
        ResponseInput::Text(text) => super::mask_text(engine, text)?,
        ResponseInput::Items(items) => {
            for item in items {
                match item {
                    ResponseInputItem::Message(message) => match &mut message.content {
                        ResponseInputContent::Text(text) => super::mask_text(engine, text)?,
                        ResponseInputContent::Parts(parts) => {
                            for part in parts {
                                match part {
                                    ResponseInputContentPart::InputText { text }
                                    | ResponseInputContentPart::OutputText { text } => {
                                        super::mask_text(engine, text)?;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    },
                    ResponseInputItem::FunctionCall(call) => {
                        reject_unprojectable_mask(engine, &call.arguments)?;
                    }
                    ResponseInputItem::CustomToolCall(call) => {
                        reject_unprojectable_mask(engine, &call.input)?;
                    }
                    ResponseInputItem::FunctionCallOutput(output) => {
                        mask_tool_output(engine, &mut output.output)?;
                    }
                    ResponseInputItem::CustomToolCallOutput(output) => {
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
        CodexToolOutput::Text(text) => super::mask_text(engine, text),
        CodexToolOutput::ContentItems(items) => {
            for item in items {
                if let CodexToolOutputContent::InputText { text } = item {
                    super::mask_text(engine, text)?;
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
}
