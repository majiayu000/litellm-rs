use crate::core::guardrails::GuardrailEngine;
use crate::core::models::openai::responses_api::{
    ResponseInput, ResponseInputContent, ResponseInputContentPart, ResponseInputItem,
    ResponsesApiRequest,
};
use crate::core::types::codex::wire::{CodexToolOutput, CodexToolOutputContent};
use crate::utils::error::gateway_error::GatewayError;

pub(crate) async fn mask_responses_input_for_storage(
    engine: &GuardrailEngine,
    request: &ResponsesApiRequest,
) -> Result<ResponsesApiRequest, GatewayError> {
    let input_checks_enabled = engine.input_checks_enabled();
    let output_checks_enabled = engine.is_enabled() && engine.config().check_output;
    if !input_checks_enabled && !output_checks_enabled {
        return Ok(request.clone());
    }

    if output_checks_enabled && let Some(metadata) = request.metadata.as_ref() {
        let content = metadata
            .iter()
            .flat_map(|(key, value)| [key.as_str(), value.as_str()])
            .collect::<Vec<_>>()
            .join(super::FRAGMENT_SEPARATOR);
        super::enforce(engine.check_output(&content).await, "output")?;
    }
    let mut projected = request.clone();
    if !input_checks_enabled {
        super::mask_metadata(engine, projected.metadata.as_mut())?;
        return Ok(projected);
    }
    mask_metadata(engine, projected.metadata.as_mut()).await?;
    if let Some(user) = projected.user.as_mut() {
        mask_projectable_text(engine, user).await?;
    }
    let will_store = request.store.unwrap_or(true) || request.background.unwrap_or(false);
    match &mut projected.input {
        ResponseInput::Text(text) => {
            mask_projectable_text(engine, text).await?;
        }
        ResponseInput::Items(items) => {
            for item in items {
                match item {
                    ResponseInputItem::Message(message) => {
                        if will_store {
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
                            )
                            .await?;
                        }
                        match &mut message.content {
                            ResponseInputContent::Text(text) => {
                                mask_projectable_text(engine, text).await?;
                            }
                            ResponseInputContent::Parts(parts) => {
                                for part in &mut *parts {
                                    match part {
                                        ResponseInputContentPart::InputText { text }
                                        | ResponseInputContentPart::OutputText { text } => {
                                            mask_projectable_text(engine, text).await?;
                                        }
                                        ResponseInputContentPart::InputImage {
                                            image_url: Some(image_url),
                                            detail,
                                        } => {
                                            reject_identifiers(engine, [detail.as_deref()]).await?;
                                            mask_projectable_url(engine, image_url).await?;
                                        }
                                        ResponseInputContentPart::InputAudio { audio_url } => {
                                            mask_projectable_text(engine, audio_url).await?;
                                        }
                                        ResponseInputContentPart::InputImage {
                                            image_url: None,
                                            detail,
                                        } => {
                                            reject_identifiers(engine, [detail.as_deref()]).await?;
                                        }
                                    }
                                }
                                reject_adjacent_text_matches(engine, parts).await?;
                            }
                        }
                    }
                    ResponseInputItem::FunctionCall(call) => {
                        reject_identifiers(
                            engine,
                            [Some(call.call_id.as_str()), Some(call.name.as_str())],
                        )
                        .await?;
                        if will_store {
                            reject_identifiers(
                                engine,
                                [
                                    call.id.as_deref(),
                                    call.namespace.as_deref(),
                                    call.status.as_deref(),
                                    call.internal_chat_message_metadata_passthrough
                                        .as_ref()
                                        .and_then(|metadata| metadata.turn_id.as_deref()),
                                ],
                            )
                            .await?;
                        }
                        reject_unprojectable_mask(engine, &call.arguments).await?;
                    }
                    ResponseInputItem::CustomToolCall(call) => {
                        reject_identifiers(
                            engine,
                            [Some(call.call_id.as_str()), Some(call.name.as_str())],
                        )
                        .await?;
                        if will_store {
                            reject_identifiers(
                                engine,
                                [
                                    call.id.as_deref(),
                                    call.namespace.as_deref(),
                                    call.status.as_deref(),
                                    call.internal_chat_message_metadata_passthrough
                                        .as_ref()
                                        .and_then(|metadata| metadata.turn_id.as_deref()),
                                ],
                            )
                            .await?;
                        }
                        reject_unprojectable_mask(engine, &call.input).await?;
                    }
                    ResponseInputItem::FunctionCallOutput(output) => {
                        reject_identifiers(engine, [Some(output.call_id.as_str())]).await?;
                        if will_store {
                            reject_identifiers(
                                engine,
                                [
                                    output.id.as_deref(),
                                    output
                                        .internal_chat_message_metadata_passthrough
                                        .as_ref()
                                        .and_then(|metadata| metadata.turn_id.as_deref()),
                                ],
                            )
                            .await?;
                        }
                        mask_tool_output(engine, &mut output.output).await?;
                    }
                    ResponseInputItem::CustomToolCallOutput(output) => {
                        reject_identifiers(engine, [Some(output.call_id.as_str())]).await?;
                        if will_store {
                            reject_identifiers(
                                engine,
                                [
                                    output.id.as_deref(),
                                    output.name.as_deref(),
                                    output
                                        .internal_chat_message_metadata_passthrough
                                        .as_ref()
                                        .and_then(|metadata| metadata.turn_id.as_deref()),
                                ],
                            )
                            .await?;
                        }
                        mask_tool_output(engine, &mut output.output).await?;
                    }
                    ResponseInputItem::Unsupported(_) | ResponseInputItem::Unknown(_) => {}
                }
            }
        }
    }
    Ok(projected)
}

