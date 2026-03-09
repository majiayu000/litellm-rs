//! A2A Agent Configuration
//!
//! Configuration types for A2A agents including authentication and provider settings.

use crate::core::types::config::defaults::default_true;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Agent provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    /// Generic A2A-compatible agent
    #[default]
    A2A,

    /// LangGraph agent (LangChain)
    LangGraph,

    /// Google Vertex AI Agent Engine
    VertexAI,

    /// Azure AI Foundry agent
    AzureAIFoundry,

    /// AWS Bedrock AgentCore
    BedrockAgentCore,

    /// Pydantic AI agent
    PydanticAI,

    /// Custom provider
    Custom,
}

impl AgentProvider {
    /// Get provider display name
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentProvider::A2A => "A2A",
            AgentProvider::LangGraph => "LangGraph",
            AgentProvider::VertexAI => "Vertex AI Agent Engine",
            AgentProvider::AzureAIFoundry => "Azure AI Foundry",
            AgentProvider::BedrockAgentCore => "Bedrock AgentCore",
            AgentProvider::PydanticAI => "Pydantic AI",
            AgentProvider::Custom => "Custom",
        }
    }

    /// Check if provider supports streaming
    pub fn supports_streaming(&self) -> bool {
        matches!(
            self,
            AgentProvider::LangGraph
                | AgentProvider::VertexAI
                | AgentProvider::AzureAIFoundry
                | AgentProvider::A2A
        )
    }

    /// Check if provider supports async tasks
    pub fn supports_async_tasks(&self) -> bool {
        matches!(
            self,
            AgentProvider::LangGraph | AgentProvider::BedrockAgentCore | AgentProvider::A2A
        )
    }
}

impl std::fmt::Display for AgentProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for AgentProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "a2a" | "generic" => Ok(AgentProvider::A2A),
            "langgraph" | "langchain" => Ok(AgentProvider::LangGraph),
            "vertex" | "vertexai" | "vertex_ai" | "google" => Ok(AgentProvider::VertexAI),
            "azure" | "azureai" | "azure_ai_foundry" => Ok(AgentProvider::AzureAIFoundry),
            "bedrock" | "aws" | "bedrock_agentcore" => Ok(AgentProvider::BedrockAgentCore),
            "pydantic" | "pydanticai" | "pydantic_ai" => Ok(AgentProvider::PydanticAI),
            "custom" => Ok(AgentProvider::Custom),
            _ => Err(format!("Unknown agent provider: {}", s)),
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent name/identifier
    pub name: String,

    /// Agent provider type
    #[serde(default)]
    pub provider: AgentProvider,

    /// Agent URL (invocation endpoint)
    pub url: String,

    /// API key for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// Additional headers
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub headers: HashMap<String, String>,

    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// Whether this agent is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Agent description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Agent capabilities
    #[serde(default)]
    pub capabilities: AgentCapabilities,

    /// Rate limit (requests per minute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_rpm: Option<u32>,

    /// Cost per request (USD)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_request: Option<f64>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Provider-specific configuration
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub provider_config: HashMap<String, serde_json::Value>,
}

fn default_timeout() -> u64 {
    60000 // 60 seconds (agents can be slow)
}

fn default_enabled() -> bool {
    true
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            provider: AgentProvider::default(),
            url: String::new(),
            api_key: None,
            headers: HashMap::new(),
            timeout_ms: default_timeout(),
            enabled: true,
            description: None,
            capabilities: AgentCapabilities::default(),
            rate_limit_rpm: None,
            cost_per_request: None,
            tags: Vec::new(),
            provider_config: HashMap::new(),
        }
    }
}

impl AgentConfig {
    /// Create a new agent config
    pub fn new(name: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            ..Default::default()
        }
    }

    /// Set provider type
    pub fn with_provider(mut self, provider: AgentProvider) -> Self {
        self.provider = provider;
        self
    }

    /// Set API key
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Add header
    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Agent name cannot be empty".to_string());
        }
        if self.url.is_empty() {
            return Err("Agent URL cannot be empty".to_string());
        }
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(format!(
                "Agent URL must start with http:// or https://, got: {}",
                self.url
            ));
        }

        // SSRF protection: extract and validate the host
        let host = extract_url_host(&self.url)
            .ok_or_else(|| format!("Agent URL has an invalid or missing host: {}", self.url))?;

        if is_private_or_reserved_host(&host) {
            return Err(format!(
                "Agent URL targets a private or reserved address '{}', which is not allowed (SSRF protection)",
                host
            ));
        }

        Ok(())
    }
}

