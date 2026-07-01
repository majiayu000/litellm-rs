use super::*;

// ==================== ResponseTransformConfig Tests ====================

#[test]
fn test_response_transform_config_default() {
    let config = ResponseTransformConfig::default();

    assert!(!config.strip_azure_metadata);
    assert!(config.normalize_field_names);
    assert!(config.include_content_filters);
    assert!(config.field_mappings.is_empty());
    assert!(matches!(
        config.response_format,
        ResponseFormat::OpenAICompatible
    ));
}

#[test]
fn test_response_transform_config_custom() {
    let mut mappings = HashMap::new();
    mappings.insert("old_field".to_string(), "new_field".to_string());

    let config = ResponseTransformConfig {
        strip_azure_metadata: true,
        normalize_field_names: false,
        include_content_filters: false,
        field_mappings: mappings.clone(),
        response_format: ResponseFormat::Minimal,
    };

    assert!(config.strip_azure_metadata);
    assert!(!config.normalize_field_names);
    assert!(!config.include_content_filters);
    assert_eq!(config.field_mappings.len(), 1);
}

#[test]
fn test_response_transform_config_clone() {
    let config = ResponseTransformConfig::default();
    let cloned = config.clone();

    assert_eq!(config.strip_azure_metadata, cloned.strip_azure_metadata);
    assert_eq!(config.normalize_field_names, cloned.normalize_field_names);
}

#[test]
fn test_response_transform_config_debug() {
    let config = ResponseTransformConfig::default();
    let debug = format!("{:?}", config);
    assert!(debug.contains("ResponseTransformConfig"));
}

// ==================== ResponseFormat Tests ====================

#[test]
fn test_response_format_variants() {
    let native = ResponseFormat::Native;
    let openai = ResponseFormat::OpenAICompatible;
    let minimal = ResponseFormat::Minimal;

    assert!(matches!(native, ResponseFormat::Native));
    assert!(matches!(openai, ResponseFormat::OpenAICompatible));
    assert!(matches!(minimal, ResponseFormat::Minimal));
}

#[test]
fn test_response_format_clone() {
    let format = ResponseFormat::OpenAICompatible;
    let cloned = format;
    assert!(matches!(cloned, ResponseFormat::OpenAICompatible));
}

#[test]
fn test_response_format_debug() {
    let format = ResponseFormat::Native;
    let debug = format!("{:?}", format);
    assert!(debug.contains("Native"));
}

// ==================== AzureResponseTransformation Creation Tests ====================

#[test]
fn test_azure_response_transformation_new() {
    let transformation = AzureResponseTransformation::new();
    assert!(transformation.config.normalize_field_names);
}

#[test]
fn test_azure_response_transformation_default() {
    let transformation = AzureResponseTransformation::default();
    assert!(transformation.config.include_content_filters);
}

#[test]
fn test_azure_response_transformation_with_config() {
    let config = ResponseTransformConfig {
        strip_azure_metadata: true,
        ..Default::default()
    };

    let transformation = AzureResponseTransformation::with_config(config);
    assert!(transformation.config.strip_azure_metadata);
}

// ==================== Transform Response Tests ====================

#[test]
fn test_transform_response_native() {
    let config = ResponseTransformConfig {
        response_format: ResponseFormat::Native,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "choices": [{"message": {"content": "Hello"}}],
        "azure_specific_field": "value"
    });

    let result = transformation.transform_response(response).unwrap();
    assert!(result.get("azure_specific_field").is_some());
}

#[test]
fn test_transform_response_openai_compatible() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{"message": {"content": "Hello"}}],
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    let result = transformation.transform_response(response).unwrap();
    assert!(result.get("choices").is_some());
    // Should normalize input_tokens to prompt_tokens
    assert!(result.get("usage").unwrap().get("prompt_tokens").is_some());
}

#[test]
fn test_transform_response_minimal() {
    let config = ResponseTransformConfig {
        response_format: ResponseFormat::Minimal,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "id": "123",
        "model": "gpt-4",
        "choices": [{"message": {"content": "Hello"}}],
        "usage": {"total_tokens": 15, "prompt_tokens": 10, "completion_tokens": 5},
        "extra_field": "ignored"
    });

    let result = transformation.transform_response(response).unwrap();
    assert!(result.get("choices").is_some());
    assert!(result.get("usage").is_some());
    assert!(result.get("id").is_none()); // Should be stripped
    assert!(result.get("extra_field").is_none()); // Should be stripped
}

// ==================== Transform Chat Response Tests ====================

