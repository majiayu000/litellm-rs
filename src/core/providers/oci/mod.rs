//! Oracle Cloud Infrastructure Generative AI contracts.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::providers::enterprise::{EnterpriseOpenAiProvider, EnterpriseOpenAiSettings};
use crate::core::rerank::{
    RerankDocument, RerankProvider, RerankRequest, RerankResponse, RerankResult,
};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::health::HealthStatus;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{ChatChunk, ChatResponse, EmbeddingData, EmbeddingResponse};
use crate::utils::error::gateway_error::GatewayError;
use async_trait::async_trait;
use base64::Engine as _;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;

static NATIVE_CAPABILITIES: &[ProviderCapability] =
    &[ProviderCapability::Embeddings, ProviderCapability::Rerank];

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OciApiMode {
    #[default]
    OpenAiCompatible,
    Native,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum OciAuth {
    ApiKey {
        token: String,
    },
    Iam {
        tenancy_ocid: String,
        user_ocid: String,
        fingerprint: String,
        private_key_pem: String,
    },
}
impl fmt::Debug for OciAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ApiKey { .. } => f
                .debug_struct("ApiKey")
                .field("token", &"[REDACTED]")
                .finish(),
            Self::Iam {
                tenancy_ocid,
                user_ocid,
                fingerprint,
                ..
            } => f
                .debug_struct("Iam")
                .field("tenancy_ocid", tenancy_ocid)
                .field("user_ocid", user_ocid)
                .field("fingerprint", fingerprint)
                .field("private_key_pem", &"[REDACTED]")
                .finish(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OciConfig {
    pub region: String,
    pub compartment_id: Option<String>,
    pub auth: OciAuth,
    #[serde(default)]
    pub api_mode: OciApiMode,
    pub base_url: Option<String>,
    #[serde(default)]
    pub endpoint_access: ProviderEndpointAccess,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default = "default_retries")]
    pub max_retries: u32,
    #[serde(default)]
    pub models: Vec<String>,
}

