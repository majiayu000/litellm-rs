//! OpenAI Real-time API Module
//!
//! Real-time conversational AI functionality following the unified architecture

use serde::{Deserialize, Serialize};

use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Real-time Session Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeSessionConfig {
    /// The model to use for conversation
    pub model: String,

    /// Modalities to enable (text, audio, both)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<Modality>>,

    /// Instructions for the conversation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Voice configuration for audio output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<Voice>,

    /// Input audio format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_format: Option<AudioFormat>,

    /// Output audio format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_audio_format: Option<AudioFormat>,

    /// Input audio transcription configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_audio_transcription: Option<TranscriptionConfig>,

    /// Turn detection configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub turn_detection: Option<TurnDetectionConfig>,

    /// Tools available to the assistant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<RealtimeTool>>,

    /// Tool choice configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Temperature for response generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Maximum response output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_response_output_tokens: Option<u32>,
}

/// Supported modalities
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Audio,
}

/// Supported voices
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Voice {
    Alloy,
    Ash,
    Ballad,
    Coral,
    Echo,
    Nova,
    Onyx,
    Fable,
    Sage,
    Shimmer,
    Verse,
}

/// Audio format options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AudioFormat {
    #[default]
    Pcm16,
    G711Ulaw,
    G711Alaw,
}

/// Transcription configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// The model to use for transcription
    pub model: String,
}

/// Turn detection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnDetectionConfig {
    /// Type of turn detection
    #[serde(rename = "type")]
    pub detection_type: TurnDetectionType,

    /// Threshold for voice activity detection
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,

    /// Prefix padding in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix_padding_ms: Option<u32>,

    /// Silence duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silence_duration_ms: Option<u32>,
}

/// Turn detection types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum TurnDetectionType {
    #[default]
    ServerVad,
    None,
}

/// Real-time tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeTool {
    /// Tool type
    #[serde(rename = "type")]
    pub tool_type: String,

    /// Function definition
    pub function: RealtimeFunction,
}

/// Real-time function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeFunction {
    /// Function name
    pub name: String,

    /// Function description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Function parameters schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool choice options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    /// Auto tool selection
    Auto,
    /// No tools
    None,
    /// Required tool usage
    Required,
    /// Specific function
    Function { function: FunctionChoice },
}

/// Function choice specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionChoice {
    pub name: String,
}

/// Real-time event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RealtimeEvent {
    /// Session configuration update
    #[serde(rename = "session.update")]
    SessionUpdate { session: RealtimeSessionConfig },

    /// Input audio buffer append
    #[serde(rename = "input_audio_buffer.append")]
    InputAudioBufferAppend { audio: String },

    /// Input audio buffer commit
    #[serde(rename = "input_audio_buffer.commit")]
    InputAudioBufferCommit,

    /// Input audio buffer clear
    #[serde(rename = "input_audio_buffer.clear")]
    InputAudioBufferClear,

    /// Conversation item create
    #[serde(rename = "conversation.item.create")]
    ConversationItemCreate {
        previous_item_id: Option<String>,
        item: ConversationItem,
    },

    /// Conversation item truncate
    #[serde(rename = "conversation.item.truncate")]
    ConversationItemTruncate {
        item_id: String,
        content_index: u32,
        audio_end_ms: u32,
    },

    /// Conversation item delete
    #[serde(rename = "conversation.item.delete")]
    ConversationItemDelete { item_id: String },

    /// Response create
    #[serde(rename = "response.create")]
    ResponseCreate {
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<RealtimeResponse>,
    },

    /// Response cancel
    #[serde(rename = "response.cancel")]
    ResponseCancel,
}

/// Conversation item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationItem {
    /// Item ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Item type
    #[serde(rename = "type")]
    pub item_type: ItemType,

    /// Item status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<ItemStatus>,

    /// Item role (for message items)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<ItemRole>,

    /// Item content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Vec<ContentPart>>,

    /// Call ID (for function calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub call_id: Option<String>,

    /// Function name (for function calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Function arguments (for function calls)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,

    /// Function output (for function call outputs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Item types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemType {
    Message,
    FunctionCall,
    FunctionCallOutput,
}

/// Item status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Completed,
    InProgress,
    Incomplete,
}

/// Item roles
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemRole {
    User,
    Assistant,
    System,
}

/// Content part types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "input_text")]
    InputText { text: String },

    #[serde(rename = "input_audio")]
    InputAudio {
        audio: Option<String>,
        transcript: Option<String>,
    },

    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "audio")]
    Audio {
        audio: Option<String>,
        transcript: Option<String>,
    },
}

/// Real-time response configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeResponse {
    /// Modalities for the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modalities: Option<Vec<Modality>>,

    /// Instructions for the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,

    /// Voice for audio response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<Voice>,

    /// Output audio format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_audio_format: Option<AudioFormat>,

    /// Tools available for the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<RealtimeTool>>,

    /// Tool choice
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// Temperature
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,

    /// Maximum output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
}