#[test]
fn test_transform_chat_response_basic() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "message": {"content": "Hello", "role": "assistant"},
            "finish_reason": "stop"
        }],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5}
    });

    let result = transformation.transform_chat_response(response).unwrap();
    assert!(result.get("choices").is_some());
}

#[test]
fn test_transform_chat_response_with_function_call() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "function_call": {"name": "get_weather", "arguments": "{}"}
            },
            "finish_reason": "function_call"
        }]
    });

    let result = transformation.transform_chat_response(response).unwrap();
    assert!(result.get("choices").is_some());
}

#[test]
fn test_transform_chat_response_with_tool_calls() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "message": {
                "tool_calls": [{"id": "call_1", "function": {"name": "test"}}]
            },
            "finish_reason": "tool_calls"
        }]
    });

    let result = transformation.transform_chat_response(response).unwrap();
    assert!(result.get("choices").is_some());
}

#[test]
fn test_transform_chat_response_with_delta() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "delta": {"content": "Hello"},
            "finish_reason": null
        }]
    });

    let result = transformation.transform_chat_response(response).unwrap();
    let choice = &result.get("choices").unwrap().as_array().unwrap()[0];
    assert!(choice.get("delta").is_some());
}

#[test]
fn test_transform_chat_response_removes_content_filters() {
    let config = ResponseTransformConfig {
        include_content_filters: false,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "choices": [{
            "message": {"content": "Hello"},
            "content_filter_results": {"hate": {"filtered": false}}
        }],
        "prompt_filter_results": []
    });

    let result = transformation.transform_chat_response(response).unwrap();
    let choice = &result.get("choices").unwrap().as_array().unwrap()[0];
    assert!(choice.get("content_filter_results").is_none());
    assert!(result.get("prompt_filter_results").is_none());
}

// ==================== Transform Completion Response Tests ====================

#[test]
fn test_transform_completion_response_basic() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "text": "This is a completion",
            "finish_reason": "stop"
        }]
    });

    let result = transformation
        .transform_completion_response(response)
        .unwrap();
    assert!(result.get("choices").is_some());
}

#[test]
fn test_transform_completion_response_max_tokens() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "choices": [{
            "text": "Truncated...",
            "finish_reason": "max_tokens"
        }]
    });

    let result = transformation
        .transform_completion_response(response)
        .unwrap();
    let choice = &result.get("choices").unwrap().as_array().unwrap()[0];
    // Should normalize max_tokens to length
    assert_eq!(
        choice.get("finish_reason").unwrap().as_str().unwrap(),
        "length"
    );
}

// ==================== Transform Embedding Response Tests ====================

#[test]
fn test_transform_embedding_response_basic() {
    let transformation = AzureResponseTransformation::new();
    let response = serde_json::json!({
        "data": [{"embedding": [0.1, 0.2, 0.3], "index": 0}],
        "model": "text-embedding-ada-002",
        "usage": {"prompt_tokens": 5, "total_tokens": 5}
    });

    let result = transformation
        .transform_embedding_response(response)
        .unwrap();
    assert!(result.get("data").is_some());
    assert!(result.get("usage").is_some());
}

#[test]
fn test_transform_embedding_response_removes_filters() {
    let config = ResponseTransformConfig {
        include_content_filters: false,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "data": [{"embedding": [0.1, 0.2]}],
        "content_filter_results": {}
    });

    let result = transformation
        .transform_embedding_response(response)
        .unwrap();
    assert!(result.get("content_filter_results").is_none());
}

// ==================== Normalize Finish Reason Tests ====================

#[test]
fn test_normalize_finish_reason_max_tokens() {
    let transformation = AzureResponseTransformation::new();
    let mut finish_reason = serde_json::json!("max_tokens");
    transformation
        .normalize_finish_reason(&mut finish_reason)
        .unwrap();
    assert_eq!(finish_reason.as_str().unwrap(), "length");
}

#[test]
fn test_normalize_finish_reason_stop_sequence() {
    let transformation = AzureResponseTransformation::new();
    let mut finish_reason = serde_json::json!("stop_sequence");
    transformation
        .normalize_finish_reason(&mut finish_reason)
        .unwrap();
    assert_eq!(finish_reason.as_str().unwrap(), "stop");
}

#[test]
fn test_normalize_finish_reason_content_filter() {
    let transformation = AzureResponseTransformation::new();
    let mut finish_reason = serde_json::json!("content_filter");
    transformation
        .normalize_finish_reason(&mut finish_reason)
        .unwrap();
    assert_eq!(finish_reason.as_str().unwrap(), "content_filter");
}

