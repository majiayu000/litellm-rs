use crate::core::providers::unified_provider::ProviderError;

use super::capabilities::ModelCapabilities;

pub struct ModelUtils;

impl ModelUtils {
    pub fn get_model_capabilities(model: &str) -> ModelCapabilities {
        let model_lower = model.to_lowercase();

        if model_lower.starts_with("gpt-5") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: true,
                supports_tool_choice: true,
                supports_response_schema: true,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: true,
                supports_streaming: true,
                max_tokens: Some(128000),
                context_window: Some(400000),
            }
        } else if model_lower.starts_with("gpt-image-") || model_lower.starts_with("chatgpt-image-")
        {
            ModelCapabilities {
                supports_function_calling: false,
                supports_parallel_function_calling: false,
                supports_tool_choice: false,
                supports_response_schema: false,
                supports_system_messages: false,
                supports_web_search: false,
                supports_url_context: false,
                supports_vision: true,
                supports_streaming: false,
                max_tokens: Some(16384),
                context_window: Some(128000),
            }
        } else if model_lower.starts_with("gpt-4.1") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: true,
                supports_tool_choice: true,
                supports_response_schema: true,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: true,
                supports_streaming: true,
                max_tokens: Some(32768),
                context_window: Some(128000),
            }
        } else if model_lower.starts_with("o3") || model_lower.starts_with("o4") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: true,
                supports_response_schema: true,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: true,
                supports_streaming: true,
                max_tokens: Some(100000),
                context_window: Some(200000),
            }
        } else if model_lower.starts_with("gpt-4") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: true,
                supports_tool_choice: true,
                supports_response_schema: true,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: model_lower.contains("vision") || model_lower.contains("turbo"),
                supports_streaming: true,
                max_tokens: Some(if model_lower.contains("32k") {
                    32768
                } else {
                    8192
                }),
                context_window: Some(if model_lower.contains("32k") {
                    32768
                } else {
                    8192
                }),
            }
        } else if model_lower.starts_with("gpt-3.5") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: true,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: false,
                supports_vision: false,
                supports_streaming: true,
                max_tokens: Some(if model_lower.contains("16k") {
                    16384
                } else {
                    4096
                }),
                context_window: Some(if model_lower.contains("16k") {
                    16384
                } else {
                    4096
                }),
            }
        } else if model_lower.starts_with("claude-opus-4-6") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: true,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: true,
                supports_streaming: true,
                max_tokens: Some(1_000_000),
                context_window: Some(1_000_000),
            }
        } else if model_lower.starts_with("claude-opus-4")
            || model_lower.starts_with("claude-sonnet-4")
            || model_lower.starts_with("claude-3")
        {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: true,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: true,
                supports_vision: true,
                supports_streaming: true,
                max_tokens: Some(200000),
                context_window: Some(200000),
            }
        } else if model_lower.starts_with("claude-2") || model_lower.starts_with("claude-instant") {
            ModelCapabilities {
                supports_function_calling: false,
                supports_parallel_function_calling: false,
                supports_tool_choice: false,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: false,
                supports_url_context: false,
                supports_vision: false,
                supports_streaming: true,
                max_tokens: Some(100000),
                context_window: Some(100000),
            }
        } else if model_lower.starts_with("gemini") {
            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: false,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: true,
                supports_url_context: true,
                supports_vision: model_lower.contains("vision") || model_lower.contains("pro"),
                supports_streaming: true,
                max_tokens: Some(32768),
                context_window: Some(32768),
            }
        } else {
            ModelCapabilities::default()
        }
    }

    pub fn supports_function_calling(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_function_calling
    }

    pub fn supports_parallel_function_calling(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_parallel_function_calling
    }

    pub fn supports_tool_choice(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_tool_choice
    }

    pub fn supports_response_schema(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_response_schema
    }

    pub fn supports_system_messages(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_system_messages
    }

    pub fn supports_web_search(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_web_search
    }

    pub fn supports_url_context(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_url_context
    }

    pub fn supports_vision(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_vision
    }

    pub fn supports_streaming(model: &str) -> bool {
        Self::get_model_capabilities(model).supports_streaming
    }

    pub fn get_provider_from_model(model: &str) -> Option<String> {
        let model_lower = model.to_lowercase();

        if model_lower.starts_with("gpt-")
            || model_lower.starts_with("chatgpt-image-")
            || model_lower.contains("openai")
        {
            Some("openai".to_string())
        } else if model_lower.starts_with("claude-") || model_lower.contains("anthropic") {
            Some("anthropic".to_string())
        } else if model_lower.starts_with("gemini-") || model_lower.contains("google") {
            Some("google".to_string())
        } else if model_lower.starts_with("command") || model_lower.contains("cohere") {
            Some("cohere".to_string())
        } else if model_lower.contains("mistral") {
            Some("mistral".to_string())
        } else if model_lower.contains("llama") {
            Some("meta".to_string())
        } else {
            None
        }
    }

    pub fn get_base_model(model: &str) -> String {
        let model_lower = model.to_lowercase();

        if model_lower.starts_with("gpt-5") {
            if model_lower.contains("nano") {
                "gpt-5-nano".to_string()
            } else if model_lower.contains("mini") {
                "gpt-5-mini".to_string()
            } else if model_lower.contains("codex") {
                if model_lower.contains("5.2") {
                    "gpt-5.2-codex".to_string()
                } else {
                    "gpt-5-codex".to_string()
                }
            } else if model_lower.contains("chat") {
                "gpt-5.2-chat".to_string()
            } else {
                "gpt-5.2".to_string()
            }
        } else if model_lower.starts_with("gpt-image-") || model_lower.starts_with("chatgpt-image-")
        {
            if model_lower.contains("1-mini") {
                "gpt-image-1-mini".to_string()
            } else if model_lower.contains("1.5") || model_lower.starts_with("chatgpt-image-") {
                "gpt-image-1.5".to_string()
            } else {
                "gpt-image-1".to_string()
            }
        } else if model_lower.starts_with("gpt-4.1") {
            if model_lower.contains("nano") {
                "gpt-4.1-nano".to_string()
            } else if model_lower.contains("mini") {
                "gpt-4.1-mini".to_string()
            } else {
                "gpt-4.1".to_string()
            }
        } else if model_lower.starts_with("o3-pro") {
            "o3-pro".to_string()
        } else if model_lower.starts_with("gpt-4") {
            if model_lower.contains("32k") {
                "gpt-4-32k".to_string()
            } else if model_lower.contains("turbo") {
                "gpt-4-turbo".to_string()
            } else {
                "gpt-4".to_string()
            }
        } else if model_lower.starts_with("gpt-3.5") {
            if model_lower.contains("16k") {
                "gpt-3.5-turbo-16k".to_string()
            } else {
                "gpt-3.5-turbo".to_string()
            }
        } else if model_lower.starts_with("claude-opus-4-6") {
            "claude-opus-4-6".to_string()
        } else if model_lower.starts_with("claude-opus-4-5") {
            "claude-opus-4-5".to_string()
        } else if model_lower.starts_with("claude-sonnet-4-5") {
            "claude-sonnet-4-5".to_string()
        } else if model_lower.starts_with("claude-sonnet-4") {
            "claude-sonnet-4".to_string()
        } else if model_lower.starts_with("claude-3") {
            if model_lower.contains("opus") {
                "claude-3-opus".to_string()
            } else if model_lower.contains("sonnet") {
                "claude-3-sonnet".to_string()
            } else if model_lower.contains("haiku") {
                "claude-3-haiku".to_string()
            } else {
                "claude-3".to_string()
            }
        } else {
            model.to_string()
        }
    }

    pub fn is_valid_model(model: &str) -> bool {
        let known_providers = [
            "openai",
            "anthropic",
            "google",
            "cohere",
            "mistral",
            "meta",
            "azure",
            "replicate",
        ];

        let known_models = [
            "gpt-5.2",
            "gpt-5-codex",
            "gpt-5-mini",
            "gpt-5-nano",
            "gpt-image-1",
            "gpt-4.1",
            "gpt-4",
            "gpt-3.5-turbo",
            "o3-pro",
            "o3-mini",
            "o4-mini",
            "claude-opus-4",
            "claude-sonnet-4",
            "claude-3",
            "claude-2",
            "gemini",
            "command",
            "mistral",
        ];

        let model_lower = model.to_lowercase();

        for provider in &known_providers {
            if model_lower.contains(provider) {
                return true;
            }
        }

        for base_model in &known_models {
            if model_lower.starts_with(base_model) {
                return true;
            }
        }

        false
    }

    pub fn get_model_family(model: &str) -> String {
        let model_lower = model.to_lowercase();

        if model_lower.starts_with("gpt-") {
            "gpt".to_string()
        } else if model_lower.starts_with("claude-") {
            "claude".to_string()
        } else if model_lower.starts_with("gemini-") {
            "gemini".to_string()
        } else if model_lower.starts_with("command") {
            "command".to_string()
        } else if model_lower.contains("llama") {
            "llama".to_string()
        } else if model_lower.contains("mistral") {
            "mistral".to_string()
        } else {
            "unknown".to_string()
        }
    }

    pub fn validate_model_with_provider(model: &str, provider: &str) -> Result<(), ProviderError> {
        let compatible_models = Self::get_compatible_models_for_provider(provider);

        if compatible_models.is_empty() {
            return Ok(());
        }

        let model_matches = compatible_models.iter().any(|compatible_model| {
            model
                .to_lowercase()
                .starts_with(&compatible_model.to_lowercase())
        });

        if !model_matches {
            return Err(ProviderError::ModelNotFound {
                provider: "unknown",
                model: format!(
                    "Model '{}' is not compatible with provider '{}'",
                    model, provider
                ),
            });
        }

        Ok(())
    }

    pub fn get_compatible_models_for_provider(provider: &str) -> Vec<String> {
        match provider.to_lowercase().as_str() {
            "openai" => vec![
                "gpt-5.2".to_string(),
                "gpt-5.2-chat".to_string(),
                "gpt-5.2-codex".to_string(),
                "gpt-5-codex".to_string(),
                "gpt-5.1".to_string(),
                "gpt-5.1-thinking".to_string(),
                "gpt-5-mini".to_string(),
                "gpt-5-nano".to_string(),
                "gpt-image-1".to_string(),
                "gpt-image-1-mini".to_string(),
                "gpt-image-1.5".to_string(),
                "chatgpt-image-latest".to_string(),
                "o3-pro".to_string(),
                "o3-mini".to_string(),
                "o4-mini".to_string(),
                "gpt-4.1".to_string(),
                "gpt-4.1-mini".to_string(),
                "gpt-4.1-nano".to_string(),
                "gpt-4".to_string(),
                "gpt-4-turbo".to_string(),
                "gpt-4-32k".to_string(),
                "gpt-3.5-turbo".to_string(),
                "gpt-3.5-turbo-16k".to_string(),
            ],
            "anthropic" => vec![
                "claude-opus-4-6".to_string(),
                "claude-opus-4-5".to_string(),
                "claude-sonnet-4-5".to_string(),
                "claude-sonnet-4".to_string(),
                "claude-3-opus".to_string(),
                "claude-3-sonnet".to_string(),
                "claude-3-haiku".to_string(),
                "claude-2".to_string(),
                "claude-instant".to_string(),
            ],
            "google" => vec![
                "gemini-pro".to_string(),
                "gemini-pro-vision".to_string(),
                "gemini-1.5-pro".to_string(),
            ],
            "cohere" => vec![
                "command".to_string(),
                "command-r".to_string(),
                "command-r-plus".to_string(),
            ],
            "mistral" => vec![
                "mistral-tiny".to_string(),
                "mistral-small".to_string(),
                "mistral-medium".to_string(),
                "mistral-large".to_string(),
            ],
            _ => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== get_model_capabilities Tests ====================

    #[test]
    fn test_get_model_capabilities_gpt4() {
        let caps = ModelUtils::get_model_capabilities("gpt-4");
        assert!(caps.supports_function_calling);
        assert!(caps.supports_parallel_function_calling);
        assert!(caps.supports_tool_choice);
        assert!(caps.supports_response_schema);
        assert_eq!(caps.max_tokens, Some(8192));
    }

    #[test]
    fn test_get_model_capabilities_gpt4_32k() {
        let caps = ModelUtils::get_model_capabilities("gpt-4-32k");
        assert_eq!(caps.max_tokens, Some(32768));
        assert_eq!(caps.context_window, Some(32768));
    }

    #[test]
    fn test_get_model_capabilities_gpt4_turbo_vision() {
        let caps = ModelUtils::get_model_capabilities("gpt-4-turbo-preview");
        assert!(caps.supports_vision);
    }

    #[test]
    fn test_get_model_capabilities_gpt35() {
        let caps = ModelUtils::get_model_capabilities("gpt-3.5-turbo");
        assert!(caps.supports_function_calling);
        assert!(!caps.supports_parallel_function_calling);
        assert!(!caps.supports_response_schema);
        assert_eq!(caps.max_tokens, Some(4096));
    }

    #[test]
    fn test_get_model_capabilities_gpt35_16k() {
        let caps = ModelUtils::get_model_capabilities("gpt-3.5-turbo-16k");
        assert_eq!(caps.max_tokens, Some(16384));
        assert_eq!(caps.context_window, Some(16384));
    }

    #[test]
    fn test_get_model_capabilities_claude3() {
        let caps = ModelUtils::get_model_capabilities("claude-3-opus");
        assert!(caps.supports_function_calling);
        assert!(caps.supports_vision);
        assert!(caps.supports_url_context);
        assert_eq!(caps.max_tokens, Some(200000));
    }

    #[test]
    fn test_get_model_capabilities_claude_opus_46() {
        let caps = ModelUtils::get_model_capabilities("claude-opus-4-6");
        assert!(caps.supports_function_calling);
        assert!(caps.supports_vision);
        assert_eq!(caps.max_tokens, Some(1_000_000));
        assert_eq!(caps.context_window, Some(1_000_000));
    }

    #[test]
    fn test_get_model_capabilities_claude2() {
        let caps = ModelUtils::get_model_capabilities("claude-2.1");
        assert!(!caps.supports_function_calling);
        assert!(!caps.supports_vision);
        assert_eq!(caps.max_tokens, Some(100000));
    }

    #[test]
    fn test_get_model_capabilities_claude_instant() {
        let caps = ModelUtils::get_model_capabilities("claude-instant-1.2");
        assert!(!caps.supports_function_calling);
        assert_eq!(caps.max_tokens, Some(100000));
    }

    #[test]
    fn test_get_model_capabilities_gemini() {
        let caps = ModelUtils::get_model_capabilities("gemini-pro");
        assert!(caps.supports_function_calling);
        assert!(caps.supports_web_search);
        assert!(caps.supports_vision);
        assert_eq!(caps.max_tokens, Some(32768));
    }

    #[test]
    fn test_get_model_capabilities_unknown() {
        let caps = ModelUtils::get_model_capabilities("unknown-model");
        assert!(!caps.supports_function_calling);
        assert!(caps.max_tokens.is_none());
    }

    // ==================== supports_* convenience function Tests ====================

    #[test]
    fn test_supports_function_calling() {
        assert!(ModelUtils::supports_function_calling("gpt-4"));
        assert!(!ModelUtils::supports_function_calling("claude-2"));
    }

    #[test]
    fn test_supports_parallel_function_calling() {
        assert!(ModelUtils::supports_parallel_function_calling("gpt-4"));
        assert!(!ModelUtils::supports_parallel_function_calling(
            "gpt-3.5-turbo"
        ));
    }

    #[test]
    fn test_supports_tool_choice() {
        assert!(ModelUtils::supports_tool_choice("gpt-4"));
        assert!(ModelUtils::supports_tool_choice("claude-3-sonnet"));
    }

    #[test]
    fn test_supports_response_schema() {
        assert!(ModelUtils::supports_response_schema("gpt-4"));
        assert!(!ModelUtils::supports_response_schema("gpt-3.5-turbo"));
    }

    #[test]
    fn test_supports_system_messages() {
        assert!(ModelUtils::supports_system_messages("gpt-4"));
        assert!(ModelUtils::supports_system_messages("claude-3-opus"));
    }

    #[test]
    fn test_supports_web_search() {
        assert!(ModelUtils::supports_web_search("gemini-pro"));
        assert!(!ModelUtils::supports_web_search("gpt-4"));
    }

    #[test]
    fn test_supports_url_context() {
        assert!(ModelUtils::supports_url_context("gpt-4"));
        assert!(ModelUtils::supports_url_context("claude-3-opus"));
        assert!(!ModelUtils::supports_url_context("gpt-3.5-turbo"));
    }

    #[test]
    fn test_supports_vision() {
        assert!(ModelUtils::supports_vision("gpt-4-turbo"));
        assert!(ModelUtils::supports_vision("claude-3-opus"));
        assert!(!ModelUtils::supports_vision("gpt-3.5-turbo"));
        // o3 and o4-mini support vision
        assert!(ModelUtils::supports_vision("o3"));
        assert!(ModelUtils::supports_vision("o3-mini"));
        assert!(ModelUtils::supports_vision("o4-mini"));
        // GPT-5.4 family supports vision (covered by gpt-5 prefix)
        assert!(ModelUtils::supports_vision("gpt-5.4"));
        assert!(ModelUtils::supports_vision("gpt-5.4-mini"));
        assert!(ModelUtils::supports_vision("gpt-5.4-turbo"));
    }

    #[test]
    fn test_supports_streaming() {
        assert!(ModelUtils::supports_streaming("gpt-4"));
        assert!(ModelUtils::supports_streaming("claude-3-opus"));
    }

    // ==================== get_provider_from_model Tests ====================

    #[test]
    fn test_get_provider_from_model_openai() {
        assert_eq!(
            ModelUtils::get_provider_from_model("gpt-4"),
            Some("openai".to_string())
        );
        assert_eq!(
            ModelUtils::get_provider_from_model("gpt-3.5-turbo"),
            Some("openai".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_anthropic() {
        assert_eq!(
            ModelUtils::get_provider_from_model("claude-3-opus"),
            Some("anthropic".to_string())
        );
        assert_eq!(
            ModelUtils::get_provider_from_model("claude-2"),
            Some("anthropic".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_google() {
        assert_eq!(
            ModelUtils::get_provider_from_model("gemini-pro"),
            Some("google".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_cohere() {
        assert_eq!(
            ModelUtils::get_provider_from_model("command-r-plus"),
            Some("cohere".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_mistral() {
        assert_eq!(
            ModelUtils::get_provider_from_model("mistral-large"),
            Some("mistral".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_meta() {
        assert_eq!(
            ModelUtils::get_provider_from_model("llama-2-70b"),
            Some("meta".to_string())
        );
    }

    #[test]
    fn test_get_provider_from_model_unknown() {
        assert_eq!(ModelUtils::get_provider_from_model("unknown-model"), None);
    }

    // ==================== get_base_model Tests ====================

    #[test]
    fn test_get_base_model_gpt4() {
        assert_eq!(ModelUtils::get_base_model("gpt-4-0613"), "gpt-4");
        assert_eq!(ModelUtils::get_base_model("gpt-4-32k-0613"), "gpt-4-32k");
        assert_eq!(
            ModelUtils::get_base_model("gpt-4-turbo-preview"),
            "gpt-4-turbo"
        );
    }

    #[test]
    fn test_get_base_model_gpt35() {
        assert_eq!(
            ModelUtils::get_base_model("gpt-3.5-turbo-0613"),
            "gpt-3.5-turbo"
        );
        assert_eq!(
            ModelUtils::get_base_model("gpt-3.5-turbo-16k-0613"),
            "gpt-3.5-turbo-16k"
        );
    }

    #[test]
    fn test_get_base_model_claude3() {
        assert_eq!(
            ModelUtils::get_base_model("claude-3-opus-20240229"),
            "claude-3-opus"
        );
        assert_eq!(
            ModelUtils::get_base_model("claude-3-sonnet-20240229"),
            "claude-3-sonnet"
        );
        assert_eq!(
            ModelUtils::get_base_model("claude-3-haiku-20240307"),
            "claude-3-haiku"
        );
    }

    #[test]
    fn test_get_base_model_claude4() {
        assert_eq!(
            ModelUtils::get_base_model("claude-opus-4-6-20260114"),
            "claude-opus-4-6"
        );
        assert_eq!(
            ModelUtils::get_base_model("claude-sonnet-4-5-20250929"),
            "claude-sonnet-4-5"
        );
    }

    #[test]
    fn test_get_base_model_unknown() {
        assert_eq!(ModelUtils::get_base_model("unknown-model"), "unknown-model");
    }

    // ==================== is_valid_model Tests ====================

    #[test]
    fn test_is_valid_model_known() {
        assert!(ModelUtils::is_valid_model("gpt-4"));
        assert!(ModelUtils::is_valid_model("gpt-3.5-turbo"));
        assert!(ModelUtils::is_valid_model("claude-3-opus"));
        assert!(ModelUtils::is_valid_model("gemini-pro"));
        assert!(ModelUtils::is_valid_model("command-r"));
        assert!(ModelUtils::is_valid_model("mistral-large"));
    }

    #[test]
    fn test_is_valid_model_with_provider() {
        assert!(ModelUtils::is_valid_model("openai/gpt-4"));
        assert!(ModelUtils::is_valid_model("anthropic/claude-3"));
    }

    #[test]
    fn test_is_valid_model_unknown() {
        assert!(!ModelUtils::is_valid_model("unknown-xyz-123"));
    }

    // ==================== get_model_family Tests ====================

    #[test]
    fn test_get_model_family_gpt() {
        assert_eq!(ModelUtils::get_model_family("gpt-4"), "gpt");
        assert_eq!(ModelUtils::get_model_family("gpt-3.5-turbo"), "gpt");
    }

    #[test]
    fn test_get_model_family_claude() {
        assert_eq!(ModelUtils::get_model_family("claude-3-opus"), "claude");
        assert_eq!(ModelUtils::get_model_family("claude-2"), "claude");
    }

    #[test]
    fn test_get_model_family_gemini() {
        assert_eq!(ModelUtils::get_model_family("gemini-pro"), "gemini");
    }

    #[test]
    fn test_get_model_family_command() {
        assert_eq!(ModelUtils::get_model_family("command-r-plus"), "command");
    }

    #[test]
    fn test_get_model_family_llama() {
        assert_eq!(ModelUtils::get_model_family("llama-2-70b"), "llama");
    }

    #[test]
    fn test_get_model_family_mistral() {
        assert_eq!(ModelUtils::get_model_family("mistral-large"), "mistral");
    }

    #[test]
    fn test_get_model_family_unknown() {
        assert_eq!(ModelUtils::get_model_family("unknown-model"), "unknown");
    }

    // ==================== validate_model_with_provider Tests ====================

    #[test]
    fn test_validate_model_with_provider_valid() {
        assert!(ModelUtils::validate_model_with_provider("gpt-4", "openai").is_ok());
        assert!(ModelUtils::validate_model_with_provider("claude-3-opus", "anthropic").is_ok());
        assert!(ModelUtils::validate_model_with_provider("gemini-pro", "google").is_ok());
    }

    #[test]
    fn test_validate_model_with_provider_invalid() {
        assert!(ModelUtils::validate_model_with_provider("gpt-4", "anthropic").is_err());
        assert!(ModelUtils::validate_model_with_provider("claude-3-opus", "openai").is_err());
    }

    #[test]
    fn test_validate_model_with_provider_unknown_provider() {
        assert!(ModelUtils::validate_model_with_provider("any-model", "unknown-provider").is_ok());
    }

    // ==================== get_compatible_models_for_provider Tests ====================

    #[test]
    fn test_get_compatible_models_openai() {
        let models = ModelUtils::get_compatible_models_for_provider("openai");
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
    }

    #[test]
    fn test_get_compatible_models_anthropic() {
        let models = ModelUtils::get_compatible_models_for_provider("anthropic");
        assert!(models.contains(&"claude-3-opus".to_string()));
        assert!(models.contains(&"claude-2".to_string()));
    }

    #[test]
    fn test_get_compatible_models_google() {
        let models = ModelUtils::get_compatible_models_for_provider("google");
        assert!(models.contains(&"gemini-pro".to_string()));
    }

    #[test]
    fn test_get_compatible_models_cohere() {
        let models = ModelUtils::get_compatible_models_for_provider("cohere");
        assert!(models.contains(&"command".to_string()));
        assert!(models.contains(&"command-r-plus".to_string()));
    }

    #[test]
    fn test_get_compatible_models_mistral() {
        let models = ModelUtils::get_compatible_models_for_provider("mistral");
        assert!(models.contains(&"mistral-large".to_string()));
    }

    #[test]
    fn test_get_compatible_models_unknown() {
        let models = ModelUtils::get_compatible_models_for_provider("unknown");
        assert!(models.is_empty());
    }

    #[test]
    fn test_get_compatible_models_case_insensitive() {
        let models = ModelUtils::get_compatible_models_for_provider("OPENAI");
        assert!(!models.is_empty());
    }
}
