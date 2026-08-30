use crate::core::audio::types::{
    SpeechRequest, SpeechResponse, TranscriptionRequest, TranscriptionResponse, TranslationRequest,
    TranslationResponse, WordInfo, format_to_content_type,
};
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::providers::shared::parse_retry_after_from_body;
use crate::core::providers::{Provider, ProviderError};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::ChatResponse;
use reqwest::{Method, StatusCode, Url, header};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

impl Provider {
    /// Transcribe audio to text.
    ///
    /// Route selection must confirm `ProviderCapability::AudioTranscription`
    /// before calling this optional dispatch method.
    pub async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
        context: RequestContext,
    ) -> Result<TranscriptionResponse, ProviderError> {
        dispatch_provider!(async_err, self, audio_transcription, request, context)
    }

    /// Translate audio to English text.
    ///
    /// Route selection must confirm `ProviderCapability::AudioTranslation`
    /// before calling this optional dispatch method.
    pub async fn audio_translation(
        &self,
        request: TranslationRequest,
        context: RequestContext,
    ) -> Result<TranslationResponse, ProviderError> {
        dispatch_provider!(async_err, self, audio_translation, request, context)
    }

    /// Generate speech audio from text.
    ///
    /// Route selection must confirm `ProviderCapability::TextToSpeech` before
    /// calling this optional dispatch method.
    pub async fn text_to_speech(
        &self,
        request: SpeechRequest,
        context: RequestContext,
    ) -> Result<SpeechResponse, ProviderError> {
        dispatch_provider!(async_err, self, text_to_speech, request, context)
    }
}

const DEEPGRAM_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::AudioTranscription,
    ProviderCapability::TextToSpeech,
];

/// Native Deepgram audio transport.
#[derive(Debug, Clone)]
pub struct DeepgramProvider {
    client: BaseHttpClient,
    api_key: String,
    api_base: String,
    models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct DeepgramResponse {
    metadata: Option<DeepgramMetadata>,
    results: DeepgramResults,
}

#[derive(Deserialize)]
struct DeepgramMetadata {
    duration: Option<f64>,
}

#[derive(Deserialize)]
struct DeepgramResults {
    channels: Vec<DeepgramChannel>,
}

#[derive(Deserialize)]
struct DeepgramChannel {
    alternatives: Vec<DeepgramAlternative>,
}

#[derive(Deserialize)]
struct DeepgramAlternative {
    transcript: String,
    #[serde(default)]
    languages: Vec<String>,
    words: Option<Vec<DeepgramWord>>,
}

#[derive(Deserialize)]
struct DeepgramWord {
    word: String,
    start: f64,
    end: f64,
}

impl DeepgramProvider {
    pub fn new(mut config: BaseConfig) -> Result<Self, ProviderError> {
        if config.api_base.is_none() {
            config.api_base = Some("https://api.deepgram.com".to_string());
        }
        let api_key = config
            .get_effective_api_key("deepgram")
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| ProviderError::configuration("deepgram", "API key is required"))?;
        let api_base = config.get_effective_api_base("deepgram");
        let client = BaseHttpClient::new_for_provider("deepgram", config)?;
        Ok(Self {
            client,
            api_key,
            api_base,
            models: native_audio_models(
                "deepgram",
                &[
                    (
                        "nova-3",
                        ProviderCapability::AudioTranscription,
                        "audio_second",
                    ),
                    (
                        "nova-3-general",
                        ProviderCapability::AudioTranscription,
                        "audio_second",
                    ),
                    (
                        "nova-3-medical",
                        ProviderCapability::AudioTranscription,
                        "audio_second",
                    ),
                    (
                        "aura-2-thalia-en",
                        ProviderCapability::TextToSpeech,
                        "character",
                    ),
                ],
            ),
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        provider_endpoint(&self.api_base, path, "deepgram")
    }
}

#[allow(async_fn_in_trait)]
impl LLMProvider for DeepgramProvider {
    fn name(&self) -> &str {
        "deepgram"
    }