/// Real-time utilities
pub struct OpenAIRealtimeUtils;

impl OpenAIRealtimeUtils {
    /// Get supported real-time models
    pub fn get_supported_models() -> Vec<&'static str> {
        vec![
            "gpt-4o-realtime-preview",
            "gpt-4o-realtime-preview-2024-10-01",
        ]
    }

    /// Check if model supports real-time API
    pub fn supports_realtime(model_id: &str) -> bool {
        Self::get_supported_models().contains(&model_id)
    }

    /// Create default session configuration
    pub fn create_session_config(
        model: String,
        voice: Option<Voice>,
        instructions: Option<String>,
    ) -> RealtimeSessionConfig {
        RealtimeSessionConfig {
            model,
            modalities: Some(vec![Modality::Text, Modality::Audio]),
            instructions,
            voice: voice.or(Some(Voice::Alloy)),
            input_audio_format: Some(AudioFormat::Pcm16),
            output_audio_format: Some(AudioFormat::Pcm16),
            input_audio_transcription: Some(TranscriptionConfig {
                model: "whisper-1".to_string(),
            }),
            turn_detection: Some(TurnDetectionConfig {
                detection_type: TurnDetectionType::ServerVad,
                threshold: Some(0.5),
                prefix_padding_ms: Some(300),
                silence_duration_ms: Some(500),
            }),
            tools: None,
            tool_choice: None,
            temperature: Some(0.8),
            max_response_output_tokens: Some(4096),
        }
    }

    /// Create text input event
    pub fn create_text_input(text: String, previous_item_id: Option<String>) -> RealtimeEvent {
        RealtimeEvent::ConversationItemCreate {
            previous_item_id,
            item: ConversationItem {
                id: None,
                item_type: ItemType::Message,
                status: Some(ItemStatus::Completed),
                role: Some(ItemRole::User),
                content: Some(vec![ContentPart::InputText { text }]),
                call_id: None,
                name: None,
                arguments: None,
                output: None,
            },
        }
    }

    /// Create audio input event
    pub fn create_audio_input(audio_data: String) -> RealtimeEvent {
        RealtimeEvent::InputAudioBufferAppend { audio: audio_data }
    }

    /// Create function call output event
    pub fn create_function_output(
        call_id: String,
        output: String,
        previous_item_id: Option<String>,
    ) -> RealtimeEvent {
        RealtimeEvent::ConversationItemCreate {
            previous_item_id,
            item: ConversationItem {
                id: None,
                item_type: ItemType::FunctionCallOutput,
                status: Some(ItemStatus::Completed),
                role: None,
                content: None,
                call_id: Some(call_id),
                name: None,
                arguments: None,
                output: Some(output),
            },
        }
    }

    /// Validate session configuration
    pub fn validate_session_config(config: &RealtimeSessionConfig) -> Result<(), ProviderError> {
        // Check model support
        if !Self::supports_realtime(&config.model) {
            return Err(ProviderError::ModelNotFound {
                provider: "openai",
                model: config.model.clone(),
            });
        }

        // Check temperature range
        if let Some(temperature) = config.temperature
            && !(0.0..=2.0).contains(&temperature)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "temperature must be between 0.0 and 2.0".to_string(),
            });
        }

        // Check max tokens
        if let Some(max_tokens) = config.max_response_output_tokens
            && (max_tokens == 0 || max_tokens > 4096)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "max_response_output_tokens must be between 1 and 4096".to_string(),
            });
        }

        // Validate turn detection config
        if let Some(turn_detection) = &config.turn_detection
            && let Some(threshold) = turn_detection.threshold
            && !(0.0..=1.0).contains(&threshold)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "turn detection threshold must be between 0.0 and 1.0".to_string(),
            });
        }

        Ok(())
    }

    /// Get recommended voice for use case
    pub fn get_recommended_voice(use_case: &str) -> Voice {
        match use_case.to_lowercase().as_str() {
            "customer_service" => Voice::Alloy,
            "assistant" => Voice::Nova,
            "narrator" => Voice::Onyx,
            "casual" => Voice::Echo,
            "professional" => Voice::Fable,
            _ => Voice::Alloy,
        }
    }

    /// Get audio format sample rate
    pub fn get_sample_rate(format: &AudioFormat) -> u32 {
        match format {
            AudioFormat::Pcm16 => 24000,
            AudioFormat::G711Ulaw => 8000,
            AudioFormat::G711Alaw => 8000,
        }
    }

    /// Estimate real-time usage cost per minute
    pub fn estimate_cost_per_minute(
        model: &str,
        include_audio: bool,
    ) -> Result<f64, ProviderError> {
        let base_cost = match model {
            "gpt-4o-realtime-preview" | "gpt-4o-realtime-preview-2024-10-01" => {
                if include_audio {
                    0.06 // $0.06 per minute for audio + text
                } else {
                    0.03 // $0.03 per minute for text only
                }
            }
            _ => {
                return Err(ProviderError::InvalidRequest {
                    provider: "openai",
                    message: format!("Unknown real-time model: {}", model),
                });
            }
        };

        Ok(base_cost)
    }
}

