use super::{AnthropicConfig, AnthropicProvider};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    context::RequestContext,
    message::{MessageContent, MessageRole},
    thinking::{ThinkingConfig, ThinkingContent},
    tools::FunctionCall,
};

fn first_party_provider() -> AnthropicProvider {
    AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"))
}

fn compatible_provider() -> AnthropicProvider {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
        .with_allow_unknown_models(true)
        .with_configured_models(vec!["mimo-v2.5".to_string()]);
    AnthropicProvider::new(config).unwrap_or_else(|err| panic!("provider should build: {err}"))
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

#[test]
fn current_claude_5_supported_params_exclude_top_k() {
    let provider = first_party_provider();

    for model in ["claude-fable-5", "claude-opus-5", "claude-sonnet-5"] {
        assert!(
            !provider
                .get_supported_openai_params(model)
                .contains(&"top_k")
        );
    }
    assert!(
        provider
            .get_supported_openai_params("claude-3-opus-20240229")
            .contains(&"top_k")
    );
}

#[tokio::test]
async fn current_claude_5_public_transform_uses_client_contract() {
    let provider = first_party_provider();
    let mut request = ChatRequest::new("claude-opus-5").add_user_message("Solve this");
    request.thinking = Some(ThinkingConfig::medium_effort());

    let transformed = provider
        .transform_request(request, RequestContext::new())
        .await
        .expect("public transformer should accept adaptive thinking");

    assert_eq!(transformed["thinking"]["type"], "adaptive");
    assert_eq!(transformed["output_config"]["effort"], "medium");

    let mut top_k = ChatRequest::new("claude-opus-5").add_user_message("Hello");
    top_k
        .extra_params
        .insert("top_k".to_string(), serde_json::json!(1));
    let error = provider
        .transform_request(top_k, RequestContext::new())
        .await
        .expect_err("public transformer must share Claude 5 validation");
    assert!(error.to_string().contains("top_k"));
}
