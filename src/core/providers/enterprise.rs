//! Shared dispatch for governed enterprise inference runtimes.

use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
use crate::core::providers::{ProviderError, ProviderType};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::health::HealthStatus;
use crate::core::types::image::ImageGenerationRequest;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{
    ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse,
};
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

static COMPAT_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
    ProviderCapability::FunctionCalling,
];

#[derive(Clone)]
pub struct EnterpriseOpenAiProvider {
    name: &'static str,
    inner: OpenAILikeProvider,
    models: Vec<ModelInfo>,
}

impl std::fmt::Debug for EnterpriseOpenAiProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnterpriseOpenAiProvider")
            .field("name", &self.name)
            .field("models", &self.models)
            .finish_non_exhaustive()
    }
}

pub(crate) struct EnterpriseOpenAiSettings {
    pub api_base: String,
    pub api_key: String,
    pub model_prefix: &'static str,
    pub endpoint_access: crate::core::net::ProviderEndpointAccess,
    pub timeout: u64,
    pub max_retries: u32,
    pub headers: HashMap<String, String>,
    pub models: Vec<String>,
}

pub(crate) fn normalize_enterprise_base_url(
    provider: &'static str,
    raw: &str,
    origin_only: bool,
) -> Result<String, ProviderError> {
    let raw = raw.trim();
    let parsed = url::Url::parse(raw).map_err(|error| {
        ProviderError::configuration(provider, format!("invalid base URL: {error}"))
    })?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(ProviderError::configuration(
            provider,
            "base URL must use HTTP(S) and include a host",
        ));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(ProviderError::configuration(
            provider,
            "base URL must not contain user information",
        ));
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(ProviderError::configuration(
            provider,
            "base URL must not contain a query or fragment",
        ));
    }
    if origin_only && parsed.path() != "/" {
        return Err(ProviderError::configuration(
            provider,
            "base URL must be an origin without a path",
        ));
    }
    Ok(raw.trim_end_matches('/').to_string())
}

impl EnterpriseOpenAiProvider {
    pub(crate) async fn new(
        name: &'static str,
        settings: EnterpriseOpenAiSettings,
    ) -> Result<Self, ProviderError> {
        let mut config = OpenAILikeConfig::with_api_key(settings.api_base, settings.api_key);
        config.provider_name = name.to_string();
        config.model_prefix = Some(settings.model_prefix.to_string());
        config.base.endpoint_access = settings.endpoint_access;
        config.base.timeout = settings.timeout;
        config.base.max_retries = settings.max_retries;
        config.custom_headers = settings.headers;
        let inner = OpenAILikeProvider::new_for_catalog(config, COMPAT_CAPABILITIES)
            .await
            .map_err(|error| rebrand_error(error, name))?;
        let models = settings
            .models
            .into_iter()
            .map(|model| ModelInfo {
                id: model.clone(),
                name: model,
                provider: name.to_string(),
                supports_streaming: true,
                supports_tools: true,
                capabilities: COMPAT_CAPABILITIES.to_vec(),
                ..ModelInfo::default()
            })
            .collect();
        Ok(Self {
            name,
            inner,
            models,
        })
    }
}

impl LLMProvider for EnterpriseOpenAiProvider {
    fn name(&self) -> &str {
        self.name
    }
    fn error_provider_name(&self) -> &'static str {
        self.name
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        COMPAT_CAPABILITIES
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn supports_model(&self, model: &str) -> bool {
        !model.trim().is_empty()
            && (self.models.is_empty()
                || self
                    .models
                    .iter()
                    .any(|item| item.id == model || format!("{}/{}", self.name, item.id) == model))
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.inner
            .chat_completion(request, context)
            .await
            .map_err(|error| rebrand_error(error, self.name))
    }
    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        let stream = self
            .inner
            .chat_completion_stream(request, context)
            .await
            .map_err(|error| rebrand_error(error, self.name))?;
        let name = self.name;
        Ok(Box::pin(futures::StreamExt::map(stream, move |item| {
            item.map_err(|error| rebrand_error(error, name))
        })))
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
        Err(ProviderError::not_supported(self.name, "runtime pricing"))
    }
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        self.inner.get_supported_openai_params(model)
    }
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        self.inner
            .map_openai_params(params, model)
            .await
            .map_err(|error| rebrand_error(error, self.name))
    }
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError> {
        self.inner
            .transform_request(request, context)
            .await
            .map_err(|error| rebrand_error(error, self.name))
    }
    async fn transform_response(
        &self,
        raw: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        self.inner
            .transform_response(raw, model, request_id)
            .await
            .map_err(|error| rebrand_error(error, self.name))
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        self.inner.get_error_mapper()
    }
}

#[derive(Debug, Clone)]
pub enum EnterpriseProvider {
    Databricks(crate::core::providers::databricks::DatabricksProvider),
    Snowflake(crate::core::providers::snowflake::SnowflakeProvider),
    Oci(crate::core::providers::oci::OciProvider),
    Watsonx(crate::core::providers::watsonx::WatsonxProvider),
    SageMaker(crate::core::providers::sagemaker::SageMakerProvider),
}

