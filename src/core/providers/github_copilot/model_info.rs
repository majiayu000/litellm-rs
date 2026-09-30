//! GitHub Copilot Model Information
//!
//! Contains model metadata and capability information for GitHub Copilot models.

/// GitHub Copilot model information
#[derive(Debug, Clone)]
pub struct GitHubCopilotModel {
    /// Model ID used in API calls
    pub model_id: &'static str,
    /// Display name for the model
    pub display_name: &'static str,
    /// Context window size
    pub max_context_length: u32,
    /// Maximum output tokens
    pub max_output_length: Option<u32>,
    /// Whether the model supports tools/function calling
    pub supports_tools: bool,
    /// Whether the model supports vision/images
    pub supports_multimodal: bool,
    /// Whether the model supports streaming
    pub supports_streaming: bool,
    /// Whether the model supports extended thinking/reasoning
    pub supports_reasoning: bool,
}

/// Static model registry for GitHub Copilot
/// These are models accessible through the GitHub Copilot API
static GITHUB_COPILOT_MODELS: &[GitHubCopilotModel] = &[
    // Exact IDs documented by GitHub's CLI reference, reviewed 2026-10-01.
    // https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference
    // Limits depend on the account/client; zero/None means not published here.
    GitHubCopilotModel {
        model_id: "gpt-6-astra",
        display_name: "GPT-6 Astra",
        max_context_length: 0,
        max_output_length: None,
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "gpt-6-sol",
        display_name: "GPT-6 Sol",
        max_context_length: 0,
        max_output_length: None,
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "gpt-6-luna",
        display_name: "GPT-6 Luna",
        max_context_length: 0,
        max_output_length: None,
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "claude-opus-5.5",
        display_name: "Claude Opus 5.5",
        max_context_length: 0,
        max_output_length: None,
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    // GPT-4 series
    GitHubCopilotModel {
        model_id: "gpt-4o",
        display_name: "GPT-4o",
        max_context_length: 128000,
        max_output_length: Some(16384),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: false,
    },
    GitHubCopilotModel {
        model_id: "gpt-4o-mini",
        display_name: "GPT-4o Mini",
        max_context_length: 128000,
        max_output_length: Some(16384),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: false,
    },
    GitHubCopilotModel {
        model_id: "gpt-4-turbo",
        display_name: "GPT-4 Turbo",
        max_context_length: 128000,
        max_output_length: Some(4096),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: false,
    },
    // O1 Reasoning models
    GitHubCopilotModel {
        model_id: "o1-preview",
        display_name: "O1 Preview",
        max_context_length: 128000,
        max_output_length: Some(32768),
        supports_tools: false,
        supports_multimodal: false,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "o1-mini",
        display_name: "O1 Mini",
        max_context_length: 128000,
        max_output_length: Some(65536),
        supports_tools: false,
        supports_multimodal: false,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "o1",
        display_name: "O1",
        max_context_length: 200000,
        max_output_length: Some(100000),
        supports_tools: false,
        supports_multimodal: false,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "o3-mini",
        display_name: "O3 Mini",
        max_context_length: 200000,
        max_output_length: Some(100000),
        supports_tools: false,
        supports_multimodal: false,
        supports_streaming: true,
        supports_reasoning: true,
    },
    // Claude models (via Copilot)
    GitHubCopilotModel {
        model_id: "claude-3.5-sonnet",
        display_name: "Claude 3.5 Sonnet",
        max_context_length: 200000,
        max_output_length: Some(8192),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: false,
    },
    GitHubCopilotModel {
        model_id: "claude-3-7-sonnet",
        display_name: "Claude 3.7 Sonnet",
        max_context_length: 200000,
        max_output_length: Some(16384),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    GitHubCopilotModel {
        model_id: "claude-sonnet-4",
        display_name: "Claude Sonnet 4",
        max_context_length: 200000,
        max_output_length: Some(16384),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: true,
    },
    // Codex models (code-specific)
    GitHubCopilotModel {
        model_id: "gpt-5.1-codex",
        display_name: "GPT-5.1 Codex",
        max_context_length: 256000,
        max_output_length: Some(32768),
        supports_tools: true,
        supports_multimodal: false,
        supports_streaming: true,
        supports_reasoning: false,
    },
    // Gemini models (via Copilot)
    GitHubCopilotModel {
        model_id: "gemini-2.0-flash",
        display_name: "Gemini 2.0 Flash",
        max_context_length: 1000000,
        max_output_length: Some(8192),
        supports_tools: true,
        supports_multimodal: true,
        supports_streaming: true,
        supports_reasoning: false,
    },
];

/// Get all available GitHub Copilot models
pub fn get_available_models() -> Vec<&'static str> {
    GITHUB_COPILOT_MODELS.iter().map(|m| m.model_id).collect()
}

/// Get model information by ID
pub fn get_model_info(model_id: &str) -> Option<&'static GitHubCopilotModel> {
    GITHUB_COPILOT_MODELS
        .iter()
        .find(|m| m.model_id == model_id)
}

/// Check if a model supports vision
#[cfg(test)]
pub fn is_vision_model(model_id: &str) -> bool {
    get_model_info(model_id).is_some_and(|m| m.supports_multimodal)
}

/// Check if a model supports tools
#[cfg(test)]
pub fn supports_tools(model_id: &str) -> bool {
    get_model_info(model_id).is_some_and(|m| m.supports_tools)
}

/// Check if a model supports reasoning/extended thinking
pub fn supports_reasoning(model_id: &str) -> bool {
    get_model_info(model_id).is_some_and(|m| m.supports_reasoning)
}

/// Check if a model is a Claude model (for special handling)
pub fn is_claude_model(model_id: &str) -> bool {
    model_id.to_lowercase().contains("claude")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"gpt-4o"));
        assert!(models.contains(&"claude-3.5-sonnet"));
        assert!(models.contains(&"o1-preview"));
    }

    #[test]
    fn test_get_model_info() {
        let model = get_model_info("gpt-4o");
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.model_id, "gpt-4o");
        assert!(model.supports_tools);
        assert!(model.supports_multimodal);
    }

    #[test]
    fn test_get_model_info_nonexistent() {
        let model = get_model_info("nonexistent-model");
        assert!(model.is_none());
    }

    #[test]
    fn test_is_vision_model() {
        assert!(is_vision_model("gpt-4o"));
        assert!(is_vision_model("claude-3.5-sonnet"));
        assert!(!is_vision_model("o1-preview"));
    }

    #[test]
    fn test_supports_tools() {
        assert!(supports_tools("gpt-4o"));
        assert!(supports_tools("claude-3.5-sonnet"));
        assert!(!supports_tools("o1-preview"));
    }

    #[test]
    fn test_supports_reasoning() {
        assert!(supports_reasoning("o1-preview"));
        assert!(supports_reasoning("o1-mini"));
        assert!(supports_reasoning("claude-3-7-sonnet"));
        assert!(!supports_reasoning("gpt-4o"));
    }

    #[test]
    fn test_is_claude_model() {
        assert!(is_claude_model("claude-3.5-sonnet"));
        assert!(is_claude_model("claude-3-7-sonnet"));
        assert!(is_claude_model("claude-sonnet-4"));
        assert!(!is_claude_model("gpt-4o"));
        assert!(!is_claude_model("o1-preview"));
    }
}