    fn error_provider_name(&self) -> &'static str {
        "deepgram"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        DEEPGRAM_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        Err(ProviderError::not_supported("deepgram", "chat completion"))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported("deepgram", "chat completion"))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported("deepgram", "chat completion"))
    }

    async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
        _context: RequestContext,
    ) -> Result<TranscriptionResponse, ProviderError> {
        let mut url = self.endpoint("v1/listen")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("model", &request.model);
            if let Some(language) = request.language.as_deref() {
                query.append_pair("language", language);
            } else {
                query.append_pair("detect_language", "true");
            }
        }
        let response = self
            .client
            .request_preserving_endpoint_policy(Method::POST, url)?
            .header(header::AUTHORIZATION, format!("Token {}", self.api_key))
            .header(header::CONTENT_TYPE, audio_content_type(&request.filename))
            .body(request.file)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let response = response_or_error(response, "deepgram", false).await?;
        let parsed: DeepgramResponse = response.json().await.map_err(|_| {
            ProviderError::response_parsing("deepgram", "invalid transcription response")
        })?;
        let alternative = parsed
            .results
            .channels
            .into_iter()
            .next()
            .and_then(|channel| channel.alternatives.into_iter().next())
            .ok_or_else(|| {
                ProviderError::response_parsing("deepgram", "transcription response has no result")
            })?;
        Ok(TranscriptionResponse {
            text: alternative.transcript,
            task: Some("transcribe".to_string()),
            language: alternative
                .languages
                .into_iter()
                .next()
                .or(request.language),
            duration: parsed.metadata.and_then(|metadata| metadata.duration),
            words: alternative.words.map(|words| {
                words
                    .into_iter()
                    .map(|word| WordInfo {
                        word: word.word,
                        start: word.start,
                        end: word.end,
                    })
                    .collect()
            }),
            segments: None,
        })
    }

    async fn text_to_speech(
        &self,
        request: SpeechRequest,
        _context: RequestContext,
    ) -> Result<SpeechResponse, ProviderError> {
        let requested_format = request.response_format.as_deref().unwrap_or("mp3");
        let (encoding, container) = match requested_format {
            "mp3" => ("mp3", None),
            "wav" => ("linear16", Some("wav")),
            "opus" => ("opus", Some("ogg")),
            "pcm" => ("linear16", None),
            format => {
                return Err(ProviderError::invalid_request(
                    "deepgram",
                    format!("unsupported Deepgram speech format '{format}'"),
                ));
            }
        };
        let mut url = self.endpoint("v1/speak")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("model", &request.model);
            query.append_pair("encoding", encoding);
            if let Some(container) = container {
                query.append_pair("container", container);
            }
            if let Some(speed) = request.speed {
                query.append_pair("speed", &speed.to_string());
            }
        }
        let response = self
            .client
            .request_preserving_endpoint_policy(Method::POST, url)?
            .header(header::AUTHORIZATION, format!("Token {}", self.api_key))
            .json(&json!({"text": request.input}))
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let response = response_or_error(response, "deepgram", false).await?;
        speech_response(response, requested_format, "deepgram").await
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unknown
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        Err(ProviderError::not_supported(
            "deepgram",
            "token-based audio cost calculation",
        ))
    }
}

const ELEVENLABS_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::AudioTranscription,
    ProviderCapability::TextToSpeech,
];

/// Native ElevenLabs audio transport.
#[derive(Debug, Clone)]
pub struct ElevenLabsProvider {
    client: BaseHttpClient,
    api_key: String,
    api_base: String,
    models: Vec<ModelInfo>,
}

#[derive(Deserialize)]
struct ElevenLabsTranscription {
    text: String,
    language_code: Option<String>,
    words: Option<Vec<ElevenLabsWord>>,
}

#[derive(Deserialize)]
struct ElevenLabsWord {
    text: String,
    start: f64,
    end: f64,
    #[serde(rename = "type")]
    word_type: String,
}

#[derive(serde::Serialize)]
struct ElevenLabsSpeech<'a> {
    text: &'a str,
    model_id: &'a str,
}

