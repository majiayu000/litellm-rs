//! Speech-to-Text (STT) Module for Deepgram
//!
//! Provides audio transcription capabilities using Deepgram's advanced speech recognition API.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::providers::unified_provider::ProviderError;

/// STT API endpoint path
pub const STT_ENDPOINT_PATH: &str = "/listen";

/// Transcription request
#[derive(Debug, Clone)]
pub struct TranscriptionRequest {
    /// Audio file bytes
    pub file: Vec<u8>,

    /// Model to use for transcription
    /// Options: "nova-2", "nova-2-general", "nova-2-meeting", "nova-2-phonecall",
    /// "nova-2-finance", "nova-2-conversationalai", "nova-2-voicemail",
    /// "nova-2-video", "nova-2-medical", "nova-2-drivethru", "nova-2-automotive",
    /// "enhanced", "base"
    pub model: String,

    /// Language of the audio (BCP-47 format)
    /// Example: "en", "en-US", "es", "fr", "de", "zh"
    pub language: Option<String>,

    /// Enable smart formatting (punctuation, casing, etc.)
    pub smart_format: Option<bool>,

    /// Enable punctuation
    pub punctuate: Option<bool>,

    /// Enable speaker diarization
    pub diarize: Option<bool>,

    /// Enable paragraphs in output
    pub paragraphs: Option<bool>,

    /// Enable utterances (sentences/phrases)
    pub utterances: Option<bool>,

    /// Enable word-level timestamps
    pub words: Option<bool>,

    /// Enable search terms highlighting
    pub search: Option<Vec<String>>,

    /// Keywords to boost recognition
    pub keywords: Option<Vec<String>>,

    /// Filler words handling
    pub filler_words: Option<bool>,

    /// Detect language automatically
    pub detect_language: Option<bool>,

    /// Original filename (for content type detection)
    pub filename: Option<String>,
}

impl Default for TranscriptionRequest {
    fn default() -> Self {
        Self {
            file: Vec::new(),
            model: "nova-2".to_string(),
            language: None,
            smart_format: None,
            punctuate: None,
            diarize: None,
            paragraphs: None,
            utterances: None,
            words: None,
            search: None,
            keywords: None,
            filler_words: None,
            detect_language: None,
            filename: None,
        }
    }
}

/// Deepgram transcription response
#[derive(Debug, Clone, Deserialize)]
pub struct DeepgramResponse {
    /// Metadata about the request
    pub metadata: ResponseMetadata,

    /// Transcription results
    pub results: TranscriptionResults,
}

/// Response metadata
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseMetadata {
    /// Transaction key
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_key: Option<String>,

    /// Request ID
    pub request_id: String,

    /// SHA256 hash of audio
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,

    /// Created timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,

    /// Duration of audio in seconds
    pub duration: f32,

    /// Number of audio channels
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,

    /// Models used for transcription
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,

    /// Model info (alternative format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_info: Option<HashMap<String, serde_json::Value>>,
}

/// Transcription results
#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionResults {
    /// Results per channel
    pub channels: Vec<ChannelResult>,

    /// Utterances (if enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utterances: Option<Vec<Utterance>>,
}

/// Result for a single audio channel
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelResult {
    /// Alternative transcriptions
    pub alternatives: Vec<TranscriptionAlternative>,

    /// Detected language
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_language: Option<String>,

    /// Language confidence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_confidence: Option<f32>,
}

/// A single transcription alternative
#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionAlternative {
    /// Full transcript text
    pub transcript: String,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Word-level information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<WordInfo>>,

    /// Paragraph information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraphs: Option<Paragraphs>,
}

/// Word information with timestamps
#[derive(Debug, Clone, Deserialize)]
pub struct WordInfo {
    /// The word text
    pub word: String,

    /// Start time in seconds
    pub start: f32,

    /// End time in seconds
    pub end: f32,

    /// Confidence score
    pub confidence: f32,

    /// Speaker ID (if diarization enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<u32>,

    /// Punctuated word (with punctuation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub punctuated_word: Option<String>,
}

/// Paragraph information
#[derive(Debug, Clone, Deserialize)]
pub struct Paragraphs {
    /// Full transcript with paragraphs
    pub transcript: String,

    /// Individual paragraphs
    pub paragraphs: Vec<Paragraph>,
}

