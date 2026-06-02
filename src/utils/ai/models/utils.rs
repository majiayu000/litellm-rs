use crate::core::providers::shared::gemini_context_window;
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
                supports_streaming: !model_lower.starts_with("gpt-5.5-pro"),
                max_tokens: Some(128000),
                context_window: Some(
                    if (model_lower.starts_with("gpt-5.5") || model_lower.starts_with("gpt-5.4"))
                        && !model_lower.contains("mini")
                        && !model_lower.contains("nano")
                    {
                        1_048_576
                    } else {
                        400_000
                    },
                ),
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
        } else if model_lower.starts_with("claude-opus-4-7")
            || model_lower.starts_with("claude-opus-4-6")
            || model_lower.starts_with("claude-sonnet-4-6")
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
                max_tokens: Some(1_000_000),
                context_window: Some(1_000_000),
            }
        } else if model_lower.starts_with("claude-opus-4")
            || model_lower.starts_with("claude-sonnet-4")
            || model_lower.starts_with("claude-haiku-4-5")
            || model_lower.starts_with("claude-haiku-4.5")
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
            let is_gemini_3_or_25 =
                model_lower.contains("gemini-3") || model_lower.contains("gemini-2.5");
            let is_gemini_20 =
                model_lower.contains("gemini-2.0") || model_lower.contains("gemini-20");
            let is_gemini_15 =
                model_lower.contains("gemini-1.5") || model_lower.contains("gemini-15");

            ModelCapabilities {
                supports_function_calling: true,
                supports_parallel_function_calling: false,
                supports_tool_choice: false,
                supports_response_schema: false,
                supports_system_messages: true,
                supports_web_search: true,
                supports_url_context: true,
                supports_vision: model_lower.contains("vision")
                    || model_lower.contains("pro")
                    || model_lower.contains("flash"),
                supports_streaming: true,
                max_tokens: Some(if is_gemini_3_or_25 {
                    65536
                } else if is_gemini_20 || is_gemini_15 {
                    8192
                } else {
                    32768
                }),
                context_window: gemini_context_window(&model_lower)
                    .map(|context_window| context_window as usize)
                    .or(Some(32768)),
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
            if model_lower.contains("5.5-pro") {
                "gpt-5.5-pro".to_string()
            } else if model_lower.contains("5.5") {
                "gpt-5.5".to_string()
            } else if model_lower.contains("5.4-nano") {
                "gpt-5.4-nano".to_string()
            } else if model_lower.contains("5.4-pro") {
                "gpt-5.4-pro".to_string()
            } else if model_lower.contains("5.4-mini") {
                "gpt-5.4-mini".to_string()
            } else if model_lower.contains("5.4") {
                "gpt-5.4".to_string()
            } else if model_lower.contains("nano") {
                "gpt-5-nano".to_string()
            } else if model_lower.contains("mini") {
                "gpt-5-mini".to_string()
            } else if model_lower.contains("codex") {
                if model_lower.contains("5.2") {
                    "gpt-5.2-codex".to_string()
                } else {
                    "gpt-5-codex".to_string()
                }
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
        } else if model_lower.starts_with("claude-opus-4-7") {
            "claude-opus-4-7".to_string()
        } else if model_lower.starts_with("claude-opus-4-6") {
            "claude-opus-4-6".to_string()
        } else if model_lower.starts_with("claude-sonnet-4-6") {
            "claude-sonnet-4-6".to_string()
        } else if model_lower.starts_with("claude-haiku-4-5") {
            "claude-haiku-4-5".to_string()
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
        } else if model_lower.starts_with("gemini-3.1-pro") {
            "gemini-3.1-pro-preview".to_string()
        } else if model_lower.starts_with("gemini-3.1-flash-lite") {
            "gemini-3.1-flash-lite-preview".to_string()
        } else if model_lower.starts_with("gemini-3.1-flash") {
            "gemini-3.1-flash".to_string()
        } else if model_lower.starts_with("gemini-3-flash") {
            "gemini-3-flash-preview".to_string()
        } else if model_lower.starts_with("gemini-2.0-flash-thinking")
            || model_lower.starts_with("gemini-20-flash-thinking")
        {
            "gemini-2.0-flash-thinking-exp".to_string()
        } else if model_lower.starts_with("gemini-2.0-flash-lite")
            || model_lower.starts_with("gemini-20-flash-lite")
        {
            "gemini-2.0-flash-lite".to_string()
        } else if model_lower.starts_with("gemini-2.0-flash")
            || model_lower.starts_with("gemini-20-flash")
        {
            "gemini-2.0-flash".to_string()
        } else if model_lower.starts_with("gemini-2.5-flash-lite") {
            "gemini-2.5-flash-lite".to_string()
        } else if model_lower.starts_with("gemini-2.5-flash") {
            "gemini-2.5-flash".to_string()
        } else if model_lower.starts_with("gemini-2.5-pro") {
            "gemini-2.5-pro".to_string()
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
            "gpt-5.5-pro",
            "gpt-5.5",
            "gpt-5.4",
            "gpt-5.4-mini",
            "gpt-5.4-nano",
            "gpt-5.4-pro",
            "gpt-image-1",
            "gpt-4.1",
            "gpt-4",
            "gpt-3.5-turbo",
            "o3-pro",
            "o3-mini",
            "o4-mini",
            "claude-opus-4",
            "claude-sonnet-4",
            "claude-opus-4-7",
            "claude-sonnet-4-6",
            "claude-haiku-4-5",
            "claude-3",
            "claude-2",
            "gemini",
            "gemini-3.1-pro-preview",
            "gemini-3-flash-preview",
            "gemini-3.1-flash-lite-preview",
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
                "gpt-5.5".to_string(),
                "gpt-5.5-pro".to_string(),
                "gpt-5.4".to_string(),
                "gpt-5.4-mini".to_string(),
                "gpt-5.4-nano".to_string(),
                "gpt-5.4-pro".to_string(),
                "gpt-5.2".to_string(),
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
                "claude-opus-4-7".to_string(),
                "claude-sonnet-4-6".to_string(),
                "claude-haiku-4-5".to_string(),
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
                "gemini-1.5-flash".to_string(),
                "gemini-1.5-flash-8b".to_string(),
                "gemini-2.0-flash".to_string(),
                "gemini-2.0-flash-lite".to_string(),
                "gemini-2.0-flash-thinking-exp".to_string(),
                "gemini-3.1-pro-preview".to_string(),
                "gemini-3.1-flash".to_string(),
                "gemini-3-flash-preview".to_string(),
                "gemini-3.1-flash-lite-preview".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.5-flash".to_string(),
                "gemini-2.5-flash-lite".to_string(),
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
#[path = "utils_tests.rs"]
mod utils_tests;
