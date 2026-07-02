use super::*;

// ==================== should_retry_request Tests ====================

#[test]
fn test_should_retry_rate_limited() {
    assert!(ClientUtils::should_retry_request(429, 0, 3));
    assert!(ClientUtils::should_retry_request(429, 1, 3));
    assert!(ClientUtils::should_retry_request(429, 2, 3));
    assert!(!ClientUtils::should_retry_request(429, 3, 3));
}

#[test]
fn test_should_retry_server_errors() {
    assert!(ClientUtils::should_retry_request(500, 0, 3));
    assert!(ClientUtils::should_retry_request(502, 0, 3));
    assert!(ClientUtils::should_retry_request(503, 0, 3));
    assert!(ClientUtils::should_retry_request(504, 0, 3));
    assert!(ClientUtils::should_retry_request(599, 0, 3));
}

#[test]
fn test_should_retry_request_timeout() {
    assert!(ClientUtils::should_retry_request(408, 0, 3));
}

#[test]
fn test_should_not_retry_client_errors() {
    assert!(!ClientUtils::should_retry_request(400, 0, 3));
    assert!(!ClientUtils::should_retry_request(401, 0, 3));
    assert!(!ClientUtils::should_retry_request(403, 0, 3));
    assert!(!ClientUtils::should_retry_request(404, 0, 3));
}

#[test]
fn test_should_not_retry_success() {
    assert!(!ClientUtils::should_retry_request(200, 0, 3));
    assert!(!ClientUtils::should_retry_request(201, 0, 3));
    assert!(!ClientUtils::should_retry_request(204, 0, 3));
}

#[test]
fn test_should_not_retry_max_attempts() {
    assert!(!ClientUtils::should_retry_request(500, 5, 3));
    assert!(!ClientUtils::should_retry_request(429, 10, 5));
}

// ==================== calculate_retry_delay Tests ====================

#[test]
fn test_calculate_retry_delay_respects_server_delay() {
    let config = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 100,
        max_delay_ms: 30000,
        backoff_multiplier: 2.0,
        jitter: false,
        retryable_errors: vec![],
    };

    let server_delay = Duration::from_secs(10);
    let delay = ClientUtils::calculate_retry_delay(&config, 0, Some(server_delay));
    assert_eq!(delay, server_delay);
}

#[test]
fn test_calculate_retry_delay_exponential_backoff() {
    let config = RetryConfig {
        max_retries: 5,
        initial_delay_ms: 100,
        max_delay_ms: 30000,
        backoff_multiplier: 2.0,
        jitter: false,
        retryable_errors: vec![],
    };

    let delay0 = ClientUtils::calculate_retry_delay(&config, 0, None);
    let delay1 = ClientUtils::calculate_retry_delay(&config, 1, None);
    let delay2 = ClientUtils::calculate_retry_delay(&config, 2, None);

    assert_eq!(delay0, Duration::from_millis(100));
    assert_eq!(delay1, Duration::from_millis(200));
    assert_eq!(delay2, Duration::from_millis(400));
}

#[test]
fn test_calculate_retry_delay_respects_max() {
    let config = RetryConfig {
        max_retries: 10,
        initial_delay_ms: 100,
        max_delay_ms: 500,
        backoff_multiplier: 2.0,
        jitter: false,
        retryable_errors: vec![],
    };

    // At attempt 5: 100 * 2^5 = 3200ms, but max is 500ms
    let delay = ClientUtils::calculate_retry_delay(&config, 5, None);
    assert_eq!(delay, Duration::from_millis(500));
}

#[test]
fn test_calculate_retry_delay_with_jitter() {
    let config = RetryConfig {
        max_retries: 3,
        initial_delay_ms: 100,
        max_delay_ms: 30000,
        backoff_multiplier: 2.0,
        jitter: true,
        retryable_errors: vec![],
    };

    // With jitter, delay should be around 100ms +/- 10%
    let delay = ClientUtils::calculate_retry_delay(&config, 0, None);
    assert!(delay >= Duration::from_millis(90));
    assert!(delay <= Duration::from_millis(110));
}

// ==================== get_timeout_for_provider Tests ====================

#[test]
fn test_get_timeout_openai() {
    let timeout = ClientUtils::get_timeout_for_provider("openai");
    assert_eq!(timeout, Duration::from_secs(120));
}

#[test]
fn test_get_timeout_anthropic() {
    let timeout = ClientUtils::get_timeout_for_provider("anthropic");
    assert_eq!(timeout, Duration::from_secs(180));
}

#[test]
fn test_get_timeout_google() {
    let timeout = ClientUtils::get_timeout_for_provider("google");
    assert_eq!(timeout, Duration::from_secs(90));
}

#[test]
fn test_get_timeout_azure() {
    let timeout = ClientUtils::get_timeout_for_provider("azure");
    assert_eq!(timeout, Duration::from_secs(120));
}

#[test]
fn test_get_timeout_cohere() {
    let timeout = ClientUtils::get_timeout_for_provider("cohere");
    assert_eq!(timeout, Duration::from_secs(60));
}

