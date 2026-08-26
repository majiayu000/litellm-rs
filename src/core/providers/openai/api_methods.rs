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

use reqwest::header::CONTENT_TYPE;
use reqwest::multipart;
use serde_json::Value;

use crate::core::audio::types::{
    SpeechRequest, SpeechResponse, TranscriptionRequest, TranscriptionResponse, TranslationRequest,
    TranslationResponse, format_to_content_type,
};
use crate::core::providers::base::{BaseHttpClient, HttpMethod, apply_provider_headers};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::responses::EmbeddingResponse;

use super::client::OpenAIProvider;
use super::config::OpenAIFeature;
use super::error::OpenAIError;
use super::error_mapper::OpenAIErrorMapper;

/// Additional OpenAI-specific API methods
impl OpenAIProvider {
    fn multipart_client(&self) -> Result<BaseHttpClient, OpenAIError> {
        BaseHttpClient::new_for_provider("openai", self.config.base.clone())
    }

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

        let form = transcription_form(request);
        let url = format!("{}/audio/transcriptions", self.config.get_api_base());
        let response = apply_provider_headers(
            self.multipart_client()?.post(url)?,
            self.get_request_headers(),
        )
        .multipart(form)
        .send()
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

        let form = translation_form(request);
        let url = format!("{}/audio/translations", self.config.get_api_base());
        let response = apply_provider_headers(
            self.multipart_client()?.post(url)?,
            self.get_request_headers(),
        )
        .multipart(form)
        .send()
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

        let response_format = request.response_format.clone();
        let body = serde_json::json!({
            "model": request.model,
            "input": request.input,
            "voice": request.voice,
            "response_format": request.response_format,
            "speed": request.speed,
        });
        let url = format!("{}/audio/speech", self.config.get_api_base());
        let response = self
            .pool_manager
            .execute_request(
                &url,
                HttpMethod::POST,
                self.get_request_headers(),
                Some(body),
            )
            .await
            .map_err(|e| OpenAIError::Network {
                provider: "openai",
                message: e.to_string(),
            })?;

        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format_to_content_type(response_format.as_deref().unwrap_or("mp3")).to_string()
            });
        let response_bytes = read_success_response_bytes(response).await?;

        Ok(SpeechResponse {
            audio: response_bytes.to_vec(),
            content_type,
        })
    }
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
