//! Ollama Model Information
//!
//! Contains model configurations and dynamic model listing for Ollama.
//! Unlike cloud providers, Ollama models are locally managed and can be
//! dynamically discovered via the API.

use serde::{Deserialize, Serialize};

/// Ollama model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    /// Model name/ID
    pub name: String,

    /// Model display name
    pub display_name: String,

    /// Context length (if known)
    pub max_context_length: Option<u32>,

    /// Whether the model supports tools/function calling
    pub supports_tools: bool,

    /// Whether the model supports vision/images
    pub supports_multimodal: bool,

    /// Model size in bytes (if known)
    pub size: Option<u64>,

    /// Model family (llama, mistral, etc.)
    pub family: Option<String>,

    /// Model parameter count (e.g., "7B", "70B")
    pub parameter_size: Option<String>,

    /// Quantization level (e.g., "Q4_0", "Q8_0")
    pub quantization: Option<String>,

    /// Model modified date
    pub modified_at: Option<String>,
}

impl OllamaModelInfo {
    /// Create a new model info with defaults
    pub fn new(name: impl Into<String>) -> Self {
        let name = name.into();
        let display_name = name.clone();
        Self {
            name,
            display_name,
            max_context_length: None,
            supports_tools: false,
            supports_multimodal: false,
            size: None,
            family: None,
            parameter_size: None,
            quantization: None,
            modified_at: None,
        }
    }

    /// Infer model capabilities from name
    pub fn infer_capabilities_from_name(mut self) -> Self {
        let name_lower = self.name.to_lowercase();

        // Infer family
        if name_lower.contains("llama") {
            self.family = Some("llama".to_string());
        } else if name_lower.contains("mistral") {
            self.family = Some("mistral".to_string());
        } else if name_lower.contains("gemma") {
            self.family = Some("gemma".to_string());
        } else if name_lower.contains("phi") {
            self.family = Some("phi".to_string());
        } else if name_lower.contains("qwen") {
            self.family = Some("qwen".to_string());
        } else if name_lower.contains("codellama") || name_lower.contains("code-llama") {
            self.family = Some("codellama".to_string());
        } else if name_lower.contains("deepseek") {
            self.family = Some("deepseek".to_string());
        } else if name_lower.contains("yi") {
            self.family = Some("yi".to_string());
        } else if name_lower.contains("mixtral") {
            self.family = Some("mixtral".to_string());
        } else if name_lower.contains("nomic") {
            self.family = Some("nomic".to_string());
        }

        // Infer vision support
        if name_lower.contains("vision")
            || name_lower.contains("llava")
            || name_lower.contains("-v")
            || name_lower.contains("moondream")
            || name_lower.contains("bakllava")
        {
            self.supports_multimodal = true;
        }

        // Infer tool support (newer models typically support tools)
        if name_lower.contains("llama3")
            || name_lower.contains("llama-3")
            || name_lower.contains("mistral")
            || name_lower.contains("mixtral")
            || name_lower.contains("qwen2")
            || name_lower.contains("qwen-2")
            || name_lower.contains("deepseek-coder")
            || name_lower.contains("command-r")
        {
            self.supports_tools = true;
        }

        // Infer parameter size
        for size in [
            "1b", "3b", "7b", "8b", "13b", "14b", "32b", "33b", "34b", "70b", "72b", "180b",
        ] {
            if name_lower.contains(size) {
                self.parameter_size = Some(size.to_uppercase());
                break;
            }
        }

        // Infer context length based on model
        self.max_context_length = infer_context_length(&name_lower);

        self
    }
}

/// Infer context length from model name
fn infer_context_length(name_lower: &str) -> Option<u32> {
    // Check more specific patterns first
    if name_lower.contains("llama3.2") || name_lower.contains("llama-3.2") {
        return Some(128000);
    }
    if name_lower.contains("llama3.1") || name_lower.contains("llama-3.1") {
        return Some(131072);
    }
    // Models with known large context windows
    if name_lower.contains("llama3") || name_lower.contains("llama-3") {
        return Some(8192);
    }
    if name_lower.contains("mistral") && !name_lower.contains("mixtral") {
        return Some(32768);
    }
    if name_lower.contains("mixtral") {
        return Some(32768);
    }
    if name_lower.contains("qwen2") || name_lower.contains("qwen-2") {
        return Some(32768);
    }
    if name_lower.contains("gemma") {
        return Some(8192);
    }
    if name_lower.contains("phi") {
        return Some(4096);
    }
    if name_lower.contains("deepseek") {
        return Some(16384);
    }

    // Default context for unknown models
    Some(4096)
}