#[test]
fn test_get_timeout_unknown() {
    let timeout = ClientUtils::get_timeout_for_provider("unknown-provider");
    assert_eq!(timeout, Duration::from_secs(60));
}

#[test]
fn test_get_timeout_case_insensitive() {
    assert_eq!(
        ClientUtils::get_timeout_for_provider("OpenAI"),
        ClientUtils::get_timeout_for_provider("openai")
    );
    assert_eq!(
        ClientUtils::get_timeout_for_provider("ANTHROPIC"),
        ClientUtils::get_timeout_for_provider("anthropic")
    );
}

// ==================== supports_httpx_timeout Tests ====================

#[test]
fn test_supports_httpx_timeout_openai() {
    assert!(ClientUtils::supports_httpx_timeout("openai"));
}

#[test]
fn test_supports_httpx_timeout_anthropic() {
    assert!(ClientUtils::supports_httpx_timeout("anthropic"));
}

#[test]
fn test_supports_httpx_timeout_google() {
    assert!(ClientUtils::supports_httpx_timeout("google"));
}

#[test]
fn test_supports_httpx_timeout_azure() {
    assert!(ClientUtils::supports_httpx_timeout("azure"));
}

#[test]
fn test_supports_httpx_timeout_cohere() {
    assert!(ClientUtils::supports_httpx_timeout("cohere"));
}

#[test]
fn test_supports_httpx_timeout_mistral() {
    assert!(ClientUtils::supports_httpx_timeout("mistral"));
}

#[test]
fn test_supports_httpx_timeout_replicate() {
    assert!(ClientUtils::supports_httpx_timeout("replicate"));
}

#[test]
fn test_supports_httpx_timeout_unknown() {
    assert!(!ClientUtils::supports_httpx_timeout("unknown"));
}

#[test]
fn test_supports_httpx_timeout_case_insensitive() {
    assert!(ClientUtils::supports_httpx_timeout("OPENAI"));
    assert!(ClientUtils::supports_httpx_timeout("Anthropic"));
}

// ==================== get_user_agent_for_provider Tests ====================

#[test]
fn test_user_agent_openai() {
    assert_eq!(
        ClientUtils::get_user_agent_for_provider("openai"),
        "litellm-rust-openai/1.0"
    );
}

#[test]
fn test_user_agent_anthropic() {
    assert_eq!(
        ClientUtils::get_user_agent_for_provider("anthropic"),
        "litellm-rust-anthropic/1.0"
    );
}

#[test]
fn test_user_agent_google() {
    assert_eq!(
        ClientUtils::get_user_agent_for_provider("google"),
        "litellm-rust-google/1.0"
    );
}

#[test]
fn test_user_agent_unknown() {
    assert_eq!(
        ClientUtils::get_user_agent_for_provider("unknown"),
        "litellm-rust/1.0"
    );
}

// ==================== add_path_to_api_base Tests ====================

#[test]
fn test_add_path_basic() {
    assert_eq!(
        ClientUtils::add_path_to_api_base("https://api.example.com", "v1/chat"),
        "https://api.example.com/v1/chat"
    );
}

#[test]
fn test_add_path_with_trailing_slash() {
    assert_eq!(
        ClientUtils::add_path_to_api_base("https://api.example.com/", "v1/chat"),
        "https://api.example.com/v1/chat"
    );
}

#[test]
fn test_add_path_with_leading_slash() {
    assert_eq!(
        ClientUtils::add_path_to_api_base("https://api.example.com", "/v1/chat"),
        "https://api.example.com/v1/chat"
    );
}

#[test]
fn test_add_path_with_both_slashes() {
    assert_eq!(
        ClientUtils::add_path_to_api_base("https://api.example.com/", "/v1/chat"),
        "https://api.example.com/v1/chat"
    );
}

#[test]
fn test_add_path_multiple_trailing_slashes() {
    // trim_end_matches('/') removes all trailing slashes
    // trim_start_matches('/') removes all leading slashes
    assert_eq!(
        ClientUtils::add_path_to_api_base("https://api.example.com///", "///v1/chat"),
        "https://api.example.com/v1/chat"
    );
}

// ==================== validate_url Tests ====================

#[test]
fn test_validate_url_https() {
    assert!(ClientUtils::validate_url("https://api.example.com").is_ok());
}

#[test]
fn test_validate_url_http() {
    assert!(ClientUtils::validate_url("http://api.example.com").is_ok());
}

#[test]
fn test_validate_url_with_path() {
    assert!(ClientUtils::validate_url("https://api.example.com/v1/chat").is_ok());
}

#[test]
fn test_validate_url_with_query() {
    assert!(ClientUtils::validate_url("https://api.example.com?key=value").is_ok());
}

#[test]
fn test_validate_url_invalid_scheme() {
    let result = ClientUtils::validate_url("ftp://files.example.com");
    assert!(result.is_err());
}

#[test]
fn test_validate_url_invalid_format() {
    let result = ClientUtils::validate_url("not a valid url");
    assert!(result.is_err());
}

#[test]
fn test_validate_url_empty() {
    let result = ClientUtils::validate_url("");
    assert!(result.is_err());
}

// ==================== parse_content_type Tests ====================