impl EnterpriseProvider {
    pub fn provider_type(&self) -> ProviderType {
        match self {
            Self::Databricks(_) => ProviderType::Databricks,
            Self::Snowflake(_) => ProviderType::Snowflake,
            Self::Oci(_) => ProviderType::Oci,
            Self::Watsonx(_) => ProviderType::Watsonx,
            Self::SageMaker(_) => ProviderType::SageMaker,
        }
    }
}

macro_rules! delegate {
    ($self:expr, $method:ident $(, $arg:expr)*) => { match $self {
        EnterpriseProvider::Databricks(provider) => provider.$method($($arg),*),
        EnterpriseProvider::Snowflake(provider) => provider.$method($($arg),*),
        EnterpriseProvider::Oci(provider) => provider.$method($($arg),*),
        EnterpriseProvider::Watsonx(provider) => provider.$method($($arg),*),
        EnterpriseProvider::SageMaker(provider) => provider.$method($($arg),*),
    }};
}

impl LLMProvider for EnterpriseProvider {
    fn name(&self) -> &str {
        delegate!(self, name)
    }
    fn error_provider_name(&self) -> &'static str {
        delegate!(self, error_provider_name)
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        delegate!(self, capabilities)
    }
    fn models(&self) -> &[ModelInfo] {
        delegate!(self, models)
    }
    fn supports_model(&self, model: &str) -> bool {
        delegate!(self, supports_model, model)
    }
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Databricks(p) => p.chat_completion(request, context).await,
            Self::Snowflake(p) => p.chat_completion(request, context).await,
            Self::Oci(p) => p.chat_completion(request, context).await,
            Self::Watsonx(p) => p.chat_completion(request, context).await,
            Self::SageMaker(p) => p.chat_completion(request, context).await,
        }
    }
    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        match self {
            Self::Databricks(p) => p.chat_completion_stream(request, context).await,
            Self::Snowflake(p) => p.chat_completion_stream(request, context).await,
            Self::Oci(p) => p.chat_completion_stream(request, context).await,
            Self::Watsonx(p) => p.chat_completion_stream(request, context).await,
            Self::SageMaker(p) => p.chat_completion_stream(request, context).await,
        }
    }
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        match self {
            Self::Databricks(p) => p.embeddings(request, context).await,
            Self::Snowflake(p) => p.embeddings(request, context).await,
            Self::Oci(p) => p.embeddings(request, context).await,
            Self::Watsonx(p) => p.embeddings(request, context).await,
            Self::SageMaker(p) => p.embeddings(request, context).await,
        }
    }
    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        match self {
            Self::Databricks(p) => p.image_generation(request, context).await,
            Self::Snowflake(p) => p.image_generation(request, context).await,
            Self::Oci(p) => p.image_generation(request, context).await,
            Self::Watsonx(p) => p.image_generation(request, context).await,
            Self::SageMaker(p) => p.image_generation(request, context).await,
        }
    }
    async fn health_check(&self) -> HealthStatus {
        match self {
            Self::Databricks(p) => p.health_check().await,
            Self::Snowflake(p) => p.health_check().await,
            Self::Oci(p) => p.health_check().await,
            Self::Watsonx(p) => p.health_check().await,
            Self::SageMaker(p) => p.health_check().await,
        }
    }
    async fn calculate_cost(
        &self,
        model: &str,
        input: u32,
        output: u32,
    ) -> Result<f64, ProviderError> {
        match self {
            Self::Databricks(p) => p.calculate_cost(model, input, output).await,
            Self::Snowflake(p) => p.calculate_cost(model, input, output).await,
            Self::Oci(p) => p.calculate_cost(model, input, output).await,
            Self::Watsonx(p) => p.calculate_cost(model, input, output).await,
            Self::SageMaker(p) => p.calculate_cost(model, input, output).await,
        }
    }
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        delegate!(self, get_supported_openai_params, model)
    }
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        match self {
            Self::Databricks(p) => p.map_openai_params(params, model).await,
            Self::Snowflake(p) => p.map_openai_params(params, model).await,
            Self::Oci(p) => p.map_openai_params(params, model).await,
            Self::Watsonx(p) => p.map_openai_params(params, model).await,
            Self::SageMaker(p) => p.map_openai_params(params, model).await,
        }
    }
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError> {
        match self {
            Self::Databricks(p) => p.transform_request(request, context).await,
            Self::Snowflake(p) => p.transform_request(request, context).await,
            Self::Oci(p) => p.transform_request(request, context).await,
            Self::Watsonx(p) => p.transform_request(request, context).await,
            Self::SageMaker(p) => p.transform_request(request, context).await,
        }
    }
    async fn transform_response(
        &self,
        raw: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Databricks(p) => p.transform_response(raw, model, request_id).await,
            Self::Snowflake(p) => p.transform_response(raw, model, request_id).await,
            Self::Oci(p) => p.transform_response(raw, model, request_id).await,
            Self::Watsonx(p) => p.transform_response(raw, model, request_id).await,
            Self::SageMaker(p) => p.transform_response(raw, model, request_id).await,
        }
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        delegate!(self, get_error_mapper)
    }
}

