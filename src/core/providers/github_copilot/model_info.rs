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
    // Exact IDs documented by GitHub's CLI reference, reviewed 2026-10-03.
    // https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-command-reference
    // Retired and utility-only models are not advertised as selectable models.
    // https://docs.github.com/en/copilot/reference/ai-models/supported-models
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
        assert!(models.contains(&"gpt-6-sol"));
        assert!(models.contains(&"claude-opus-5.5"));
        assert!(models.contains(&"gpt-6-astra"));
    }

    #[test]
    fn retired_and_utility_only_models_are_not_advertised() {
        for id in [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "o1-preview",
            "o1-mini",
            "o1",
            "o3-mini",
            "claude-3.5-sonnet",
            "claude-3-7-sonnet",
            "claude-sonnet-4",
            "gpt-5.1-codex",
            "gemini-2.0-flash",
        ] {
            assert!(get_model_info(id).is_none(), "{id}");
            assert!(!get_available_models().contains(&id), "{id}");
        }
    }

    #[test]
    fn test_get_model_info() {
        let model = get_model_info("gpt-6-sol");
        assert!(model.is_some());
        let model = model.unwrap();
        assert_eq!(model.model_id, "gpt-6-sol");
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
        assert!(is_vision_model("gpt-6-sol"));
        assert!(is_vision_model("claude-opus-5.5"));
        assert!(!is_vision_model("unknown"));
    }

    #[test]
    fn test_supports_tools() {
        assert!(supports_tools("gpt-6-sol"));
        assert!(supports_tools("claude-opus-5.5"));
        assert!(!supports_tools("unknown"));
    }

    #[test]
    fn test_supports_reasoning() {
        assert!(supports_reasoning("gpt-6-astra"));
        assert!(supports_reasoning("gpt-6-luna"));
        assert!(supports_reasoning("claude-opus-5.5"));
        assert!(!supports_reasoning("unknown"));
    }

    #[test]
    fn test_is_claude_model() {
        assert!(is_claude_model("claude-opus-5.5"));
        assert!(is_claude_model("claude-3-7-sonnet"));
        assert!(is_claude_model("claude-sonnet-4"));
        assert!(!is_claude_model("gpt-6-sol"));
        assert!(!is_claude_model("gpt-6-astra"));
    }
}
