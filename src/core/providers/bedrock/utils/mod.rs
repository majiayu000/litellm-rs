//! Utility modules for Bedrock provider
//!
//! Contains shared utilities for AWS authentication, region management,
//! cost calculation, and other common functionality.

pub mod auth;
pub mod cost;
pub mod region;

// Re-export main types and functions
pub use auth::{AwsAuth, AwsCredentials};
pub use cost::{CostCalculator, ModelPricing};
pub use region::{AWS_REGIONS, is_model_available_in_region, validate_region};

/// Normalize Bedrock model IDs coming from external callers.
/// - Strips optional "bedrock/" prefix
/// - Strips optional region prefix like "us." or "us-east-1."
pub fn normalize_bedrock_model_id(model_id: &str) -> String {
    crate::core::providers::bedrock::parse_bedrock_model_id(model_id)
        .canonical_metadata_id()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_bedrock_model_id;

    #[test]
    fn test_normalize_bedrock_model_id() {
        assert_eq!(
            normalize_bedrock_model_id("bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
        assert_eq!(
            normalize_bedrock_model_id("bedrock/anthropic.claude-3-opus-20240229"),
            "anthropic.claude-3-opus-20240229"
        );
        assert_eq!(
            normalize_bedrock_model_id("us-east-1.anthropic.claude-3-haiku-20240307"),
            "anthropic.claude-3-haiku-20240307"
        );
        assert_eq!(
            normalize_bedrock_model_id("anthropic.claude-3-opus-20240229"),
            "anthropic.claude-3-opus-20240229"
        );
    }
}
