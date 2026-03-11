use crate::core::providers::unified_provider::ProviderError;
use std::collections::HashMap;
use std::env;

pub struct AuthUtils;

impl AuthUtils {
    pub fn check_valid_key(model: &str, api_key: &str) -> Result<bool, ProviderError> {
        if api_key.is_empty() {
            return Err(ProviderError::Authentication {
                provider: "unknown",
                message: "API key cannot be empty".to_string(),
            });
        }

        match model {
            model if model.starts_with("gpt-") => Self::validate_openai_key(api_key),
            model if model.starts_with("claude-") => Self::validate_anthropic_key(api_key),
            model if model.starts_with("gemini-") => Self::validate_google_key(api_key),
            _ => Ok(api_key.len() >= 8),
        }
    }

    pub fn validate_openai_key(api_key: &str) -> Result<bool, ProviderError> {
        if !api_key.starts_with("sk-") {
            return Err(ProviderError::Authentication {
                provider: "openai",
                message: "OpenAI API key must start with 'sk-'".to_string(),
            });
        }

        if api_key.len() < 20 {
            return Err(ProviderError::Authentication {
                provider: "openai",
                message: "OpenAI API key too short".to_string(),
            });
        }

        Ok(true)
    }

    pub fn validate_anthropic_key(api_key: &str) -> Result<bool, ProviderError> {
        if !api_key.starts_with("sk-ant-") {
            return Err(ProviderError::Authentication {
                provider: "anthropic",
                message: "Anthropic API key must start with 'sk-ant-'".to_string(),
            });
        }

        Ok(true)
    }

    pub fn validate_google_key(api_key: &str) -> Result<bool, ProviderError> {
        if api_key.len() < 10 {
            return Err(ProviderError::Authentication {
                provider: "google",
                message: "Google API key too short".to_string(),
            });
        }

        Ok(true)
    }

    pub fn load_credentials_from_list(
        kwargs: &mut HashMap<String, String>,
    ) -> Result<(), ProviderError> {
        if let Some(credential_name) = kwargs.get("credential_name").cloned() {
            let env_key = format!("{}_API_KEY", credential_name.to_uppercase());

            if let Ok(api_key) = env::var(&env_key) {
                kwargs.insert("api_key".to_string(), api_key);
            }

            let env_base = format!("{}_API_BASE", credential_name.to_uppercase());
            if let Ok(api_base) = env::var(&env_base) {
                kwargs.insert("api_base".to_string(), api_base);
            }

            let env_version = format!("{}_API_VERSION", credential_name.to_uppercase());
            if let Ok(api_version) = env::var(&env_version) {
                kwargs.insert("api_version".to_string(), api_version);
            }
        }

        Ok(())
    }

    pub fn get_api_key_from_env(provider: &str) -> Option<String> {
        let env_vars = match provider.to_lowercase().as_str() {
            "openai" => vec!["OPENAI_API_KEY", "OPENAI_KEY"],
            "anthropic" => vec!["ANTHROPIC_API_KEY", "CLAUDE_API_KEY"],
            "google" => vec!["GOOGLE_API_KEY", "GEMINI_API_KEY"],
            "azure" => vec!["AZURE_API_KEY", "AZURE_OPENAI_API_KEY"],
            "cohere" => vec!["COHERE_API_KEY"],
            "mistral" => vec!["MISTRAL_API_KEY"],
            _ => vec!["API_KEY"],
        };

        for env_var in env_vars {
            if let Ok(key) = env::var(env_var)
                && !key.is_empty()
            {
                return Some(key);
            }
        }

        None
    }

    pub fn mask_api_key(api_key: &str) -> String {
        if api_key.len() <= 8 {
            return "*".repeat(api_key.len());
        }

        let start = &api_key[..4];
        let end = &api_key[api_key.len() - 4..];
        let middle = "*".repeat(api_key.len() - 8);

        format!("{}{}{}", start, middle, end)
    }

    pub fn validate_environment_for_provider(provider: &str) -> Result<(), ProviderError> {
        match provider.to_lowercase().as_str() {
            "openai" => {
                if Self::get_api_key_from_env("openai").is_none() {
                    return Err(ProviderError::InvalidRequest {
                        provider: "openai",
                        message: "Missing OpenAI API key. Set OPENAI_API_KEY environment variable"
                            .to_string(),
                    });
                }
            }
            "anthropic" => {
                if Self::get_api_key_from_env("anthropic").is_none() {
                    return Err(ProviderError::InvalidRequest {
                        provider: "anthropic",
                        message:
                            "Missing Anthropic API key. Set ANTHROPIC_API_KEY environment variable"
                                .to_string(),
                    });
                }
            }
            "google" => {
                if Self::get_api_key_from_env("google").is_none() {
                    return Err(ProviderError::InvalidRequest {
                        provider: "google",
                        message: "Missing Google API key. Set GOOGLE_API_KEY environment variable"
                            .to_string(),
                    });
                }
            }
            _ => {}
        }

        Ok(())
    }

    pub fn get_bearer_token(api_key: &str) -> String {
        if api_key.starts_with("Bearer ") {
            api_key.to_string()
        } else {
            format!("Bearer {}", api_key)
        }
    }

    pub fn extract_api_key_from_bearer(bearer_token: &str) -> String {
        if bearer_token.starts_with("Bearer ") {
            bearer_token
                .strip_prefix("Bearer ")
                .unwrap_or(bearer_token)
                .to_string()
        } else {
            bearer_token.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_openai_key() {
        assert!(AuthUtils::validate_openai_key("sk-1234567890abcdef1234").is_ok());
        assert!(AuthUtils::validate_openai_key("invalid-key").is_err());
        assert!(AuthUtils::validate_openai_key("sk-short").is_err());
    }

    #[test]
    fn test_validate_anthropic_key() {
        assert!(AuthUtils::validate_anthropic_key("sk-ant-api03-1234567890").is_ok());
        assert!(AuthUtils::validate_anthropic_key("invalid-key").is_err());
    }

    #[test]
    fn test_mask_api_key() {
        let key = "sk-1234567890abcdef";
        let masked = AuthUtils::mask_api_key(key);
        assert_eq!(masked, "sk-1***********cdef");

        let short_key = "short";
        let masked_short = AuthUtils::mask_api_key(short_key);
        assert_eq!(masked_short, "*****");
    }

    #[test]
    fn test_bearer_token() {
        let key = "sk-1234567890";
        let bearer = AuthUtils::get_bearer_token(key);
        assert_eq!(bearer, "Bearer sk-1234567890");

        let already_bearer = "Bearer sk-1234567890";
        let bearer2 = AuthUtils::get_bearer_token(already_bearer);
        assert_eq!(bearer2, "Bearer sk-1234567890");
    }
}