impl ElevenLabsProvider {
    pub fn new(mut config: BaseConfig) -> Result<Self, ProviderError> {
        if config.api_base.is_none() {
            config.api_base = Some("https://api.elevenlabs.io".to_string());
        }
        let api_key = config
            .get_effective_api_key("elevenlabs")
            .filter(|key| !key.trim().is_empty())
            .ok_or_else(|| ProviderError::configuration("elevenlabs", "API key is required"))?;
        let api_base = config.get_effective_api_base("elevenlabs");
        let client = BaseHttpClient::new_for_provider("elevenlabs", config)?;
        Ok(Self {
            client,
            api_key,
            api_base,
            models: native_audio_models(
                "elevenlabs",
                &[
                    (
                        "scribe_v1",
                        ProviderCapability::AudioTranscription,
                        "audio_second",
                    ),
                    (
                        "scribe_v1_experimental",
                        ProviderCapability::AudioTranscription,
                        "audio_second",
                    ),
                    ("eleven_v3", ProviderCapability::TextToSpeech, "character"),
                    (
                        "eleven_multilingual_v2",
                        ProviderCapability::TextToSpeech,
                        "character",
                    ),
                ],
            ),
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, ProviderError> {
        provider_endpoint(&self.api_base, path, "elevenlabs")
    }

    fn speech_endpoint(&self, voice: &str, output_format: &str) -> Result<Url, ProviderError> {
        if voice.trim().is_empty() {
            return Err(ProviderError::invalid_request(
                "elevenlabs",
                "voice is required",
            ));
        }
        let mut url = self.endpoint("v1/text-to-speech")?;
        url.path_segments_mut()
            .map_err(|_| {
                ProviderError::configuration("elevenlabs", "invalid ElevenLabs API base URL")
            })?
            .push(voice);
        url.query_pairs_mut()
            .append_pair("output_format", output_format);
        Ok(url)
    }
}

#[allow(async_fn_in_trait)]
impl LLMProvider for ElevenLabsProvider {
    fn name(&self) -> &str {
        "elevenlabs"
    }

    fn error_provider_name(&self) -> &'static str {
        "elevenlabs"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        ELEVENLABS_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        Ok(params)
    }

    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        Err(ProviderError::not_supported(
            "elevenlabs",
            "chat completion",
        ))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "elevenlabs",
            "chat completion",
        ))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "elevenlabs",
            "chat completion",
        ))
    }

    async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
        _context: RequestContext,
    ) -> Result<TranscriptionResponse, ProviderError> {
        let file = reqwest::multipart::Part::bytes(request.file)
            .file_name(request.filename.clone())
            .mime_str(audio_content_type(&request.filename))
            .map_err(|_| ProviderError::invalid_request("elevenlabs", "invalid audio MIME type"))?;
        let mut form = reqwest::multipart::Form::new()
            .part("file", file)
            .text("model_id", request.model);
        if let Some(language) = request.language.as_deref() {
            form = form.text("language_code", language.to_string());
        }
        if let Some(temperature) = request.temperature {
            form = form.text("temperature", temperature.to_string());
        }
        if request
            .timestamp_granularities
            .as_ref()
            .is_some_and(|values| values.iter().any(|value| value == "word"))
        {
            form = form.text("timestamps_granularity", "word");
        }
        let response = self
            .client
            .request_preserving_endpoint_policy(Method::POST, self.endpoint("v1/speech-to-text")?)?
            .header("xi-api-key", &self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let response = response_or_error(response, "elevenlabs", true).await?;
        let parsed: ElevenLabsTranscription = response.json().await.map_err(|_| {
            ProviderError::response_parsing("elevenlabs", "invalid transcription response")
        })?;
        Ok(TranscriptionResponse {
            text: parsed.text,
            task: Some("transcribe".to_string()),
            language: parsed.language_code.or(request.language),
            duration: None,
            words: parsed.words.map(|words| {
                words
                    .into_iter()
                    .filter(|word| word.word_type == "word")
                    .map(|word| WordInfo {
                        word: word.text,
                        start: word.start,
                        end: word.end,
                    })
                    .collect()
            }),
            segments: None,
        })
    }

    async fn text_to_speech(
        &self,
        request: SpeechRequest,
        _context: RequestContext,
    ) -> Result<SpeechResponse, ProviderError> {
        if request.speed.is_some_and(|speed| speed != 1.0) {
            return Err(ProviderError::invalid_request(
                "elevenlabs",
                "speed adjustment is not supported by ElevenLabs standard TTS",
            ));
        }
        let requested_format = request.response_format.as_deref().unwrap_or("mp3");
        let output_format = match requested_format {
            "mp3" => "mp3_44100_128",
            "opus" => "opus_48000_128",
            "pcm" => "pcm_44100",
            "wav" => "wav_44100",
            native
                if native.starts_with("mp3_")
                    || native.starts_with("opus_")
                    || native.starts_with("pcm_")
                    || native.starts_with("wav_")
                    || native.starts_with("ulaw_") =>
            {
                native
            }
            format => {
                return Err(ProviderError::invalid_request(
                    "elevenlabs",
                    format!("unsupported ElevenLabs speech format '{format}'"),
                ));
            }
        };
        let url = self.speech_endpoint(&request.voice, output_format)?;
        let body = ElevenLabsSpeech {
            text: &request.input,
            model_id: &request.model,
        };
        let response = self
            .client
            .request_preserving_endpoint_policy(Method::POST, url)?
            .header("xi-api-key", &self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let response = response_or_error(response, "elevenlabs", true).await?;
        speech_response(response, requested_format, "elevenlabs").await
    }

    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unknown
    }

    async fn calculate_cost(
        &self,
        _model: &str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> Result<f64, ProviderError> {
        Err(ProviderError::not_supported(
            "elevenlabs",
            "token-based audio cost calculation",
        ))
    }
}

fn native_audio_models(
    provider: &str,
    models: &[(&str, ProviderCapability, &str)],
) -> Vec<ModelInfo> {
    models
        .iter()
        .map(|(id, capability, unit)| ModelInfo {
            id: (*id).to_string(),
            name: (*id).to_string(),
            provider: provider.to_string(),
            capabilities: vec![capability.clone()],
            metadata: HashMap::from([("pricing_unit".to_string(), json!(unit))]),
            ..ModelInfo::default()
        })
        .collect()
}

fn provider_endpoint(base: &str, path: &str, provider: &'static str) -> Result<Url, ProviderError> {
    let base = base.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    let path = if base.ends_with("/v1") {
        path.strip_prefix("v1/").unwrap_or(path)
    } else {
        path
    };
    Url::parse(&format!("{}/{}", base, path))
        .map_err(|_| ProviderError::configuration(provider, "invalid provider API base URL"))
}

fn audio_content_type(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("wav") => "audio/wav",
        Some("mp3" | "mpeg" | "mpga") => "audio/mpeg",
        Some("flac") => "audio/flac",
        Some("ogg" | "oga") => "audio/ogg",
        Some("webm") => "audio/webm",
        Some("m4a" | "mp4") => "audio/mp4",
        _ => "application/octet-stream",
    }
}

