//! OpenAI Provider Additional API Methods
//!
//! Inherent methods called by the LLMProvider trait impl in `client.rs`:
//! - `embeddings`
//! - `generate_images`
//! - `audio_transcription`
//! - `audio_translation`
//! - `text_to_speech`
//!
//! Other side-API methods (completions, fine-tuning, image edit / variations,
//! vector stores, realtime, advanced chat) were declared but never reached from
//! any live code path and have been removed.

use reqwest::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use reqwest::multipart;
use serde_json::Value;

use crate::core::audio::types::{
    SpeechRequest, SpeechResponse, TranscriptionRequest, TranscriptionResponse, TranslationRequest,
    TranslationResponse, format_to_content_type,
};
use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, HttpErrorMapper, HttpMethod, apply_provider_headers,
};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::image::ImageEditRequest;
use crate::core::types::responses::{EmbeddingResponse, ImageGenerationResponse};

use super::client::OpenAIProvider;
use super::config::OpenAIFeature;
use super::error::OpenAIError;
use super::error_mapper::OpenAIErrorMapper;

/// Additional OpenAI-specific API methods
impl OpenAIProvider {
    /// Generate embeddings
    pub async fn embeddings(
        &self,
        request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse, OpenAIError> {
        // Like Python LiteLLM, we don't validate models locally
        // OpenAI API will handle invalid models

        // Transform to OpenAI format
        let openai_request = serde_json::json!({
            "input": request.input,
            "model": request.model,
            "encoding_format": request.encoding_format,
            "dimensions": request.dimensions,
            "user": request.user
        });

        // Execute request using high-performance connection pool
        let url = format!("{}/embeddings", self.config.get_api_base());

        let headers = self.get_request_headers();
        let body = Some(openai_request);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await
            .map_err(|e| OpenAIError::Network {
                provider: "openai",
                message: e.to_string(),
            })?;

        let response_bytes = read_success_response_bytes(response).await?;

        let response_json: Value =
            serde_json::from_slice(&response_bytes).map_err(|e| OpenAIError::ResponseParsing {
                provider: "openai",
                message: e.to_string(),
            })?;

        // Transform response
        serde_json::from_value(response_json).map_err(|e| OpenAIError::ResponseParsing {
            provider: "openai",
            message: e.to_string(),
        })
    }

    /// Generate images
    pub async fn generate_images(
        &self,
        prompt: String,
        model: Option<String>,
        n: Option<u32>,
        size: Option<String>,
        quality: Option<String>,
        style: Option<String>,
    ) -> Result<Value, OpenAIError> {
        let model = model.unwrap_or_else(|| "gpt-image-2".to_string());

        // Validate image generation capability
        if !self
            .config
            .is_feature_enabled(OpenAIFeature::ImageGeneration)
        {
            return Err(OpenAIError::NotSupported {
                provider: "openai",
                feature: "Image generation is disabled in configuration".to_string(),
            });
        }

        let request = serde_json::json!({
            "prompt": prompt,
            "model": model,
            "n": n,
            "size": size,
            "quality": quality,
            "style": style
        });

        let url = format!("{}/images/generations", self.config.get_api_base());

        let headers = self.get_request_headers();
        let body = Some(request);

        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, body)
            .await
            .map_err(|e| OpenAIError::Network {
                provider: "openai",
                message: e.to_string(),
            })?;

        let response_bytes = read_success_response_bytes(response).await?;

        serde_json::from_slice(&response_bytes).map_err(|e| OpenAIError::ResponseParsing {
            provider: "openai",
            message: e.to_string(),
        })
    }