async fn mask_tool_output(
    engine: &GuardrailEngine,
    output: &mut CodexToolOutput,
) -> Result<(), GatewayError> {
    match output {
        CodexToolOutput::Text(text) => mask_projectable_text(engine, text).await,
        CodexToolOutput::ContentItems(items) => {
            for item in items {
                match item {
                    CodexToolOutputContent::InputText { text } => {
                        mask_projectable_text(engine, text).await?;
                    }
                    CodexToolOutputContent::InputImage { image_url, detail } => {
                        reject_identifiers(engine, [detail.as_deref()]).await?;
                        mask_projectable_url(engine, image_url).await?;
                    }
                    CodexToolOutputContent::InputAudio { audio_url } => {
                        mask_projectable_text(engine, audio_url).await?;
                    }
                    CodexToolOutputContent::EncryptedContent { encrypted_content } => {
                        reject_unprojectable_mask(engine, encrypted_content).await?;
                    }
                }
            }
            Ok(())
        }
    }
}

async fn mask_projectable_url(
    engine: &GuardrailEngine,
    url: &mut String,
) -> Result<(), GatewayError> {
    match super::enforce(
        engine.check_input(super::image_url::scannable(url)).await,
        "input",
    )? {
        super::Enforcement::Pass => Ok(()),
        super::Enforcement::Mask => {
            super::image_url::mask(engine, url)?;
            reject_unprojectable_mask(engine, super::image_url::scannable(url)).await
        }
    }
}

async fn reject_adjacent_text_matches(
    engine: &GuardrailEngine,
    parts: &[ResponseInputContentPart],
) -> Result<(), GatewayError> {
    let mut adjacent = String::new();
    for part in parts {
        match part {
            ResponseInputContentPart::InputText { text }
            | ResponseInputContentPart::OutputText { text } => adjacent.push_str(text),
            _ => {
                reject_unprojectable_mask(engine, &adjacent).await?;
                adjacent.clear();
            }
        }
    }
    reject_unprojectable_mask(engine, &adjacent).await
}

async fn mask_projectable_text(
    engine: &GuardrailEngine,
    content: &mut String,
) -> Result<(), GatewayError> {
    match super::enforce(engine.check_input(content).await, "input")? {
        super::Enforcement::Pass => Ok(()),
        super::Enforcement::Mask => {
            super::mask_text(engine, content)?;
            reject_unprojectable_mask(engine, content).await
        }
    }
}

async fn mask_metadata(
    engine: &GuardrailEngine,
    metadata: Option<&mut std::collections::HashMap<String, String>>,
) -> Result<(), GatewayError> {
    for (key, value) in metadata.into_iter().flatten() {
        reject_unprojectable_mask(engine, key).await?;
        mask_projectable_text(engine, value).await?;
    }
    Ok(())
}

async fn reject_unprojectable_mask(
    engine: &GuardrailEngine,
    content: &str,
) -> Result<(), GatewayError> {
    match super::enforce(engine.check_input(content).await, "input")? {
        super::Enforcement::Pass => Ok(()),
        super::Enforcement::Mask => Err(super::projection_error("input")),
    }
}

