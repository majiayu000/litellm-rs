use super::{AnthropicConfig, AnthropicProvider};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    context::RequestContext,
    message::{MessageContent, MessageRole},
    thinking::{ThinkingConfig, ThinkingContent},
    tools::{FunctionCall, FunctionDefinition, Tool, ToolChoice, ToolType},
};

fn compatible_provider() -> AnthropicProvider {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
        .with_allow_unknown_models(true)
        .with_configured_models(vec!["mimo-v2.5".to_string()]);
    AnthropicProvider::new(config).unwrap_or_else(|err| panic!("provider should build: {err}"))
}

#[tokio::test]
async fn public_transform_uses_native_anthropic_serialization() {
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"));
    let request = ChatRequest::new("claude-sonnet-4-6")
        .add_system_message("Answer briefly")
        .add_user_message("Explain the result")
        .with_max_tokens(2_048)
        .with_thinking(ThinkingConfig::new().enabled().with_budget(1_024));

    let transformed = provider
        .transform_request(request, RequestContext::new())
        .await
        .unwrap_or_else(|err| panic!("public transform should accept the request: {err}"));

    assert_eq!(transformed["system"], "Answer briefly");
    assert_eq!(transformed["messages"].as_array().map(Vec::len), Some(1));
    assert_eq!(transformed["thinking"]["type"], "enabled");
    assert_eq!(transformed["thinking"]["budget_tokens"], 1_024);
}

#[tokio::test]
async fn public_transform_allows_forced_tools_with_adaptive_thinking() {
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"));
    let mut request = ChatRequest::new("claude-opus-5").add_user_message("Use lookup");
    request.reasoning_effort = Some("high".to_string());
    request.tools = Some(vec![Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "lookup".to_string(),
            description: None,
            parameters: Some(serde_json::json!({"type": "object"})),
        },
    }]);
    request.tool_choice = Some(ToolChoice::String("required".to_string()));

    let transformed = provider
        .transform_request(request, RequestContext::new())
        .await
        .unwrap_or_else(|err| panic!("adaptive thinking should allow forced tools: {err}"));

    assert_eq!(transformed["thinking"]["type"], "adaptive");
    assert_eq!(transformed["output_config"]["effort"], "high");
    assert_eq!(transformed["tool_choice"]["type"], "any");
}

#[tokio::test]
async fn compatible_models_allow_empty_tools_without_forwarding_tools() {
    let provider = compatible_provider();
    let mut request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");
    request.tools = Some(vec![]);

    let transformed = provider
        .transform_request(request, RequestContext::new())
        .await
        .unwrap_or_else(|err| panic!("empty tools should not declare tool support: {err}"));

    assert!(transformed.get("tools").is_none());
}

#[tokio::test]
async fn compatible_models_reject_legacy_functions() {
    let provider = compatible_provider();
    let mut request = ChatRequest::new("mimo-v2.5").add_user_message("Hello");
    request.functions = Some(vec![serde_json::json!({"name": "lookup"})]);

    let err = match provider
        .transform_request(request, RequestContext::new())
        .await
    {
        Ok(_) => panic!("compatible models must reject legacy function definitions"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("tool calling support"));
}

#[tokio::test]
async fn compatible_models_reject_tool_role_messages() {
    let provider = compatible_provider();
    let request = ChatRequest::new("mimo-v2.5").add_message(
        MessageRole::Tool,
        MessageContent::Text("tool result".to_string()),
    );

    let err = match provider
        .transform_request(request, RequestContext::new())
        .await
    {
        Ok(_) => panic!("compatible models must reject tool-role messages"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("only supports text and image content"));
}

#[tokio::test]
async fn compatible_models_reject_message_function_calls() {
    let provider = compatible_provider();
    let mut request = ChatRequest::new("mimo-v2.5");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("calling lookup".to_string())),
        function_call: Some(FunctionCall {
            name: "lookup".to_string(),
            arguments: "{}".to_string(),
        }),
        ..Default::default()
    });

    let err = match provider
        .transform_request(request, RequestContext::new())
        .await
    {
        Ok(_) => panic!("compatible models must reject message-level function calls"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("only supports text and image content"));
}

#[tokio::test]
async fn compatible_models_reject_message_thinking() {
    let provider = compatible_provider();
    let mut request = ChatRequest::new("mimo-v2.5");
    request.messages.push(ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text("answer".to_string())),
        thinking: Some(ThinkingContent::text("hidden reasoning")),
        ..Default::default()
    });

    let err = match provider
        .transform_request(request, RequestContext::new())
        .await
    {
        Ok(_) => panic!("compatible models must reject message-level thinking"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("only supports text and image content"));
}
