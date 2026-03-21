//! Audio transcription functionality

use crate::utils::error::gateway_error::{GatewayError, Result};
use tracing::info;

use super::types::{TranscriptionRequest, TranscriptionResponse};

/// Audio service for handling audio transcription requests
pub struct TranscriptionService;

impl TranscriptionService {
    /// Create a new transcription service
    pub fn new() -> Self {
        Self
    }

    /// Transcribe audio to text
    pub async fn transcribe(&self, request: TranscriptionRequest) -> Result<TranscriptionResponse> {
        info!(
            "Transcribing audio: model={}, file_size={}",
            request.model,
            request.file.len()
        );

        // Validate file size (max 25MB)
        if request.file.len() > 25 * 1024 * 1024 {
            return Err(GatewayError::validation("Audio file too large (max 25MB)"));
        }

        Err(GatewayError::not_implemented(format!(
            "Audio transcription is not implemented for model {}",
            request.model
        )))
    }
}

/// Parse model string to extract provider and model name
/// Format: "provider/model" or just "model"
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_model_string(model: &str) -> (&str, &str) {
    if let Some(idx) = model.find('/') {
        let provider = &model[..idx];
        let model_name = &model[idx + 1..];
        (provider, model_name)
    } else {
        // Default provider based on model name
        if model.starts_with("whisper") {
            ("groq", model) // Default Whisper to Groq (faster)
        } else {
            // Default to OpenAI for TTS and other models
            ("openai", model)
        }
    }
}