impl OciConfig {
    pub fn api_base(&self) -> Result<String, ProviderError> {
        if let Some(base) = &self.base_url {
            return Ok(base.trim_end_matches('/').to_string());
        }
        let region = self.region.trim();
        if region.is_empty()
            || !region
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
        {
            return Err(ProviderError::configuration(
                "oci",
                "region contains invalid characters",
            ));
        }
        let suffix = if self.api_mode == OciApiMode::OpenAiCompatible {
            "/openai/v1"
        } else {
            "/20231130"
        };
        Ok(format!(
            "https://inference.generativeai.{region}.oci.oraclecloud.com{suffix}"
        ))
    }
    pub async fn build(self) -> Result<OciProvider, ProviderError> {
        let base = self.api_base()?;
        match (&self.api_mode, &self.auth) {
            (OciApiMode::OpenAiCompatible, OciAuth::ApiKey { token })
                if !token.trim().is_empty() =>
            {
                Ok(OciProvider::Compatible(
                    EnterpriseOpenAiProvider::new(
                        "oci",
                        EnterpriseOpenAiSettings {
                            api_base: base,
                            api_key: token.clone(),
                            model_prefix: "oci/",
                            endpoint_access: self.endpoint_access,
                            timeout: self.timeout,
                            max_retries: self.max_retries,
                            headers: Default::default(),
                            models: self.models.clone(),
                        },
                    )
                    .await?,
                ))
            }
            (OciApiMode::OpenAiCompatible, OciAuth::Iam { .. }) => Err(
                ProviderError::configuration("oci", "IAM authentication requires api_mode=native"),
            ),
            (OciApiMode::Native, OciAuth::Iam { .. }) => {
                Ok(OciProvider::Native(OciNativeProvider::new(self)?))
            }
            (OciApiMode::Native, OciAuth::ApiKey { .. }) => Err(ProviderError::configuration(
                "oci",
                "native retrieval requires IAM authentication",
            )),
            _ => Err(ProviderError::configuration(
                "oci",
                "API key token cannot be empty",
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OciProvider {
    Compatible(EnterpriseOpenAiProvider),
    Native(OciNativeProvider),
}

impl OciProvider {
    pub fn rerank_adapter(&self) -> Result<OciRerankProvider, ProviderError> {
        match self {
            Self::Native(provider) => Ok(OciRerankProvider(provider.clone())),
            Self::Compatible(_) => Err(ProviderError::not_supported("oci", "native rerank")),
        }
    }
}

impl LLMProvider for OciProvider {
    fn name(&self) -> &str {
        "oci"
    }
    fn error_provider_name(&self) -> &'static str {
        "oci"
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        match self {
            Self::Compatible(provider) => provider.capabilities(),
            Self::Native(_) => NATIVE_CAPABILITIES,
        }
    }
    fn models(&self) -> &[ModelInfo] {
        match self {
            Self::Compatible(provider) => provider.models(),
            Self::Native(provider) => &provider.models,
        }
    }
    fn supports_model(&self, model: &str) -> bool {
        match self {
            Self::Compatible(provider) => provider.supports_model(model),
            Self::Native(provider) => {
                provider.models.is_empty()
                    || provider
                        .models
                        .iter()
                        .any(|item| item.id == model || format!("oci/{}", item.id) == model)
            }
        }
    }
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Compatible(provider) => provider.chat_completion(request, context).await,
            Self::Native(_) => Err(ProviderError::not_supported("oci", "native chat")),
        }
    }
    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        match self {
            Self::Compatible(provider) => provider.chat_completion_stream(request, context).await,
            Self::Native(_) => Err(ProviderError::not_supported("oci", "native chat streaming")),
        }
    }
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        match self {
            Self::Compatible(_) => Err(ProviderError::not_supported(
                "oci",
                "OpenAI-compatible embeddings",
            )),
            Self::Native(provider) => provider.embeddings(request, context).await,
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
        Err(ProviderError::not_supported("oci", "runtime pricing"))
    }
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        match self {
            Self::Compatible(provider) => provider.get_supported_openai_params(model),
            Self::Native(_) => &[],
        }
    }
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        match self {
            Self::Compatible(provider) => provider.map_openai_params(params, model).await,
            Self::Native(_) => Ok(params),
        }
    }
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError> {
        match self {
            Self::Compatible(provider) => provider.transform_request(request, context).await,
            Self::Native(_) => Err(ProviderError::not_supported("oci", "native chat")),
        }
    }
    async fn transform_response(
        &self,
        raw: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Compatible(provider) => provider.transform_response(raw, model, request_id).await,
            Self::Native(_) => Err(ProviderError::not_supported("oci", "native chat")),
        }
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        match self {
            Self::Compatible(provider) => provider.get_error_mapper(),
            Self::Native(_) => Box::new(DefaultErrorMapper),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OciNativeProvider {
    config: OciConfig,
    base_url: String,
    client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl OciNativeProvider {
    pub fn new(config: OciConfig) -> Result<Self, ProviderError> {
        let compartment = config
            .compartment_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::configuration(
                    "oci",
                    "compartment_id is required for native retrieval",
                )
            })?;
        if !compartment.starts_with("ocid1.compartment.") {
            return Err(ProviderError::configuration(
                "oci",
                "compartment_id must be an OCI compartment OCID",
            ));
        }
        let base_url = config.api_base()?;
        let client = BaseHttpClient::new_for_provider(
            "oci",
            BaseConfig {
                api_base: Some(base_url.clone()),
                endpoint_access: config.endpoint_access,
                timeout: config.timeout,
                max_retries: config.max_retries,
                ..BaseConfig::default()
            },
        )?;
        let models = config
            .models
            .iter()
            .map(|model| ModelInfo {
                id: model.clone(),
                name: model.clone(),
                provider: "oci".to_string(),
                capabilities: NATIVE_CAPABILITIES.to_vec(),
                ..ModelInfo::default()
            })
            .collect();
        Ok(Self {
            config,
            base_url,
            client,
            models,
        })
    }
    pub fn rerank_adapter(&self) -> OciRerankProvider {
        OciRerankProvider(self.clone())
    }
    async fn post_action(&self, action: &str, body: Value) -> Result<Value, ProviderError> {
        let url = format!("{}/actions/{action}", self.base_url);
        let body = serde_json::to_string(&body)
            .map_err(|error| ProviderError::serialization("oci", error.to_string()))?;
        let headers = oci_iam_headers(&self.config.auth, "POST", &url, &body, chrono::Utc::now())?;
        let mut request = self.client.post(url)?.body(body);
        for (key, value) in headers {
            request = request.header(key, value);
        }
        let response = request.send().await.map_err(|error| {
            if error.is_timeout() {
                ProviderError::timeout("oci", "request timed out")
            } else {
                ProviderError::network("oci", "request failed")
            }
        })?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ProviderError::network("oci", "failed to read response"))?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "oci",
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderError::response_parsing("oci", error.to_string()))
    }
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let model = request
            .model
            .strip_prefix("oci/")
            .unwrap_or(&request.model)
            .to_string();
        let inputs = request.input.to_vec();
        let response = self.post_action("embedText", serde_json::json!({"compartmentId": self.config.compartment_id, "servingMode": {"servingType": "ON_DEMAND", "modelId": model}, "inputs": inputs})).await?;
        let embeddings = response
            .get("embeddings")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ProviderError::response_parsing("oci", "embedding response missing embeddings")
            })?;
        let mut data = Vec::with_capacity(embeddings.len());
        for (index, embedding) in embeddings.iter().enumerate() {
            let embedding = serde_json::from_value::<Vec<f32>>(embedding.clone())
                .map_err(|error| ProviderError::response_parsing("oci", error.to_string()))?;
            data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: index as u32,
                embedding,
            });
        }
        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model,
            usage: None,
            embeddings: None,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OciRerankProvider(OciNativeProvider);
