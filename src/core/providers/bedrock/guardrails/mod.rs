//! Bedrock Guardrails Module
//!
//! Provides content filtering, PII detection, and policy enforcement

use crate::core::providers::unified_provider::ProviderError;
use serde::{Deserialize, Serialize};

/// Guardrail configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailConfig {
    pub guardrail_id: String,
    pub guardrail_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<bool>,
}

/// Guardrail apply request
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailApplyRequest {
    pub content: Vec<GuardrailContent>,
    pub source: GuardrailSource,
}

/// Guardrail content
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailContent {
    pub text: GuardrailText,
}

/// Guardrail text content
#[derive(Debug, Serialize, Deserialize)]
pub struct GuardrailText {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualifiers: Option<Vec<String>>,
}

/// Guardrail source
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum GuardrailSource {
    Input,
    Output,
}

/// Guardrail response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailResponse {
    pub usage: GuardrailUsage,
    pub outputs: Vec<GuardrailOutput>,
    pub action: GuardrailAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessments: Option<Vec<GuardrailAssessment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_coverage: Option<GuardrailCoverage>,
}

/// Guardrail usage
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailUsage {
    pub topic_policy_units: u32,
    pub content_policy_units: u32,
    pub word_policy_units: u32,
    pub sensitive_information_policy_units: u32,
    pub sensitive_information_policy_free_units: u32,
    pub contextual_grounding_policy_units: u32,
}

/// Guardrail output
#[derive(Debug, Deserialize)]
pub struct GuardrailOutput {
    pub text: String,
}

/// Guardrail action
#[derive(Debug, Deserialize)]
pub enum GuardrailAction {
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "GUARDRAIL_INTERVENED", alias = "GUARDRAIL")]
    Guardrail,
}

/// Guardrail assessment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailAssessment {
    pub topic_policy: Option<TopicPolicyAssessment>,
    pub content_policy: Option<ContentPolicyAssessment>,
    pub word_policy: Option<WordPolicyAssessment>,
    pub sensitive_information_policy: Option<SensitiveInformationPolicyAssessment>,
}

/// Topic policy assessment
#[derive(Debug, Deserialize)]
pub struct TopicPolicyAssessment {
    pub topics: Vec<Topic>,
}

/// Topic
#[derive(Debug, Deserialize)]
pub struct Topic {
    pub name: String,
    #[serde(rename = "type")]
    pub topic_type: String,
    pub action: String,
}

/// Content policy assessment
#[derive(Debug, Deserialize)]
pub struct ContentPolicyAssessment {
    pub filters: Vec<ContentFilter>,
}

/// Content filter
#[derive(Debug, Deserialize)]
pub struct ContentFilter {
    #[serde(rename = "type")]
    pub filter_type: String,
    pub confidence: String,
    pub action: String,
}

/// Word policy assessment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordPolicyAssessment {
    pub custom_words: Vec<String>,
    pub managed_word_lists: Vec<String>,
}

/// Sensitive information policy assessment
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveInformationPolicyAssessment {
    pub pii_entities: Vec<PiiEntity>,
    pub regexes: Vec<RegexMatch>,
}

/// PII entity
#[derive(Debug, Deserialize)]
pub struct PiiEntity {
    #[serde(rename = "type")]
    pub entity_type: String,
    pub action: String,
}

/// Regex match
#[derive(Debug, Deserialize)]
pub struct RegexMatch {
    pub name: String,
    pub action: String,
}

/// Guardrail coverage
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailCoverage {
    pub text_characters: TextCharactersCoverage,
}

/// Text characters coverage
#[derive(Debug, Deserialize)]
pub struct TextCharactersCoverage {
    pub guarded: u32,
    pub total: u32,
}

/// Guardrail client
pub struct GuardrailClient<'a> {
    client: &'a crate::core::providers::bedrock::client::BedrockClient,
}

impl<'a> GuardrailClient<'a> {
    /// Create a new guardrail client
    pub fn new(client: &'a crate::core::providers::bedrock::client::BedrockClient) -> Self {
        Self { client }
    }

    /// Apply guardrails to content
    pub async fn apply(
        &self,
        guardrail_id: &str,
        guardrail_version: &str,
        content: &str,
        source: GuardrailSource,
    ) -> Result<GuardrailResponse, ProviderError> {
        let request = GuardrailApplyRequest {
            content: vec![GuardrailContent {
                text: GuardrailText {
                    text: content.to_string(),
                    qualifiers: None,
                },
            }],
            source,
        };

        let url = format!(
            "guardrail/{}/version/{}/apply",
            guardrail_id, guardrail_version
        );
        let response = self
            .client
            .send_request("", &url, &serde_json::to_value(request)?)
            .await?;

        let guardrail_response: GuardrailResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

        Ok(guardrail_response)
    }

    /// Check if content passes guardrails
    pub async fn check_content(
        &self,
        guardrail_id: &str,
        guardrail_version: &str,
        content: &str,
        source: GuardrailSource,
    ) -> Result<bool, ProviderError> {
        let response = self
            .apply(guardrail_id, guardrail_version, content, source)
            .await?;
        Ok(matches!(response.action, GuardrailAction::None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervened_action_deserializes_and_fails_content_check() {
        let action =
            serde_json::from_value::<GuardrailAction>(serde_json::json!("GUARDRAIL_INTERVENED"))
                .unwrap_or_else(|error| panic!("intervened action should parse: {error}"));

        assert!(matches!(action, GuardrailAction::Guardrail));
    }
}
