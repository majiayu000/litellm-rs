//! Chat completion methods

use super::llm_client::LLMClient;
use super::provider_payloads::{
    build_anthropic_request_body, build_openai_request_body, convert_anthropic_response,
};
use crate::sdk::{errors::*, types::*};
use serde::de::DeserializeOwned;
use std::time::SystemTime;
use tracing::{debug, error};

async fn api_error_from_response(response: reqwest::Response) -> SDKError {
    let status = response.status();
    let error_text = response.text().await.unwrap_or_default();
    SDKError::ApiError(format!("HTTP {}: {}", status, error_text))
}

async fn send_json_request(
    request_builder: reqwest::RequestBuilder,
    body: &serde_json::Value,
) -> Result<reqwest::Response> {
    request_builder
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| SDKError::NetworkError(e.to_string()))
}

async fn parse_json_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    response
        .json()
        .await
        .map_err(|e| SDKError::ParseError(e.to_string()))
}

impl LLMClient {
    /// Send chat message (using load balancing)
    pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse> {
        let request = SdkChatRequest {
            model: String::new(),
            messages,
            options: ChatOptions::default(),
        };

        self.chat_with_options(request).await
    }

    /// Send chat message (with options)
    pub async fn chat_with_options(&self, request: SdkChatRequest) -> Result<ChatResponse> {
        let start_time = SystemTime::now();
        let provider = self.select_provider(&request).await?;
        let result = self.execute_chat_request(&provider.id, request).await;

        self.update_provider_stats(&provider.id, start_time, &result)
            .await;

        result
    }

    /// Streaming chat
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
    ) -> Result<impl futures::Stream<Item = Result<ChatChunk>>> {
        let provider = self.select_provider_for_stream(&messages).await?;
        self.execute_stream_request(&provider.id, messages).await
    }

    /// Execute chat request with a specific provider
    pub(crate) async fn execute_chat_request(
        &self,
        provider_id: &str,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let provider = self.provider_config(provider_id)?;

        debug!("Executing chat request with provider: {}", provider_id);

        match provider.provider_type {
            crate::sdk::config::ProviderType::Anthropic => {
                self.call_anthropic_api(provider, request).await
            }
            crate::sdk::config::ProviderType::OpenAI => {
                self.call_openai_api(provider, request).await
            }
            crate::sdk::config::ProviderType::Google => {
                self.call_google_api(provider, request).await
            }
            _ => Err(SDKError::ProviderError(format!(
                "Provider type {:?} is not implemented in SDK client",
                provider.provider_type
            ))),
        }
    }

    /// Execute stream request
    pub(crate) async fn execute_stream_request(
        &self,
        provider_id: &str,
        _messages: Vec<Message>,
    ) -> Result<impl futures::Stream<Item = Result<ChatChunk>>> {
        let provider = self.provider_config(provider_id)?;

        Err::<futures::stream::Empty<Result<ChatChunk>>, _>(SDKError::ProviderError(format!(
            "Streaming is not implemented for provider type {:?}",
            provider.provider_type
        )))
    }

    /// Call Anthropic API
    async fn call_anthropic_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let model = self.provider_default_model(provider, "claude-sonnet-4-5");
        let body = build_anthropic_request_body(&request, model);
        let url = self.anthropic_messages_endpoint(provider);

        debug!("Calling Anthropic API: {}", url);

        let response = send_json_request(
            self.http_client
                .post(&url)
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01"),
            &body,
        )
        .await?;

        if !response.status().is_success() {
            let error = api_error_from_response(response).await;
            error!("Anthropic API error: {}", error);
            return Err(error);
        }

        let anthropic_response: serde_json::Value = parse_json_response(response).await?;
        convert_anthropic_response(anthropic_response, model)
    }

    /// Call OpenAI API
    async fn call_openai_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let model = self.provider_default_model(provider, "gpt-5.2-chat");
        let body = build_openai_request_body(&request, model);
        let url = self.provider_endpoint(provider, "https://api.openai.com", "v1/chat/completions");

        debug!("Calling OpenAI API: {}", url);

        let response = send_json_request(
            self.http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", provider.api_key)),
            &body,
        )
        .await?;

        if !response.status().is_success() {
            return Err(api_error_from_response(response).await);
        }

        parse_json_response(response).await
    }

    /// Call Google API
    async fn call_google_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        _request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        Err(SDKError::ProviderError(format!(
            "Provider '{}' (Google) is not implemented in SDK client",
            provider.id
        )))
    }
}