/// Extract the host portion from a URL string.
///
/// Supports `http://` and `https://` schemes. Returns `None` if the host
/// cannot be determined.
fn extract_url_host(url: &str) -> Option<String> {
    // Strip scheme
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;

    // Take everything up to the first '/', '?', or '#' to isolate host[:port]
    let authority = after_scheme.split(['/', '?', '#']).next()?;

    if authority.is_empty() {
        return None;
    }

    // Strip optional port number
    // Handle IPv6 addresses like [::1]:8080
    let host = if authority.starts_with('[') {
        // IPv6 bracketed: [addr] or [addr]:port
        let end_bracket = authority.find(']')?;
        &authority[1..end_bracket]
    } else {
        // IPv4 or hostname: strip trailing :port
        match authority.rfind(':') {
            Some(pos) => &authority[..pos],
            None => authority,
        }
    };

    Some(host.to_lowercase())
}

/// Returns `true` if the host is a private, loopback, link-local, or otherwise
/// reserved address that must not be reachable from the gateway (SSRF guard).
///
/// Covered ranges:
/// - 127.0.0.0/8   — IPv4 loopback
/// - 10.0.0.0/8    — RFC 1918 private
/// - 172.16.0.0/12 — RFC 1918 private
/// - 192.168.0.0/16 — RFC 1918 private
/// - 169.254.0.0/16 — IPv4 link-local / cloud metadata (169.254.169.254)
/// - 0.0.0.0        — unspecified
/// - ::1            — IPv6 loopback
/// - fc00::/7       — IPv6 unique-local (fc00:: – fdff::)
/// - localhost and common internal hostnames
fn is_private_or_reserved_host(host: &str) -> bool {
    // Reject well-known internal hostnames directly
    if host == "localhost"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host == "169.254.169.254"
    {
        return true;
    }

    // Try to parse as an IP address and apply CIDR checks
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_private_or_reserved_ip(ip);
    }

    false
}

/// Check whether a parsed IP address falls within private or reserved ranges.
fn is_private_or_reserved_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // 0.0.0.0 — unspecified
            if octets == [0, 0, 0, 0] {
                return true;
            }
            // 127.0.0.0/8 — loopback
            if octets[0] == 127 {
                return true;
            }
            // 10.0.0.0/8 — RFC 1918
            if octets[0] == 10 {
                return true;
            }
            // 172.16.0.0/12 — RFC 1918 (172.16.x.x – 172.31.x.x)
            if octets[0] == 172 && (16..=31).contains(&octets[1]) {
                return true;
            }
            // 192.168.0.0/16 — RFC 1918
            if octets[0] == 192 && octets[1] == 168 {
                return true;
            }
            // 169.254.0.0/16 — link-local / AWS/GCP metadata endpoint
            if octets[0] == 169 && octets[1] == 254 {
                return true;
            }
            false
        }
        IpAddr::V6(v6) => {
            // ::1 — loopback
            if v6.is_loopback() {
                return true;
            }
            // fc00::/7 — unique-local (covers fc00:: and fd00::)
            let segments = v6.segments();
            if (segments[0] & 0xfe00) == 0xfc00 {
                return true;
            }
            // ::ffff:0:0/96 — IPv4-mapped; recurse with the embedded IPv4 address
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_private_or_reserved_ip(IpAddr::V4(v4));
            }
            false
        }
    }
}

/// Agent capabilities
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentCapabilities {
    /// Supports streaming responses
    #[serde(default)]
    pub streaming: bool,

    /// Supports push notifications
    #[serde(default)]
    pub push_notifications: bool,

    /// Supports task cancellation
    #[serde(default)]
    pub task_cancellation: bool,

    /// Supports multi-turn conversations
    #[serde(default = "default_true")]
    pub multi_turn: bool,

    /// Supports file attachments
    #[serde(default)]
    pub file_attachments: bool,

    /// Maximum input length
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_length: Option<u32>,

    /// Supported input content types
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_types: Vec<String>,

    /// Supported output content types
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub output_types: Vec<String>,
}

impl AgentCapabilities {
    /// Create capabilities with all features enabled
    pub fn full() -> Self {
        Self {
            streaming: true,
            push_notifications: true,
            task_cancellation: true,
            multi_turn: true,
            file_attachments: true,
            max_input_length: None,
            input_types: vec!["text".to_string(), "image".to_string()],
            output_types: vec!["text".to_string(), "image".to_string()],
        }
    }