    /// Transcribe audio through OpenAI's `/audio/transcriptions` endpoint.
    pub async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
    ) -> Result<TranscriptionResponse, OpenAIError> {
        if !self
            .config
            .is_feature_enabled(OpenAIFeature::AudioTranscription)
        {
            return Err(OpenAIError::NotSupported {
                provider: "openai",
                feature: "Audio transcription is disabled in configuration".to_string(),
            });
        }

        execute_audio_transcription(
            self.config.base.clone(),
            &self.config.get_api_base(),
            self.get_request_headers(),
            request,
            "openai",
        )
        .await
    }

    /// Translate audio through OpenAI's `/audio/translations` endpoint.
    pub async fn audio_translation(
        &self,
        request: TranslationRequest,
    ) -> Result<TranslationResponse, OpenAIError> {
        if !self.config.is_feature_enabled(OpenAIFeature::AudioModels) {
            return Err(OpenAIError::NotSupported {
                provider: "openai",
                feature: "Audio models are disabled in configuration".to_string(),
            });
        }

        execute_audio_translation(
            self.config.base.clone(),
            &self.config.get_api_base(),
            self.get_request_headers(),
            request,
            "openai",
        )
        .await
    }

    /// Generate speech through OpenAI's `/audio/speech` endpoint.
    pub async fn text_to_speech(
        &self,
        request: SpeechRequest,
    ) -> Result<SpeechResponse, OpenAIError> {
        if !self.config.is_feature_enabled(OpenAIFeature::AudioModels) {
            return Err(OpenAIError::NotSupported {
                provider: "openai",
                feature: "Audio models are disabled in configuration".to_string(),
            });
        }

        execute_text_to_speech(
            self.config.base.clone(),
            &self.config.get_api_base(),
            self.get_request_headers(),
            request,
            "openai",
        )
        .await
    }
}

pub(crate) async fn execute_image_edit(
    base: BaseConfig,
    api_base: &str,
    headers: Vec<HeaderPair>,
    request: ImageEditRequest,
    provider: &'static str,
) -> Result<ImageGenerationResponse, OpenAIError> {
    for (name, value) in &headers {
        HeaderName::from_bytes(name.as_ref().as_bytes())
            .map_err(|_| OpenAIError::configuration(provider, "invalid image edit header name"))?;
        HeaderValue::from_str(value.as_ref())
            .map_err(|_| OpenAIError::configuration(provider, "invalid image edit header value"))?;
    }
    let client = BaseHttpClient::new_for_provider_no_redirect(provider, base)?;
    let mut form = multipart::Form::new()
        .part(
            "image",
            multipart::Part::bytes(request.image).file_name("image.png"),
        )
        .text("prompt", request.prompt)
        .optional_text("model", request.model)
        .optional_text("n", request.n.map(|value| value.to_string()))
        .optional_text("size", request.size)
        .optional_text("response_format", request.response_format)
        .optional_text("user", request.user);
    if let Some(mask) = request.mask {
        form = form.part("mask", multipart::Part::bytes(mask).file_name("mask.png"));
    }
    let response = apply_provider_headers(
        client.post(format!("{}/images/edits", api_base.trim_end_matches('/')))?,
        headers,
    )
    .multipart(form)
    .send()
    .await
    .map_err(|error| client.map_preserved_request_error(error))?;
    let status = response.status();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(_) if !status.is_success() => {
            return Err(HttpErrorMapper::map_status_code(
                provider,
                status.as_u16(),
                &format!("Provider returned HTTP {status}, but its error body was unavailable"),
            ));
        }
        Err(error) => return Err(client.map_preserved_request_error(error)),
    };
    if !status.is_success() {
        return Err(HttpErrorMapper::map_status_code(
            provider,
            status.as_u16(),
            &String::from_utf8_lossy(&bytes),
        ));
    }
    let response: ImageGenerationResponse = serde_json::from_slice(&bytes).map_err(|error| {
        OpenAIError::response_parsing(provider, format!("invalid image edit response: {error}"))
    })?;
    if response.data.is_empty()
        || response.data.iter().any(|image| {
            image
                .url
                .as_deref()
                .is_none_or(|value| value.trim().is_empty())
                && image
                    .b64_json
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
        })
    {
        return Err(OpenAIError::response_parsing(
            provider,
            "image edit response did not contain usable image data",
        ));
    }
    Ok(response)
}

