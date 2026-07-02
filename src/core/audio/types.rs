//! Audio API type definitions
//!
//! Provides unified audio types for speech-to-text and text-to-speech operations.

use serde::{Deserialize, Serialize};

/// Audio transcription request (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    /// Audio file bytes
    #[serde(skip)]
    pub file: Vec<u8>,

    /// Original filename
    #[serde(skip)]
    pub filename: String,

    /// Model to use (e.g., "whisper-1", "whisper-large-v3")
    pub model: String,

    /// Language of the audio (ISO-639-1 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Optional text to guide the model's style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// Response format: "json", "text", "srt", "verbose_json", "vtt"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,

    /// Temperature for sampling (0.0 to 1.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Timestamp granularities: "segment", "word"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp_granularities: Option<Vec<String>>,
}

/// Audio transcription response (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    /// Transcribed text
    pub text: String,

    /// Task type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// Detected or specified language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Duration of the audio in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,

    /// Word-level timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<WordInfo>>,

    /// Segment-level timestamps
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<SegmentInfo>>,
}

/// Word-level timestamp information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordInfo {
    /// The word
    pub word: String,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
}

/// Segment-level timestamp information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentInfo {
    /// Segment ID
    pub id: u32,
    /// Start time in seconds
    pub start: f64,
    /// End time in seconds
    pub end: f64,
    /// Transcribed text for this segment
    pub text: String,
}

/// Audio translation request (translate to English)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationRequest {
    /// Audio file bytes
    #[serde(skip)]
    pub file: Vec<u8>,

    /// Original filename
    #[serde(skip)]
    pub filename: String,

    /// Model to use
    pub model: String,

    /// Optional text to guide the model's style
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// Response format: "json", "text", "srt", "verbose_json", "vtt"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,

    /// Temperature for sampling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// Audio translation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResponse {
    /// Translated text (always in English)
    pub text: String,

    /// Task type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,

    /// Source language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Duration of the audio in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,

    /// Segments
    #[serde(skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<SegmentInfo>>,
}

/// Text-to-speech request (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeechRequest {
    /// Text to convert to speech
    pub input: String,

    /// Model to use (e.g., "tts-1", "tts-1-hd")
    pub model: String,

    /// Voice to use (e.g., "alloy", "echo", "fable", "onyx", "nova", "shimmer")
    pub voice: String,

    /// Audio format: "mp3", "opus", "aac", "flac", "wav", "pcm"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<String>,

    /// Speed of speech (0.25 to 4.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
}

/// Text-to-speech response
pub struct SpeechResponse {
    /// Audio data bytes
    pub audio: Vec<u8>,

    /// Content type (e.g., "audio/mpeg", "audio/opus")
    pub content_type: String,
}

/// Supported audio formats
pub fn supported_audio_formats() -> &'static [&'static str] {
    &[
        "flac", "m4a", "mp3", "mp4", "mpeg", "mpga", "oga", "ogg", "wav", "webm",
    ]
}

/// Get content type from format
pub fn format_to_content_type(format: &str) -> &'static str {
    match format.to_lowercase().as_str() {
        "mp3" => "audio/mpeg",
        "opus" => "audio/opus",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "pcm" => "audio/pcm",
        _ => "audio/mpeg",
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
