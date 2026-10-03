//! Module

use crate::sdk::{config::*, errors::*};
use std::collections::HashMap;
use std::env;

/// Configuration
pub struct AutoConfig;

impl AutoConfig {
    /// Create
    /// 
    /// # Example
    /// ```
    /// Model
    /// // - "openrouter/google/palm-2-chat-bison"
    /// // - "anthropic/claude-3-sonnet-20240229"  
    /// // - "openai/gpt-4"
    /// // - "azure/gpt-35-turbo"
    /// ```
    pub fn from_model(model: &str) -> Result<ClientConfig> {
        let (provider_type, provider_id, actual_model) = Self::parse_model_name(model)?;
        
        let provider_config = Self::create_provider_config(
            provider_id,
            provider_type,
            actual_model,
        )?;
        
        Ok(ConfigBuilder::new()
            .add_provider(provider_config)
            .default_provider(&provider_id)
            .build())
    }
    
    /// Model
    fn parse_model_name(model: &str) -> Result<(ProviderType, String, String)> {
        let parts: Vec<&str> = model.splitn(2, '/').collect();
        
        if parts.len() < 2 {
            return Err(SDKError::ConfigError(
                format!("Invalid model format '{}'. Expected format: 'provider/model'", model)
            ));
        }
        
        let provider_prefix = parts[0];
        let model_name = parts[1];
        
        let provider_type = match provider_prefix {
            "openai" => ProviderType::OpenAI,
            "anthropic" => ProviderType::Anthropic,
            "openrouter" => ProviderType::OpenAI, // OpenRouter uses OpenAI compatible API
            "azure" => ProviderType::Azure,
            "google" => ProviderType::Google,
            "cohere" => ProviderType::Cohere,
            "mistral" => ProviderType::Mistral,
            "groq" => ProviderType::OpenAI, // Groq also uses OpenAI compatible API
            "perplexity" => ProviderType::OpenAI,
            "together" => ProviderType::OpenAI,
            "fireworks" => ProviderType::OpenAI,
            "deepinfra" => ProviderType::OpenAI,
            "anyscale" => ProviderType::OpenAI,
            _ => {
                return Err(SDKError::ConfigError(
                    format!("Unsupported provider: '{}'. Supported providers: openai, anthropic, openrouter, azure, google, cohere, mistral, groq, perplexity, together, fireworks, deepinfra, anyscale", provider_prefix)
                ));
            }
        };
        
        let provider_id = provider_prefix.to_string();
        let actual_model = model_name.to_string();
        
        Ok((provider_type, provider_id, actual_model))
    }
    
    /// Create
    fn create_provider_config(
        provider_id: String,
        provider_type: ProviderType,
        model: String,
    ) -> Result<ProviderConfig> {
        let (api_key, base_url, extra_settings) = Self::get_provider_auth_config(&provider_id)?;
        
        Ok(ProviderConfig {
            id: provider_id.clone(),
            provider_type,
            name: Self::get_provider_display_name(&provider_id),
            api_key,
            base_url: Some(base_url),
            models: vec![model],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: extra_settings,
        })
    }
    
