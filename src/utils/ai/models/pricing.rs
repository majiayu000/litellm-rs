use super::utils::ModelUtils;

impl ModelUtils {
    pub fn get_model_pricing(model: &str) -> Option<(f64, f64)> {
        let model_lower = model.to_lowercase();

        if let Some(provider) = infer_pricing_provider(&model_lower)
            && let Ok(pricing) = crate::core::cost::calculator::get_model_pricing(model, provider)
        {
            return Some((
                pricing.input_cost_per_1k_tokens,
                pricing.output_cost_per_1k_tokens,
            ));
        }

        match model_lower.as_str() {
            m if m.starts_with("gpt-5.5-pro") => Some((0.030, 0.180)),
            m if m.starts_with("gpt-5.5") => Some((0.005, 0.030)),
            m if m.starts_with("gpt-5.4-pro") => Some((0.030, 0.180)),
            m if m.starts_with("gpt-5.4-mini") => Some((0.00075, 0.0045)),
            m if m.starts_with("gpt-5.4-nano") => Some((0.0002, 0.00125)),
            m if m.starts_with("gpt-5.4") => Some((0.0025, 0.015)),
            m if m.starts_with("gpt-5.2-pro") => Some((0.021, 0.168)),
            m if m.starts_with("gpt-5.2-codex") => Some((0.00175, 0.014)),
            m if m.starts_with("gpt-5-codex") => Some((0.00125, 0.010)),
            m if m.starts_with("gpt-5.2") => Some((0.00175, 0.014)),
            m if m.starts_with("gpt-5.1-thinking") => Some((0.0025, 0.020)),
            m if m.starts_with("gpt-5.1") => Some((0.00125, 0.010)),
            m if m.starts_with("gpt-5-mini") => Some((0.00025, 0.002)),
            m if m.starts_with("gpt-5-nano") => Some((0.00005, 0.0004)),
            m if m.starts_with("gpt-image-1-mini") => Some((0.0025, 0.010)),
            m if m.starts_with("gpt-image-1.5") => Some((0.005, 0.020)),
            m if m.starts_with("chatgpt-image-latest") => Some((0.005, 0.020)),
            m if m.starts_with("gpt-image-1") => Some((0.005, 0.020)),
            m if m.starts_with("o3-pro") => Some((0.020, 0.080)),
            m if m.starts_with("o3-mini") || m.starts_with("o4-mini") => Some((0.0011, 0.0044)),
            m if m.starts_with("gpt-4.1") => Some((0.002, 0.008)),
            m if m.starts_with("gpt-4-turbo") => Some((0.01, 0.03)),
            m if m.starts_with("gpt-4") => Some((0.03, 0.06)),
            m if m.starts_with("gpt-3.5-turbo") => Some((0.0015, 0.002)),
            m if m.contains("claude-opus-4-7") => Some((0.005, 0.025)),
            m if m.contains("claude-opus-4-6") => Some((0.005, 0.025)),
            m if m.contains("claude-opus-4-5") => Some((0.005, 0.025)),
            m if m.contains("claude-sonnet-4-6") => Some((0.003, 0.015)),
            m if m.contains("claude-haiku-4-5") => Some((0.001, 0.005)),
            m if m.contains("claude-sonnet-4-5") => Some((0.003, 0.015)),
            m if m.contains("claude-sonnet-4") => Some((0.003, 0.015)),
            m if m.contains("claude-3-opus") => Some((0.015, 0.075)),
            m if m.contains("claude-3-sonnet") => Some((0.003, 0.015)),
            m if m.contains("claude-3-haiku") => Some((0.00025, 0.00125)),
            m if m.starts_with("gemini-3.1-pro-preview") => Some((0.002, 0.012)),
            m if m.starts_with("gemini-3.1-flash-lite") => Some((0.0000375, 0.00015)),
            m if m.starts_with("gemini-3.1-flash") => Some((0.000075, 0.0003)),
            m if m.starts_with("gemini-3-flash-preview") => Some((0.0005, 0.003)),
            m if m.starts_with("gemini-2.5-pro") => Some((0.00125, 0.010)),
            m if m.starts_with("gemini-2.5-flash-lite") => Some((0.0001, 0.0004)),
            m if m.starts_with("gemini-2.5-flash") => Some((0.0003, 0.0025)),
            m if m.starts_with("gemini-2.0-flash-thinking-exp") => Some((0.00001, 0.00004)),
            m if m.starts_with("gemini-2.0-flash-lite") => Some((0.000075, 0.0003)),
            m if m.starts_with("gemini-2.0-flash") => Some((0.0001, 0.0004)),
            m if m.starts_with("gemini-1.5-flash") => Some((0.000075, 0.0003)),
            m if m.starts_with("gemini-pro") => Some((0.0005, 0.0015)),
            _ => None,
        }
    }

