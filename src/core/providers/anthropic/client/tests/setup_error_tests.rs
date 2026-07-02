use super::*;

// ==================== Client Creation Tests ====================

#[test]
fn test_client_creation() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config);
    assert!(client.is_ok());
}

#[test]
fn test_client_creation_with_custom_config() {
    let mut config = AnthropicConfig::new_test("test-key");
    config.request_timeout = 120;
    config.connect_timeout = 30;
    let client = AnthropicClient::new(config);
    assert!(client.is_ok());
}

// ==================== Header Building Tests ====================

/// Helper to check if a header key exists in Vec<HeaderPair>
fn has_header(headers: &[HeaderPair], key: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(key))
}

/// Helper to get a header value from Vec<HeaderPair>
fn get_header<'a>(headers: &'a [HeaderPair], key: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_ref())
}

#[test]
fn test_header_building() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    // Anthropic uses x-api-key header instead of Authorization
    assert!(has_header(&headers, "x-api-key"));
    assert!(has_header(&headers, "anthropic-version"));
    assert!(has_header(&headers, "Content-Type"));
    assert!(has_header(&headers, "User-Agent"));
}

#[test]
fn test_header_content_type() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert_eq!(
        get_header(&headers, "Content-Type").unwrap(),
        "application/json"
    );
}

#[test]
fn test_header_user_agent() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert_eq!(
        get_header(&headers, "User-Agent").unwrap(),
        "LiteLLM-Rust/1.0"
    );
}

#[test]
fn test_header_with_custom_headers() {
    let mut config = AnthropicConfig::new_test("test-key");
    config
        .custom_headers
        .insert("X-Custom-Header".to_string(), "custom-value".to_string());
    let client = AnthropicClient::new(config).unwrap();
    let headers = client.get_request_headers();

    assert!(has_header(&headers, "X-Custom-Header"));
}

// ==================== Error Mapping Tests ====================

#[test]
fn test_map_http_error_400() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(400, "invalid request");

    // Should return an API error for 400
    let error_string = format!("{}", error);
    assert!(error_string.contains("400") || error_string.contains("request"));
}

#[test]
fn test_map_http_error_401() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(401, "unauthorized");

    // Should return an authentication error
    let error_string = format!("{}", error);
    assert!(error_string.to_lowercase().contains("auth") || error_string.contains("key"));
}

#[test]
fn test_map_http_error_403() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(403, "forbidden");

    // Should return an authentication error
    let error_string = format!("{}", error);
    assert!(
        error_string.to_lowercase().contains("forbidden")
            || error_string.to_lowercase().contains("permission")
            || error_string.to_lowercase().contains("auth")
    );
}

#[test]
fn test_map_http_error_404() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(404, "not found");

    let error_string = format!("{}", error);
    assert!(error_string.contains("404") || error_string.contains("not found"));
}

#[test]
fn test_map_http_error_429() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(429, "rate limited");

    // Should return a rate limit error
    let error_string = format!("{}", error);
    assert!(
        error_string.to_lowercase().contains("rate")
            || error_string.to_lowercase().contains("limit")
    );
}

#[test]
fn test_map_http_error_500() {
    let config = AnthropicConfig::new_test("test-key");
    let client = AnthropicClient::new(config).unwrap();
    let error = client.map_http_error(500, "server error");

    let error_string = format!("{}", error);
    assert!(error_string.contains("500") || error_string.to_lowercase().contains("server"));
}

// ==================== Retry-After Extraction Tests ====================

#[test]
fn test_extract_retry_after_from_root() {
    let body = r#"{"retry_after": 60}"#;
    let retry = parse_retry_after_from_body(body);
    assert_eq!(retry, Some(60));
}

#[test]
fn test_extract_retry_after_from_error() {
    let body = r#"{"error": {"retry_after": 30}}"#;
    let retry = parse_retry_after_from_body(body);
    assert_eq!(retry, Some(30));
}

#[test]
fn test_extract_retry_after_missing() {
    let body = r#"{"message": "no retry info"}"#;
    let retry = parse_retry_after_from_body(body);
    assert!(retry.is_none());
}

#[test]
fn test_extract_retry_after_invalid_json() {
    let body = "not json";
    let retry = parse_retry_after_from_body(body);
    assert!(retry.is_none());
}
