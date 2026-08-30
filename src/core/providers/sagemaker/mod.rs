//! Amazon SageMaker Runtime InvokeEndpoint adapter.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::providers::bedrock::SigV4Signer;
use crate::core::providers::enterprise::normalize_enterprise_base_url;
use crate::core::providers::enterprise::validate_request_header_value;
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::health::HealthStatus;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::ChatResponse;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

static CAPABILITIES: &[ProviderCapability] = &[ProviderCapability::ChatCompletion];

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SageMakerPayloadTransformer {
    OpenAiChat,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SageMakerConfig {
    pub aws_access_key_id: String,
    pub aws_secret_access_key: String,
    pub aws_session_token: Option<String>,
    pub region: String,
    pub endpoint_name: String,
    pub payload_transformer: SageMakerPayloadTransformer,
    pub target_model: Option<String>,
    pub target_variant: Option<String>,
    pub base_url: Option<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
}

impl std::fmt::Debug for SageMakerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SageMakerConfig")
            .field("aws_access_key_id", &"[REDACTED]")
            .field("aws_secret_access_key", &"[REDACTED]")
            .field(
                "aws_session_token",
                &self.aws_session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("region", &self.region)
            .field("endpoint_name", &self.endpoint_name)
            .field("payload_transformer", &self.payload_transformer)
            .field("target_model", &self.target_model)
            .field("target_variant", &self.target_variant)
            .field("base_url", &self.base_url)
            .field("models", &self.models)
            .field("endpoint_access", &self.endpoint_access)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

impl SageMakerConfig {
    fn validate_segment(
        provider: &'static str,
        field: &'static str,
        value: &str,
    ) -> Result<(), ProviderError> {
        if value.is_empty()
            || !value
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err(ProviderError::configuration(
                provider,
                format!("{field} contains invalid characters"),
            ));
        }
        Ok(())
    }
    pub fn api_base(&self) -> Result<String, ProviderError> {
        if let Some(base) = &self.base_url {
            return normalize_enterprise_base_url("sagemaker", base, false);
        }
        Self::validate_segment("sagemaker", "region", &self.region)?;
        Ok(format!(
            "https://runtime.sagemaker.{}.amazonaws.com",
            self.region
        ))
    }
    fn validate(&self) -> Result<(), ProviderError> {
        if self.aws_access_key_id.trim().is_empty() || self.aws_secret_access_key.trim().is_empty()
        {
            return Err(ProviderError::configuration(
                "sagemaker",
                "AWS credentials are required",
            ));
        }
        validate_request_header_value(
            "sagemaker",
            "aws_access_key_id",
            &format!("Credential={}", self.aws_access_key_id),
        )?;
        if let Some(token) = &self.aws_session_token {
            validate_request_header_value("sagemaker", "aws_session_token", token)?;
        }
        if let Some(target) = &self.target_model {
            validate_request_header_value("sagemaker", "target_model", target)?;
        }
        if let Some(variant) = &self.target_variant {
            validate_request_header_value("sagemaker", "target_variant", variant)?;
        }
        Self::validate_segment("sagemaker", "endpoint_name", &self.endpoint_name)?;
        if self.target_model.as_deref().is_some_and(str::is_empty)
            || self.target_variant.as_deref().is_some_and(str::is_empty)
        {
            return Err(ProviderError::configuration(
                "sagemaker",
                "target_model and target_variant cannot be empty",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SageMakerProvider {
    config: SageMakerConfig,
    url: String,
    client: BaseHttpClient,
    signer: SigV4Signer,
    models: Vec<ModelInfo>,
}

impl SageMakerProvider {
    pub fn new(config: SageMakerConfig) -> Result<Self, ProviderError> {
        config.validate()?;
        let base_url = config.api_base()?;
        let url = format!("{base_url}/endpoints/{}/invocations", config.endpoint_name);
        let client = BaseHttpClient::new_for_provider_no_redirect(
            "sagemaker",
            BaseConfig {
                api_base: Some(base_url),
                endpoint_access: config.endpoint_access,
                timeout: config.timeout,
                max_retries: config.max_retries,
                ..BaseConfig::default()
            },
        )?;
        let signer = SigV4Signer::new_for_service(
            config.aws_access_key_id.clone(),
            config.aws_secret_access_key.clone(),
            config.aws_session_token.clone(),
            config.region.clone(),
            "sagemaker",
        );
        let configured_models = if config.models.is_empty() {
            vec![config.endpoint_name.clone()]
        } else {
            config.models.clone()
        };
        let models = configured_models
            .into_iter()
            .map(|model| ModelInfo {
                id: model.clone(),
                name: model,
                provider: "sagemaker".to_string(),
                capabilities: CAPABILITIES.to_vec(),
                ..ModelInfo::default()
            })
            .collect();
        Ok(Self {
            config,
            url,
            client,
            signer,
            models,
        })
    }
    fn transform_payload(&self, request: ChatRequest) -> Result<String, ProviderError> {
        match self.config.payload_transformer {
            SageMakerPayloadTransformer::OpenAiChat => serde_json::to_string(&request)
                .map_err(|error| ProviderError::serialization("sagemaker", error.to_string())),
        }
    }
    fn signed_headers(
        &self,
        body: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<HashMap<String, String>, ProviderError> {
        let mut headers = HashMap::from([
            ("content-type".to_string(), "application/json".to_string()),
            ("accept".to_string(), "application/json".to_string()),
        ]);
        if let Some(target) = &self.config.target_model {
            headers.insert("x-amzn-sagemaker-target-model".to_string(), target.clone());
        }
        if let Some(variant) = &self.config.target_variant {
            headers.insert(
                "x-amzn-sagemaker-target-variant".to_string(),
                variant.clone(),
            );
        }
        self.signer
            .sign_request("POST", &self.url, &headers, body, timestamp)
            .map_err(|error| {
                ProviderError::configuration(
                    "sagemaker",
                    format!("failed to sign request: {error}"),
                )
            })
    }
}

impl LLMProvider for SageMakerProvider {
    fn name(&self) -> &str {
        "sagemaker"
    }
    fn error_provider_name(&self) -> &'static str {
        "sagemaker"
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        CAPABILITIES
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn supports_model(&self, model: &str) -> bool {
        self.models.iter().any(|item| {
            item.id == model || model.strip_prefix("sagemaker/") == Some(item.id.as_str())
        })
    }
    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        let body = self.transform_payload(request)?;
        let headers = self.signed_headers(&body, chrono::Utc::now())?;
        let mut outgoing = self.client.post(&self.url)?.body(body);
        for (key, value) in headers {
            outgoing = outgoing.header(key, value);
        }
        let response = outgoing
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let bytes = response.bytes().await.map_err(|_| {
            ProviderError::network("sagemaker", "failed to read InvokeEndpoint response")
        })?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "sagemaker",
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
        match self.config.payload_transformer {
            SageMakerPayloadTransformer::OpenAiChat => serde_json::from_slice(&bytes)
                .map_err(|error| ProviderError::response_parsing("sagemaker", error.to_string())),
        }
    }
    async fn health_check(&self) -> HealthStatus {
        HealthStatus::Unknown
    }
    async fn calculate_cost(
        &self,
        _model: &str,
        _input: u32,
        _output: u32,
    ) -> Result<f64, ProviderError> {
        Err(ProviderError::not_supported("sagemaker", "runtime pricing"))
    }
    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &["temperature", "max_tokens", "top_p", "stop"]
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
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        serde_json::to_value(request)
            .map_err(|error| ProviderError::serialization("sagemaker", error.to_string()))
    }
    async fn transform_response(
        &self,
        raw: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw)
            .map_err(|error| ProviderError::response_parsing("sagemaker", error.to_string()))
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }
}

const fn default_timeout() -> u64 {
    60
}
const fn default_retries() -> u32 {
    2
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    fn config() -> SageMakerConfig {
        SageMakerConfig {
            aws_access_key_id: "AKIATEST".to_string(),
            aws_secret_access_key: "secret".to_string(),
            aws_session_token: None,
            region: "us-east-1".to_string(),
            endpoint_name: "governed-chat".to_string(),
            payload_transformer: SageMakerPayloadTransformer::OpenAiChat,
            target_model: Some("tenant-model.tar.gz".to_string()),
            target_variant: Some("blue".to_string()),
            base_url: None,
            models: vec!["tenant-chat".to_string()],
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
        }
    }
    #[test]
    fn payload_transformer_is_required_and_unknown_schemas_fail_closed() {
        let missing = serde_json::from_value::<SageMakerConfig>(
            serde_json::json!({"aws_access_key_id":"a","aws_secret_access_key":"b","aws_session_token":null,"region":"us-east-1","endpoint_name":"demo","target_model":null,"target_variant":null,"base_url":null}),
        );
        let unknown = serde_json::from_value::<SageMakerConfig>(
            serde_json::json!({"aws_access_key_id":"a","aws_secret_access_key":"b","aws_session_token":null,"region":"us-east-1","endpoint_name":"demo","payload_transformer":"tgi_guess","target_model":null,"target_variant":null,"base_url":null}),
        );
        assert!(missing.is_err());
        assert!(unknown.is_err());
    }
    #[test]
    fn configured_model_directory_is_accepted() {
        let parsed = serde_json::from_value::<SageMakerConfig>(serde_json::json!({
            "aws_access_key_id":"a",
            "aws_secret_access_key":"b",
            "aws_session_token":null,
            "region":"us-east-1",
            "endpoint_name":"demo",
            "payload_transformer":"open_ai_chat",
            "target_model":null,
            "target_variant":null,
            "base_url":null,
            "max_retries":3,
            "models":["tenant-chat"]
        }));
        let provider = SageMakerProvider::new(parsed.expect("gateway model directory must parse"))
            .expect("gateway model directory must build");
        assert!(provider.supports_model("sagemaker/tenant-chat"));
        assert!(!provider.supports_model("sagemaker/other-model"));
    }
    #[test]
    fn sigv4_and_target_headers_use_invoke_endpoint_contract() {
        let provider = SageMakerProvider::new(config()).expect("valid config should build");
        assert_eq!(
            provider.url,
            "https://runtime.sagemaker.us-east-1.amazonaws.com/endpoints/governed-chat/invocations"
        );
        let headers = provider
            .signed_headers(
                "{}",
                chrono::Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
            )
            .expect("request should sign");
        assert_eq!(
            headers
                .get("x-amzn-sagemaker-target-model")
                .map(String::as_str),
            Some("tenant-model.tar.gz")
        );
        assert_eq!(
            headers
                .get("x-amzn-sagemaker-target-variant")
                .map(String::as_str),
            Some("blue")
        );
        assert!(headers["Authorization"].contains("/sagemaker/aws4_request"));
    }
    #[test]
    fn endpoint_name_cannot_escape_route() {
        assert!(
            SageMakerProvider::new(SageMakerConfig {
                endpoint_name: "../evil".to_string(),
                ..config()
            })
            .is_err()
        );
    }
    #[test]
    fn custom_endpoint_rejects_userinfo_query_and_fragment() {
        for endpoint in [
            "https://user:password@sagemaker.example.com",
            "https://sagemaker.example.com?tenant=other",
            "https://sagemaker.example.com#fragment",
        ] {
            assert!(
                SageMakerProvider::new(SageMakerConfig {
                    base_url: Some(endpoint.to_string()),
                    ..config()
                })
                .is_err(),
                "custom endpoint must reject {endpoint}"
            );
        }
    }
}
