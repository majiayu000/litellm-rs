//! Vertex AI Text-to-Speech Module
//!
//! Support for converting text to speech using Google Cloud Text-to-Speech API

use crate::ProviderError;
use serde::{Deserialize, Serialize};

/// Text-to-speech request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToSpeechRequest {
    pub input: TextInput,
    pub voice: VoiceSelectionParams,
    pub audio_config: AudioConfig,
}

/// Text input for synthesis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextInput {
    pub text: Option<String>,
    pub ssml: Option<String>,
}

/// Voice selection parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceSelectionParams {
    pub language_code: String,
    pub name: Option<String>,
    pub ssml_gender: Option<SsmlVoiceGender>,
}

/// Audio configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub audio_encoding: AudioEncoding,
    pub speaking_rate: Option<f32>,
    pub pitch: Option<f32>,
    pub volume_gain_db: Option<f32>,
    pub sample_rate_hertz: Option<i32>,
    pub effects_profile_id: Option<Vec<String>>,
}

/// SSML voice gender
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SsmlVoiceGender {
    SsmlVoiceGenderUnspecified,
    Male,
    Female,
    Neutral,
}

/// Audio encoding format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AudioEncoding {
    AudioEncodingUnspecified,
    Linear16,
    Mp3,
    OggOpus,
    Mulaw,
    Alaw,
}

/// Text-to-speech response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextToSpeechResponse {
    pub audio_content: String, // Base64 encoded audio
}

/// Voice information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub language_codes: Vec<String>,
    pub name: String,
    pub ssml_gender: SsmlVoiceGender,
    pub natural_sample_rate_hertz: i32,
}

/// Text-to-speech handler
pub struct TextToSpeechHandler;

impl TextToSpeechHandler {
    /// Create new text-to-speech handler
    pub fn new(_project_id: String) -> Self {
        Self
    }

    /// Synthesize speech from text
    pub async fn synthesize_speech(
        &self,
        request: TextToSpeechRequest,
    ) -> Result<TextToSpeechResponse, ProviderError> {
        self.validate_request(&request)?;

        // TODO: Implement actual Google Cloud Text-to-Speech API call
        Ok(TextToSpeechResponse {
            audio_content: "UklGRnoGAABXQVZFZm10IBAAAAABAAEAQB8AAEAfAAABAAgAZGF0YQoGAACBhYqFbF1fdJivrJBhNjVgodDbq2EcBj+a2/LDciUFLIHO8tiJNwgZaLvt559NEAxQp+PwtmMcBjiR1/LMeSwFJHfH8N2QQAoUXrTp66hVFApGn+DyvmwhBSuH0fPJdSgHKYDF8OOUQw".to_string(),
        })
    }

    /// List available voices
    pub async fn list_voices(
        &self,
        _language_code: Option<&str>,
    ) -> Result<Vec<Voice>, ProviderError> {
        // TODO: Implement actual voice listing
        Ok(vec![
            Voice {
                language_codes: vec!["en-US".to_string()],
                name: "en-US-Journey-D".to_string(),
                ssml_gender: SsmlVoiceGender::Male,
                natural_sample_rate_hertz: 24000,
            },
            Voice {
                language_codes: vec!["en-US".to_string()],
                name: "en-US-Journey-F".to_string(),
                ssml_gender: SsmlVoiceGender::Female,
                natural_sample_rate_hertz: 24000,
            },
        ])
    }

