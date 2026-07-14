//! OpenAI Provider Unit Tests
//!
//! Comprehensive tests for the OpenAI provider implementation.

use super::*;
use crate::core::providers::base::GlobalPoolManager;
use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::context::RequestContext;
use crate::core::types::model::ProviderCapability;
use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, message::MessageContent, message::MessageRole,
};
use std::collections::HashMap;
use std::sync::Arc;

fn create_test_config() -> OpenAIConfig {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    config
}

fn create_test_provider() -> OpenAIProvider {
    OpenAIProvider {
        pool_manager: Arc::new(GlobalPoolManager::shared()),
        config: create_test_config(),
        model_registry: get_openai_registry(),
    }
}

fn chat_request_with_typed_params(extra_key: &str) -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        frequency_penalty: Some(0.2),
        presence_penalty: Some(0.4),
        logit_bias: Some(HashMap::from([("50256".to_string(), -1.5)])),
        logprobs: Some(true),
        top_logprobs: Some(3),
        reasoning_effort: Some("medium".to_string()),
        store: Some(true),
        metadata: Some(HashMap::from([(
            "trace_id".to_string(),
            "trace-123".to_string(),
        )])),
        service_tier: Some("flex".to_string()),
        parallel_tool_calls: Some(false),
        extra_params: HashMap::from([
            (extra_key.to_string(), serde_json::json!("kept")),
            ("model".to_string(), serde_json::json!("wrong-model")),
            ("messages".to_string(), serde_json::json!("wrong-messages")),
            ("frequency_penalty".to_string(), serde_json::json!(1.9)),
        ]),
        ..Default::default()
    }
}

fn assert_typed_params_forwarded(json: &serde_json::Value, extra_key: &str) {
    assert_eq!(json["frequency_penalty"], serde_json::json!(0.2_f32));
    assert_eq!(json["presence_penalty"], serde_json::json!(0.4_f32));
    assert_eq!(json["logit_bias"], serde_json::json!({ "50256": -1.5_f32 }));
    assert_eq!(json["logprobs"], true);
    assert_eq!(json["top_logprobs"], 3);
    assert_eq!(json["reasoning_effort"], "medium");
    assert_eq!(json["store"], true);
    assert_eq!(json["metadata"]["trace_id"], "trace-123");
    assert_eq!(json["service_tier"], "flex");
    assert_eq!(json["parallel_tool_calls"], false);
    assert_eq!(json[extra_key], "kept");
    assert_eq!(json["model"], "gpt-4");
    assert!(json["messages"].is_array());
}

mod convenience_tests;
mod error_header_tests;
mod provider_support_tests;
mod request_transform_tests;
