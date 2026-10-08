//! Integration tests for GitHub Copilot provider

use super::*;

#[tokio::test]
async fn test_github_copilot_provider_creation_default() {
    let config = GitHubCopilotConfig::default();
    let provider = GitHubCopilotProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_github_copilot_config_from_env() {
    let _env_lock = crate::core::providers::factory::CONSTRUCTION_ENV_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let previous = std::env::var("GITHUB_COPILOT_TOKEN_DIR").ok();
    unsafe { std::env::set_var("GITHUB_COPILOT_TOKEN_DIR", "/custom/path") };
    let config = GitHubCopilotConfig::default();
    assert_eq!(config.get_token_dir(), "/custom/path");
    for blank in ["", " "] {
        let explicit = GitHubCopilotConfig {
            token_dir: Some(blank.into()),
            access_token_file: Some(blank.into()),
            api_key_file: Some(blank.into()),
            ..Default::default()
        };
        assert_eq!(explicit.get_token_dir(), blank);
        assert_eq!(explicit.get_access_token_file(), blank);
        assert_eq!(explicit.get_api_key_file(), blank);
    }
    match previous {
        Some(value) => unsafe { std::env::set_var("GITHUB_COPILOT_TOKEN_DIR", value) },
        None => unsafe { std::env::remove_var("GITHUB_COPILOT_TOKEN_DIR") },
    }
}

// Note: Error conversion tests removed - GitHubCopilotError is now a type alias to ProviderError

#[test]
fn test_github_copilot_model_info_completeness() {
    // Ensure all models have required fields populated
    for model_id in get_available_models() {
        let info = get_model_info(model_id).unwrap();
        assert!(!info.model_id.is_empty());
        assert!(!info.display_name.is_empty());
        if matches!(
            model_id,
            "gpt-6-astra" | "gpt-6-sol" | "gpt-6-luna" | "claude-opus-5.5"
        ) {
            assert_eq!(
                info.max_context_length, 0,
                "unpublished account limits are unknown"
            );
            assert_eq!(info.max_output_length, None);
        } else {
            assert!(info.max_context_length > 0);
            assert!(info.max_output_length.is_some_and(|limit| limit > 0));
        }
    }
}

#[test]
fn test_github_copilot_config_serialization_roundtrip() {
    let config = GitHubCopilotConfig {
        token_dir: Some("/custom/path".to_string()),
        api_base: Some("https://custom.api.com".to_string()),
        timeout: 45,
        max_retries: 5,
        disable_system_to_assistant: true,
        debug: true,
        ..Default::default()
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: GitHubCopilotConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.token_dir, deserialized.token_dir);
    assert_eq!(config.api_base, deserialized.api_base);
    assert_eq!(config.timeout, deserialized.timeout);
    assert_eq!(config.max_retries, deserialized.max_retries);
    assert_eq!(
        config.disable_system_to_assistant,
        deserialized.disable_system_to_assistant
    );
    assert_eq!(config.debug, deserialized.debug);
}

#[test]
fn test_authenticator_creation() {
    let config = GitHubCopilotConfig::default();
    let auth = CopilotAuthenticator::new(&config);

    // Authenticator should be created successfully
    let _ = format!("{:?}", auth);
}

#[test]
fn test_api_key_info_serialization() {
    use super::authenticator::ApiKeyInfo;

    let info = ApiKeyInfo {
        token: "test-token".to_string(),
        expires_at: 1234567890,
        endpoints: super::authenticator::Endpoints {
            api: Some("https://api.example.com".to_string()),
        },
    };

    let json = serde_json::to_string(&info).unwrap();
    let deserialized: ApiKeyInfo = serde_json::from_str(&json).unwrap();

    assert_eq!(info.token, deserialized.token);
    assert_eq!(info.expires_at, deserialized.expires_at);
    assert_eq!(info.endpoints.api, deserialized.endpoints.api);
}