/// Get model info by name (constructs from name with inferred capabilities)
pub fn get_model_info(model_name: &str) -> OllamaModelInfo {
    OllamaModelInfo::new(model_name).infer_capabilities_from_name()
}

/// Ollama model tags response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaModelEntry>,
}

/// Individual model entry from tags API
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModelEntry {
    pub name: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub modified_at: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub digest: Option<String>,
    #[serde(default)]
    pub details: Option<OllamaModelDetails>,
}

/// Model details from tags API
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaModelDetails {
    #[serde(default)]
    pub parent_model: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub families: Option<Vec<String>>,
    #[serde(default)]
    pub parameter_size: Option<String>,
    #[serde(default)]
    pub quantization_level: Option<String>,
}

/// Ollama model show response
#[derive(Debug, Clone, Deserialize)]
pub struct OllamaShowResponse {
    #[serde(default)]
    pub modelfile: Option<String>,
    #[serde(default)]
    pub parameters: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub details: Option<OllamaModelDetails>,
    #[serde(default)]
    pub model_info: Option<serde_json::Value>,
}

impl OllamaShowResponse {
    /// Check if the model template supports tools
    pub fn supports_tools(&self) -> bool {
        if let Some(template) = &self.template {
            let template_lower = template.to_lowercase();
            template_lower.contains("tools") || template_lower.contains("function")
        } else {
            false
        }
    }

    /// Get context length from model info
    pub fn get_context_length(&self) -> Option<u32> {
        if let Some(model_info) = &self.model_info {
            // Try various known keys for context length
            for key in [
                "context_length",
                "num_ctx",
                "max_position_embeddings",
                "n_ctx",
            ] {
                if let Some(val) = model_info.get(key)
                    && let Some(num) = val.as_u64()
                {
                    return Some(num as u32);
                }
            }

            // Try nested structure
            if let Some(general) = model_info.get("general")
                && let Some(ctx) = general.get("context_length")
                && let Some(num) = ctx.as_u64()
            {
                return Some(num as u32);
            }
        }
        None
    }
}