pub(crate) async fn execute_audio_transcription(
    base: BaseConfig,
    api_base: &str,
    headers: Vec<HeaderPair>,
    request: TranscriptionRequest,
    provider: &'static str,
) -> Result<TranscriptionResponse, OpenAIError> {
    let bytes = execute_audio_multipart(
        base,
        headers,
        provider,
        format!("{}/audio/transcriptions", api_base.trim_end_matches('/')),
        "audio transcription",
        transcription_form(request),
    )
    .await?;
    serde_json::from_slice(&bytes).map_err(|error| {
        OpenAIError::response_parsing(provider, format!("invalid transcription response: {error}"))
    })
}

pub(crate) async fn execute_audio_translation(
    base: BaseConfig,
    api_base: &str,
    headers: Vec<HeaderPair>,
    request: TranslationRequest,
    provider: &'static str,
) -> Result<TranslationResponse, OpenAIError> {
    let bytes = execute_audio_multipart(
        base,
        headers,
        provider,
        format!("{}/audio/translations", api_base.trim_end_matches('/')),
        "audio translation",
        translation_form(request),
    )
    .await?;
    serde_json::from_slice(&bytes).map_err(|error| {
        OpenAIError::response_parsing(provider, format!("invalid translation response: {error}"))
    })
}

pub(crate) async fn execute_text_to_speech(
    base: BaseConfig,
    api_base: &str,
    headers: Vec<HeaderPair>,
    request: SpeechRequest,
    provider: &'static str,
) -> Result<SpeechResponse, OpenAIError> {
    validate_outbound_headers(&headers, provider, "speech")?;
    let response_format = request.response_format.clone();
    let body = serde_json::json!({
        "model": request.model,
        "input": request.input,
        "voice": request.voice,
        "response_format": request.response_format,
        "speed": request.speed,
    });
    let client = BaseHttpClient::new_for_provider_no_redirect(provider, base)?;
    let response = apply_provider_headers(
        client.post(format!("{}/audio/speech", api_base.trim_end_matches('/')))?,
        headers,
    )
    .json(&body)
    .send()
    .await
    .map_err(|error| client.map_preserved_request_error(error))?;
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| {
            format_to_content_type(response_format.as_deref().unwrap_or("mp3")).to_string()
        });
    let audio = read_mapped_response_bytes(&client, response, provider).await?;
    Ok(SpeechResponse {
        audio,
        content_type,
    })
}

async fn execute_audio_multipart(
    base: BaseConfig,
    headers: Vec<HeaderPair>,
    provider: &'static str,
    url: String,
    operation: &str,
    form: multipart::Form,
) -> Result<Vec<u8>, OpenAIError> {
    validate_outbound_headers(&headers, provider, operation)?;
    let client = BaseHttpClient::new_for_provider_no_redirect(provider, base)?;
    let response = apply_provider_headers(client.post(url)?, headers)
        .multipart(form)
        .send()
        .await
        .map_err(|error| client.map_preserved_request_error(error))?;
    read_mapped_response_bytes(&client, response, provider).await
}

fn validate_outbound_headers(
    headers: &[HeaderPair],
    provider: &'static str,
    operation: &str,
) -> Result<(), OpenAIError> {
    for (name, value) in headers {
        HeaderName::from_bytes(name.as_ref().as_bytes()).map_err(|_| {
            OpenAIError::configuration(provider, format!("invalid {operation} header name"))
        })?;
        HeaderValue::from_str(value.as_ref()).map_err(|_| {
            OpenAIError::configuration(provider, format!("invalid {operation} header value"))
        })?;
    }
    Ok(())
}

