//! Convenience functions for common configurations

use super::types::{ProviderConfigBuilder, ServerConfigBuilder};
use crate::utils::error::gateway_error::Result;
use std::time::Duration;

/// Create a development server configuration
pub fn dev_server() -> ServerConfigBuilder {
    ServerConfigBuilder::new()
        .host("127.0.0.1")
        .port(8080)
        .workers(1)
        .enable_cors()
        .add_cors_origin("*")
}

/// Create a production server configuration
pub fn prod_server() -> ServerConfigBuilder {
    ServerConfigBuilder::new()
        .host("0.0.0.0")
        .port(8080)
        .workers(std::thread::available_parallelism().map_or(1, |n| n.get()))
        .max_connections(10000)
        .timeout(Duration::from_secs(60))
}

/// Create an OpenAI provider configuration
pub fn openai_provider(name: &str, api_key: &str) -> Result<ProviderConfigBuilder> {
    Ok(ProviderConfigBuilder::new()
        .name(name)?
        .provider_type("openai")?
        .api_key(api_key)
        .add_model("gpt-5.4")
        .add_model("gpt-5.5")
        .add_model("gpt-5.5-pro")
        .add_model("gpt-5.4-mini")
        .rate_limit(3000))
}

/// Create an Anthropic provider configuration
pub fn anthropic_provider(name: &str, api_key: &str) -> Result<ProviderConfigBuilder> {
    Ok(ProviderConfigBuilder::new()
        .name(name)?
        .provider_type("anthropic")?
        .api_key(api_key)
        .add_model("claude-opus-4-8")
        .add_model("claude-opus-4-7")
        .add_model("claude-sonnet-4-6")
        .add_model("claude-haiku-4-5-20251001")
        .rate_limit(1000))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== dev_server Tests ====================

    #[test]
    fn test_dev_server_host() {
        let builder = dev_server();
        assert_eq!(builder.host, Some("127.0.0.1".to_string()));
    }

    #[test]
    fn test_dev_server_port() {
        let builder = dev_server();
        assert_eq!(builder.port, Some(8080));
    }

    #[test]
    fn test_dev_server_workers() {
        let builder = dev_server();
        assert_eq!(builder.workers, Some(1));
    }

    #[test]
    fn test_dev_server_cors_enabled() {
        let builder = dev_server();
        assert!(builder.enable_cors);
    }

    #[test]
    fn test_dev_server_cors_origin() {
        let builder = dev_server();
        assert_eq!(builder.cors_origins, vec!["*"]);
    }

    #[test]
    fn test_dev_server_build() {
        let config = dev_server().build();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.workers, Some(1));
        assert!(config.cors.enabled);
    }

    // ==================== prod_server Tests ====================

    #[test]
    fn test_prod_server_host() {
        let builder = prod_server();
        assert_eq!(builder.host, Some("0.0.0.0".to_string()));
    }

    #[test]
    fn test_prod_server_port() {
        let builder = prod_server();
        assert_eq!(builder.port, Some(8080));
    }

    #[test]
    fn test_prod_server_workers() {
        let builder = prod_server();
        // Workers should be set to number of CPUs
        assert!(builder.workers.is_some());
        assert!(builder.workers.unwrap() >= 1);
    }

    #[test]
    fn test_prod_server_max_connections() {
        let builder = prod_server();
        assert_eq!(builder.max_connections, Some(10000));
    }

    #[test]
    fn test_prod_server_timeout() {
        let builder = prod_server();
        assert_eq!(builder.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_prod_server_build() {
        let config = prod_server().build();
        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 8080);
        assert_eq!(config.timeout, 60);
    }

    #[test]
    fn test_prod_server_cors_default() {
        let builder = prod_server();
        // CORS not explicitly enabled in prod_server
        assert!(!builder.enable_cors);
    }

    // ==================== openai_provider Tests ====================

    #[test]
    fn test_openai_provider_success() {
        let result = openai_provider("my-openai", "sk-test-key");
        assert!(result.is_ok());
    }

    #[test]
    fn test_openai_provider_name() {
        let builder = openai_provider("my-openai", "sk-test").unwrap();
        assert!(builder.name.is_some());
    }

    #[test]
    fn test_openai_provider_type() {
        let builder = openai_provider("test", "key").unwrap();
        assert!(builder.provider_type.is_some());
    }

    #[test]
    fn test_openai_provider_api_key() {
        let builder = openai_provider("test", "sk-my-api-key").unwrap();
        assert_eq!(builder.api_key, Some("sk-my-api-key".to_string()));
    }

    #[test]
    fn test_openai_provider_models() {
        let builder = openai_provider("test", "key").unwrap();
        assert_eq!(builder.models.len(), 4);
        assert_eq!(builder.models.first(), Some(&"gpt-5.4".to_string()));
        assert!(builder.models.contains(&"gpt-5.5".to_string()));
        assert!(builder.models.contains(&"gpt-5.5-pro".to_string()));
        assert!(builder.models.contains(&"gpt-5.4".to_string()));
        assert!(builder.models.contains(&"gpt-5.4-mini".to_string()));
    }

    #[test]
    fn test_openai_provider_rate_limit() {
        let builder = openai_provider("test", "key").unwrap();
        assert_eq!(builder.max_requests_per_minute, Some(3000));
    }

    #[test]
    fn test_openai_provider_build() {
        let config = openai_provider("openai-prod", "sk-real-key")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(config.name, "openai-prod");
        assert_eq!(config.provider_type, "openai");
        assert_eq!(config.api_key, "sk-real-key");
        assert_eq!(config.rpm, 3000);
    }

    #[test]
    fn test_openai_provider_empty_name_fails() {
        let result = openai_provider("", "key");
        assert!(result.is_err());
    }

    // ==================== anthropic_provider Tests ====================

    #[test]
    fn test_anthropic_provider_success() {
        let result = anthropic_provider("my-anthropic", "sk-ant-test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_anthropic_provider_name() {
        let builder = anthropic_provider("claude-provider", "key").unwrap();
        assert!(builder.name.is_some());
    }

    #[test]
    fn test_anthropic_provider_type() {
        let builder = anthropic_provider("test", "key").unwrap();
        assert!(builder.provider_type.is_some());
    }

    #[test]
    fn test_anthropic_provider_api_key() {
        let builder = anthropic_provider("test", "sk-ant-key123").unwrap();
        assert_eq!(builder.api_key, Some("sk-ant-key123".to_string()));
    }

    #[test]
    fn test_anthropic_provider_models() {
        let builder = anthropic_provider("test", "key").unwrap();
        assert_eq!(builder.models.len(), 4);
        assert!(builder.models.contains(&"claude-opus-4-8".to_string()));
        assert!(builder.models.contains(&"claude-opus-4-7".to_string()));
        assert!(builder.models.contains(&"claude-sonnet-4-6".to_string()));
        assert!(
            builder
                .models
                .contains(&"claude-haiku-4-5-20251001".to_string())
        );
    }

    #[test]
    fn test_anthropic_provider_rate_limit() {
        let builder = anthropic_provider("test", "key").unwrap();
        assert_eq!(builder.max_requests_per_minute, Some(1000));
    }

    #[test]
    fn test_anthropic_provider_build() {
        let config = anthropic_provider("anthropic-prod", "sk-ant-real")
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(config.name, "anthropic-prod");
        assert_eq!(config.provider_type, "anthropic");
        assert_eq!(config.api_key, "sk-ant-real");
        assert_eq!(config.rpm, 1000);
    }

    #[test]
    fn test_anthropic_provider_empty_name_fails() {
        let result = anthropic_provider("", "key");
        assert!(result.is_err());
    }

    // ==================== Comparison Tests ====================

    #[test]
    fn test_dev_vs_prod_workers() {
        let dev = dev_server();
        let prod = prod_server();

        // Dev has 1 worker, prod has multiple (CPU count)
        assert_eq!(dev.workers, Some(1));
        assert!(prod.workers.unwrap() >= 1);
    }

    #[test]
    fn test_dev_vs_prod_host() {
        let dev = dev_server();
        let prod = prod_server();

        // Dev binds to localhost, prod to all interfaces
        assert_eq!(dev.host, Some("127.0.0.1".to_string()));
        assert_eq!(prod.host, Some("0.0.0.0".to_string()));
    }

    #[test]
    fn test_openai_vs_anthropic_rate_limit() {
        let openai = openai_provider("o", "k").unwrap();
        let anthropic = anthropic_provider("a", "k").unwrap();

        // OpenAI has higher rate limit
        assert!(
            openai.max_requests_per_minute.unwrap() > anthropic.max_requests_per_minute.unwrap()
        );
    }
}