    /// Validate text-to-speech request
    fn validate_request(&self, request: &TextToSpeechRequest) -> Result<(), ProviderError> {
        // Check that either text or SSML is provided
        if request.input.text.is_none() && request.input.ssml.is_none() {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Either text or SSML input is required",
            ));
        }

        // Validate text length
        if let Some(text) = &request.input.text
            && text.len() > 5000
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Text input too long (max 5000 characters)",
            ));
        }

        // Validate SSML length
        if let Some(ssml) = &request.input.ssml
            && ssml.len() > 5000
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "SSML input too long (max 5000 characters)",
            ));
        }

        // Validate speaking rate
        if let Some(rate) = request.audio_config.speaking_rate
            && !(0.25..=4.0).contains(&rate)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Speaking rate must be between 0.25 and 4.0",
            ));
        }

        // Validate pitch
        if let Some(pitch) = request.audio_config.pitch
            && !(-20.0..=20.0).contains(&pitch)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Pitch must be between -20.0 and 20.0",
            ));
        }

        // Validate volume gain
        if let Some(volume) = request.audio_config.volume_gain_db
            && !(-96.0..=16.0).contains(&volume)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Volume gain must be between -96.0 and 16.0 dB",
            ));
        }

        Ok(())
    }

    /// Get supported languages
    pub fn get_supported_languages(&self) -> Vec<&str> {
        vec![
            "af-ZA", "ar-XA", "bg-BG", "bn-IN", "ca-ES", "cmn-CN", "cmn-TW", "cs-CZ", "da-DK",
            "de-DE", "el-GR", "en-AU", "en-GB", "en-IN", "en-US", "es-ES", "es-US", "fi-FI",
            "fil-PH", "fr-CA", "fr-FR", "gu-IN", "he-IL", "hi-IN", "hr-HR", "hu-HU", "id-ID",
            "is-IS", "it-IT", "ja-JP", "kn-IN", "ko-KR", "lt-LT", "lv-LV", "ml-IN", "mr-IN",
            "ms-MY", "nb-NO", "nl-BE", "nl-NL", "pa-IN", "pl-PL", "pt-BR", "pt-PT", "ro-RO",
            "ru-RU", "sk-SK", "sr-RS", "sv-SE", "ta-IN", "te-IN", "th-TH", "tr-TR", "uk-UA",
            "vi-VN", "yue-HK", "zh-CN", "zh-TW",
        ]
    }

    /// Calculate synthesis cost
    pub fn calculate_cost(&self, character_count: usize) -> f64 {
        // Google Cloud Text-to-Speech pricing: $16.00 per 1 million characters
        (character_count as f64 / 1_000_000.0) * 16.0
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            audio_encoding: AudioEncoding::Mp3,
            speaking_rate: None,
            pitch: None,
            volume_gain_db: None,
            sample_rate_hertz: None,
            effects_profile_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_languages() {
        let handler = TextToSpeechHandler::new("test".to_string());
        let languages = handler.get_supported_languages();
        assert!(languages.contains(&"en-US"));
        assert!(languages.contains(&"fr-FR"));
        assert!(languages.contains(&"ja-JP"));
    }

    #[test]
    fn test_validate_request() {
        let handler = TextToSpeechHandler::new("test".to_string());

        let valid_request = TextToSpeechRequest {
            input: TextInput {
                text: Some("Hello, world!".to_string()),
                ssml: None,
            },
            voice: VoiceSelectionParams {
                language_code: "en-US".to_string(),
                name: Some("en-US-Journey-D".to_string()),
                ssml_gender: Some(SsmlVoiceGender::Male),
            },
            audio_config: AudioConfig::default(),
        };

        assert!(handler.validate_request(&valid_request).is_ok());

        let invalid_request = TextToSpeechRequest {
            input: TextInput {
                text: None,
                ssml: None,
            },
            voice: VoiceSelectionParams {
                language_code: "en-US".to_string(),
                name: None,
                ssml_gender: None,
            },
            audio_config: AudioConfig::default(),
        };

        assert!(handler.validate_request(&invalid_request).is_err());
    }

    #[test]
    fn test_calculate_cost() {
        let handler = TextToSpeechHandler::new("test".to_string());
        let cost = handler.calculate_cost(1_000_000);
        assert_eq!(cost, 16.0);

        let small_cost = handler.calculate_cost(1000);
        assert_eq!(small_cost, 0.016);
    }
}