    pub fn get_model_aliases(model: &str) -> Vec<String> {
        let model_lower = model.to_lowercase();
        let mut aliases = vec![];

        match model_lower.as_str() {
            "gpt-5.5" => {
                aliases.extend_from_slice(&[
                    "openai/gpt-5.5".to_string(),
                    "gpt-5.5-2026-04-23".to_string(),
                ]);
            }
            "gpt-5.5-pro" => {
                aliases.extend_from_slice(&[
                    "openai/gpt-5.5-pro".to_string(),
                    "gpt-5.5-pro-2026-04-23".to_string(),
                ]);
            }
            "gpt-5.4" => {
                aliases.extend_from_slice(&[
                    "openai/gpt-5.4".to_string(),
                    "gpt-5.4-mini".to_string(),
                    "gpt-5.4-nano".to_string(),
                    "gpt-5.4-pro".to_string(),
                ]);
            }
            "gpt-5.2" => {
                aliases.extend_from_slice(&[
                    "openai/gpt-5.2".to_string(),
                    "gpt-5.2-chat".to_string(),
                    "gpt-5.2-codex".to_string(),
                    "gpt-5-codex".to_string(),
                ]);
            }
            "claude-opus-4-7" => {
                aliases.extend_from_slice(&[
                    "anthropic/claude-opus-4.7".to_string(),
                    "claude-opus-4-7-latest".to_string(),
                ]);
            }
            "claude-opus-4-6" => {
                aliases.extend_from_slice(&[
                    "anthropic/claude-opus-4.6".to_string(),
                    "claude-opus-4-6-20260114".to_string(),
                ]);
            }
            "claude-sonnet-4-6" => {
                aliases.extend_from_slice(&[
                    "anthropic/claude-sonnet-4.6".to_string(),
                    "claude-sonnet-4-6-20251001".to_string(),
                ]);
            }
            "claude-sonnet-4-5" => {
                aliases.extend_from_slice(&[
                    "anthropic/claude-sonnet-4.5".to_string(),
                    "claude-sonnet-4-5-20250929".to_string(),
                ]);
            }
            "gpt-4" => {
                aliases.extend_from_slice(&[
                    "openai/gpt-4".to_string(),
                    "gpt-4-0314".to_string(),
                    "gpt-4-0613".to_string(),
                ]);
            }
            "claude-3-opus" => {
                aliases.extend_from_slice(&[
                    "anthropic/claude-3-opus".to_string(),
                    "claude-3-opus-20240229".to_string(),
                ]);
            }
            "gemini-3.1-pro-preview" => {
                aliases.extend_from_slice(&[
                    "google/gemini-3.1-pro-preview".to_string(),
                    "gemini-3.1-pro-preview-customtools".to_string(),
                ]);
            }
            "gemini-pro" => {
                aliases.extend_from_slice(&[
                    "google/gemini-pro".to_string(),
                    "gemini-1.0-pro".to_string(),
                ]);
            }
            _ => {}
        }

        aliases
    }

