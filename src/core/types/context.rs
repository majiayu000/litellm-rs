//! Request context types

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

const TEAM_ID_METADATA_KEY: &str = "team_id";
const API_KEY_ID_METADATA_KEY: &str = "api_key_id";
const API_KEY_BUDGET_ID_METADATA_KEY: &str = "api_key_budget_id";
const API_KEY_MAX_TOKENS_PER_REQUEST_KEY: &str = "api_key_max_tokens_per_request";

/// Request context for tracking and metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RequestContext {
    /// Request ID
    pub request_id: String,
    /// User ID
    pub user_id: Option<String>,
    /// Client IP
    pub client_ip: Option<String>,
    /// User agent
    pub user_agent: Option<String>,
    /// Custom headers
    pub headers: HashMap<String, String>,
    /// Start time
    pub start_time: SystemTime,
    /// Extra metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Trace ID (for distributed tracing)
    pub trace_id: Option<String>,
    /// Span ID
    pub span_id: Option<String>,
}

impl Default for RequestContext {
    fn default() -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            user_id: None,
            client_ip: None,
            user_agent: None,
            headers: HashMap::new(),
            start_time: SystemTime::now(),
            metadata: HashMap::new(),
            trace_id: None,
            span_id: None,
        }
    }
}

impl RequestContext {
    /// Create new request context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set user ID
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set user and optional team information.
    pub fn with_user(mut self, user_id: Uuid, team_id: Option<Uuid>) -> Self {
        self.user_id = Some(user_id.to_string());
        if let Some(team_id) = team_id {
            self.set_team_id(team_id);
        } else {
            self.clear_team_id();
        }
        self
    }

    /// Set API key information.
    pub fn with_api_key(mut self, api_key_id: Uuid) -> Self {
        self.set_api_key_id(api_key_id);
        self
    }

    /// Set API key budget information.
    pub fn with_api_key_budget(mut self, budget_id: Uuid) -> Self {
        self.set_api_key_budget_id(budget_id);
        self
    }

    /// Set client IP
    pub fn with_client_ip(mut self, client_ip: impl Into<String>) -> Self {
        self.client_ip = Some(client_ip.into());
        self
    }