    /// Create minimal capabilities
    pub fn minimal() -> Self {
        Self {
            streaming: false,
            push_notifications: false,
            task_cancellation: false,
            multi_turn: false,
            file_attachments: false,
            max_input_length: None,
            input_types: vec!["text".to_string()],
            output_types: vec!["text".to_string()],
        }
    }
}

/// A2A Gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct A2AGatewayConfig {
    /// Registered agents
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,

    /// Default timeout for all agents
    #[serde(default = "default_timeout")]
    pub default_timeout_ms: u64,

    /// Enable request logging
    #[serde(default = "default_true")]
    pub enable_logging: bool,

    /// Enable cost tracking
    #[serde(default)]
    pub enable_cost_tracking: bool,

    /// Global rate limit
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_rate_limit: Option<u32>,
}

impl A2AGatewayConfig {
    /// Add an agent to the configuration
    pub fn add_agent(&mut self, config: AgentConfig) {
        self.agents.insert(config.name.clone(), config);
    }

    /// Get an agent by name
    pub fn get_agent(&self, name: &str) -> Option<&AgentConfig> {
        self.agents.get(name)
    }

    /// Validate all agent configurations
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let errors: Vec<String> = self
            .agents
            .values()
            .filter_map(|a| a.validate().err())
            .collect();

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_provider_display() {
        assert_eq!(AgentProvider::LangGraph.display_name(), "LangGraph");
        assert_eq!(
            AgentProvider::VertexAI.display_name(),
            "Vertex AI Agent Engine"
        );
    }

    #[test]
    fn test_agent_provider_from_str() {
        assert_eq!(
            "langgraph".parse::<AgentProvider>().unwrap(),
            AgentProvider::LangGraph
        );
        assert_eq!(
            "vertex".parse::<AgentProvider>().unwrap(),
            AgentProvider::VertexAI
        );
        assert_eq!(
            "azure".parse::<AgentProvider>().unwrap(),
            AgentProvider::AzureAIFoundry
        );
        assert_eq!(
            "bedrock".parse::<AgentProvider>().unwrap(),
            AgentProvider::BedrockAgentCore
        );
    }

    #[test]
    fn test_agent_provider_streaming_support() {
        assert!(AgentProvider::LangGraph.supports_streaming());
        assert!(AgentProvider::VertexAI.supports_streaming());
        assert!(!AgentProvider::BedrockAgentCore.supports_streaming());
    }

    #[test]
    fn test_agent_config_new() {
        let config = AgentConfig::new("my-agent", "https://api.example.com/agent");
        assert_eq!(config.name, "my-agent");
        assert_eq!(config.url, "https://api.example.com/agent");
        assert!(config.enabled);
    }

    #[test]
    fn test_agent_config_builder() {
        let config = AgentConfig::new("my-agent", "https://api.example.com/agent")
            .with_provider(AgentProvider::LangGraph)
            .with_api_key("sk-test123")
            .with_timeout(30000)
            .with_description("Test agent");

        assert_eq!(config.provider, AgentProvider::LangGraph);
        assert_eq!(config.api_key.as_deref(), Some("sk-test123"));
        assert_eq!(config.timeout_ms, 30000);
        assert!(config.description.is_some());
    }

    #[test]
    fn test_agent_config_validation() {
        // Valid config
        let config = AgentConfig::new("test", "https://example.com");
        assert!(config.validate().is_ok());

        // Empty name
        let config = AgentConfig::new("", "https://example.com");
        assert!(config.validate().is_err());

        // Empty URL
        let config = AgentConfig::new("test", "");
        assert!(config.validate().is_err());

        // Invalid URL
        let config = AgentConfig::new("test", "ftp://example.com");
        assert!(config.validate().is_err());
    }

    // ==================== SSRF protection tests ====================

    #[test]
    fn test_ssrf_loopback_ipv4_rejected() {
        let config = AgentConfig::new("test", "http://127.0.0.1/api");
        let err = config.validate().unwrap_err();
        assert!(err.contains("private or reserved"), "got: {}", err);
    }