#[async_trait]
impl RerankProvider for OciRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, GatewayError> {
        let model = request
            .model
            .strip_prefix("oci/")
            .unwrap_or(&request.model)
            .to_string();
        let response = self.0.post_action("rerankText", serde_json::json!({"compartmentId": self.0.config.compartment_id, "servingMode": {"servingType": "ON_DEMAND", "modelId": model}, "query": request.query, "documents": request.documents.iter().map(RerankDocument::get_text).collect::<Vec<_>>(), "topN": request.top_n})).await.map_err(GatewayError::Provider)?;
        let raw = response
            .get("rerankResults")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                GatewayError::Validation("OCI rerank response missing rerankResults".to_string())
            })?;
        let mut results = Vec::with_capacity(raw.len());
        for item in raw {
            let index = item
                .get("index")
                .and_then(Value::as_u64)
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| {
                    GatewayError::Validation("OCI rerank result missing index".to_string())
                })?;
            let relevance_score = item
                .get("relevanceScore")
                .and_then(Value::as_f64)
                .ok_or_else(|| {
                    GatewayError::Validation("OCI rerank result missing relevanceScore".to_string())
                })?;
            let document = if request.return_documents.unwrap_or(true) {
                Some(request.documents.get(index).cloned().ok_or_else(|| {
                    GatewayError::Validation("OCI rerank result index is out of range".to_string())
                })?)
            } else {
                None
            };
            results.push(RerankResult {
                index,
                relevance_score,
                document,
            });
        }
        let id = response
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| GatewayError::Validation("OCI rerank response missing id".to_string()))?
            .to_string();
        Ok(RerankResponse {
            id,
            results,
            model,
            usage: None,
            meta: HashMap::new(),
        })
    }
    fn provider_name(&self) -> &'static str {
        "oci"
    }
    fn supports_model(&self, model: &str) -> bool {
        self.0.models.is_empty()
            || self
                .0
                .models
                .iter()
                .any(|item| item.id == model || format!("oci/{}", item.id) == model)
    }
    fn supported_models(&self) -> Vec<&'static str> {
        Vec::new()
    }
}