/// A single paragraph
#[derive(Debug, Clone, Deserialize)]
pub struct Paragraph {
    /// Sentences in this paragraph
    pub sentences: Vec<Sentence>,

    /// Speaker ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<u32>,

    /// Number of words
    pub num_words: u32,

    /// Start time
    pub start: f32,

    /// End time
    pub end: f32,
}

/// A sentence within a paragraph
#[derive(Debug, Clone, Deserialize)]
pub struct Sentence {
    /// Sentence text
    pub text: String,

    /// Start time
    pub start: f32,

    /// End time
    pub end: f32,
}

/// An utterance (natural speech segment)
#[derive(Debug, Clone, Deserialize)]
pub struct Utterance {
    /// Start time
    pub start: f32,

    /// End time
    pub end: f32,

    /// Confidence score
    pub confidence: f32,

    /// Utterance text
    pub transcript: String,

    /// Speaker ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<u32>,

    /// Channel index
    pub channel: u32,

    /// Utterance ID
    pub id: String,
}

/// OpenAI-compatible transcription response
#[derive(Debug, Clone, Serialize)]
pub struct OpenAITranscriptionResponse {
    /// Transcribed text
    pub text: String,

    /// Task type (always "transcribe")
    pub task: String,

    /// Language
    pub language: String,

    /// Duration in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f32>,

    /// Word timestamps (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<OpenAIWordInfo>>,
}

/// OpenAI-compatible word info
#[derive(Debug, Clone, Serialize)]
pub struct OpenAIWordInfo {
    /// The word
    pub word: String,

    /// Start time in seconds
    pub start: f32,

    /// End time in seconds
    pub end: f32,
}

impl TryFrom<DeepgramResponse> for OpenAITranscriptionResponse {
    type Error = ProviderError;

    fn try_from(response: DeepgramResponse) -> Result<Self, Self::Error> {
        let first_channel = response.results.channels.first().ok_or_else(|| {
            ProviderError::response_parsing("deepgram", "Response contains no channels")
        })?;
        let first_alternative = first_channel.alternatives.first().ok_or_else(|| {
            ProviderError::response_parsing("deepgram", "Channel contains no alternatives")
        })?;

        // Determine if diarization is active
        let has_diarization = first_alternative
            .words
            .as_ref()
            .map(|words| words.first().map(|w| w.speaker.is_some()).unwrap_or(false))
            .unwrap_or(false);

        // Extract text based on diarization mode
        let text = if !has_diarization {
            first_alternative.transcript.clone()
        } else if let Some(ref paragraphs) = first_alternative.paragraphs {
            paragraphs.transcript.clone()
        } else if let Some(ref words) = first_alternative.words {
            reconstruct_diarized_transcript(words)
        } else {
            first_alternative.transcript.clone()
        };

        // Convert words to OpenAI format
        let words = first_alternative.words.as_ref().map(|words| {
            words
                .iter()
                .map(|w| OpenAIWordInfo {
                    word: w.word.clone(),
                    start: w.start,
                    end: w.end,
                })
                .collect()
        });

        let language = first_channel
            .detected_language
            .clone()
            .unwrap_or_else(|| "en".to_string());

        Ok(OpenAITranscriptionResponse {
            text,
            task: "transcribe".to_string(),
            language,
            duration: Some(response.metadata.duration),
            words,
        })
    }
}

/// Reconstruct diarized transcript from words with speaker information
fn reconstruct_diarized_transcript(words: &[WordInfo]) -> String {
    if words.is_empty() {
        return String::new();
    }

    let mut segments = Vec::new();
    let mut current_speaker: Option<u32> = None;
    let mut current_words: Vec<String> = Vec::new();

    for word_obj in words {
        let speaker = word_obj.speaker;
        let word_text = word_obj
            .punctuated_word
            .clone()
            .unwrap_or_else(|| word_obj.word.clone());

        if speaker != current_speaker {
            // New speaker: save previous segment and start new one
            if !current_words.is_empty()
                && let Some(sp) = current_speaker
            {
                segments.push(format!("Speaker {}: {}", sp, current_words.join(" ")));
            }
            current_speaker = speaker;
            current_words = vec![word_text];
        } else {
            current_words.push(word_text);
        }
    }

    // Add the last segment
    if !current_words.is_empty()
        && let Some(sp) = current_speaker
    {
        segments.push(format!("\nSpeaker {}: {}\n", sp, current_words.join(" ")));
    }

    segments.join("\n")
}

