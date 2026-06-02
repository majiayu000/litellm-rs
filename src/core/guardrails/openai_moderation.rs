//! OpenAI Moderation API integration
//!
//! This module provides integration with OpenAI's content moderation API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use super::config::OpenAIModerationConfig;
use super::traits::Guardrail;
use super::types::{
    CheckResult, GuardrailError, GuardrailResult, ModerationCategory, ModerationResult, Violation,
    ViolationType,
};
use crate::utils::net::http::create_custom_client;

/// OpenAI Moderation API guardrail
pub struct OpenAIModerationGuardrail {
    config: OpenAIModerationConfig,
    client: Client,
}

impl OpenAIModerationGuardrail {
    /// Create a new OpenAI moderation guardrail
    pub fn new(config: OpenAIModerationConfig) -> GuardrailResult<Self> {
        let client = create_custom_client(Duration::from_millis(config.timeout_ms))
            .map_err(|e| GuardrailError::Config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Create from environment variables
    pub fn from_env() -> GuardrailResult<Self> {
        Self::new(OpenAIModerationConfig::from_env())
    }

    /// Call the OpenAI moderation API
    async fn call_api(&self, content: &str) -> GuardrailResult<ModerationApiResponse> {
        let api_key =
            self.config.api_key.as_ref().ok_or_else(|| {
                GuardrailError::Config("OpenAI API key not configured".to_string())
            })?;

        let url = format!("{}/moderations", self.config.base_url);

        let request = ModerationApiRequest {
            input: content.to_string(),
            model: self.config.model.clone(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let response_body_bytes = response
                .bytes()
                .await
                .map_or_else(|_| "unknown".to_string(), |bytes| bytes.len().to_string());
            return Err(GuardrailError::Api(format!(
                "OpenAI API error: status={} response_body_bytes={}",
                status, response_body_bytes
            )));
        }

        let api_response: ModerationApiResponse = response.json().await?;
        Ok(api_response)
    }

    /// Convert API response to ModerationResult
    fn parse_response(&self, response: ModerationApiResponse) -> ModerationResult {
        let mut result = ModerationResult::new();

        if let Some(first_result) = response.results.first() {
            result.flagged = first_result.flagged;

            // Parse categories
            for (name, flagged) in &first_result.categories {
                if let Some(category) = ModerationCategory::from_api_name(name) {
                    result.categories.insert(category, *flagged);
                }
            }

            // Parse scores
            for (name, score) in &first_result.category_scores {
                if let Some(category) = ModerationCategory::from_api_name(name) {
                    result.category_scores.insert(category, *score);
                }
            }
        }

        result
    }

    /// Check if a category should be flagged based on config
    fn should_flag(&self, category: &ModerationCategory, score: f64) -> bool {
        // If specific categories are configured, only check those
        if !self.config.categories.is_empty() && !self.config.categories.contains(category) {
            return false;
        }

        score >= self.config.threshold
    }

    /// Create violations from moderation result
    fn create_violations(&self, result: &ModerationResult) -> Vec<Violation> {
        let mut violations = Vec::new();

        for (category, &flagged) in &result.categories {
            if flagged {
                let score = result.score(category);
                if self.should_flag(category, score) {
                    violations.push(
                        Violation::new(
                            ViolationType::Moderation(category.clone()),
                            format!(
                                "Content flagged for {}: score {:.2}",
                                category.to_api_name(),
                                score
                            ),
                        )
                        .with_severity(score),
                    );
                }
            }
        }

        violations
    }
}

#[async_trait]
impl Guardrail for OpenAIModerationGuardrail {
    fn name(&self) -> &str {
        "openai_moderation"
    }

    fn description(&self) -> &str {
        "OpenAI Content Moderation API integration"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled && self.config.api_key.is_some()
    }

    fn priority(&self) -> u32 {
        10 // High priority - run early
    }

    async fn check_input(&self, content: &str) -> GuardrailResult<CheckResult> {
        if !self.is_enabled() {
            return Ok(CheckResult::pass());
        }

        // Skip empty content
        if content.trim().is_empty() {
            return Ok(CheckResult::pass());
        }

        // Call API
        let api_response = self.call_api(content).await?;
        let moderation_result = self.parse_response(api_response);

        // Create violations
        let violations = self.create_violations(&moderation_result);

        if violations.is_empty() {
            Ok(CheckResult::pass().with_metadata(
                "moderation_result",
                serde_json::to_value(&moderation_result)?,
            ))
        } else {
            let mut result = CheckResult::block(violations);
            result.action = self.config.action;
            result.passed = self.config.action != super::types::GuardrailAction::Block;
            result = result.with_metadata(
                "moderation_result",
                serde_json::to_value(&moderation_result)?,
            );
            Ok(result)
        }
    }
}

/// OpenAI Moderation API request
#[derive(Debug, Serialize)]
struct ModerationApiRequest {
    input: String,
    model: String,
}

/// OpenAI Moderation API response
#[derive(Debug, Deserialize)]
struct ModerationApiResponse {
    #[serde(rename = "id")]
    _id: String,
    #[serde(rename = "model")]
    _model: String,
    results: Vec<ModerationApiResult>,
}

/// Single moderation result
#[derive(Debug, Deserialize)]
struct ModerationApiResult {
    flagged: bool,
    categories: HashMap<String, bool>,
    category_scores: HashMap<String, f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn create_test_guardrail() -> OpenAIModerationGuardrail {
        let config = OpenAIModerationConfig {
            enabled: true,
            api_key: Some("test-key".to_string()),
            threshold: 0.5,
            ..Default::default()
        };
        OpenAIModerationGuardrail::new(config).unwrap()
    }

    async fn start_moderation_error_server(
        status_line: &str,
        body: &str,
        include_content_length: bool,
    ) -> std::io::Result<String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let status_line = status_line.to_string();
        let body = body.to_string();

        tokio::spawn(async move {
            let (mut stream, _) = match listener.accept().await {
                Ok(connection) => connection,
                Err(error) => panic!("mock moderation server should accept request: {}", error),
            };
            let mut request_buffer = [0_u8; 1024];
            if let Err(error) = stream.read(&mut request_buffer).await {
                panic!(
                    "mock moderation server should read request bytes: {}",
                    error
                );
            }
            let content_length_header = if include_content_length {
                format!("Content-Length: {}\r\n", body.len())
            } else {
                String::new()
            };
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: application/json\r\n{}Connection: close\r\n\r\n{}",
                status_line, content_length_header, body
            );
            if let Err(error) = stream.write_all(response.as_bytes()).await {
                panic!(
                    "mock moderation server should write response bytes: {}",
                    error
                );
            }
        });

        Ok(format!("http://{}", address))
    }

    #[test]
    fn test_guardrail_creation() {
        let guardrail = create_test_guardrail();
        assert_eq!(guardrail.name(), "openai_moderation");
        assert!(guardrail.is_enabled());
        assert_eq!(guardrail.priority(), 10);
    }

    #[test]
    fn test_guardrail_disabled_without_key() {
        let config = OpenAIModerationConfig {
            enabled: true,
            api_key: None,
            ..Default::default()
        };
        let guardrail = OpenAIModerationGuardrail::new(config).unwrap();
        assert!(!guardrail.is_enabled());
    }

    #[test]
    fn test_parse_response() {
        let guardrail = create_test_guardrail();

        let mut categories = HashMap::new();
        categories.insert("hate".to_string(), true);
        categories.insert("violence".to_string(), false);

        let mut scores = HashMap::new();
        scores.insert("hate".to_string(), 0.8);
        scores.insert("violence".to_string(), 0.1);

        let response = ModerationApiResponse {
            _id: "test-id".to_string(),
            _model: "text-moderation-latest".to_string(),
            results: vec![ModerationApiResult {
                flagged: true,
                categories,
                category_scores: scores,
            }],
        };

        let result = guardrail.parse_response(response);
        assert!(result.is_flagged());
        assert_eq!(result.score(&ModerationCategory::Hate), 0.8);
        assert_eq!(result.score(&ModerationCategory::Violence), 0.1);
    }

    #[test]
    fn test_should_flag() {
        let guardrail = create_test_guardrail();

        // Above threshold
        assert!(guardrail.should_flag(&ModerationCategory::Hate, 0.6));

        // Below threshold
        assert!(!guardrail.should_flag(&ModerationCategory::Hate, 0.3));

        // At threshold
        assert!(guardrail.should_flag(&ModerationCategory::Hate, 0.5));
    }

    #[test]
    fn test_should_flag_with_category_filter() {
        let config = OpenAIModerationConfig {
            enabled: true,
            api_key: Some("test-key".to_string()),
            threshold: 0.5,
            categories: [ModerationCategory::Hate].into_iter().collect(),
            ..Default::default()
        };
        let guardrail = OpenAIModerationGuardrail::new(config).unwrap();

        // Configured category
        assert!(guardrail.should_flag(&ModerationCategory::Hate, 0.6));

        // Not configured category
        assert!(!guardrail.should_flag(&ModerationCategory::Violence, 0.9));
    }

    #[test]
    fn test_create_violations() {
        let guardrail = create_test_guardrail();

        let mut result = ModerationResult::new();
        result.flagged = true;
        result.categories.insert(ModerationCategory::Hate, true);
        result
            .categories
            .insert(ModerationCategory::Violence, false);
        result.category_scores.insert(ModerationCategory::Hate, 0.8);
        result
            .category_scores
            .insert(ModerationCategory::Violence, 0.1);

        let violations = guardrail.create_violations(&result);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0].violation_type,
            ViolationType::Moderation(ModerationCategory::Hate)
        ));
    }

    #[tokio::test]
    async fn test_check_empty_content() {
        let guardrail = create_test_guardrail();
        let result = guardrail.check_input("").await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_check_whitespace_content() {
        let guardrail = create_test_guardrail();
        let result = guardrail.check_input("   \n\t  ").await.unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn test_non_success_api_error_redacts_response_body()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = r#"{"error":{"message":"sensitive-upstream-token should not be logged"}}"#;
        let base_url = start_moderation_error_server("429 Too Many Requests", body, false).await?;
        let config = OpenAIModerationConfig {
            enabled: true,
            api_key: Some("test-key".to_string()),
            base_url,
            threshold: 0.5,
            ..Default::default()
        };
        let guardrail = OpenAIModerationGuardrail::new(config)?;

        let error = match guardrail.check_input("content to moderate").await {
            Ok(_) => panic!("non-2xx moderation response should fail"),
            Err(error) => error.to_string(),
        };

        assert!(error.contains("status=429 Too Many Requests"));
        assert!(error.contains(&format!("response_body_bytes={}", body.len())));
        assert!(!error.contains("sensitive-upstream-token"));
        assert!(!error.contains("should not be logged"));
        Ok(())
    }

    #[test]
    fn test_from_env() {
        // This should not panic even without env vars
        let result = OpenAIModerationGuardrail::from_env();
        assert!(result.is_ok());
    }
}