fn oci_iam_headers(
    auth: &OciAuth,
    method: &str,
    url: &str,
    body: &str,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Result<HashMap<String, String>, ProviderError> {
    let OciAuth::Iam {
        tenancy_ocid,
        user_ocid,
        fingerprint,
        private_key_pem,
    } = auth
    else {
        return Err(ProviderError::configuration(
            "oci",
            "IAM credentials are required",
        ));
    };
    let parsed = reqwest::Url::parse(url).map_err(|error| {
        ProviderError::configuration("oci", format!("invalid endpoint: {error}"))
    })?;
    let mut host = parsed
        .host_str()
        .ok_or_else(|| ProviderError::configuration("oci", "endpoint is missing a host"))?
        .to_string();
    if let Some(port) = parsed.port() {
        host.push_str(&format!(":{port}"));
    }
    let target = if let Some(query) = parsed.query() {
        format!("{}?{query}", parsed.path())
    } else {
        parsed.path().to_string()
    };
    let date = timestamp.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
    let digest = base64::engine::general_purpose::STANDARD.encode(Sha256::digest(body.as_bytes()));
    let content_length = body.len().to_string();
    let canonical = format!(
        "(request-target): {} {target}\nhost: {host}\ndate: {date}\nx-content-sha256: {digest}\ncontent-type: application/json\ncontent-length: {content_length}",
        method.to_ascii_lowercase()
    );
    let key =
        jsonwebtoken::EncodingKey::from_rsa_pem(private_key_pem.as_bytes()).map_err(|_| {
            ProviderError::configuration("oci", "private_key_pem is not a valid RSA private key")
        })?;
    let signature_url =
        jsonwebtoken::crypto::sign(canonical.as_bytes(), &key, jsonwebtoken::Algorithm::RS256)
            .map_err(|_| ProviderError::configuration("oci", "failed to sign request"))?;
    let raw_signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(signature_url)
        .map_err(|_| ProviderError::configuration("oci", "failed to encode request signature"))?;
    let signature = base64::engine::general_purpose::STANDARD.encode(raw_signature);
    let authorization = format!(
        "Signature version=\"1\",keyId=\"{tenancy_ocid}/{user_ocid}/{fingerprint}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date x-content-sha256 content-type content-length\",signature=\"{signature}\""
    );
    Ok(HashMap::from([
        ("host".to_string(), host),
        ("date".to_string(), date),
        ("x-content-sha256".to_string(), digest),
        ("content-type".to_string(), "application/json".to_string()),
        ("content-length".to_string(), content_length),
        ("authorization".to_string(), authorization),
    ]))
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
    #[test]
    fn official_endpoints_are_mode_specific_and_region_is_validated() {
        let config = OciConfig {
            region: "us-chicago-1".to_string(),
            compartment_id: None,
            auth: OciAuth::ApiKey {
                token: "key".to_string(),
            },
            api_mode: OciApiMode::OpenAiCompatible,
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert_eq!(
            config.api_base().expect("valid compatible region"),
            "https://inference.generativeai.us-chicago-1.oci.oraclecloud.com/openai/v1"
        );
        assert_eq!(
            OciConfig {
                api_mode: OciApiMode::Native,
                ..config.clone()
            }
            .api_base()
            .expect("valid native region"),
            "https://inference.generativeai.us-chicago-1.oci.oraclecloud.com/20231130"
        );
        assert!(
            OciConfig {
                region: "us/evil".to_string(),
                ..config
            }
            .api_base()
            .is_err()
        );
    }
    #[tokio::test]
    async fn auth_and_mode_combinations_fail_closed() {
        let config = OciConfig {
            region: "us-chicago-1".to_string(),
            compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
            auth: OciAuth::ApiKey {
                token: "key".to_string(),
            },
            api_mode: OciApiMode::Native,
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert!(config.build().await.is_err());
    }
}