/// Default implementations
impl Default for Voice {
    fn default() -> Self {
        Voice::Alloy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_realtime() {
        assert!(OpenAIRealtimeUtils::supports_realtime(
            "gpt-4o-realtime-preview"
        ));
        assert!(OpenAIRealtimeUtils::supports_realtime(
            "gpt-4o-realtime-preview-2024-10-01"
        ));
        assert!(!OpenAIRealtimeUtils::supports_realtime("gpt-4"));
        assert!(!OpenAIRealtimeUtils::supports_realtime("gpt-3.5-turbo"));
    }

    #[test]
    fn test_create_session_config() {
        let config = OpenAIRealtimeUtils::create_session_config(
            "gpt-4o-realtime-preview".to_string(),
            Some(Voice::Echo),
            Some("You are a helpful assistant.".to_string()),
        );

        assert_eq!(config.model, "gpt-4o-realtime-preview");
        assert_eq!(config.voice, Some(Voice::Echo));
        assert_eq!(
            config.instructions,
            Some("You are a helpful assistant.".to_string())
        );
        assert!(matches!(config.modalities, Some(ref modalities) if modalities.len() == 2));
    }

    #[test]
    fn test_validate_session_config() {
        let valid_config = OpenAIRealtimeUtils::create_session_config(
            "gpt-4o-realtime-preview".to_string(),
            None,
            None,
        );
        assert!(OpenAIRealtimeUtils::validate_session_config(&valid_config).is_ok());

        // Test invalid model
        let mut invalid_model = valid_config.clone();
        invalid_model.model = "gpt-4".to_string();
        assert!(OpenAIRealtimeUtils::validate_session_config(&invalid_model).is_err());

        // Test invalid temperature
        let mut invalid_temp = valid_config.clone();
        invalid_temp.temperature = Some(3.0);
        assert!(OpenAIRealtimeUtils::validate_session_config(&invalid_temp).is_err());

        // Test invalid max tokens
        let mut invalid_tokens = valid_config.clone();
        invalid_tokens.max_response_output_tokens = Some(0);
        assert!(OpenAIRealtimeUtils::validate_session_config(&invalid_tokens).is_err());
    }

    #[test]
    fn test_create_text_input() {
        if let RealtimeEvent::ConversationItemCreate { item, .. } =
            OpenAIRealtimeUtils::create_text_input("Hello".to_string(), None)
        {
            assert!(matches!(item.item_type, ItemType::Message));
            assert!(matches!(item.role, Some(ItemRole::User)));
            if let Some(content) = item.content {
                assert!(matches!(content[0], ContentPart::InputText { .. }));
            }
        } else {
            panic!("Expected ConversationItemCreate event");
        }
    }

    #[test]
    fn test_create_audio_input() {
        if let RealtimeEvent::InputAudioBufferAppend { audio } =
            OpenAIRealtimeUtils::create_audio_input("audio_data".to_string())
        {
            assert_eq!(audio, "audio_data");
        } else {
            panic!("Expected InputAudioBufferAppend event");
        }
    }

    #[test]
    fn test_get_recommended_voice() {
        assert!(matches!(
            OpenAIRealtimeUtils::get_recommended_voice("customer_service"),
            Voice::Alloy
        ));
        assert!(matches!(
            OpenAIRealtimeUtils::get_recommended_voice("narrator"),
            Voice::Onyx
        ));
        assert!(matches!(
            OpenAIRealtimeUtils::get_recommended_voice("unknown"),
            Voice::Alloy
        ));
    }

    #[test]
    fn test_get_sample_rate() {
        assert_eq!(
            OpenAIRealtimeUtils::get_sample_rate(&AudioFormat::Pcm16),
            24000
        );
        assert_eq!(
            OpenAIRealtimeUtils::get_sample_rate(&AudioFormat::G711Ulaw),
            8000
        );
        assert_eq!(
            OpenAIRealtimeUtils::get_sample_rate(&AudioFormat::G711Alaw),
            8000
        );
    }

    #[test]
    fn test_estimate_cost_per_minute() {
        let cost_with_audio =
            OpenAIRealtimeUtils::estimate_cost_per_minute("gpt-4o-realtime-preview", true).unwrap();
        assert_eq!(cost_with_audio, 0.06);

        let cost_text_only =
            OpenAIRealtimeUtils::estimate_cost_per_minute("gpt-4o-realtime-preview", false)
                .unwrap();
        assert_eq!(cost_text_only, 0.03);

        assert!(OpenAIRealtimeUtils::estimate_cost_per_minute("unknown-model", true).is_err());
    }
}