    pub fn is_chat_model(model: &str) -> bool {
        let model_lower = model.to_lowercase();

        let chat_patterns = ["gpt-", "claude-", "gemini-", "command", "llama", "mistral"];

        chat_patterns
            .iter()
            .any(|pattern| model_lower.contains(pattern))
    }

    pub fn is_completion_model(model: &str) -> bool {
        let model_lower = model.to_lowercase();

        let completion_patterns = [
            "text-davinci",
            "text-curie",
            "text-babbage",
            "text-ada",
            "davinci",
            "curie",
        ];

        completion_patterns
            .iter()
            .any(|pattern| model_lower.contains(pattern))
    }

    pub fn get_recommended_temperature(model: &str) -> f32 {
        match Self::get_model_family(model).as_str() {
            "gpt" => 0.7,
            "claude" => 0.9,
            "gemini" => 0.8,
            "command" => 0.8,
            _ => 0.7,
        }
    }
}

fn infer_pricing_provider(model_lower: &str) -> Option<&'static str> {
    if model_lower.starts_with("gpt-")
        || model_lower.starts_with("o3")
        || model_lower.starts_with("o4")
        || model_lower.starts_with("chatgpt-")
    {
        Some("openai")
    } else if model_lower.starts_with("claude-") {
        Some("anthropic")
    } else if model_lower.starts_with("gemini-") {
        Some("vertex_ai")
    } else if model_lower.starts_with("deepseek-") {
        Some("deepseek")
    } else if model_lower.starts_with("moonshot-") || model_lower.starts_with("kimi-") {
        Some("moonshot")
    } else if model_lower.starts_with("minimax-") || model_lower.starts_with("m2.") {
        Some("minimax")
    } else if model_lower.starts_with("glm-") {
        Some("zhipuai")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== get_model_pricing Tests ====================

    #[test]
    fn test_get_model_pricing_gpt52() {
        let pricing = ModelUtils::get_model_pricing("gpt-5.2");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.00175).abs() < f64::EPSILON);
        assert!((output - 0.014).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gpt54() {
        let pricing = ModelUtils::get_model_pricing("gpt-5.4");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.0025).abs() < f64::EPSILON);
        assert!((output - 0.015).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gpt4_turbo() {
        let pricing = ModelUtils::get_model_pricing("gpt-4-turbo-preview");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.01).abs() < f64::EPSILON);
        assert!((output - 0.03).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gpt4() {
        let pricing = ModelUtils::get_model_pricing("gpt-4");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.03).abs() < f64::EPSILON);
        assert!((output - 0.06).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gpt35() {
        let pricing = ModelUtils::get_model_pricing("gpt-3.5-turbo");
        assert!(pricing.is_some());
        let (input, _output) = pricing.unwrap();
        assert!((input - 0.0015).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gpt4o_mini_uses_cost_source() {
        let pricing = ModelUtils::get_model_pricing("gpt-4o-mini");
        assert_eq!(pricing, Some((0.00015, 0.0006)));
    }

    #[test]
    fn test_get_model_pricing_claude_opus() {
        let pricing = ModelUtils::get_model_pricing("claude-3-opus-20240229");
        assert!(pricing.is_some());
    }

    #[test]
    fn test_get_model_pricing_claude_opus_47() {
        let pricing = ModelUtils::get_model_pricing("claude-opus-4-7");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.005).abs() < f64::EPSILON);
        assert!((output - 0.025).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_claude_sonnet() {
        let pricing = ModelUtils::get_model_pricing("claude-3-sonnet");
        assert!(pricing.is_some());
    }

    #[test]
    fn test_get_model_pricing_claude_haiku() {
        let pricing = ModelUtils::get_model_pricing("claude-3-haiku");
        assert!(pricing.is_some());
    }

    #[test]
    fn test_get_model_pricing_gemini() {
        let pricing = ModelUtils::get_model_pricing("gemini-3.1-pro-preview");
        assert!(pricing.is_some());
    }

    #[test]
    fn test_get_model_pricing_gemini_20_flash() {
        let pricing = ModelUtils::get_model_pricing("gemini-2.0-flash");
        assert_eq!(pricing, Some((0.0001, 0.0004)));
    }

    #[test]
    fn test_get_model_pricing_gemini_20_flash_thinking() {
        let pricing = ModelUtils::get_model_pricing("gemini-2.0-flash-thinking-exp");
        assert_eq!(pricing, Some((0.00001, 0.00004)));
    }

    #[cfg(feature = "providers-extended")]
    #[test]
    fn test_get_model_pricing_gemini_20_flash_thinking_matches_registry() {
        let model_id = "gemini-2.0-flash-thinking-exp";
        let utility_pricing = ModelUtils::get_model_pricing(model_id);
        let registry = crate::core::providers::gemini::get_gemini_registry();
        let registry_pricing = registry.get_model_spec(model_id).map(|spec| {
            (
                spec.model_info
                    .input_cost_per_1k_tokens
                    .expect("Gemini thinking model should define input pricing"),
                spec.model_info
                    .output_cost_per_1k_tokens
                    .expect("Gemini thinking model should define output pricing"),
            )
        });

        assert_eq!(utility_pricing, registry_pricing);
        assert_eq!(utility_pricing, Some((0.00001, 0.00004)));
    }

    #[test]
    fn test_get_model_pricing_gemini_25_flash_lite_prefers_lite_rate() {
        let pricing = ModelUtils::get_model_pricing("gemini-2.5-flash-lite");
        assert_eq!(pricing, Some((0.0001, 0.0004)));
    }

    #[test]
    fn test_get_model_pricing_gemini_15_flash() {
        let pricing = ModelUtils::get_model_pricing("gemini-1.5-flash");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.000075).abs() < f64::EPSILON);
        assert!((output - 0.0003).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gemini_31_flash() {
        let pricing = ModelUtils::get_model_pricing("gemini-3.1-flash");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.000075).abs() < f64::EPSILON);
        assert!((output - 0.0003).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_gemini_31_flash_lite() {
        let pricing = ModelUtils::get_model_pricing("gemini-3.1-flash-lite");
        assert!(pricing.is_some());
        let (input, output) = pricing.unwrap();
        assert!((input - 0.0000375).abs() < f64::EPSILON);
        assert!((output - 0.00015).abs() < f64::EPSILON);
    }

    #[test]
    fn test_get_model_pricing_unknown() {
        let pricing = ModelUtils::get_model_pricing("unknown-model-xyz");
        assert!(pricing.is_none());
    }

    #[test]
    fn test_get_model_pricing_case_insensitive() {
        let pricing = ModelUtils::get_model_pricing("GPT-4-TURBO");
        assert!(pricing.is_some());
    }

    // ==================== get_model_aliases Tests ====================

    #[test]
    fn test_get_model_aliases_gpt4() {
        let aliases = ModelUtils::get_model_aliases("gpt-4");
        assert!(!aliases.is_empty());
        assert!(aliases.iter().any(|a| a.contains("openai")));
    }

    #[test]
    fn test_get_model_aliases_gpt55_tiers_are_separate() {
        let base_aliases = ModelUtils::get_model_aliases("gpt-5.5");
        assert!(base_aliases.contains(&"openai/gpt-5.5".to_string()));
        assert!(base_aliases.contains(&"gpt-5.5-2026-04-23".to_string()));
        assert!(!base_aliases.iter().any(|alias| alias.contains("pro")));

        let pro_aliases = ModelUtils::get_model_aliases("gpt-5.5-pro");
        assert!(pro_aliases.contains(&"openai/gpt-5.5-pro".to_string()));
        assert!(pro_aliases.contains(&"gpt-5.5-pro-2026-04-23".to_string()));
        assert!(!pro_aliases.contains(&"gpt-5.5-2026-04-23".to_string()));
    }

    #[test]
    fn test_get_model_aliases_claude() {
        let aliases = ModelUtils::get_model_aliases("claude-3-opus");
        assert!(!aliases.is_empty());
        assert!(aliases.iter().any(|a| a.contains("anthropic")));
    }

    #[test]
    fn test_get_model_aliases_claude_opus_47() {
        let aliases = ModelUtils::get_model_aliases("claude-opus-4-7");
        assert!(!aliases.is_empty());
        assert!(aliases.iter().any(|a| a.contains("4.7")));
    }

    #[test]
    fn test_get_model_aliases_gemini() {
        let aliases = ModelUtils::get_model_aliases("gemini-3.1-pro-preview");
        assert!(!aliases.is_empty());
        assert!(aliases.iter().any(|a| a.contains("google")));
    }

    #[test]
    fn test_get_model_aliases_unknown() {
        let aliases = ModelUtils::get_model_aliases("unknown-model");
        assert!(aliases.is_empty());
    }

    // ==================== is_chat_model Tests ====================

    #[test]
    fn test_is_chat_model_gpt() {
        assert!(ModelUtils::is_chat_model("gpt-4-turbo"));
        assert!(ModelUtils::is_chat_model("gpt-3.5-turbo"));
    }

    #[test]
    fn test_is_chat_model_claude() {
        assert!(ModelUtils::is_chat_model("claude-3-opus"));
        assert!(ModelUtils::is_chat_model("claude-2.1"));
    }

    #[test]
    fn test_is_chat_model_gemini() {
        assert!(ModelUtils::is_chat_model("gemini-pro"));
    }

    #[test]
    fn test_is_chat_model_command() {
        assert!(ModelUtils::is_chat_model("command-r-plus"));
    }

    #[test]
    fn test_is_chat_model_llama() {
        assert!(ModelUtils::is_chat_model("llama-2-70b"));
    }

    #[test]
    fn test_is_chat_model_mistral() {
        assert!(ModelUtils::is_chat_model("mistral-large"));
    }

    #[test]
    fn test_is_chat_model_false() {
        assert!(!ModelUtils::is_chat_model("text-embedding-ada-002"));
    }

    // ==================== is_completion_model Tests ====================

    #[test]
    fn test_is_completion_model_davinci() {
        assert!(ModelUtils::is_completion_model("text-davinci-003"));
        assert!(ModelUtils::is_completion_model("davinci"));
    }

    #[test]
    fn test_is_completion_model_curie() {
        assert!(ModelUtils::is_completion_model("text-curie-001"));
        assert!(ModelUtils::is_completion_model("curie"));
    }

    #[test]
    fn test_is_completion_model_babbage() {
        assert!(ModelUtils::is_completion_model("text-babbage-001"));
    }

    #[test]
    fn test_is_completion_model_ada() {
        assert!(ModelUtils::is_completion_model("text-ada-001"));
    }

    #[test]
    fn test_is_completion_model_false() {
        assert!(!ModelUtils::is_completion_model("gpt-4"));
    }

    // ==================== get_recommended_temperature Tests ====================

    #[test]
    fn test_get_recommended_temperature_gpt() {
        let temp = ModelUtils::get_recommended_temperature("gpt-4");
        assert!((temp - 0.7).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_recommended_temperature_claude() {
        let temp = ModelUtils::get_recommended_temperature("claude-3-opus");
        assert!((temp - 0.9).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_recommended_temperature_gemini() {
        let temp = ModelUtils::get_recommended_temperature("gemini-pro");
        assert!((temp - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_recommended_temperature_command() {
        let temp = ModelUtils::get_recommended_temperature("command-r");
        assert!((temp - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_recommended_temperature_unknown() {
        let temp = ModelUtils::get_recommended_temperature("unknown-model");
        assert!((temp - 0.7).abs() < f32::EPSILON);
    }
}
