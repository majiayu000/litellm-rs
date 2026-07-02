use super::RequestValidator;
use crate::utils::error::gateway_error::{GatewayError, Result};
use regex::Regex;

impl RequestValidator {
    /// Validate model name
    pub(super) fn validate_model_name(model: &str) -> Result<()> {
        if model.trim().is_empty() {
            return Err(GatewayError::Validation(
                "Model name cannot be empty".to_string(),
            ));
        }

        // Check for valid characters
        let model_regex = Regex::new(r"^[a-zA-Z0-9._/-]+$")
            .map_err(|e| GatewayError::Internal(format!("Regex error: {}", e)))?;

        if !model_regex.is_match(model) {
            return Err(GatewayError::Validation(
                "Model name contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate function name
    pub(super) fn validate_function_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(GatewayError::Validation(
                "Function name cannot be empty".to_string(),
            ));
        }

        // Function names should follow identifier rules
        let name_regex = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
            .map_err(|e| GatewayError::Internal(format!("Regex error: {}", e)))?;

        if !name_regex.is_match(name) {
            return Err(GatewayError::Validation(
                "Function name must be a valid identifier".to_string(),
            ));
        }

        Ok(())
    }
}
