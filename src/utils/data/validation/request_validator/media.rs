use super::RequestValidator;
use crate::utils::error::gateway_error::{GatewayError, Result};

impl RequestValidator {
    /// Validate image URL
    pub(super) fn validate_image_url(url: &str) -> Result<()> {
        if url.starts_with("data:image/") {
            // Base64 encoded image
            Self::validate_base64_image(url)?;
        } else {
            // Regular URL
            url::Url::parse(url)
                .map_err(|e| GatewayError::Validation(format!("Invalid image URL: {}", e)))?;
        }
        Ok(())
    }

    /// Validate base64 image data
    pub(super) fn validate_base64_image(data_url: &str) -> Result<()> {
        if !data_url.starts_with("data:image/") {
            return Err(GatewayError::Validation(
                "Invalid image data URL format".to_string(),
            ));
        }

        let parts: Vec<&str> = data_url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Validation(
                "Invalid image data URL format".to_string(),
            ));
        }

        // Validate base64 data
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, parts[1])
            .map_err(|e| GatewayError::Validation(format!("Invalid base64 image data: {}", e)))?;

        Ok(())
    }

    /// Validate audio data
    pub(super) fn validate_audio_data(data: &str) -> Result<()> {
        Self::validate_base64_payload(data, "audio")?;
        Ok(())
    }

    pub(super) fn validate_base64_payload(data: &str, kind: &str) -> Result<()> {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data).map_err(|e| {
            GatewayError::Validation(format!("Invalid base64 {} data: {}", kind, e))
        })?;
        Ok(())
    }

    /// Validate audio format
    pub(super) fn validate_audio_format(format: &str) -> Result<()> {
        let valid_formats = ["mp3", "wav", "flac", "m4a", "ogg", "webm"];
        if !valid_formats.contains(&format) {
            return Err(GatewayError::Validation(format!(
                "Invalid audio format: {}. Supported formats: {:?}",
                format, valid_formats
            )));
        }
        Ok(())
    }
}
