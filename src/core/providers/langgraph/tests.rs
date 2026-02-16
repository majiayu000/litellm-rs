//! LangGraph Provider Tests

use super::models::{
    CreateThreadRequest, RunGraphRequest, RunResponse, RunStatus, get_model_registry,
};
use super::*;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::ProviderConfig;

// ==================== Config Tests ====================

#[test]
fn test_config_default() {
    let config = LangGraphConfig::default();
    assert!(config.base.api_key.is_none());
    assert_eq!(
        config.base.api_base,
        Some("https://api.smith.langchain.com".to_string())
    );
    assert_eq!(config.base.timeout, 120);
    assert!(config.enable_checkpointing);
    assert_eq!(config.max_iterations, 25);
}

#[test]
fn test_config_with_api_key() {
    let config = LangGraphConfig::with_api_key("lsv2_test_key_123");
    assert_eq!(config.base.api_key, Some("lsv2_test_key_123".to_string()));
}

#[test]
fn test_config_builder_chain() {
    let config = LangGraphConfig::with_api_key("test-key")
        .with_graph_id("my-graph-id")
        .with_assistant_id("asst-123")
        .with_api_base("https://custom.langchain.com");

    assert_eq!(config.base.api_key, Some("test-key".to_string()));
    assert_eq!(config.graph_id, Some("my-graph-id".to_string()));
    assert_eq!(config.assistant_id, Some("asst-123".to_string()));
    assert_eq!(
        config.base.api_base,
        Some("https://custom.langchain.com".to_string())
    );
}

#[test]
fn test_config_validation_missing_api_key() {
    let config = LangGraphConfig::default();
    let result = config.validate();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("API key"));
}

#[test]
fn test_config_validation_success() {
    let config = LangGraphConfig::with_api_key("lsv2_valid_key");
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_zero_timeout() {
    let mut config = LangGraphConfig::with_api_key("test-key");
    config.base.timeout = 0;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Timeout"));
}

#[test]
fn test_config_validation_max_retries() {
    let mut config = LangGraphConfig::with_api_key("test-key");
    config.base.max_retries = 15;
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("retries"));
}

#[test]
fn test_provider_config_trait() {
    let config = LangGraphConfig::with_api_key("test-key");
    assert_eq!(config.api_key(), Some("test-key"));
    assert_eq!(config.api_base(), Some("https://api.smith.langchain.com"));
    assert_eq!(config.timeout(), std::time::Duration::from_secs(120));
    assert_eq!(config.max_retries(), 3);
}

// ==================== Error Mapper Tests ====================

#[test]
fn test_error_mapper_authentication_401() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(401, "Invalid credentials");
    assert!(matches!(err, ProviderError::Authentication { .. }));
}

#[test]
fn test_error_mapper_forbidden_403() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(403, "Access denied");
    assert!(matches!(err, ProviderError::Authentication { .. }));
}

#[test]
fn test_error_mapper_not_found_graph_404() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(404, r#"{"detail": "graph not found"}"#);
    assert!(matches!(err, ProviderError::ModelNotFound { .. }));
}

#[test]
fn test_error_mapper_not_found_thread_404() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(404, r#"{"detail": "thread not found"}"#);
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

#[test]
fn test_error_mapper_rate_limit_429() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(429, "Rate limit exceeded");
    assert!(matches!(err, ProviderError::RateLimit { .. }));
}

#[test]
fn test_error_mapper_validation_422() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(422, r#"{"detail": "Invalid input format"}"#);
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

#[test]
fn test_error_mapper_server_error_500() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(500, "Internal server error");
    assert!(matches!(err, ProviderError::ProviderUnavailable { .. }));
}