async fn read_mapped_response_bytes(
    client: &BaseHttpClient,
    response: reqwest::Response,
    provider: &'static str,
) -> Result<Vec<u8>, OpenAIError> {
    let status = response.status();
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(_) if !status.is_success() => {
            return Err(HttpErrorMapper::map_status_code(
                provider,
                status.as_u16(),
                &format!("Provider returned HTTP {status}, but its error body was unavailable"),
            ));
        }
        Err(error) => return Err(client.map_preserved_request_error(error)),
    };
    if !status.is_success() {
        return Err(HttpErrorMapper::map_status_code(
            provider,
            status.as_u16(),
            &String::from_utf8_lossy(&bytes),
        ));
    }
    Ok(bytes.to_vec())
}

fn transcription_form(request: TranscriptionRequest) -> multipart::Form {
    let form = audio_file_form(request.file, request.filename)
        .text("model", request.model)
        .optional_text("language", request.language)
        .optional_text("prompt", request.prompt)
        .optional_text("response_format", request.response_format)
        .optional_text(
            "temperature",
            request.temperature.map(|value| value.to_string()),
        );

    if let Some(granularities) = request.timestamp_granularities {
        granularities.into_iter().fold(form, |form, granularity| {
            form.text("timestamp_granularities[]", granularity)
        })
    } else {
        form
    }
}

fn translation_form(request: TranslationRequest) -> multipart::Form {
    audio_file_form(request.file, request.filename)
        .text("model", request.model)
        .optional_text("prompt", request.prompt)
        .optional_text("response_format", request.response_format)
        .optional_text(
            "temperature",
            request.temperature.map(|value| value.to_string()),
        )
}

fn audio_file_form(file: Vec<u8>, filename: String) -> multipart::Form {
    let filename = if filename.trim().is_empty() {
        "audio.mp3".to_string()
    } else {
        filename
    };

    multipart::Form::new().part("file", multipart::Part::bytes(file).file_name(filename))
}

trait OptionalMultipartText {
    fn optional_text(self, name: &'static str, value: Option<String>) -> Self;
}

impl OptionalMultipartText for multipart::Form {
    fn optional_text(self, name: &'static str, value: Option<String>) -> Self {
        match value {
            Some(value) => self.text(name, value),
            None => self,
        }
    }
}

pub(super) async fn read_success_response_bytes(
    response: reqwest::Response,
) -> Result<Vec<u8>, OpenAIError> {
    let status = response.status();
    let response_bytes = match response.bytes().await {
        Ok(response_bytes) => response_bytes,
        Err(_) if !status.is_success() => {
            return Err(OpenAIErrorMapper
                .map_http_error(status.as_u16(), "failed to read upstream error body"));
        }
        Err(error) => {
            return Err(OpenAIError::Network {
                provider: "openai",
                message: error.to_string(),
            });
        }
    };

    if !status.is_success() {
        let body = String::from_utf8_lossy(&response_bytes);
        return Err(OpenAIErrorMapper.map_http_error(status.as_u16(), &body));
    }

    Ok(response_bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::super::config::OpenAIConfig;
    use super::*;
    use crate::core::net::ProviderEndpointAccess;

    #[test]
    fn credentialed_image_edit_client_explicitly_disables_redirects() {
        let source = include_str!("api_methods.rs");
        let image_edit = source
            .split_once("pub(crate) async fn execute_image_edit")
            .expect("image edit adapter should exist")
            .1
            .split_once("fn transcription_form")
            .expect("image edit adapter should end before transcription helpers")
            .0;

        assert!(image_edit.contains("BaseHttpClient::new_for_provider_no_redirect"));
    }

    #[tokio::test]
    async fn public_multipart_loopback_fails_before_connect() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("multipart listener should bind");
        let address = listener
            .local_addr()
            .expect("listener address should exist");
        let mut config = OpenAIConfig::default();
        config.base.api_key = Some("sk-test".to_string());
        config.base.api_base = Some(format!("http://{address}"));
        config.base.endpoint_access = ProviderEndpointAccess::PublicOnly;
        let error = OpenAIProvider::new(config)
            .await
            .expect_err("public-only loopback must fail during provider construction");
        assert!(error.to_string().contains("private or reserved"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err(),
            "public-only multipart request must not reach loopback"
        );
    }
}
