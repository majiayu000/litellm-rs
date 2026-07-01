use super::*;

// ==================== CreateBatchRequest Tests ====================

#[test]
fn test_create_batch_request_creation() {
    let request = CreateBatchRequest {
        input_file_id: "file-abc123".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    assert_eq!(request.input_file_id, "file-abc123");
    assert_eq!(request.endpoint, "/v1/chat/completions");
    assert_eq!(request.completion_window, "24h");
}

#[test]
fn test_create_batch_request_serialization() {
    let request = CreateBatchRequest {
        input_file_id: "file-xyz789".to_string(),
        endpoint: "/v1/embeddings".to_string(),
        completion_window: "24h".to_string(),
    };

    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["input_file_id"], "file-xyz789");
    assert_eq!(json["endpoint"], "/v1/embeddings");
    assert_eq!(json["completion_window"], "24h");
}

#[test]
fn test_create_batch_request_deserialization() {
    let json = r#"{
            "input_file_id": "file-test",
            "endpoint": "/v1/completions",
            "completion_window": "24h"
        }"#;

    let request: CreateBatchRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.input_file_id, "file-test");
    assert_eq!(request.endpoint, "/v1/completions");
}

#[test]
fn test_create_batch_request_clone() {
    let request = CreateBatchRequest {
        input_file_id: "file-clone".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    let cloned = request.clone();
    assert_eq!(cloned.input_file_id, request.input_file_id);
    assert_eq!(cloned.endpoint, request.endpoint);
}

#[test]
fn test_create_batch_request_debug() {
    let request = CreateBatchRequest {
        input_file_id: "file-debug".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    let debug = format!("{:?}", request);
    assert!(debug.contains("CreateBatchRequest"));
    assert!(debug.contains("file-debug"));
}

// ==================== CreateBatchResponse Tests ====================

#[test]
fn test_create_batch_response_creation() {
    let response = CreateBatchResponse {
        id: "batch_abc123".to_string(),
        object: "batch".to_string(),
        status: "validating".to_string(),
    };

    assert_eq!(response.id, "batch_abc123");
    assert_eq!(response.object, "batch");
    assert_eq!(response.status, "validating");
}

#[test]
fn test_create_batch_response_serialization() {
    let response = CreateBatchResponse {
        id: "batch_xyz".to_string(),
        object: "batch".to_string(),
        status: "in_progress".to_string(),
    };

    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["id"], "batch_xyz");
    assert_eq!(json["object"], "batch");
    assert_eq!(json["status"], "in_progress");
}

#[test]
fn test_create_batch_response_deserialization() {
    let json = r#"{
            "id": "batch_test",
            "object": "batch",
            "status": "completed"
        }"#;

    let response: CreateBatchResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "batch_test");
    assert_eq!(response.status, "completed");
}

// ==================== ListBatchesResponse Tests ====================

#[test]
fn test_list_batches_response_empty() {
    let response = ListBatchesResponse { data: vec![] };
    assert!(response.data.is_empty());
}

#[test]
fn test_list_batches_response_with_data() {
    let batch1 = serde_json::json!({"id": "batch_1", "status": "completed"});
    let batch2 = serde_json::json!({"id": "batch_2", "status": "in_progress"});

    let response = ListBatchesResponse {
        data: vec![batch1, batch2],
    };

    assert_eq!(response.data.len(), 2);
}

#[test]
fn test_list_batches_response_serialization() {
    let response = ListBatchesResponse {
        data: vec![serde_json::json!({"id": "batch_test"})],
    };

    let json = serde_json::to_value(&response).unwrap();
    assert!(json["data"].is_array());
    assert_eq!(json["data"][0]["id"], "batch_test");
}

#[test]
fn test_list_batches_response_deserialization() {
    let json = r#"{"data": [{"id": "batch_1"}, {"id": "batch_2"}]}"#;
    let response: ListBatchesResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.data.len(), 2);
}

// ==================== RetrieveBatchResponse Tests ====================