fn rebrand_error(error: ProviderError, provider: &'static str) -> ProviderError {
    match error {
        ProviderError::Authentication { message, .. } => {
            ProviderError::authentication(provider, message)
        }
        ProviderError::RateLimit {
            message,
            retry_after,
            rpm_limit,
            tpm_limit,
            current_usage,
            ..
        } => ProviderError::RateLimit {
            provider,
            message,
            retry_after,
            rpm_limit,
            tpm_limit,
            current_usage,
        },
        ProviderError::QuotaExceeded { message, .. } => {
            ProviderError::quota_exceeded(provider, message)
        }
        ProviderError::ModelNotFound { model, .. } => {
            ProviderError::model_not_found(provider, model)
        }
        ProviderError::InvalidRequest { message, .. } => {
            ProviderError::invalid_request(provider, message)
        }
        ProviderError::Network { message, .. } => ProviderError::network(provider, message),
        ProviderError::ProviderUnavailable { message, .. } => {
            ProviderError::provider_unavailable(provider, message)
        }
        ProviderError::NotSupported { feature, .. } => {
            ProviderError::not_supported(provider, feature)
        }
        ProviderError::NotImplemented { feature, .. } => {
            ProviderError::not_implemented(provider, feature)
        }
        ProviderError::Configuration { message, .. } => {
            ProviderError::configuration(provider, message)
        }
        ProviderError::Serialization { message, .. } => {
            ProviderError::serialization(provider, message)
        }
        ProviderError::Timeout { message, .. } => ProviderError::timeout(provider, message),
        ProviderError::ContextLengthExceeded { max, actual, .. } => {
            ProviderError::ContextLengthExceeded {
                provider,
                max,
                actual,
            }
        }
        ProviderError::ContentFiltered {
            reason,
            policy_violations,
            potentially_retryable,
            ..
        } => ProviderError::ContentFiltered {
            provider,
            reason,
            policy_violations,
            potentially_retryable,
        },
        ProviderError::ApiError {
            status, message, ..
        } => ProviderError::api_error(provider, status, message),
        ProviderError::TokenLimitExceeded { message, .. } => {
            ProviderError::TokenLimitExceeded { provider, message }
        }
        ProviderError::FeatureDisabled { feature, .. } => {
            ProviderError::FeatureDisabled { provider, feature }
        }
        ProviderError::DeploymentError {
            deployment,
            message,
            ..
        } => ProviderError::DeploymentError {
            provider,
            deployment,
            message,
        },
        ProviderError::ResponseParsing { message, .. } => {
            ProviderError::response_parsing(provider, message)
        }
        ProviderError::RoutingError {
            attempted_providers,
            message,
            ..
        } => ProviderError::RoutingError {
            provider,
            attempted_providers,
            message,
        },
        ProviderError::TransformationError {
            from_format,
            to_format,
            message,
            ..
        } => ProviderError::TransformationError {
            provider,
            from_format,
            to_format,
            message,
        },
        ProviderError::Cancelled {
            operation_type,
            cancellation_reason,
            ..
        } => ProviderError::Cancelled {
            provider,
            operation_type,
            cancellation_reason,
        },
        ProviderError::Streaming {
            stream_type,
            position,
            last_chunk,
            message,
            ..
        } => ProviderError::Streaming {
            provider,
            stream_type,
            position,
            last_chunk,
            message,
        },
        ProviderError::Other { message, .. } => ProviderError::other(provider, message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn compatible_transport_errors_keep_platform_identity_and_category() {
        let error = rebrand_error(
            ProviderError::authentication("openai_like", "denied"),
            "snowflake",
        );
        assert!(
            matches!(error, ProviderError::Authentication { provider: "snowflake", message } if message == "denied")
        );
        let error = rebrand_error(
            ProviderError::rate_limit_with_retry("openai_like", "limited", Some(9)),
            "databricks",
        );
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                provider: "databricks",
                retry_after: Some(9),
                ..
            }
        ));
        let error = rebrand_error(
            ProviderError::ContentFiltered {
                provider: "openai_like",
                reason: "policy".to_string(),
                policy_violations: Some(vec!["safety".to_string()]),
                potentially_retryable: Some(false),
            },
            "snowflake",
        );
        assert!(matches!(
            error,
            ProviderError::ContentFiltered {
                provider: "snowflake",
                policy_violations: Some(violations),
                potentially_retryable: Some(false),
                ..
            } if violations == ["safety"]
        ));
    }
}
