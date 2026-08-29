use super::{AnthropicConfig, AnthropicProvider};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    context::RequestContext,
    message::{MessageContent, MessageRole},
    thinking::ThinkingContent,
    tools::FunctionCall,
};

fn compatible_provider() -> AnthropicProvider {
    let config = AnthropicConfig::new_test("test-key")
        .with_base_url("https://token-plan-sgp.xiaomimimo.com/anthropic")
        .with_allow_unknown_models(true)
        .with_configured_models(vec!["mimo-v2.5".to_string()]);
    AnthropicProvider::new(config).unwrap_or_else(|err| panic!("provider should build: {err}"))
}

#[test]
fn only_priced_standalone_claude_5_is_published_as_supported() {
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"));

    let published = provider.models();
    let fable = published
        .iter()
        .find(|model| model.id == "claude-fable-5")
        .expect("the exact priced standalone Fable ID must be listed for gateway construction");
    assert_eq!(fable.provider, "anthropic");
    assert_eq!(fable.max_context_length, 1_000_000);
    assert_eq!(fable.max_output_length, Some(128_000));
    assert!(fable.supports_streaming);
    assert!(fable.supports_tools);
    assert!(fable.supports_multimodal);
    assert_eq!(fable.input_cost_per_1k_tokens, None);
    assert_eq!(fable.output_cost_per_1k_tokens, None);

    assert!(provider.supports_model("claude-fable-5"));
    for unsupported in [
        "claude-opus-5",
        "claude-sonnet-5",
        "Claude-fable-5",
        "claude-fable-5-latest",
    ] {
        assert!(!provider.supports_model(unsupported));
        assert!(published.iter().all(|model| model.id != unsupported));
    }
}

#[test]
fn fable_supported_params_match_sampling_validation_exactly() {
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"));

    let fable = provider.get_supported_openai_params("claude-fable-5");
    assert!(!fable.contains(&"temperature"));
    assert!(!fable.contains(&"top_p"));
    assert!(fable.contains(&"max_tokens"));
    assert!(fable.contains(&"tools"));

    for model in ["claude-3-opus-20240229", "Claude-fable-5"] {
        let params = provider.get_supported_openai_params(model);
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"top_p"));
    }
}

#[tokio::test]
async fn unregistered_claude_5_models_fail_before_unpriced_budget_reservation() {
    let provider = AnthropicProvider::new(AnthropicConfig::new_test("test-key"))
        .unwrap_or_else(|err| panic!("provider should build: {err}"));

    for model in ["claude-opus-5", "claude-sonnet-5"] {
        let error = provider
            .transform_request(
                ChatRequest::new(model).add_user_message("Hello"),
                RequestContext::new(),
            )
            .await
            .expect_err("catalog-less Claude 5 models must fail at provider validation");
        let message = error.to_string();
        assert!(message.contains(&format!("Unsupported model: {model}")));
        assert!(!message.contains("model_not_priced"));
    }
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