#[test]
fn test_retrieve_batch_response_creation() {
    let response = RetrieveBatchResponse {
        id: "batch_retrieve".to_string(),
        object: "batch".to_string(),
        status: "completed".to_string(),
    };

    assert_eq!(response.id, "batch_retrieve");
    assert_eq!(response.object, "batch");
    assert_eq!(response.status, "completed");
}

#[test]
fn test_retrieve_batch_response_different_statuses() {
    let statuses = vec![
        "validating",
        "in_progress",
        "completed",
        "failed",
        "expired",
        "cancelled",
    ];

    for status in statuses {
        let response = RetrieveBatchResponse {
            id: "batch_test".to_string(),
            object: "batch".to_string(),
            status: status.to_string(),
        };
        assert_eq!(response.status, status);
    }
}

// ==================== CancelBatchResponse Tests ====================

#[test]
fn test_cancel_batch_response_creation() {
    let response = CancelBatchResponse {
        id: "batch_cancel".to_string(),
        object: "batch".to_string(),
        status: "cancelling".to_string(),
    };

    assert_eq!(response.id, "batch_cancel");
    assert_eq!(response.status, "cancelling");
}

#[test]
fn test_cancel_batch_response_cancelled() {
    let response = CancelBatchResponse {
        id: "batch_cancelled".to_string(),
        object: "batch".to_string(),
        status: "cancelled".to_string(),
    };

    assert_eq!(response.status, "cancelled");
}

// ==================== BatchError Tests ====================

#[test]
fn test_batch_error_authentication() {
    let error = ProviderError::authentication("azure", "Invalid API key".to_string());
    let msg = error.to_string();
    assert!(msg.contains("azure") || msg.contains("Invalid API key"));
}

