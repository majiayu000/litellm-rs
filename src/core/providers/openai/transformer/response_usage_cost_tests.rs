use super::*;
use crate::core::providers::shared::normalize_openai_chat_response_usage;
use serde_json::json;

#[test]
fn test_non_numeric_usage_cost_keeps_token_usage() {
    // Perplexity-style cost object, and a number sent as a string.
    for cost in [json!({"total_cost": 0.005}), json!("0.005")] {
        let mut wire = json!({
            "id":"gen-1","object":"chat.completion","created":1,"model":"sonar",
            "choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],
            "usage":{"prompt_tokens":1000,"completion_tokens":500,"total_tokens":1500,"cost":cost}
        });
        normalize_openai_chat_response_usage(&mut wire);
        let response: OpenAIChatResponse = serde_json::from_value(wire).unwrap();
        let usage = OpenAIResponseTransformer::transform(response)
            .unwrap()
            .usage
            .expect("token usage must survive a non-numeric cost");
        assert_eq!((usage.prompt_tokens, usage.completion_tokens), (1000, 500));
        assert_eq!(usage.cost, None);
    }
}