#[test]
fn test_normalize_finish_reason_unknown() {
    let transformation = AzureResponseTransformation::new();
    let mut finish_reason = serde_json::json!("custom_reason");
    transformation
        .normalize_finish_reason(&mut finish_reason)
        .unwrap();
    assert_eq!(finish_reason.as_str().unwrap(), "custom_reason");
}

#[test]
fn test_normalize_finish_reason_null() {
    let transformation = AzureResponseTransformation::new();
    let mut finish_reason = serde_json::json!(null);
    transformation
        .normalize_finish_reason(&mut finish_reason)
        .unwrap();
    assert!(finish_reason.is_null());
}

// ==================== Usage Transformation Tests ====================

#[test]
fn test_transform_usage_input_to_prompt() {
    let transformation = AzureResponseTransformation::new();
    let mut usage = serde_json::json!({
        "input_tokens": 100,
        "output_tokens": 50,
        "total_tokens": 150
    });

    transformation.transform_usage_object(&mut usage).unwrap();
    assert_eq!(usage.get("prompt_tokens").unwrap().as_u64().unwrap(), 100);
    assert!(usage.get("input_tokens").is_none());
}

#[test]
fn test_transform_usage_output_to_completion() {
    let transformation = AzureResponseTransformation::new();
    let mut usage = serde_json::json!({
        "input_tokens": 100,
        "output_tokens": 50,
        "total_tokens": 150
    });

    transformation.transform_usage_object(&mut usage).unwrap();
    assert_eq!(
        usage.get("completion_tokens").unwrap().as_u64().unwrap(),
        50
    );
    assert!(usage.get("output_tokens").is_none());
}

#[test]
fn test_transform_usage_already_normalized() {
    let transformation = AzureResponseTransformation::new();
    let mut usage = serde_json::json!({
        "prompt_tokens": 100,
        "completion_tokens": 50,
        "total_tokens": 150
    });

    transformation.transform_usage_object(&mut usage).unwrap();
    // Should not change already normalized fields
    assert_eq!(usage.get("prompt_tokens").unwrap().as_u64().unwrap(), 100);
    assert_eq!(
        usage.get("completion_tokens").unwrap().as_u64().unwrap(),
        50
    );
}

// ==================== Field Mapping Tests ====================