    #[test]
    fn test_ssrf_loopback_ipv4_any_port_rejected() {
        let config = AgentConfig::new("test", "https://127.0.0.1:8080/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_localhost_hostname_rejected() {
        let config = AgentConfig::new("test", "http://localhost/api");
        let err = config.validate().unwrap_err();
        assert!(err.contains("private or reserved"), "got: {}", err);
    }

    #[test]
    fn test_ssrf_localhost_subdomain_rejected() {
        let config = AgentConfig::new("test", "http://my.localhost/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_rfc1918_10_rejected() {
        let config = AgentConfig::new("test", "http://10.0.0.1/api");
        assert!(config.validate().is_err());

        let config = AgentConfig::new("test", "http://10.255.255.255/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_rfc1918_172_rejected() {
        let config = AgentConfig::new("test", "http://172.16.0.1/api");
        assert!(config.validate().is_err());

        let config = AgentConfig::new("test", "http://172.31.255.255/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_rfc1918_172_boundary_allowed() {
        // 172.15.x.x is NOT in RFC 1918 range
        let config = AgentConfig::new("test", "https://172.15.0.1/api");
        assert!(config.validate().is_ok());

        // 172.32.x.x is NOT in RFC 1918 range
        let config = AgentConfig::new("test", "https://172.32.0.1/api");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ssrf_rfc1918_192_168_rejected() {
        let config = AgentConfig::new("test", "http://192.168.1.1/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_link_local_metadata_rejected() {
        // Cloud metadata endpoint
        let config = AgentConfig::new("test", "http://169.254.169.254/latest/meta-data/");
        assert!(config.validate().is_err());

        let config = AgentConfig::new("test", "http://169.254.0.1/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_unspecified_ipv4_rejected() {
        let config = AgentConfig::new("test", "http://0.0.0.0/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_ipv6_loopback_rejected() {
        let config = AgentConfig::new("test", "http://[::1]/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_ipv6_unique_local_rejected() {
        let config = AgentConfig::new("test", "http://[fc00::1]/api");
        assert!(config.validate().is_err());

        let config = AgentConfig::new("test", "http://[fd00::1]/api");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_google_metadata_hostname_rejected() {
        let config = AgentConfig::new(
            "test",
            "http://metadata.google.internal/computeMetadata/v1/",
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ssrf_public_ip_allowed() {
        // 8.8.8.8 is a public Google DNS — should be allowed
        let config = AgentConfig::new("test", "https://8.8.8.8/api");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ssrf_public_domain_allowed() {
        let config = AgentConfig::new("test", "https://api.example.com/v1/agent");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_extract_url_host_basic() {
        assert_eq!(
            extract_url_host("https://example.com/path"),
            Some("example.com".to_string())
        );
        assert_eq!(
            extract_url_host("http://10.0.0.1:8080/api"),
            Some("10.0.0.1".to_string())
        );
        assert_eq!(
            extract_url_host("http://[::1]:9000/api"),
            Some("::1".to_string())
        );
        assert_eq!(extract_url_host("ftp://example.com"), None);
        assert_eq!(extract_url_host("http://"), None);
    }

    #[test]
    fn test_is_private_or_reserved_ip_public() {
        use std::net::Ipv4Addr;
        assert!(!is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            8, 8, 8, 8
        ))));
        assert!(!is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            1, 1, 1, 1
        ))));
    }

    #[test]
    fn test_is_private_or_reserved_ip_private() {
        use std::net::Ipv4Addr;
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            127, 0, 0, 1
        ))));
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            10, 1, 2, 3
        ))));
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            172, 20, 0, 1
        ))));
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            192, 168, 0, 1
        ))));
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            169, 254, 169, 254
        ))));
        assert!(is_private_or_reserved_ip(IpAddr::V4(Ipv4Addr::new(
            0, 0, 0, 0
        ))));
    }

    #[test]
    fn test_agent_capabilities_full() {
        let caps = AgentCapabilities::full();
        assert!(caps.streaming);
        assert!(caps.push_notifications);
        assert!(caps.task_cancellation);
        assert!(caps.multi_turn);
        assert!(caps.file_attachments);
    }

    #[test]
    fn test_agent_capabilities_minimal() {
        let caps = AgentCapabilities::minimal();
        assert!(!caps.streaming);
        assert!(!caps.push_notifications);
    }

    #[test]
    fn test_gateway_config() {
        let mut config = A2AGatewayConfig::default();
        config.add_agent(AgentConfig::new("agent1", "https://example.com/agent1"));

        assert!(config.get_agent("agent1").is_some());
        assert!(config.get_agent("nonexistent").is_none());
    }

    #[test]
    fn test_config_serialization() {
        let config =
            AgentConfig::new("test", "https://example.com").with_provider(AgentProvider::LangGraph);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "test");
        assert_eq!(deserialized.provider, AgentProvider::LangGraph);
    }
}
