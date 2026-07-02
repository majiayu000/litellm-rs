//! Unit tests for BedrockProvider
//!
//! Tests for provider creation, capabilities, message conversion,
//! request/response transformation, and cost calculation.

use super::client::BedrockClient;
use super::config::BedrockConfig;
use super::provider::BedrockProvider;

fn create_test_config() -> BedrockConfig {
    BedrockConfig {
        aws_access_key_id: "AKIAIOSFODNN7EXAMPLE".to_string(),
        aws_secret_access_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
        aws_session_token: None,
        aws_region: "us-east-1".to_string(),
        timeout_seconds: 30,
        max_retries: 3,
    }
}

fn create_test_provider() -> BedrockProvider {
    let config = create_test_config();
    BedrockProvider::new_for_test(BedrockClient::new(config).unwrap(), vec![])
}

mod cost_and_access_tests;
mod creation_capability_tests;
mod prompt_param_tests;
mod request_transform_tests;
mod response_transform_tests;