#[test]
fn test_batch_error_request() {
    let error = ProviderError::invalid_request("azure", "Bad request format".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Bad request format") || msg.contains("invalid"));
}

#[test]
fn test_batch_error_network() {
    let error = ProviderError::network("azure", "Connection refused".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Connection refused") || msg.contains("network"));
}

#[test]
fn test_batch_error_configuration() {
    let error = ProviderError::configuration("azure", "Missing endpoint".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Missing endpoint") || msg.contains("configuration"));
}

#[test]
fn test_batch_error_parsing() {
    let error = ProviderError::serialization("azure", "Invalid JSON".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Invalid JSON") || msg.contains("serialization"));
}

#[test]
fn test_batch_error_validation() {
    let error = ProviderError::invalid_request("azure", "Invalid file ID".to_string());
    let msg = error.to_string();
    assert!(msg.contains("Invalid file ID") || msg.contains("invalid"));
}

#[test]
fn test_batch_error_api() {
    let error = ProviderError::api_error("azure", 429, "Rate limit exceeded");
    let msg = error.to_string();
    assert!(msg.contains("429"));
    assert!(msg.contains("Rate limit exceeded"));
}

#[test]
fn test_batch_error_api_various_codes() {
    let test_cases = vec![
        (400_u16, "Bad Request"),
        (401, "Unauthorized"),
        (403, "Forbidden"),
        (404, "Not Found"),
        (500, "Internal Server Error"),
        (503, "Service Unavailable"),
    ];

    for (status, message) in test_cases {
        let error = ProviderError::api_error("azure", status, message);
        let msg = error.to_string();
        assert!(msg.contains(&status.to_string()));
        assert!(msg.contains(message));
    }
}

// ==================== AzureBatchJob Tests ====================

#[test]
fn test_azure_batch_job_minimal() {
    let job = AzureBatchJob {
        id: "batch_job_1".to_string(),
        object: "batch".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        errors: None,
        input_file_id: "file-input".to_string(),
        completion_window: "24h".to_string(),
        status: "in_progress".to_string(),
        output_file_id: None,
        error_file_id: None,
        created_at: 1700000000,
        in_progress_at: Some(1700000100),
        expires_at: Some(1700086400),
        finalizing_at: None,
        completed_at: None,
        failed_at: None,
        expired_at: None,
        cancelling_at: None,
        cancelled_at: None,
        request_counts: AzureBatchRequestCounts {
            total: 100,
            completed: 50,
            failed: 0,
        },
        metadata: None,
    };

    assert_eq!(job.id, "batch_job_1");
    assert_eq!(job.status, "in_progress");
    assert!(job.errors.is_none());
    assert!(job.output_file_id.is_none());
}

#[test]
fn test_azure_batch_job_completed() {
    let job = AzureBatchJob {
        id: "batch_completed".to_string(),
        object: "batch".to_string(),
        endpoint: "/v1/embeddings".to_string(),
        errors: None,
        input_file_id: "file-input".to_string(),
        completion_window: "24h".to_string(),
        status: "completed".to_string(),
        output_file_id: Some("file-output".to_string()),
        error_file_id: None,
        created_at: 1700000000,
        in_progress_at: Some(1700000100),
        expires_at: None,
        finalizing_at: Some(1700003000),
        completed_at: Some(1700003600),
        failed_at: None,
        expired_at: None,
        cancelling_at: None,
        cancelled_at: None,
        request_counts: AzureBatchRequestCounts {
            total: 500,
            completed: 500,
            failed: 0,
        },
        metadata: Some({
            let mut m = HashMap::new();
            m.insert("project".to_string(), "test".to_string());
            m
        }),
    };

    assert_eq!(job.status, "completed");
    assert!(job.output_file_id.is_some());
    assert_eq!(job.request_counts.completed, 500);
}

#[test]
fn test_azure_batch_job_with_errors() {
    let job = AzureBatchJob {
        id: "batch_errors".to_string(),
        object: "batch".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        errors: Some(AzureBatchErrors {
            object: "list".to_string(),
            data: vec![AzureBatchErrorData {
                code: "invalid_request".to_string(),
                message: "Missing required field".to_string(),
                param: Some("messages".to_string()),
                line: Some(42),
            }],
        }),
        input_file_id: "file-input".to_string(),
        completion_window: "24h".to_string(),
        status: "failed".to_string(),
        output_file_id: None,
        error_file_id: Some("file-errors".to_string()),
        created_at: 1700000000,
        in_progress_at: Some(1700000100),
        expires_at: None,
        finalizing_at: None,
        completed_at: None,
        failed_at: Some(1700001000),
        expired_at: None,
        cancelling_at: None,
        cancelled_at: None,
        request_counts: AzureBatchRequestCounts {
            total: 100,
            completed: 50,
            failed: 50,
        },
        metadata: None,
    };

    assert_eq!(job.status, "failed");
    assert!(job.errors.is_some());
    assert!(job.error_file_id.is_some());
    assert_eq!(job.request_counts.failed, 50);
}

#[test]
fn test_azure_batch_job_serialization() {
    let job = AzureBatchJob {
        id: "batch_serialize".to_string(),
        object: "batch".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        errors: None,
        input_file_id: "file-123".to_string(),
        completion_window: "24h".to_string(),
        status: "validating".to_string(),
        output_file_id: None,
        error_file_id: None,
        created_at: 1700000000,
        in_progress_at: None,
        expires_at: None,
        finalizing_at: None,
        completed_at: None,
        failed_at: None,
        expired_at: None,
        cancelling_at: None,
        cancelled_at: None,
        request_counts: AzureBatchRequestCounts {
            total: 10,
            completed: 0,
            failed: 0,
        },
        metadata: None,
    };

    let json = serde_json::to_value(&job).unwrap();
    assert_eq!(json["id"], "batch_serialize");
    assert_eq!(json["status"], "validating");
    assert_eq!(json["request_counts"]["total"], 10);
}

// ==================== AzureBatchErrors Tests ====================

#[test]
fn test_azure_batch_errors_creation() {
    let errors = AzureBatchErrors {
        object: "list".to_string(),
        data: vec![],
    };

    assert_eq!(errors.object, "list");
    assert!(errors.data.is_empty());
}

#[test]
fn test_azure_batch_errors_with_data() {
    let errors = AzureBatchErrors {
        object: "list".to_string(),
        data: vec![
            AzureBatchErrorData {
                code: "error1".to_string(),
                message: "First error".to_string(),
                param: None,
                line: Some(1),
            },
            AzureBatchErrorData {
                code: "error2".to_string(),
                message: "Second error".to_string(),
                param: Some("field".to_string()),
                line: Some(2),
            },
        ],
    };

    assert_eq!(errors.data.len(), 2);
    assert_eq!(errors.data[0].code, "error1");
    assert_eq!(errors.data[1].param, Some("field".to_string()));
}

// ==================== AzureBatchErrorData Tests ====================

#[test]
fn test_azure_batch_error_data_minimal() {
    let error = AzureBatchErrorData {
        code: "validation_error".to_string(),
        message: "Invalid input".to_string(),
        param: None,
        line: None,
    };

    assert_eq!(error.code, "validation_error");
    assert_eq!(error.message, "Invalid input");
    assert!(error.param.is_none());
    assert!(error.line.is_none());
}

#[test]
fn test_azure_batch_error_data_full() {
    let error = AzureBatchErrorData {
        code: "content_filter".to_string(),
        message: "Content filtered".to_string(),
        param: Some("messages[0].content".to_string()),
        line: Some(15),
    };

    assert_eq!(error.param, Some("messages[0].content".to_string()));
    assert_eq!(error.line, Some(15));
}

#[test]
fn test_azure_batch_error_data_serialization() {
    let error = AzureBatchErrorData {
        code: "rate_limit".to_string(),
        message: "Rate limit exceeded".to_string(),
        param: None,
        line: Some(100),
    };

    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["code"], "rate_limit");
    assert_eq!(json["line"], 100);
}

// ==================== AzureBatchRequestCounts Tests ====================

#[test]
fn test_azure_batch_request_counts_creation() {
    let counts = AzureBatchRequestCounts {
        total: 1000,
        completed: 800,
        failed: 50,
    };

    assert_eq!(counts.total, 1000);
    assert_eq!(counts.completed, 800);
    assert_eq!(counts.failed, 50);
}

#[test]
fn test_azure_batch_request_counts_all_completed() {
    let counts = AzureBatchRequestCounts {
        total: 500,
        completed: 500,
        failed: 0,
    };

    assert_eq!(counts.total, counts.completed);
    assert_eq!(counts.failed, 0);
}

#[test]
fn test_azure_batch_request_counts_all_failed() {
    let counts = AzureBatchRequestCounts {
        total: 100,
        completed: 0,
        failed: 100,
    };

    assert_eq!(counts.total, counts.failed);
    assert_eq!(counts.completed, 0);
}

#[test]
fn test_azure_batch_request_counts_serialization() {
    let counts = AzureBatchRequestCounts {
        total: 250,
        completed: 200,
        failed: 25,
    };

    let json = serde_json::to_value(&counts).unwrap();
    assert_eq!(json["total"], 250);
    assert_eq!(json["completed"], 200);
    assert_eq!(json["failed"], 25);
}

// ==================== AzureBatchUtils Tests ====================

#[test]
fn test_get_supported_batch_endpoints() {
    let endpoints = AzureBatchUtils::get_supported_batch_endpoints();

    assert!(endpoints.contains(&"/v1/chat/completions"));
    assert!(endpoints.contains(&"/v1/completions"));
    assert!(endpoints.contains(&"/v1/embeddings"));
    assert_eq!(endpoints.len(), 3);
}

#[test]
fn test_validate_batch_request_valid_chat() {
    let request = CreateBatchRequest {
        input_file_id: "file-abc123".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_ok());
}

#[test]
fn test_validate_batch_request_valid_completions() {
    let request = CreateBatchRequest {
        input_file_id: "file-xyz".to_string(),
        endpoint: "/v1/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_ok());
}

#[test]
fn test_validate_batch_request_valid_embeddings() {
    let request = CreateBatchRequest {
        input_file_id: "file-emb".to_string(),
        endpoint: "/v1/embeddings".to_string(),
        completion_window: "24h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_ok());
}

#[test]
fn test_validate_batch_request_invalid_endpoint() {
    let request = CreateBatchRequest {
        input_file_id: "file-abc".to_string(),
        endpoint: "/v1/images/generations".to_string(),
        completion_window: "24h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_err());
    if let Err(ProviderError::InvalidRequest { message, .. }) = result {
        assert!(message.contains("Unsupported batch endpoint"));
    }
}

#[test]
fn test_validate_batch_request_empty_file_id() {
    let request = CreateBatchRequest {
        input_file_id: "".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_err());
    if let Err(ProviderError::InvalidRequest { message, .. }) = result {
        assert!(message.contains("Input file ID is required"));
    }
}

#[test]
fn test_validate_batch_request_invalid_completion_window() {
    let request = CreateBatchRequest {
        input_file_id: "file-abc".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "48h".to_string(),
    };

    let result = AzureBatchUtils::validate_batch_request(&request);
    assert!(result.is_err());
    if let Err(ProviderError::InvalidRequest { message, .. }) = result {
        assert!(message.contains("Only 24h completion window is supported"));
    }
}

#[test]
fn test_estimate_batch_processing_time() {
    let duration = AzureBatchUtils::estimate_batch_processing_time(100);
    assert_eq!(duration, std::time::Duration::from_secs(100));
}

#[test]
fn test_estimate_batch_processing_time_zero() {
    let duration = AzureBatchUtils::estimate_batch_processing_time(0);
    assert_eq!(duration, std::time::Duration::from_secs(0));
}

#[test]
fn test_estimate_batch_processing_time_large() {
    let duration = AzureBatchUtils::estimate_batch_processing_time(10000);
    assert_eq!(duration, std::time::Duration::from_secs(10000));
}

// ==================== AzureBatchHandler Tests ====================

#[test]
fn test_azure_batch_handler_new_success() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("https://test.openai.azure.com".to_string());

    let handler = AzureBatchHandler::new(config);
    assert!(handler.is_ok());
}

#[test]
fn test_azure_batch_handler_new_missing_endpoint() {
    let config = AzureConfig::new().with_api_key("test-key".to_string());

    let handler = AzureBatchHandler::new(config);
    assert!(handler.is_err());
}

#[test]
fn test_azure_batch_handler_build_batches_url() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("https://test.openai.azure.com/".to_string())
        .with_api_version("2024-02-01".to_string());

    let handler = AzureBatchHandler::new(config).unwrap();
    let url = handler.build_batches_url("");

    assert!(url.contains("test.openai.azure.com"));
    assert!(url.contains("openai/batches"));
    assert!(url.contains("api-version=2024-02-01"));
}

#[test]
fn test_azure_batch_handler_build_batches_url_with_path() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("https://test.openai.azure.com/".to_string())
        .with_api_version("2024-02-01".to_string());

    let handler = AzureBatchHandler::new(config).unwrap();
    let url = handler.build_batches_url("/batch_123");

    assert!(url.contains("/batch_123"));
}

#[test]
fn test_azure_batch_handler_build_batches_url_cancel() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("https://test.openai.azure.com/".to_string())
        .with_api_version("2024-02-01".to_string());

    let handler = AzureBatchHandler::new(config).unwrap();
    let url = handler.build_batches_url("/batch_123/cancel");

    assert!(url.contains("/batch_123/cancel"));
}

#[test]
fn test_azure_batch_handler_debug() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("https://test.openai.azure.com".to_string());

    let handler = AzureBatchHandler::new(config).unwrap();
    let debug = format!("{:?}", handler);
    assert!(debug.contains("AzureBatchHandler"));
}