    /// Set user agent
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = Some(user_agent.into());
        self
    }

    /// Set client information.
    pub fn with_client_info(mut self, ip: Option<String>, user_agent: Option<String>) -> Self {
        self.client_ip = ip;
        self.user_agent = user_agent;
        self
    }

    /// Add header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add header (compat helper for legacy call sites).
    pub fn add_header<K: Into<String>, V: Into<String>>(mut self, key: K, value: V) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set trace ID
    pub fn with_trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    /// Set trace/span IDs together.
    pub fn with_tracing(mut self, trace_id: String, span_id: String) -> Self {
        self.trace_id = Some(trace_id);
        self.span_id = Some(span_id);
        self
    }

    /// Set team ID in metadata.
    pub fn set_team_id(&mut self, team_id: Uuid) {
        self.metadata.insert(
            TEAM_ID_METADATA_KEY.to_string(),
            serde_json::json!(team_id.to_string()),
        );
    }

    /// Get team ID from metadata.
    pub fn team_id(&self) -> Option<Uuid> {
        self.metadata
            .get(TEAM_ID_METADATA_KEY)
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    /// Remove team ID from metadata.
    pub fn clear_team_id(&mut self) {
        self.metadata.remove(TEAM_ID_METADATA_KEY);
    }

    /// Set API key ID in metadata.
    pub fn set_api_key_id(&mut self, api_key_id: Uuid) {
        self.metadata.insert(
            API_KEY_ID_METADATA_KEY.to_string(),
            serde_json::json!(api_key_id.to_string()),
        );
    }

    /// Get API key ID from metadata.
    pub fn api_key_id(&self) -> Option<Uuid> {
        self.metadata
            .get(API_KEY_ID_METADATA_KEY)
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    /// Remove API key ID from metadata.
    pub fn clear_api_key_id(&mut self) {
        self.metadata.remove(API_KEY_ID_METADATA_KEY);
    }

    /// Set API key budget ID in metadata.
    pub fn set_api_key_budget_id(&mut self, budget_id: Uuid) {
        self.metadata.insert(
            API_KEY_BUDGET_ID_METADATA_KEY.to_string(),
            serde_json::json!(budget_id.to_string()),
        );
    }

    /// Get API key budget ID from metadata.
    pub fn api_key_budget_id(&self) -> Option<Uuid> {
        self.metadata
            .get(API_KEY_BUDGET_ID_METADATA_KEY)
            .and_then(|value| value.as_str())
            .and_then(|value| Uuid::parse_str(value).ok())
    }

    /// Remove API key budget ID from metadata.
    pub fn clear_api_key_budget_id(&mut self) {
        self.metadata.remove(API_KEY_BUDGET_ID_METADATA_KEY);
    }

    /// Set API key max output token policy in metadata.
    pub fn set_api_key_max_tokens_per_request(&mut self, max_tokens: u32) {
        self.metadata.insert(
            API_KEY_MAX_TOKENS_PER_REQUEST_KEY.to_string(),
            serde_json::json!(max_tokens),
        );
    }

    /// Get API key max output token policy from metadata.
    pub fn api_key_max_tokens_per_request(&self) -> Option<u32> {
        self.metadata
            .get(API_KEY_MAX_TOKENS_PER_REQUEST_KEY)
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok())
    }

    /// Remove API key max output token policy from metadata.
    pub fn clear_api_key_max_tokens_per_request(&mut self) {
        self.metadata.remove(API_KEY_MAX_TOKENS_PER_REQUEST_KEY);
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== RequestContext Default Tests ====================

    #[test]
    fn test_request_context_default() {
        let ctx = RequestContext::default();

        assert!(!ctx.request_id.is_empty());
        assert!(ctx.user_id.is_none());
        assert!(ctx.client_ip.is_none());
        assert!(ctx.user_agent.is_none());
        assert!(ctx.headers.is_empty());
        assert!(ctx.metadata.is_empty());
        assert!(ctx.trace_id.is_none());
        assert!(ctx.span_id.is_none());
    }

    #[test]
    fn test_request_context_new() {
        let ctx = RequestContext::new();

        assert!(!ctx.request_id.is_empty());
    }

    #[test]
    fn test_request_context_unique_ids() {
        let ctx1 = RequestContext::new();
        let ctx2 = RequestContext::new();

        assert_ne!(ctx1.request_id, ctx2.request_id);
    }

    // ==================== Builder Pattern Tests ====================

    #[test]
    fn test_with_user_id() {
        let ctx = RequestContext::new().with_user_id("user-123");

        assert_eq!(ctx.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_with_user_id_string_owned() {
        let ctx = RequestContext::new().with_user_id(String::from("user-456"));

        assert_eq!(ctx.user_id, Some("user-456".to_string()));
    }

    #[test]
    fn test_with_client_ip() {
        let ctx = RequestContext::new().with_client_ip("192.168.1.1");

        assert_eq!(ctx.client_ip, Some("192.168.1.1".to_string()));
    }

    #[test]
    fn test_with_client_ip_ipv6() {
        let ctx = RequestContext::new().with_client_ip("::1");

        assert_eq!(ctx.client_ip, Some("::1".to_string()));
    }

    #[test]
    fn test_with_user_agent() {
        let ctx = RequestContext::new().with_user_agent("Mozilla/5.0");

        assert_eq!(ctx.user_agent, Some("Mozilla/5.0".to_string()));
    }

    #[test]
    fn test_with_header() {
        let ctx = RequestContext::new().with_header("Authorization", "Bearer token");

        assert_eq!(
            ctx.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
    }

    #[test]
    fn test_with_multiple_headers() {
        let ctx = RequestContext::new()
            .with_header("Content-Type", "application/json")
            .with_header("Accept", "application/json")
            .with_header("X-Request-ID", "req-123");

        assert_eq!(ctx.headers.len(), 3);
        assert_eq!(
            ctx.headers.get("Content-Type"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_with_metadata() {
        let ctx = RequestContext::new().with_metadata("key", serde_json::json!("value"));

        assert_eq!(ctx.metadata.get("key"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn test_with_metadata_complex_value() {
        let ctx = RequestContext::new()
            .with_metadata("config", serde_json::json!({"enabled": true, "count": 5}));

        let config = ctx.metadata.get("config").unwrap();
        assert_eq!(config["enabled"], true);
        assert_eq!(config["count"], 5);
    }

    #[test]
    fn test_with_trace_id() {
        let ctx = RequestContext::new().with_trace_id("trace-abc-123");

        assert_eq!(ctx.trace_id, Some("trace-abc-123".to_string()));
    }

    #[test]
    fn test_with_user_and_team_metadata() {
        let user_id = Uuid::new_v4();
        let team_id = Uuid::new_v4();

        let ctx = RequestContext::new().with_user(user_id, Some(team_id));

        assert_eq!(ctx.user_id, Some(user_id.to_string()));
        assert_eq!(ctx.team_id(), Some(team_id));
    }

    #[test]
    fn test_with_api_key_metadata() {
        let api_key_id = Uuid::new_v4();

        let ctx = RequestContext::new().with_api_key(api_key_id);

        assert_eq!(ctx.api_key_id(), Some(api_key_id));
    }

    #[test]
    fn test_clear_team_and_api_key_metadata() {
        let mut ctx = RequestContext::new()
            .with_user(Uuid::new_v4(), Some(Uuid::new_v4()))
            .with_api_key(Uuid::new_v4());

        ctx.clear_team_id();
        ctx.clear_api_key_id();

        assert!(ctx.team_id().is_none());
        assert!(ctx.api_key_id().is_none());
    }

    // ==================== Chaining Tests ====================

    #[test]
    fn test_builder_chaining() {
        let ctx = RequestContext::new()
            .with_user_id("user-1")
            .with_client_ip("10.0.0.1")
            .with_user_agent("TestAgent/1.0")
            .with_header("X-Custom", "value")
            .with_metadata("priority", serde_json::json!(1))
            .with_trace_id("trace-001");

        assert_eq!(ctx.user_id, Some("user-1".to_string()));
        assert_eq!(ctx.client_ip, Some("10.0.0.1".to_string()));
        assert_eq!(ctx.user_agent, Some("TestAgent/1.0".to_string()));
        assert_eq!(ctx.headers.get("X-Custom"), Some(&"value".to_string()));
        assert_eq!(ctx.metadata.get("priority"), Some(&serde_json::json!(1)));
        assert_eq!(ctx.trace_id, Some("trace-001".to_string()));
    }

    // ==================== Elapsed Time Tests ====================

    #[test]
    fn test_elapsed_returns_duration() {
        let ctx = RequestContext::new();

        // Give a small delay
        std::thread::sleep(std::time::Duration::from_millis(10));

        let elapsed = ctx.elapsed();
        assert!(elapsed.as_millis() >= 10);
    }

    #[test]
    fn test_elapsed_is_non_negative() {
        let ctx = RequestContext::new();
        let elapsed = ctx.elapsed();

        assert!(elapsed >= Duration::ZERO);
    }

    // ==================== Clone Tests ====================

    #[test]
    fn test_request_context_clone() {
        let ctx = RequestContext::new()
            .with_user_id("user-clone")
            .with_client_ip("127.0.0.1");

        let cloned = ctx.clone();

        assert_eq!(ctx.request_id, cloned.request_id);
        assert_eq!(ctx.user_id, cloned.user_id);
        assert_eq!(ctx.client_ip, cloned.client_ip);
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_empty_user_id() {
        let ctx = RequestContext::new().with_user_id("");

        assert_eq!(ctx.user_id, Some("".to_string()));
    }

    #[test]
    fn test_header_override() {
        let ctx = RequestContext::new()
            .with_header("Key", "value1")
            .with_header("Key", "value2");

        assert_eq!(ctx.headers.get("Key"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_metadata_override() {
        let ctx = RequestContext::new()
            .with_metadata("key", serde_json::json!(1))
            .with_metadata("key", serde_json::json!(2));

        assert_eq!(ctx.metadata.get("key"), Some(&serde_json::json!(2)));
    }

    #[test]
    fn test_request_id_is_uuid_format() {
        let ctx = RequestContext::new();

        // UUID format: 8-4-4-4-12 (36 chars with hyphens)
        assert_eq!(ctx.request_id.len(), 36);
        assert!(ctx.request_id.contains('-'));
    }
}
