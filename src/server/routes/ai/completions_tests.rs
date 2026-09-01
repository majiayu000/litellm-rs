use super::*;
use crate::config::models::gateway::GatewayConfig;
use crate::core::guardrails::{GuardrailAction, GuardrailEngine, PIIConfig};
use crate::core::models::openai::{ChatChoice, ContentPart};

fn response(content: MessageContent) -> crate::core::models::openai::ChatCompletionResponse {
    crate::core::models::openai::ChatCompletionResponse {
        id: "chatcmpl-test".to_string(),
        object: "chat.completion".to_string(),
        created: 0,
        model: "test-model".to_string(),
        system_fingerprint: None,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: MessageRole::Assistant,
                content: Some(content),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            logprobs: None,
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    }
}

#[test]
fn guarded_prompt_extracts_text_parts_without_original_fallback() {
    let request = ChatCompletionRequest {
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "first".to_string(),
                },
                ContentPart::ImageUrl {
                    image_url: crate::core::models::openai::ImageUrl {
                        url: "https://example.com/image.png".to_string(),
                        detail: None,
                    },
                },
                ContentPart::Text {
                    text: "second".to_string(),
                },
            ])),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        ..ChatCompletionRequest::default()
    };

    assert_eq!(
        guarded_completion_prompt(&request).expect("text parts should project"),
        "first\nsecond"
    );
}

#[tokio::test]
async fn echo_is_masked_by_the_output_policy_after_transformation() {
    let mut config = GatewayConfig::default().guardrails;
    config.check_input = false;
    config.pii = Some(PIIConfig {
        enabled: true,
        action: GuardrailAction::Mask,
        mask_pattern: Some("[MASKED]".to_string()),
        ..PIIConfig::default()
    });
    let engine = GuardrailEngine::new(config).expect("PII policy must compile");
    let echoed = chat_response_with_completion_echo(
        response(MessageContent::Text("safe response".to_string())),
        "user@example.com ",
    );

    let guarded = crate::server::guardrails::apply_output_with_engine(&engine, &echoed)
        .await
        .expect("echoed response should be masked");

    assert_eq!(
        completion_text_from_message(guarded.choices[0].message.content.as_ref(), "", false),
        "[MASKED] safe response"
    );
}
