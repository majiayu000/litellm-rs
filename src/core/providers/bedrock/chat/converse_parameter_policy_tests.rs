use super::*;
use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

#[test]
fn bedrock_parameter_policy_rejects_opus_47_temperature() {
    let request = ChatRequest {
        model: "anthropic.claude-opus-4-7".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        temperature: Some(0.7),
        ..Default::default()
    };

    let error = transform_to_converse(&request)
        .expect_err("Claude Opus 4.7 should reject non-default temperature locally");
    let message = error.to_string();
    assert!(message.contains("claude-opus-4-7"));
    assert!(message.contains("temperature"));
}

#[test]
fn bedrock_parameter_policy_serializes_top_k_as_additional_model_field() {
    let mut request = ChatRequest {
        model: "anthropic.claude-3-sonnet".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    };
    request
        .extra_params
        .insert("top_k".to_string(), serde_json::json!(128));

    let converse = transform_to_converse(&request)
        .unwrap_or_else(|err| panic!("top_k should transform: {err}"));
    let fields = converse
        .additional_model_request_fields
        .unwrap_or_else(|| panic!("top_k should be emitted as additionalModelRequestFields"));

    assert_eq!(fields["top_k"], 128);
}

#[test]
fn bedrock_parameter_policy_preserves_custom_additional_model_fields() {
    let mut request = ChatRequest::new("anthropic.claude-3-sonnet").add_user_message("Hello");
    request.extra_params.insert(
        "additionalModelRequestFields".to_string(),
        serde_json::json!({
            "topK": 96,
            "vendorField": {
                "mode": "passthrough"
            }
        }),
    );

    let converse = transform_to_converse(&request).unwrap_or_else(|err| {
        panic!("custom additionalModelRequestFields should transform: {err}")
    });
    let fields = converse
        .additional_model_request_fields
        .unwrap_or_else(|| panic!("additionalModelRequestFields should be emitted"));

    assert_eq!(fields["top_k"], 96);
    assert_eq!(
        fields["vendorField"],
        serde_json::json!({
            "mode": "passthrough"
        })
    );
    assert!(fields.get("topK").is_none());
}

#[test]
fn bedrock_parameter_policy_allows_opus_47_custom_additional_model_fields() {
    let mut request = ChatRequest::new("anthropic.claude-opus-4-7").add_user_message("Hello");
    request.extra_params.insert(
        "additionalModelRequestFields".to_string(),
        serde_json::json!({
            "vendorField": {
                "mode": "passthrough"
            }
        }),
    );

    let converse = transform_to_converse(&request).unwrap_or_else(|err| {
        panic!("custom additionalModelRequestFields should transform: {err}")
    });
    let fields = converse
        .additional_model_request_fields
        .unwrap_or_else(|| panic!("additionalModelRequestFields should be emitted"));

    assert_eq!(
        fields["vendorField"],
        serde_json::json!({
            "mode": "passthrough"
        })
    );
}

#[test]
fn bedrock_parameter_policy_serializes_opus_47_adaptive_thinking() {
    let request = ChatRequest::new("anthropic.claude-opus-4-7")
        .add_user_message("Think carefully")
        .with_thinking(
            ThinkingConfig::new()
                .enabled()
                .with_effort(ThinkingEffort::Low),
        );

    let converse = transform_to_converse(&request)
        .unwrap_or_else(|err| panic!("adaptive thinking should transform: {err}"));
    let fields = converse
        .additional_model_request_fields
        .unwrap_or_else(|| panic!("thinking should be emitted as additionalModelRequestFields"));

    assert_eq!(
        fields["thinking"],
        serde_json::json!({
            "type": "adaptive",
            "effort": "low"
        })
    );
}

#[test]
fn bedrock_parameter_policy_rejects_opus_47_budget_tokens() {
    let request = ChatRequest::new("anthropic.claude-opus-4-7")
        .add_user_message("Think carefully")
        .with_thinking(ThinkingConfig::new().enabled().with_budget(32_000));

    let error = transform_to_converse(&request)
        .expect_err("Claude Opus 4.7 should reject fixed thinking budgets locally");
    let message = error.to_string();
    assert!(message.contains("claude-opus-4-7"));
    assert!(message.contains("budget_tokens"));
}

#[test]
fn bedrock_parameter_policy_rejects_raw_extra_params_thinking() {
    let mut request = ChatRequest::new("anthropic.claude-opus-4-7").add_user_message("Hello");
    request.extra_params.insert(
        "thinking".to_string(),
        serde_json::json!({"type": "adaptive"}),
    );

    let error = transform_to_converse(&request)
        .expect_err("raw Bedrock thinking should use ChatRequest.thinking");
    assert!(error.to_string().contains("extra_params.thinking"));
}