#[test]
fn test_parse_content_type_simple() {
    let (media_type, params) = ClientUtils::parse_content_type("application/json");
    assert_eq!(media_type, "application/json");
    assert!(params.is_empty());
}

#[test]
fn test_parse_content_type_with_charset() {
    let (media_type, params) = ClientUtils::parse_content_type("application/json; charset=utf-8");
    assert_eq!(media_type, "application/json");
    assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
}

#[test]
fn test_parse_content_type_multiple_params() {
    let (media_type, params) =
        ClientUtils::parse_content_type("text/html; charset=utf-8; boundary=something");
    assert_eq!(media_type, "text/html");
    assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
    assert_eq!(params.get("boundary"), Some(&"something".to_string()));
}

#[test]
fn test_parse_content_type_quoted_value() {
    let (media_type, params) =
        ClientUtils::parse_content_type("multipart/form-data; boundary=\"----WebKitFormBoundary\"");
    assert_eq!(media_type, "multipart/form-data");
    assert_eq!(
        params.get("boundary"),
        Some(&"----WebKitFormBoundary".to_string())
    );
}

#[test]
fn test_parse_content_type_case_insensitive() {
    let (media_type, _) = ClientUtils::parse_content_type("Application/JSON");
    assert_eq!(media_type, "application/json");
}

#[test]
fn test_parse_content_type_with_spaces() {
    let (media_type, params) =
        ClientUtils::parse_content_type("  application/json ;  charset = utf-8  ");
    assert_eq!(media_type, "application/json");
    assert_eq!(params.get("charset"), Some(&"utf-8".to_string()));
}

// ==================== get_default_headers_for_provider Tests ====================

#[test]
fn test_default_headers_has_content_type() {
    let headers = ClientUtils::get_default_headers_for_provider("openai");
    assert_eq!(
        headers.get("Content-Type"),
        Some(&"application/json".to_string())
    );
}

#[test]
fn test_default_headers_has_accept() {
    let headers = ClientUtils::get_default_headers_for_provider("openai");
    assert_eq!(headers.get("Accept"), Some(&"application/json".to_string()));
}

#[test]
fn test_default_headers_anthropic_version() {
    let headers = ClientUtils::get_default_headers_for_provider("anthropic");
    assert_eq!(
        headers.get("anthropic-version"),
        Some(&"2023-06-01".to_string())
    );
}

#[test]
fn test_default_headers_google_api_key() {
    let headers = ClientUtils::get_default_headers_for_provider("google");
    assert!(headers.contains_key("x-goog-api-key"));
}

#[test]
fn test_default_headers_azure_api_key() {
    let headers = ClientUtils::get_default_headers_for_provider("azure");
    assert!(headers.contains_key("api-key"));
}

// ==================== extract_retry_after_from_headers Tests ====================

#[test]
fn test_extract_retry_after_seconds() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("retry-after", "30".parse().unwrap());

    let delay = ClientUtils::extract_retry_after_from_headers(&headers);
    assert_eq!(delay, Some(Duration::from_secs(30)));
}

#[test]
fn test_extract_retry_after_missing() {
    let headers = reqwest::header::HeaderMap::new();
    let delay = ClientUtils::extract_retry_after_from_headers(&headers);
    assert!(delay.is_none());
}

#[test]
fn test_extract_retry_after_invalid() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("retry-after", "not-a-number".parse().unwrap());

    let delay = ClientUtils::extract_retry_after_from_headers(&headers);
    assert!(delay.is_none());
}

// ==================== create_http_client Tests ====================

#[test]
fn test_create_http_client_default_config() {
    let config = HttpClientConfig::default();
    let result = ClientUtils::create_http_client(&config);
    assert!(result.is_ok());
}

#[test]
fn test_create_http_client_with_timeout() {
    let config = HttpClientConfig {
        timeout: Duration::from_secs(30),
        ..Default::default()
    };
    let result = ClientUtils::create_http_client(&config);
    assert!(result.is_ok());
}

#[test]
fn test_create_http_client_with_user_agent() {
    let config = HttpClientConfig {
        user_agent: "test-agent/1.0".to_string(),
        ..Default::default()
    };
    let result = ClientUtils::create_http_client(&config);
    assert!(result.is_ok());
}

#[test]
fn test_create_http_client_with_headers() {
    let mut config = HttpClientConfig::default();
    config
        .default_headers
        .insert("X-Custom-Header".to_string(), "value".to_string());
    let result = ClientUtils::create_http_client(&config);
    assert!(result.is_ok());
}

#[test]
fn test_create_provider_specific_client_openai() {
    let result = ClientUtils::create_provider_specific_client("openai");
    assert!(result.is_ok());
}

#[test]
fn test_create_provider_specific_client_anthropic() {
    let result = ClientUtils::create_provider_specific_client("anthropic");
    assert!(result.is_ok());
}

// ==================== get_environment_proxies Tests ====================

#[test]
fn test_get_environment_proxies_empty() {
    // This test just ensures the function doesn't panic
    // Actual proxy values depend on environment
    let _proxies = ClientUtils::get_environment_proxies();
}
