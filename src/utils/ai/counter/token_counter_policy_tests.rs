use super::TokenCounter;
use crate::utils::error::gateway_error::GatewayError;

#[test]
fn test_explicit_openai_unknown_model_cannot_fall_back_to_estimation() {
    let counter = TokenCounter::new();
    let error = counter
        .count_completion_tokens("openai/gpt-future-unknown", "Hello")
        .expect_err("explicit unknown OpenAI IDs must not be approximated");

    match error {
        GatewayError::Config(message) => {
            assert!(message.contains("unsupported explicit OpenAI model"));
        }
        other => panic!("expected a clear configuration error, got {other}"),
    }
}

#[test]
fn test_azure_openai_exact_counting_uses_catalog_identity() {
    let counter = TokenCounter::new();

    for model in ["azure/gpt-4", "azure_ai/gpt-4"] {
        let estimate = counter
            .count_completion_tokens(model, "Hello")
            .expect("catalogued Azure-hosted OpenAI model should count");
        assert!(
            !estimate.is_approximate,
            "{model} used approximate counting"
        );
    }
}
