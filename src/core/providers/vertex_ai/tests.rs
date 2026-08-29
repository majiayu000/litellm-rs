use super::*;
use crate::core::traits::provider::ProviderConfig;

// ==================== VertexAIProviderConfig Tests ====================

#[test]
fn test_vertex_ai_provider_config_default() {
    let config = VertexAIProviderConfig::default();
    assert!(config.project_id.is_empty());
    assert_eq!(config.location, "us-central1");
    assert_eq!(config.api_version, "v1");
    assert_eq!(config.timeout_seconds, 60);
    assert_eq!(config.max_retries, 3);
    assert!(!config.enable_experimental);
}

#[test]
fn test_vertex_ai_provider_config_validate_empty_project() {
    let config = VertexAIProviderConfig::default();
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Project ID"));
}

#[test]
fn test_vertex_ai_provider_config_validate_empty_location() {
    let config = VertexAIProviderConfig {
        project_id: "my-project".to_string(),
        location: "".to_string(),
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Location"));
}

#[test]
fn test_vertex_ai_provider_config_validate_success() {
    let config = VertexAIProviderConfig {
        project_id: "my-project".to_string(),
        location: "us-central1".to_string(),
        ..Default::default()
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_vertex_ai_provider_config_api_key() {
    let config = VertexAIProviderConfig::default();
    assert!(config.api_key().is_none());
}

#[test]
fn test_vertex_ai_provider_config_api_base_default() {
    let config = VertexAIProviderConfig::default();
    assert_eq!(config.api_base(), Some("https://aiplatform.googleapis.com"));
}

#[test]
fn test_vertex_ai_provider_config_api_base_custom() {
    let config = VertexAIProviderConfig {
        api_base: Some("https://custom.endpoint.com".to_string()),
        ..Default::default()
    };
    assert_eq!(config.api_base(), Some("https://custom.endpoint.com"));
}

#[test]
fn test_vertex_ai_provider_config_timeout() {
    let config = VertexAIProviderConfig {
        timeout_seconds: 120,
        ..Default::default()
    };
    assert_eq!(config.timeout(), std::time::Duration::from_secs(120));
}

#[test]
fn test_vertex_ai_provider_config_max_retries() {
    let config = VertexAIProviderConfig {
        max_retries: 5,
        ..Default::default()
    };
    assert_eq!(config.max_retries(), 5);
}

// ==================== VertexAIModel Tests ====================

#[test]
fn test_vertex_ai_model_gemini_ids() {
    assert_eq!(
        VertexAIModel::Gemini31ProPreview.model_id(),
        "gemini-3.1-pro-preview"
    );
    assert_eq!(VertexAIModel::Gemini31Flash.model_id(), "gemini-3.1-flash");
    assert_eq!(
        VertexAIModel::Gemini31FlashLite.model_id(),
        "gemini-3.1-flash-lite"
    );
    assert_eq!(
        VertexAIModel::Gemini3FlashPreview.model_id(),
        "gemini-3-flash-preview"
    );
    assert_eq!(VertexAIModel::Gemini25Pro.model_id(), "gemini-2.5-pro");
    assert_eq!(VertexAIModel::Gemini25Flash.model_id(), "gemini-2.5-flash");
    assert_eq!(VertexAIModel::Gemini20Flash.model_id(), "gemini-2.0-flash");
    assert_eq!(VertexAIModel::GeminiPro.model_id(), "gemini-1.5-pro-002");
    assert_eq!(
        VertexAIModel::GeminiFlash.model_id(),
        "gemini-1.5-flash-002"
    );
}

#[test]
fn test_vertex_ai_model_claude_ids() {
    assert_eq!(VertexAIModel::ClaudeOpus47.model_id(), "claude-opus-4-7");
    assert_eq!(
        VertexAIModel::ClaudeOpus46.model_id(),
        "claude-opus-4-6@20260114"
    );
    assert_eq!(
        VertexAIModel::ClaudeOpus45.model_id(),
        "claude-opus-4-5@20251110"
    );
    assert_eq!(
        VertexAIModel::ClaudeSonnet46.model_id(),
        "claude-sonnet-4-6"
    );
    assert_eq!(
        VertexAIModel::ClaudeSonnet45.model_id(),
        "claude-sonnet-4-5@20250929"
    );
    assert_eq!(
        VertexAIModel::ClaudeHaiku45.model_id(),
        "claude-haiku-4-5@20251001"
    );
    assert_eq!(
        VertexAIModel::ClaudeSonnet4.model_id(),
        "claude-sonnet-4@20250514"
    );
    assert_eq!(
        VertexAIModel::Claude3Opus.model_id(),
        "claude-3-opus@20240229"
    );
    assert_eq!(
        VertexAIModel::Claude3Sonnet.model_id(),
        "claude-3-sonnet@20240229"
    );
    assert_eq!(
        VertexAIModel::Claude3Haiku.model_id(),
        "claude-3-haiku@20240307"
    );
    assert_eq!(
        VertexAIModel::Claude35Sonnet.model_id(),
        "claude-3-5-sonnet@20241022"
    );
}

#[test]
fn test_vertex_ai_model_llama_ids() {
    assert_eq!(
        VertexAIModel::Llama3_70B.model_id(),
        "meta/llama3-70b-instruct-maas"
    );
    assert_eq!(
        VertexAIModel::Llama31_405B.model_id(),
        "meta/llama-3.1-405b-instruct-maas"
    );
    assert_eq!(
        VertexAIModel::Llama4Scout.model_id(),
        "meta/llama-4-scout-17b-16e-instruct"
    );
}

#[test]
fn test_vertex_ai_model_custom() {
    let model = VertexAIModel::Custom("my-custom-model".to_string());
    assert_eq!(model.model_id(), "my-custom-model");
}

#[test]
fn test_vertex_ai_model_is_gemini() {
    assert!(VertexAIModel::Gemini31ProPreview.is_gemini());
    assert!(VertexAIModel::Gemini3FlashPreview.is_gemini());
    assert!(VertexAIModel::Gemini25Pro.is_gemini());
    assert!(VertexAIModel::Gemini20Flash.is_gemini());
    assert!(VertexAIModel::GeminiPro.is_gemini());
    assert!(VertexAIModel::GeminiFlash.is_gemini());
    assert!(VertexAIModel::GeminiUltra.is_gemini());

    assert!(!VertexAIModel::Claude3Opus.is_gemini());
    assert!(!VertexAIModel::Llama3_70B.is_gemini());
}

#[test]
fn test_vertex_ai_model_is_partner_model() {
    assert!(VertexAIModel::ClaudeOpus47.is_partner_model());
    assert!(VertexAIModel::ClaudeOpus46.is_partner_model());
    assert!(VertexAIModel::ClaudeOpus45.is_partner_model());
    assert!(VertexAIModel::ClaudeSonnet46.is_partner_model());
    assert!(VertexAIModel::ClaudeSonnet45.is_partner_model());
    assert!(VertexAIModel::ClaudeHaiku45.is_partner_model());
    assert!(VertexAIModel::Claude3Opus.is_partner_model());
    assert!(VertexAIModel::Claude35Sonnet.is_partner_model());
    assert!(VertexAIModel::Llama3_70B.is_partner_model());
    assert!(VertexAIModel::Llama4Scout.is_partner_model());
    assert!(VertexAIModel::Jamba15Large.is_partner_model());
    assert!(VertexAIModel::MistralLarge.is_partner_model());

    assert!(!VertexAIModel::GeminiPro.is_partner_model());
    assert!(!VertexAIModel::Gemini20Flash.is_partner_model());
}

#[test]
fn test_vertex_ai_model_supports_vision() {
    assert!(VertexAIModel::Gemini31ProPreview.supports_vision());
    assert!(VertexAIModel::Gemini3FlashPreview.supports_vision());
    assert!(VertexAIModel::Gemini25Pro.supports_vision());
    assert!(VertexAIModel::Gemini20Flash.supports_vision());
    assert!(VertexAIModel::GeminiPro.supports_vision());
    assert!(VertexAIModel::GeminiProVision.supports_vision());
    assert!(VertexAIModel::Llama32_90B.supports_vision());
    assert!(VertexAIModel::Llama4Scout.supports_vision());

    assert!(VertexAIModel::ClaudeOpus47.supports_vision());
    assert!(VertexAIModel::ClaudeOpus46.supports_vision());
    assert!(VertexAIModel::Claude3Opus.supports_vision());
    assert!(!VertexAIModel::Llama3_70B.supports_vision());
}

#[test]
fn test_vertex_ai_model_supports_system_messages() {
    assert!(VertexAIModel::GeminiPro.supports_system_messages());
    assert!(VertexAIModel::Claude3Opus.supports_system_messages());
    assert!(VertexAIModel::Llama3_70B.supports_system_messages());

    assert!(!VertexAIModel::Custom("custom".to_string()).supports_system_messages());
}

#[test]
fn test_vertex_ai_model_supports_response_schema() {
    assert!(VertexAIModel::GeminiPro.supports_response_schema());
    assert!(VertexAIModel::Gemini20Flash.supports_response_schema());

    assert!(!VertexAIModel::Claude3Opus.supports_response_schema());
    assert!(!VertexAIModel::Llama3_70B.supports_response_schema());
}

#[test]
fn test_vertex_ai_model_supports_function_calling() {
    assert!(VertexAIModel::GeminiPro.supports_function_calling());
    assert!(VertexAIModel::Claude3Opus.supports_function_calling());
    assert!(VertexAIModel::Claude35Sonnet.supports_function_calling());
    assert!(VertexAIModel::MistralLarge.supports_function_calling());

    assert!(!VertexAIModel::Llama3_70B.supports_function_calling());
    assert!(!VertexAIModel::Jamba15Large.supports_function_calling());
}

#[test]
fn test_vertex_ai_model_supports_thinking_mode() {
    assert!(VertexAIModel::Gemini3ProDeepThink.supports_thinking_mode());
    assert!(VertexAIModel::Gemini25Pro.supports_thinking_mode());
    assert!(VertexAIModel::Gemini25Flash.supports_thinking_mode());
    assert!(VertexAIModel::Gemini20FlashThinking.supports_thinking_mode());

    assert!(!VertexAIModel::GeminiPro.supports_thinking_mode());
    assert!(!VertexAIModel::Claude3Opus.supports_thinking_mode());
}

#[test]
fn test_vertex_ai_model_max_context_tokens() {
    // Gemini 3
    assert_eq!(
        VertexAIModel::Gemini31ProPreview.max_context_tokens(),
        1_048_576
    );
    assert_eq!(
        VertexAIModel::Gemini3FlashPreview.max_context_tokens(),
        1_048_576
    );
    assert_eq!(VertexAIModel::Gemini3ProImage.max_context_tokens(), 65_536);

    // Gemini 2.5
    assert_eq!(VertexAIModel::Gemini25Pro.max_context_tokens(), 1_048_576);

    // Gemini 1.5 Pro has largest
    assert_eq!(VertexAIModel::GeminiPro.max_context_tokens(), 2_097_152);

    assert_eq!(VertexAIModel::ClaudeOpus47.max_context_tokens(), 1_000_000);
    assert_eq!(VertexAIModel::ClaudeOpus46.max_context_tokens(), 1_000_000);
    assert_eq!(VertexAIModel::ClaudeOpus45.max_context_tokens(), 200_000);

    // Claude
    assert_eq!(VertexAIModel::Claude3Opus.max_context_tokens(), 200_000);

    // Llama 4 Scout
    assert_eq!(VertexAIModel::Llama4Scout.max_context_tokens(), 10_000_000);

    // Jamba
    assert_eq!(VertexAIModel::Jamba15Large.max_context_tokens(), 256_000);

    // Custom
    assert_eq!(
        VertexAIModel::Custom("custom".to_string()).max_context_tokens(),
        32_768
    );
}

// ==================== parse_vertex_model Tests ====================

#[test]
fn test_parse_vertex_model_gemini_3() {
    assert!(matches!(
        parse_vertex_model("gemini-3.1-pro-preview"),
        VertexAIModel::Gemini31ProPreview
    ));
    assert!(matches!(
        parse_vertex_model("gemini-3.1-flash"),
        VertexAIModel::Gemini31Flash
    ));
    assert!(matches!(
        parse_vertex_model("gemini-3.1-flash-lite"),
        VertexAIModel::Gemini31FlashLite
    ));
    assert!(matches!(
        parse_vertex_model("gemini-3-flash-preview"),
        VertexAIModel::Gemini3FlashPreview
    ));
    assert!(matches!(
        parse_vertex_model("gemini-3-pro-deep-think"),
        VertexAIModel::Gemini3ProDeepThink
    ));
    assert!(matches!(
        parse_vertex_model("gemini-3-pro-image-preview"),
        VertexAIModel::Gemini3ProImage
    ));
    assert!(parse_vertex_model("gemini-3.1-pro-preview").is_gemini());
}

#[test]
fn test_parse_vertex_model_gemini_25() {
    assert!(matches!(
        parse_vertex_model("gemini-2.5-pro"),
        VertexAIModel::Gemini25Pro
    ));
    assert!(matches!(
        parse_vertex_model("gemini-2.5-flash"),
        VertexAIModel::Gemini25Flash
    ));
    assert!(matches!(
        parse_vertex_model("gemini-2.5-flash-lite"),
        VertexAIModel::Gemini25FlashLite
    ));
}

#[test]
fn test_parse_vertex_model_gemini_20() {
    assert!(matches!(
        parse_vertex_model("gemini-2.0-flash"),
        VertexAIModel::Gemini20Flash
    ));
    assert!(matches!(
        parse_vertex_model("gemini-2.0-flash-exp"),
        VertexAIModel::Gemini20FlashExp
    ));
    assert!(matches!(
        parse_vertex_model("gemini-2.0-flash-thinking"),
        VertexAIModel::Gemini20FlashThinking
    ));
    assert!(matches!(
        parse_vertex_model("gemini-2.0-flash-lite"),
        VertexAIModel::Gemini20FlashLite
    ));
}

#[test]
fn test_parse_vertex_model_gemini_15() {
    assert!(matches!(
        parse_vertex_model("gemini-1.5-pro"),
        VertexAIModel::GeminiPro
    ));
    assert!(matches!(
        parse_vertex_model("gemini-pro"),
        VertexAIModel::GeminiPro
    ));
    assert!(matches!(
        parse_vertex_model("gemini-1.5-flash"),
        VertexAIModel::GeminiFlash
    ));
    assert!(matches!(
        parse_vertex_model("gemini-flash"),
        VertexAIModel::GeminiFlash
    ));
    assert!(matches!(
        parse_vertex_model("gemini-1.5-flash-8b"),
        VertexAIModel::GeminiFlash8B
    ));
}

#[test]
fn test_parse_vertex_model_claude() {
    assert!(matches!(
        parse_vertex_model("claude-opus-4-7"),
        VertexAIModel::ClaudeOpus47
    ));
    assert!(matches!(
        parse_vertex_model("claude-opus-4-6"),
        VertexAIModel::ClaudeOpus46
    ));
    assert!(matches!(
        parse_vertex_model("claude-opus-4-5"),
        VertexAIModel::ClaudeOpus45
    ));
    assert!(matches!(
        parse_vertex_model("claude-opus-4.7"),
        VertexAIModel::ClaudeOpus47
    ));
    assert!(matches!(
        parse_vertex_model("claude-sonnet-4-6"),
        VertexAIModel::ClaudeSonnet46
    ));
    assert!(matches!(
        parse_vertex_model("claude-sonnet-4-5"),
        VertexAIModel::ClaudeSonnet45
    ));
    assert!(matches!(
        parse_vertex_model("claude-haiku-4-5"),
        VertexAIModel::ClaudeHaiku45
    ));
    assert!(matches!(
        parse_vertex_model("claude-sonnet-4"),
        VertexAIModel::ClaudeSonnet4
    ));
    assert!(matches!(
        parse_vertex_model("claude-3-opus"),
        VertexAIModel::Claude3Opus
    ));
    assert!(matches!(
        parse_vertex_model("claude-3-sonnet"),
        VertexAIModel::Claude3Sonnet
    ));
    assert!(matches!(
        parse_vertex_model("claude-3-haiku"),
        VertexAIModel::Claude3Haiku
    ));
    assert!(matches!(
        parse_vertex_model("claude-3-5-sonnet"),
        VertexAIModel::Claude35Sonnet
    ));
    assert!(matches!(
        parse_vertex_model("claude-3.5-sonnet"),
        VertexAIModel::Claude35Sonnet
    ));
}

#[test]
fn test_parse_vertex_model_llama() {
    assert!(matches!(
        parse_vertex_model("llama3-70b"),
        VertexAIModel::Llama3_70B
    ));
    assert!(matches!(
        parse_vertex_model("llama-3-70b"),
        VertexAIModel::Llama3_70B
    ));
    assert!(matches!(
        parse_vertex_model("llama-3.1-405b"),
        VertexAIModel::Llama31_405B
    ));
    assert!(matches!(
        parse_vertex_model("llama-3.2-90b"),
        VertexAIModel::Llama32_90B
    ));
    assert!(matches!(
        parse_vertex_model("llama-4-scout"),
        VertexAIModel::Llama4Scout
    ));
    assert!(matches!(
        parse_vertex_model("llama-4-maverick"),
        VertexAIModel::Llama4Maverick
    ));
}

#[test]
fn test_parse_vertex_model_jamba() {
    assert!(matches!(
        parse_vertex_model("jamba-1.5-large"),
        VertexAIModel::Jamba15Large
    ));
    assert!(matches!(
        parse_vertex_model("jamba-1.5-mini"),
        VertexAIModel::Jamba15Mini
    ));
    assert!(matches!(
        parse_vertex_model("jamba-2"),
        VertexAIModel::Jamba2
    ));
}

#[test]
fn test_parse_vertex_model_mistral() {
    assert!(matches!(
        parse_vertex_model("mistral-large"),
        VertexAIModel::MistralLarge
    ));
    assert!(matches!(
        parse_vertex_model("mistral-nemo"),
        VertexAIModel::MistralNemo
    ));
    assert!(matches!(
        parse_vertex_model("codestral"),
        VertexAIModel::Codestral
    ));
}

#[test]
fn test_parse_vertex_model_custom() {
    let model = parse_vertex_model("unknown-model");
    assert!(matches!(model, VertexAIModel::Custom(_)));
    if let VertexAIModel::Custom(id) = model {
        assert_eq!(id, "unknown-model");
    }
}

#[test]
fn test_parse_vertex_model_case_insensitive() {
    assert!(matches!(
        parse_vertex_model("GEMINI-2.5-PRO"),
        VertexAIModel::Gemini25Pro
    ));
    assert!(matches!(
        parse_vertex_model("Claude-3-Opus"),
        VertexAIModel::Claude3Opus
    ));
    assert!(matches!(
        parse_vertex_model("LLAMA-3.1-405B"),
        VertexAIModel::Llama31_405B
    ));
}

#[test]
fn test_vertex_ai_model_clone() {
    let model = VertexAIModel::GeminiPro;
    let cloned = model.clone();
    assert_eq!(model.model_id(), cloned.model_id());
}

#[test]
fn test_vertex_ai_model_debug() {
    let model = VertexAIModel::GeminiPro;
    let debug = format!("{:?}", model);
    assert!(debug.contains("GeminiPro"));
}