    /// Configuration
    fn get_provider_auth_config(provider_id: &str) -> Result<(String, String, HashMap<String, String>)> {
        let mut settings = HashMap::new();
        
        let (api_key_env, base_url, extra_env_vars) = match provider_id {
            "openai" => (
                "OPENAI_API_KEY",
                "https://api.openai.com/v1",
                vec![]
            ),
            "anthropic" => (
                "ANTHROPIC_API_KEY", 
                "https://api.anthropic.com",
                vec![]
            ),
            "openrouter" => {
                // Handle
                if let Ok(site_url) = env::var("OR_SITE_URL") {
                    settings.insert("site_url".to_string(), site_url);
                }
                if let Ok(app_name) = env::var("OR_APP_NAME") {
                    settings.insert("app_name".to_string(), app_name);
                }
                (
                    "OPENROUTER_API_KEY",
                    &env::var("OPENROUTER_API_BASE").unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
                    vec!["OR_SITE_URL", "OR_APP_NAME"]
                )
            },
            "azure" => (
                "AZURE_API_KEY",
                "https://your-resource-name.openai.azure.com",
                vec!["AZURE_API_VERSION", "AZURE_DEPLOYMENT_NAME"]
            ),
            "google" => (
                "GOOGLE_API_KEY",
                "https://generativelanguage.googleapis.com/v1beta",
                vec![]
            ),
            "cohere" => (
                "COHERE_API_KEY",
                "https://api.cohere.ai/v1",
                vec![]
            ),
            "mistral" => (
                "MISTRAL_API_KEY",
                "https://api.mistral.ai/v1",
                vec![]
            ),
            "groq" => (
                "GROQ_API_KEY",
                "https://api.groq.com/openai/v1",
                vec![]
            ),
            "perplexity" => (
                "PERPLEXITY_API_KEY",
                "https://api.perplexity.ai",
                vec![]
            ),
            "together" => (
                "TOGETHER_API_KEY", 
                "https://api.together.ai/v1",
                vec![]
            ),
            "fireworks" => (
                "FIREWORKS_API_KEY",
                "https://api.fireworks.ai/inference/v1",
                vec![]
            ),
            "deepinfra" => (
                "DEEPINFRA_API_KEY",
                "https://api.deepinfra.com/v1/openai",
                vec![]
            ),
            "anyscale" => (
                "ANYSCALE_API_KEY",
                "https://api.endpoints.anyscale.com/v1",
                vec![]
            ),
            _ => {
                return Err(SDKError::ConfigError(
                    format!("Unknown provider: {}", provider_id)
                ));
            }
        };
        
        // Get
        let api_key = env::var(api_key_env)
            .map_err(|_| SDKError::ConfigError(
                format!("Environment variable '{}' not found. Please set your {} API key.", api_key_env, provider_id.to_uppercase())
            ))?;
        
        // Handle
        let final_base_url = if provider_id == "openrouter" {
            env::var("OPENROUTER_API_BASE").unwrap_or_else(|_| base_url.to_string())
        } else {
            base_url.to_string()
        };
        
        // Get
        for env_var in extra_env_vars {
            if let Ok(value) = env::var(env_var) {
                settings.insert(env_var.to_lowercase(), value);
            }
        }
        
        Ok((api_key, final_base_url, settings))
    }
    
    /// Get
    fn get_provider_display_name(provider_id: &str) -> String {
        match provider_id {
            "openai" => "OpenAI".to_string(),
            "anthropic" => "Anthropic (Claude)".to_string(),
            "openrouter" => "OpenRouter".to_string(),
            "azure" => "Azure OpenAI".to_string(),
            "google" => "Google AI".to_string(),
            "cohere" => "Cohere".to_string(),
            "mistral" => "Mistral AI".to_string(),
            "groq" => "Groq".to_string(),
            "perplexity" => "Perplexity AI".to_string(),
            "together" => "Together AI".to_string(),
            "fireworks" => "Fireworks AI".to_string(),
            "deepinfra" => "DeepInfra".to_string(),
            "anyscale" => "Anyscale".to_string(),
            _ => provider_id.to_uppercase(),
        }
    }
    
    /// Check
    pub fn check_environment(model: &str) -> Result<()> {
        let (_, provider_id, _) = Self::parse_model_name(model)?;
        Self::get_provider_auth_config(&provider_id)?;
        Ok(())
    }
    
    /// List all supported providers
    pub fn supported_providers() -> Vec<&'static str> {
        vec![
            "openai", "anthropic", "openrouter", "azure", "google", 
            "cohere", "mistral", "groq", "perplexity", "together",
            "fireworks", "deepinfra", "anyscale"
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    #[test]
    fn test_parse_model_name() {
        let (provider_type, provider_id, model) = 
            AutoConfig::parse_model_name("openrouter/google/palm-2").unwrap();
        
        assert_eq!(provider_id, "openrouter");
        assert_eq!(model, "google/palm-2");
        assert!(matches!(provider_type, ProviderType::OpenAI));
    }
    
    #[test]
    fn test_invalid_model_format() {
        assert!(AutoConfig::parse_model_name("invalid-format").is_err());
        assert!(AutoConfig::parse_model_name("unsupported/model").is_err());
    }
    
    #[test] 
    fn test_check_environment() {
        // Settings
        env::set_var("OPENROUTER_API_KEY", "test-key");
        assert!(AutoConfig::check_environment("openrouter/gpt-3.5-turbo").is_ok());
        env::remove_var("OPENROUTER_API_KEY");
        
        assert!(AutoConfig::check_environment("openrouter/gpt-3.5-turbo").is_err());
    }
}