/// Build query parameters for the request URL
pub fn build_query_params(request: &TranscriptionRequest) -> String {
    let mut params = vec![format!("model={}", request.model)];

    if let Some(ref lang) = request.language {
        params.push(format!("language={}", lang));
    }

    if let Some(smart_format) = request.smart_format {
        params.push(format!("smart_format={}", smart_format));
    }

    if let Some(punctuate) = request.punctuate {
        params.push(format!("punctuate={}", punctuate));
    }

    if let Some(diarize) = request.diarize {
        params.push(format!("diarize={}", diarize));
    }

    if let Some(paragraphs) = request.paragraphs {
        params.push(format!("paragraphs={}", paragraphs));
    }

    if let Some(utterances) = request.utterances {
        params.push(format!("utterances={}", utterances));
    }

    if let Some(words) = request.words {
        params.push(format!("words={}", words));
    }

    if let Some(filler_words) = request.filler_words {
        params.push(format!("filler_words={}", filler_words));
    }

    if let Some(detect_language) = request.detect_language {
        params.push(format!("detect_language={}", detect_language));
    }

    if let Some(ref keywords) = request.keywords {
        for keyword in keywords {
            params.push(format!("keywords={}", keyword));
        }
    }

    if let Some(ref search) = request.search {
        for term in search {
            params.push(format!("search={}", term));
        }
    }

    params.join("&")
}

/// Build the complete STT URL with query parameters
pub fn build_stt_url(base_url: &str, request: &TranscriptionRequest) -> String {
    let base = base_url.trim_end_matches('/');
    let query = build_query_params(request);
    format!(
        "{}{}{}",
        base,
        STT_ENDPOINT_PATH,
        if query.is_empty() {
            String::new()
        } else {
            format!("?{}", query)
        }
    )
}

/// Detect MIME type from filename extension
pub fn detect_audio_mime_type(filename: &str) -> &'static str {
    let extension = filename
        .rsplit('.')
        .next()
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    match extension.as_str() {
        "mp3" => "audio/mpeg",
        "mp4" | "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "webm" => "audio/webm",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        _ => "audio/mpeg",
    }
}

/// Supported audio input formats
pub fn supported_audio_formats() -> &'static [&'static str] {
    &[
        "mp3", "mp4", "mp2", "aac", "wav", "flac", "pcm", "m4a", "ogg", "opus", "webm",
    ]
}

/// Available STT models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum STTModel {
    /// Nova 2 - General purpose (default)
    #[default]
    Nova2,
    /// Nova 2 - General variant
    Nova2General,
    /// Nova 2 - Meeting transcription
    Nova2Meeting,
    /// Nova 2 - Phone call transcription
    Nova2PhoneCall,
    /// Nova 2 - Finance industry
    Nova2Finance,
    /// Nova 2 - Conversational AI
    Nova2ConversationalAI,
    /// Nova 2 - Voicemail
    Nova2Voicemail,
    /// Nova 2 - Video content
    Nova2Video,
    /// Nova 2 - Medical terminology
    Nova2Medical,
    /// Nova 2 - Drive-thru
    Nova2DriveThru,
    /// Nova 2 - Automotive
    Nova2Automotive,
    /// Enhanced - Older model with good accuracy
    Enhanced,
    /// Base - Basic transcription
    Base,
}

