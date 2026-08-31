//! Image generation request types

use serde::{Deserialize, Serialize};

/// Image request (short form)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRequest {
    /// Model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Image description prompt
    pub prompt: String,
    /// Number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Image quality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    /// Image size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Image style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Image generation request (full form)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    /// Image description prompt
    pub prompt: String,
    /// Model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Image size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    /// Image quality
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    /// Style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Unified image editing request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageEditRequest {
    pub image: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<Vec<u8>>,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Audio transcription request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTranscriptionRequest {
    /// Audio file data
    pub file: Vec<u8>,
    /// Model name
    pub model: String,
    /// Language (ISO-639-1 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// Response format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,
    /// Temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Completion request (legacy text completion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model name
    pub model: String,
    /// Input text prompt
    pub prompt: String,
    /// Sampling temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum number of tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Nucleus sampling parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Frequency penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Presence penalty
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Number of choices to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    /// Enable streaming
    #[serde(default)]
    pub stream: bool,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ImageRequest Tests ====================

    #[test]
    fn test_image_request_minimal() {
        let request = ImageRequest {
            model: None,
            prompt: "A beautiful sunset".to_string(),
            n: None,
            quality: None,
            response_format: None,
            size: None,
            style: None,
            user: None,
        };
        assert_eq!(request.prompt, "A beautiful sunset");
        assert!(request.model.is_none());
    }

    #[test]
    fn test_image_request_full() {
        let request = ImageRequest {
            model: Some("dall-e-3".to_string()),
            prompt: "A cat wearing a hat".to_string(),
            n: Some(2),
            quality: Some("hd".to_string()),
            response_format: Some("url".to_string()),
            size: Some("1024x1024".to_string()),
            style: Some("vivid".to_string()),
            user: Some("user123".to_string()),
        };
        assert_eq!(request.model, Some("dall-e-3".to_string()));
        assert_eq!(request.n, Some(2));
        assert_eq!(request.quality, Some("hd".to_string()));
    }

    #[test]
    fn test_image_request_serialization() {
        let request = ImageRequest {
            model: Some("dall-e-2".to_string()),
            prompt: "A dog".to_string(),
            n: Some(1),
            quality: None,
            response_format: None,
            size: Some("512x512".to_string()),
            style: None,
            user: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["prompt"], "A dog");
        assert_eq!(json["model"], "dall-e-2");
        assert!(!json.as_object().unwrap().contains_key("quality"));
    }

    #[test]
    fn test_image_request_deserialization() {
        let json = r#"{"prompt": "A bird", "size": "256x256"}"#;
        let request: ImageRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.prompt, "A bird");
        assert_eq!(request.size, Some("256x256".to_string()));
    }

    #[test]
    fn test_image_request_clone() {
        let request = ImageRequest {
            model: Some("dall-e-3".to_string()),
            prompt: "Test".to_string(),
            n: Some(1),
            quality: None,
            response_format: None,
            size: None,
            style: None,
            user: None,
        };
        let cloned = request.clone();
        assert_eq!(request.prompt, cloned.prompt);
        assert_eq!(request.model, cloned.model);
    }

    // ==================== ImageGenerationRequest Tests ====================

    #[test]
    fn test_image_generation_request_minimal() {
        let request = ImageGenerationRequest {
            prompt: "Generate an image".to_string(),
            model: None,
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        assert_eq!(request.prompt, "Generate an image");
    }

    #[test]
    fn test_image_generation_request_full() {
        let request = ImageGenerationRequest {
            prompt: "A mountain landscape".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(4),
            size: Some("1792x1024".to_string()),
            quality: Some("standard".to_string()),
            response_format: Some("b64_json".to_string()),
            style: Some("natural".to_string()),
            user: Some("user456".to_string()),
        };
        assert_eq!(request.n, Some(4));
        assert_eq!(request.style, Some("natural".to_string()));
    }

    #[test]
    fn test_image_generation_request_serialization() {
        let request = ImageGenerationRequest {
            prompt: "Test prompt".to_string(),
            model: Some("model".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["prompt"], "Test prompt");
        assert!(!json.as_object().unwrap().contains_key("n"));
    }

    // ==================== AudioTranscriptionRequest Tests ====================

    #[test]
    fn test_audio_transcription_request_minimal() {
        let request = AudioTranscriptionRequest {
            file: vec![1, 2, 3, 4],
            model: "whisper-1".to_string(),
            language: None,
            prompt: None,
            response_format: None,
            temperature: None,
        };
        assert_eq!(request.model, "whisper-1");
        assert_eq!(request.file.len(), 4);
    }

    #[test]
    fn test_audio_transcription_request_full() {
        let request = AudioTranscriptionRequest {
            file: vec![0; 1000],
            model: "whisper-1".to_string(),
            language: Some("en".to_string()),
            prompt: Some("A conversation about...".to_string()),
            response_format: Some("json".to_string()),
            temperature: Some(0.2),
        };
        assert_eq!(request.language, Some("en".to_string()));
        assert!((request.temperature.unwrap() - 0.2).abs() < f32::EPSILON);
    }

    #[test]
    fn test_audio_transcription_request_clone() {
        let request = AudioTranscriptionRequest {
            file: vec![5, 6, 7],
            model: "whisper-1".to_string(),
            language: Some("fr".to_string()),
            prompt: None,
            response_format: None,
            temperature: None,
        };
        let cloned = request.clone();
        assert_eq!(request.file, cloned.file);
        assert_eq!(request.language, cloned.language);
    }

    // ==================== CompletionRequest Tests ====================

    #[test]
    fn test_completion_request_minimal() {
        let request = CompletionRequest {
            model: "gpt-3.5-turbo-instruct".to_string(),
            prompt: "Complete this: ".to_string(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            n: None,
            stream: false,
            user: None,
        };
        assert_eq!(request.model, "gpt-3.5-turbo-instruct");
        assert!(!request.stream);
    }

    #[test]
    fn test_completion_request_full() {
        let request = CompletionRequest {
            model: "gpt-3.5-turbo-instruct".to_string(),
            prompt: "Write a poem".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: Some(0.9),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.5),
            stop: Some(vec!["END".to_string()]),
            n: Some(3),
            stream: true,
            user: Some("user789".to_string()),
        };
        assert_eq!(request.max_tokens, Some(100));
        assert!(request.stream);
        assert_eq!(request.stop.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_completion_request_serialization() {
        let request = CompletionRequest {
            model: "model".to_string(),
            prompt: "test".to_string(),
            temperature: Some(0.5),
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            n: None,
            stream: false,
            user: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "model");
        assert_eq!(json["prompt"], "test");
        assert!(!json.as_object().unwrap().contains_key("max_tokens"));
    }

    #[test]
    fn test_completion_request_deserialization() {
        let json = r#"{"model": "gpt-3.5-turbo-instruct", "prompt": "Hello"}"#;
        let request: CompletionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "gpt-3.5-turbo-instruct");
        assert_eq!(request.prompt, "Hello");
        assert!(!request.stream);
    }

    #[test]
    fn test_completion_request_clone() {
        let request = CompletionRequest {
            model: "model".to_string(),
            prompt: "prompt".to_string(),
            temperature: Some(1.0),
            max_tokens: Some(50),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: Some(vec!["stop".to_string()]),
            n: None,
            stream: true,
            user: None,
        };
        let cloned = request.clone();
        assert_eq!(request.model, cloned.model);
        assert_eq!(request.stop, cloned.stop);
        assert_eq!(request.stream, cloned.stream);
    }

    #[test]
    fn test_completion_request_stream_default() {
        let json = r#"{"model": "test", "prompt": "test"}"#;
        let request: CompletionRequest = serde_json::from_str(json).unwrap();
        assert!(!request.stream);
    }
}