#[test]
fn test_apply_field_mappings_basic() {
    let mut mappings = HashMap::new();
    mappings.insert("old_name".to_string(), "new_name".to_string());

    let config = ResponseTransformConfig {
        field_mappings: mappings,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let mut response = serde_json::json!({
        "old_name": "value"
    });

    transformation.apply_field_mappings(&mut response).unwrap();
    assert!(response.get("new_name").is_some());
    assert!(response.get("old_name").is_none());
}

#[test]
fn test_apply_field_mappings_nested() {
    let mut mappings = HashMap::new();
    mappings.insert("old_field".to_string(), "new_field".to_string());

    let config = ResponseTransformConfig {
        field_mappings: mappings,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let mut response = serde_json::json!({
        "nested": {
            "old_field": "value"
        }
    });

    transformation.apply_field_mappings(&mut response).unwrap();
    assert!(response.get("nested").unwrap().get("new_field").is_some());
}

#[test]
fn test_apply_field_mappings_in_array() {
    let mut mappings = HashMap::new();
    mappings.insert("old".to_string(), "new".to_string());

    let config = ResponseTransformConfig {
        field_mappings: mappings,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let mut response = serde_json::json!({
        "items": [
            {"old": "value1"},
            {"old": "value2"}
        ]
    });

    transformation.apply_field_mappings(&mut response).unwrap();
    let items = response.get("items").unwrap().as_array().unwrap();
    assert!(items[0].get("new").is_some());
    assert!(items[1].get("new").is_some());
}

#[test]
fn test_apply_field_mappings_empty() {
    let transformation = AzureResponseTransformation::new();

    let mut response = serde_json::json!({
        "field": "value"
    });

    transformation.apply_field_mappings(&mut response).unwrap();
    assert!(response.get("field").is_some());
}

// ==================== Remove Content Filters Tests ====================

#[test]
fn test_remove_content_filters_root_level() {
    let transformation = AzureResponseTransformation::new();
    let mut response = serde_json::json!({
        "choices": [{"message": {"content": "Hello"}}],
        "content_filter_results": {"hate": {"filtered": false}},
        "prompt_filter_results": []
    });

    transformation.remove_content_filters(&mut response);
    assert!(response.get("content_filter_results").is_none());
    assert!(response.get("prompt_filter_results").is_none());
}

#[test]
fn test_remove_content_filters_nested() {
    let transformation = AzureResponseTransformation::new();
    let mut response = serde_json::json!({
        "choices": [{
            "message": {"content": "Hello"},
            "content_filter_results": {"hate": {"filtered": false}}
        }]
    });

    transformation.remove_content_filters(&mut response);
    let choice = &response.get("choices").unwrap().as_array().unwrap()[0];
    assert!(choice.get("content_filter_results").is_none());
}

// ==================== Strip Azure Fields Tests ====================

#[test]
fn test_strip_azure_fields() {
    let config = ResponseTransformConfig {
        strip_azure_metadata: true,
        response_format: ResponseFormat::OpenAICompatible,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let mut response = serde_json::json!({
        "choices": [{"message": {"content": "Hello"}}],
        "deployment_id": "gpt-4",
        "azure_endpoint": "https://test.openai.azure.com",
        "region": "eastus"
    });

    transformation.strip_azure_fields(&mut response);
    assert!(response.get("deployment_id").is_none());
    assert!(response.get("azure_endpoint").is_none());
    assert!(response.get("region").is_none());
    assert!(response.get("choices").is_some());
}

// ==================== Normalize Fields Tests ====================

#[test]
fn test_normalize_fields() {
    let transformation = AzureResponseTransformation::new();
    let mut response = serde_json::json!({
        "usage": {
            "input_tokens": 100,
            "output_tokens": 50
        }
    });

    transformation.normalize_fields(&mut response).unwrap();
    let usage = response.get("usage").unwrap();
    assert!(usage.get("prompt_tokens").is_some());
    assert!(usage.get("completion_tokens").is_some());
}

// ==================== Integration Tests ====================

#[test]
fn test_full_chat_transformation_pipeline() {
    let config = ResponseTransformConfig {
        strip_azure_metadata: true,
        normalize_field_names: true,
        include_content_filters: false,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "id": "chatcmpl-123",
        "model": "gpt-4",
        "choices": [{
            "message": {"content": "Hello", "role": "assistant"},
            "finish_reason": "max_tokens",
            "content_filter_results": {"hate": {"filtered": false}}
        }],
        "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15},
        "deployment_id": "my-gpt4",
        "azure_endpoint": "https://test.openai.azure.com"
    });

    // Use transform_chat_response for full transformation including finish_reason
    let result = transformation.transform_chat_response(response).unwrap();

    // Content filters should be removed
    let choice = &result.get("choices").unwrap().as_array().unwrap()[0];
    assert!(choice.get("content_filter_results").is_none());

    // Finish reason should be normalized
    assert_eq!(
        choice.get("finish_reason").unwrap().as_str().unwrap(),
        "length"
    );
}

#[test]
fn test_transform_response_with_strip_azure_fields() {
    let config = ResponseTransformConfig {
        strip_azure_metadata: true,
        normalize_field_names: true,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "id": "chatcmpl-123",
        "choices": [{"message": {"content": "Hello"}}],
        "usage": {"input_tokens": 10, "output_tokens": 5},
        "deployment_id": "my-gpt4",
        "azure_endpoint": "https://test.openai.azure.com"
    });

    let result = transformation.transform_response(response).unwrap();

    // Azure fields should be stripped
    assert!(result.get("deployment_id").is_none());
    assert!(result.get("azure_endpoint").is_none());

    // Standard fields should remain
    assert!(result.get("id").is_some());
    assert!(result.get("choices").is_some());

    // Fields should be normalized
    assert!(result.get("usage").unwrap().get("prompt_tokens").is_some());
}

#[test]
fn test_minimal_format_only_essential_fields() {
    let config = ResponseTransformConfig {
        response_format: ResponseFormat::Minimal,
        ..Default::default()
    };
    let transformation = AzureResponseTransformation::with_config(config);

    let response = serde_json::json!({
        "id": "123",
        "object": "chat.completion",
        "model": "gpt-4",
        "choices": [{"message": {"content": "Hello"}}],
        "usage": {"total_tokens": 15, "prompt_tokens": 10},
        "system_fingerprint": "fp_123"
    });

    let result = transformation.transform_response(response).unwrap();

    // Only essential fields
    assert!(result.get("choices").is_some());
    assert!(result.get("usage").is_some());

    // Non-essential fields should be gone
    assert!(result.get("id").is_none());
    assert!(result.get("object").is_none());
    assert!(result.get("model").is_none());
    assert!(result.get("system_fingerprint").is_none());

    // Usage should only have total_tokens
    let usage = result.get("usage").unwrap();
    assert!(usage.get("total_tokens").is_some());
}