impl STTModel {
    /// Get the model ID string
    pub fn as_str(&self) -> &'static str {
        match self {
            STTModel::Nova2 => "nova-2",
            STTModel::Nova2General => "nova-2-general",
            STTModel::Nova2Meeting => "nova-2-meeting",
            STTModel::Nova2PhoneCall => "nova-2-phonecall",
            STTModel::Nova2Finance => "nova-2-finance",
            STTModel::Nova2ConversationalAI => "nova-2-conversationalai",
            STTModel::Nova2Voicemail => "nova-2-voicemail",
            STTModel::Nova2Video => "nova-2-video",
            STTModel::Nova2Medical => "nova-2-medical",
            STTModel::Nova2DriveThru => "nova-2-drivethru",
            STTModel::Nova2Automotive => "nova-2-automotive",
            STTModel::Enhanced => "enhanced",
            STTModel::Base => "base",
        }
    }

    /// Parse from string
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "nova-2" => Some(STTModel::Nova2),
            "nova-2-general" => Some(STTModel::Nova2General),
            "nova-2-meeting" => Some(STTModel::Nova2Meeting),
            "nova-2-phonecall" => Some(STTModel::Nova2PhoneCall),
            "nova-2-finance" => Some(STTModel::Nova2Finance),
            "nova-2-conversationalai" => Some(STTModel::Nova2ConversationalAI),
            "nova-2-voicemail" => Some(STTModel::Nova2Voicemail),
            "nova-2-video" => Some(STTModel::Nova2Video),
            "nova-2-medical" => Some(STTModel::Nova2Medical),
            "nova-2-drivethru" => Some(STTModel::Nova2DriveThru),
            "nova-2-automotive" => Some(STTModel::Nova2Automotive),
            "enhanced" => Some(STTModel::Enhanced),
            "base" => Some(STTModel::Base),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stt_model_as_str() {
        assert_eq!(STTModel::Nova2.as_str(), "nova-2");
        assert_eq!(STTModel::Nova2Meeting.as_str(), "nova-2-meeting");
        assert_eq!(STTModel::Enhanced.as_str(), "enhanced");
    }

    #[test]
    fn test_stt_model_from_str() {
        assert_eq!(STTModel::parse("nova-2"), Some(STTModel::Nova2));
        assert_eq!(
            STTModel::parse("nova-2-meeting"),
            Some(STTModel::Nova2Meeting)
        );
        assert_eq!(STTModel::parse("invalid"), None);
    }

    #[test]
    fn test_build_query_params() {
        let request = TranscriptionRequest {
            model: "nova-2".to_string(),
            language: Some("en".to_string()),
            punctuate: Some(true),
            diarize: Some(true),
            ..Default::default()
        };

        let params = build_query_params(&request);
        assert!(params.contains("model=nova-2"));
        assert!(params.contains("language=en"));
        assert!(params.contains("punctuate=true"));
        assert!(params.contains("diarize=true"));
    }

    #[test]
    fn test_build_stt_url() {
        let request = TranscriptionRequest {
            model: "nova-2".to_string(),
            ..Default::default()
        };

        let url = build_stt_url("https://api.deepgram.com/v1", &request);
        assert!(url.starts_with("https://api.deepgram.com/v1/listen?"));
        assert!(url.contains("model=nova-2"));
    }

    #[test]
    fn test_detect_audio_mime_type() {
        assert_eq!(detect_audio_mime_type("audio.mp3"), "audio/mpeg");
        assert_eq!(detect_audio_mime_type("audio.wav"), "audio/wav");
        assert_eq!(detect_audio_mime_type("audio.m4a"), "audio/mp4");
        assert_eq!(detect_audio_mime_type("audio.webm"), "audio/webm");
    }

    #[test]
    fn test_reconstruct_diarized_transcript() {
        let words = vec![
            WordInfo {
                word: "Hello".to_string(),
                start: 0.0,
                end: 0.5,
                confidence: 0.95,
                speaker: Some(0),
                punctuated_word: Some("Hello,".to_string()),
            },
            WordInfo {
                word: "world".to_string(),
                start: 0.5,
                end: 1.0,
                confidence: 0.90,
                speaker: Some(0),
                punctuated_word: Some("world.".to_string()),
            },
            WordInfo {
                word: "Hi".to_string(),
                start: 1.5,
                end: 2.0,
                confidence: 0.92,
                speaker: Some(1),
                punctuated_word: Some("Hi!".to_string()),
            },
        ];

        let transcript = reconstruct_diarized_transcript(&words);
        assert!(transcript.contains("Speaker 0"));
        assert!(transcript.contains("Speaker 1"));
        assert!(transcript.contains("Hello,"));
        assert!(transcript.contains("Hi!"));
    }

    #[test]
    fn test_supported_audio_formats() {
        let formats = supported_audio_formats();
        assert!(formats.contains(&"mp3"));
        assert!(formats.contains(&"wav"));
        assert!(formats.contains(&"flac"));
        assert!(formats.contains(&"webm"));
    }

    #[test]
    fn test_transcription_request_default() {
        let request = TranscriptionRequest::default();
        assert_eq!(request.model, "nova-2");
        assert!(request.language.is_none());
        assert!(request.diarize.is_none());
    }
}