impl From<OllamaModelEntry> for OllamaModelInfo {
    fn from(entry: OllamaModelEntry) -> Self {
        let name = entry.model.unwrap_or(entry.name.clone());
        let mut info = OllamaModelInfo::new(&name);

        info.display_name = entry.name;
        info.size = entry.size;
        info.modified_at = entry.modified_at;

        if let Some(details) = entry.details {
            info.family = details.family;
            info.parameter_size = details.parameter_size;
            info.quantization = details.quantization_level;
        }

        // Infer additional capabilities
        info.infer_capabilities_from_name()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_model_info_new() {
        let info = OllamaModelInfo::new("llama3:8b");
        assert_eq!(info.name, "llama3:8b");
        assert_eq!(info.display_name, "llama3:8b");
        assert!(!info.supports_tools);
        assert!(!info.supports_multimodal);
    }

    #[test]
    fn test_infer_capabilities_llama3() {
        let info = get_model_info("llama3:8b");
        assert_eq!(info.family, Some("llama".to_string()));
        assert!(info.supports_tools);
        assert!(!info.supports_multimodal);
        assert_eq!(info.parameter_size, Some("8B".to_string()));
    }

    #[test]
    fn test_infer_capabilities_llava() {
        let info = get_model_info("llava:13b");
        assert!(info.supports_multimodal);
    }

    #[test]
    fn test_infer_capabilities_mistral() {
        let info = get_model_info("mistral:7b");
        assert_eq!(info.family, Some("mistral".to_string()));
        assert!(info.supports_tools);
        assert_eq!(info.parameter_size, Some("7B".to_string()));
        assert_eq!(info.max_context_length, Some(32768));
    }

    #[test]
    fn test_infer_capabilities_mixtral() {
        let info = get_model_info("mixtral:8x7b");
        assert_eq!(info.family, Some("mixtral".to_string()));
        assert!(info.supports_tools);
    }

    #[test]
    fn test_infer_capabilities_qwen() {
        let info = get_model_info("qwen2:7b");
        assert_eq!(info.family, Some("qwen".to_string()));
        assert!(info.supports_tools);
    }

    #[test]
    fn test_infer_capabilities_gemma() {
        let info = get_model_info("gemma:7b");
        assert_eq!(info.family, Some("gemma".to_string()));
        assert_eq!(info.max_context_length, Some(8192));
    }

    #[test]
    fn test_infer_capabilities_deepseek() {
        let info = get_model_info("deepseek-coder:6.7b");
        assert_eq!(info.family, Some("deepseek".to_string()));
        assert!(info.supports_tools);
    }

    #[test]
    fn test_infer_capabilities_vision_model() {
        let info = get_model_info("llama3-vision:11b");
        assert!(info.supports_multimodal);

        let info = get_model_info("moondream:1.8b");
        assert!(info.supports_multimodal);

        let info = get_model_info("bakllava:7b");
        assert!(info.supports_multimodal);
    }

    #[test]
    fn test_infer_context_length() {
        assert_eq!(infer_context_length("llama3:8b"), Some(8192));
        assert_eq!(infer_context_length("llama3.1:70b"), Some(131072));
        assert_eq!(infer_context_length("mistral:7b"), Some(32768));
        assert_eq!(infer_context_length("phi:3b"), Some(4096));
        assert_eq!(infer_context_length("unknown-model"), Some(4096));
    }

    #[test]
    fn test_ollama_show_response_supports_tools() {
        let response = OllamaShowResponse {
            modelfile: None,
            parameters: None,
            template: Some("{{ .Tools }}".to_string()),
            details: None,
            model_info: None,
        };
        assert!(response.supports_tools());

        let response = OllamaShowResponse {
            modelfile: None,
            parameters: None,
            template: Some("{{ .System }}".to_string()),
            details: None,
            model_info: None,
        };
        assert!(!response.supports_tools());
    }

    #[test]
    fn test_ollama_show_response_get_context_length() {
        let response = OllamaShowResponse {
            modelfile: None,
            parameters: None,
            template: None,
            details: None,
            model_info: Some(serde_json::json!({
                "context_length": 8192
            })),
        };
        assert_eq!(response.get_context_length(), Some(8192));

        let response = OllamaShowResponse {
            modelfile: None,
            parameters: None,
            template: None,
            details: None,
            model_info: Some(serde_json::json!({
                "general": {
                    "context_length": 4096
                }
            })),
        };
        assert_eq!(response.get_context_length(), Some(4096));
    }

    #[test]
    fn test_ollama_model_entry_to_model_info() {
        let entry = OllamaModelEntry {
            name: "llama3:8b".to_string(),
            model: Some("llama3:8b".to_string()),
            modified_at: Some("2024-01-01T00:00:00Z".to_string()),
            size: Some(4_000_000_000),
            digest: None,
            details: Some(OllamaModelDetails {
                parent_model: None,
                format: Some("gguf".to_string()),
                family: Some("llama".to_string()),
                families: None,
                parameter_size: Some("8B".to_string()),
                quantization_level: Some("Q4_0".to_string()),
            }),
        };

        let info: OllamaModelInfo = entry.into();
        assert_eq!(info.name, "llama3:8b");
        assert_eq!(info.family, Some("llama".to_string()));
        assert_eq!(info.parameter_size, Some("8B".to_string()));
        assert_eq!(info.quantization, Some("Q4_0".to_string()));
        assert!(info.supports_tools);
    }

    #[test]
    fn test_ollama_tags_response_deserialization() {
        let json = r#"{
            "models": [
                {
                    "name": "llama3:8b",
                    "modified_at": "2024-01-01T00:00:00Z",
                    "size": 4000000000,
                    "details": {
                        "family": "llama",
                        "parameter_size": "8B"
                    }
                }
            ]
        }"#;

        let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.models.len(), 1);
        assert_eq!(response.models[0].name, "llama3:8b");
    }
}