async fn speech_response(
    response: reqwest::Response,
    requested_format: &str,
    provider: &'static str,
) -> Result<SpeechResponse, ProviderError> {
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| format_to_content_type(requested_format).to_string());
    let audio = response
        .bytes()
        .await
        .map_err(|_| ProviderError::response_parsing(provider, "failed to read speech response"))?;
    Ok(SpeechResponse {
        audio: audio.to_vec(),
        content_type,
    })
}

async fn response_or_error(
    response: reqwest::Response,
    provider: &'static str,
    unprocessable_is_invalid: bool,
) -> Result<reqwest::Response, ProviderError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let header_retry = response
        .headers()
        .get(header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let body = response.text().await.unwrap_or_default();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(ProviderError::rate_limit(
            provider,
            header_retry.or_else(|| parse_retry_after_from_body(&body)),
        ));
    }
    if unprocessable_is_invalid && status == StatusCode::UNPROCESSABLE_ENTITY {
        return Err(ProviderError::invalid_request(provider, body));
    }
    Err(HttpErrorMapper::map_status_code(
        provider,
        status.as_u16(),
        &body,
    ))
}

pub(crate) fn native_audio_base_config(
    config: &Value,
    provider: &'static str,
) -> Result<BaseConfig, ProviderError> {
    let mut base = BaseConfig::from_env(provider);
    if config
        .get("max_retries")
        .is_some_and(|value| value.as_u64() != Some(u64::from(BaseConfig::default().max_retries)))
    {
        return Err(ProviderError::configuration(
            provider,
            "max_retries is unsupported for native audio providers",
        ));
    }
    base.max_retries = 0;
    base.api_key = config
        .get("api_key")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(base.api_key);
    if let Some(api_base) = config
        .get("base_url")
        .or_else(|| config.get("api_base"))
        .and_then(Value::as_str)
    {
        base.api_base = Some(api_base.trim_end_matches('/').to_string());
    }
    if let Some(timeout) = config.get("timeout").and_then(Value::as_u64) {
        base.timeout = timeout;
    }
    if let Some(access) = config.get("endpoint_access") {
        base.endpoint_access = serde_json::from_value(access.clone()).map_err(|error| {
            ProviderError::configuration(provider, format!("invalid endpoint_access: {error}"))
        })?;
    }
    Ok(base)
}