#[test]
fn test_error_mapper_conflict_409() {
    let mapper = LangGraphErrorMapper;
    let err = mapper.map_http_error(409, r#"{"detail": "Resource modified"}"#);
    assert!(matches!(err, ProviderError::InvalidRequest { .. }));
}

// ==================== Model Tests ====================

#[test]
fn test_get_langgraph_models() {
    let models = get_langgraph_models();
    assert!(!models.is_empty());

    // All models should be LangGraph models
    for model in &models {
        assert!(model.id.starts_with("langgraph/"));
        assert_eq!(model.provider, "langgraph");
        assert!(model.supports_streaming);
        assert!(model.supports_tools);
    }
}

#[test]
fn test_model_registry_contains_agent() {
    let models = get_model_registry();
    let agent = models.iter().find(|m| m.id == "langgraph/agent");
    assert!(agent.is_some());
}

#[test]
fn test_model_registry_contains_react() {
    let models = get_model_registry();
    let react = models.iter().find(|m| m.id == "langgraph/react");
    assert!(react.is_some());
}

#[test]
fn test_model_registry_contains_rag() {
    let models = get_model_registry();
    let rag = models.iter().find(|m| m.id == "langgraph/rag");
    assert!(rag.is_some());
}

#[test]
fn test_model_registry_contains_supervisor() {
    let models = get_model_registry();
    let supervisor = models.iter().find(|m| m.id == "langgraph/supervisor");
    assert!(supervisor.is_some());
}

#[test]
fn test_model_registry_contains_custom() {
    let models = get_model_registry();
    let custom = models.iter().find(|m| m.id == "langgraph/custom");
    assert!(custom.is_some());
}

// ==================== Type Tests ====================

#[test]
fn test_run_status_display() {
    assert_eq!(format!("{}", RunStatus::Pending), "pending");
    assert_eq!(format!("{}", RunStatus::Running), "running");
    assert_eq!(format!("{}", RunStatus::Success), "success");
    assert_eq!(format!("{}", RunStatus::Error), "error");
    assert_eq!(format!("{}", RunStatus::Interrupted), "interrupted");
    assert_eq!(format!("{}", RunStatus::Timeout), "timeout");
}

#[test]
fn test_run_status_equality() {
    assert_eq!(RunStatus::Success, RunStatus::Success);
    assert_ne!(RunStatus::Success, RunStatus::Error);
    assert_ne!(RunStatus::Pending, RunStatus::Running);
}

#[test]
fn test_thread_state_default() {
    let state = ThreadState::default();
    assert!(state.thread_id.is_empty());
    assert!(state.checkpoint_id.is_none());
    assert!(state.values.is_empty());
    assert!(state.metadata.is_empty());
}

#[test]
fn test_graph_info_serialization() {
    let info = GraphInfo {
        graph_id: "test-graph".to_string(),
        name: "Test Graph".to_string(),
        description: Some("A test graph for unit tests".to_string()),
        version: Some("1.0.0".to_string()),
        config_schema: None,
        input_schema: None,
        output_schema: None,
    };

    let json = serde_json::to_string(&info).unwrap();
    assert!(json.contains("test-graph"));
    assert!(json.contains("Test Graph"));
    assert!(json.contains("1.0.0"));
}

#[test]
fn test_create_thread_request_serialization() {
    let req = CreateThreadRequest { metadata: None };
    let json = serde_json::to_string(&req).unwrap();
    // Empty metadata should result in minimal JSON
    assert!(!json.contains("metadata") || json.contains("null"));
}

#[test]
fn test_run_graph_request_serialization() {
    let req = RunGraphRequest {
        assistant_id: "asst-test-123".to_string(),
        input: serde_json::json!({
            "messages": [{"role": "user", "content": "Hello"}]
        }),
        config: None,
        metadata: None,
        stream_mode: Some(vec!["values".to_string()]),
        interrupt_before: None,
        interrupt_after: None,
    };

    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("asst-test-123"));
    assert!(json.contains("messages"));
    assert!(json.contains("values"));
}

#[test]
fn test_run_response_deserialization() {
    let json = r#"{
        "run_id": "run-123",
        "thread_id": "thread-456",
        "assistant_id": "asst-789",
        "status": "success",
        "output": {"messages": [{"role": "assistant", "content": "Hello!"}]}
    }"#;

    let response: RunResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.run_id, "run-123");
    assert_eq!(response.thread_id, "thread-456");
    assert_eq!(response.status, RunStatus::Success);
    assert!(response.output.is_some());
}

// ==================== Provider Creation Tests ====================

#[test]
fn test_provider_creation_without_api_key() {
    let config = LangGraphConfig::default();
    let result = LangGraphProvider::new(config);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e, ProviderError::Configuration { .. }));
    }
}

#[test]
fn test_provider_name() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    assert_eq!(provider.name(), "langgraph");
}

#[test]
fn test_provider_capabilities() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::model::ProviderCapability;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
}

#[test]
fn test_provider_models() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    let models = provider.models();
    assert!(!models.is_empty());
    assert!(models.iter().all(|m| m.provider == "langgraph"));
}

#[test]
fn test_provider_supports_model() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    assert!(provider.supports_model("langgraph/agent"));
    assert!(provider.supports_model("langgraph/react"));
    assert!(!provider.supports_model("nonexistent-model"));
}

#[test]
fn test_provider_supported_params() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    let params = provider.get_supported_openai_params("langgraph/agent");
    assert!(params.contains(&"messages"));
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"tools"));
}

#[test]
fn test_provider_get_error_mapper() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();
    let _mapper = provider.get_error_mapper();
    // Just verify we can get the mapper
}

// ==================== Transform Tests ====================

#[test]
fn test_transform_chat_request_to_langgraph() {
    use crate::core::types::chat::{ChatMessage, ChatRequest};
    use crate::core::types::message::{MessageContent, MessageRole};

    let config = LangGraphConfig::with_api_key("test-key");
    let provider = LangGraphProvider::new(config).unwrap();

    let request = ChatRequest {
        model: "langgraph/agent".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello, how are you?".to_string())),
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        }],
        ..Default::default()
    };

    let input = provider.transform_chat_to_langgraph_input(&request);

    assert!(input.get("messages").is_some());
    let messages = input.get("messages").unwrap().as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "Hello, how are you?");
}