async fn reject_identifiers<'a>(
    engine: &GuardrailEngine,
    identifiers: impl IntoIterator<Item = Option<&'a str>>,
) -> Result<(), GatewayError> {
    for identifier in identifiers.into_iter().flatten() {
        reject_unprojectable_mask(engine, identifier).await?;
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

    fn engine_with_input_checks(check_input: bool) -> GuardrailEngine {
        let mut config = GatewayConfig::default().guardrails;
        config.check_input = check_input;
        config.pii = Some(PIIConfig {
            enabled: true,
            action: GuardrailAction::Mask,
            mask_pattern: Some("[MASKED]".to_string()),
            ..PIIConfig::default()
        });
        GuardrailEngine::new(config).expect("PII policy must compile")
    }

    fn engine() -> GuardrailEngine {
        engine_with_input_checks(true)
    }

    fn blocking_engine(check_input: bool) -> GuardrailEngine {
        let mut config = GatewayConfig::default().guardrails;
        config.check_input = check_input;
        config.pii = Some(PIIConfig {
            enabled: true,
            action: GuardrailAction::Block,
            ..PIIConfig::default()
        });
        GuardrailEngine::new(config).expect("PII policy must compile")
    }

    #[tokio::test]
    async fn masks_plain_responses_input_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": "Email user@example.com"
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .await
            .expect("Responses input should be maskable");

        let ResponseInput::Text(input) = masked.input else {
            panic!("plain input should remain plain input");
        };
        assert_eq!(input, "Email [MASKED]");
    }

    #[tokio::test]
    async fn blocking_policies_run_before_projectable_input_is_returned_for_storage() {
        for input in [
            json!("user@example.com"),
            json!([{"type": "message", "role": "user", "content": "user@example.com"}]),
        ] {
            let request: ResponsesApiRequest = serde_json::from_value(json!({
                "model": "gpt-4o",
                "input": input,
                "background": true
            }))
            .expect("request should deserialize");

            assert!(
                mask_responses_input_for_storage(&blocking_engine(true), &request)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn adjacent_text_matches_fail_closed_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_text", "text": "123-45"},
                    {"type": "input_text", "text": "6789"}
                ]
            }],
            "background": true
        }))
        .expect("request should deserialize");

        assert!(
            mask_responses_input_for_storage(&engine(), &request)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn masks_structured_text_without_reordering_non_text_parts() {
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
            .await
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

    #[tokio::test]
    async fn rejects_pii_in_unprojectable_function_arguments() {
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

        assert!(
            mask_responses_input_for_storage(&engine(), &request)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn masks_non_text_message_parts_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "role": "user",
                "content": [
                    {"type": "input_image", "image_url": "https://example.com/user@example.com"},
                    {"type": "input_audio", "audio_url": "https://example.com/user@example.com"},
                    {"type": "input_image", "image_url": "data:image/png;base64,2125551234=="}
                ]
            }]
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .await
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
        assert_eq!(
            serialized["input"][0]["content"][2]["image_url"],
            "data:image/png;base64,2125551234=="
        );
    }

    #[tokio::test]
    async fn masks_non_text_tool_outputs_before_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "function_call_output",
                "call_id": "call-1",
                "output": [
                    {"type": "input_image", "image_url": "https://example.com/user@example.com"},
                    {"type": "input_audio", "audio_url": "https://example.com/user@example.com"},
                    {"type": "encrypted_content", "encrypted_content": "opaque-ciphertext"}
                ]
            }]
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .await
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
            "opaque-ciphertext"
        );

        for output in [
            json!([{"type": "input_image", "image_url": "safe", "detail": "user@example.com"}]),
            json!([{"type": "encrypted_content", "encrypted_content": "2125551234"}]),
        ] {
            let unsafe_request: ResponsesApiRequest = serde_json::from_value(json!({
                "model": "gpt-4o",
                "input": [{"type": "function_call_output", "call_id": "call-1", "output": output}]
            }))
            .expect("request should deserialize");
            assert!(
                mask_responses_input_for_storage(&engine(), &unsafe_request)
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn masks_responses_user_and_metadata_before_echo_or_storage() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": "safe",
            "user": "user@example.com",
            "metadata": {"owner": "second@example.com"}
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine(), &request)
            .await
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

    #[tokio::test]
    async fn output_policies_are_enforced_on_echoed_metadata() {
        let request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": "safe",
            "metadata": {"owner": "user@example.com"}
        }))
        .expect("request should deserialize");

        let masked = mask_responses_input_for_storage(&engine_with_input_checks(false), &request)
            .await
            .expect("echoed metadata should be maskable");

        assert_eq!(masked.metadata.unwrap()["owner"], "[MASKED]");

        let blocking = blocking_engine(false);
        assert!(
            mask_responses_input_for_storage(&blocking, &request)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn storage_only_message_identifiers_are_ignored_when_store_is_false() {
        let mut request: ResponsesApiRequest = serde_json::from_value(json!({
            "model": "gpt-4o",
            "input": [{
                "type": "message",
                "id": "user@example.com",
                "role": "user",
                "content": "safe"
            }],
            "store": false
        }))
        .expect("request should deserialize");

        assert!(
            mask_responses_input_for_storage(&engine(), &request)
                .await
                .is_ok()
        );
        request.background = Some(true);
        assert!(
            mask_responses_input_for_storage(&engine(), &request)
                .await
                .is_err()
        );
        request.background = None;
        request.store = Some(true);
        assert!(
            mask_responses_input_for_storage(&engine(), &request)
                .await
                .is_err()
        );
        assert!(
            mask_responses_input_for_storage(&blocking_engine(true), &request)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn rejects_pii_in_responses_call_identifiers() {
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

            assert!(
                mask_responses_input_for_storage(&engine(), &request)
                    .await
                    .is_err()
            );
        }
    }
}
