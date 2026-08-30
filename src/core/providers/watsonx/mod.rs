//! IBM watsonx.ai native chat, embeddings, and rerank transport.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::rerank::{
    RerankDocument, RerankProvider, RerankRequest, RerankResponse, RerankResult, RerankUsage,
};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::health::HealthStatus;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{ChatResponse, EmbeddingData, EmbeddingResponse, Usage};
use crate::utils::error::gateway_error::GatewayError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

static CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::Embeddings,
    ProviderCapability::Rerank,
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatsonxConfig {
    pub access_token: String,
    pub project_id: Option<String>,
    pub space_id: Option<String>,
    pub region: String,
    #[serde(default = "default_api_version")]
    pub api_version: String,
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

impl std::fmt::Debug for WatsonxConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatsonxConfig")
            .field("access_token", &"[REDACTED]")
            .field("project_id", &self.project_id)
            .field("space_id", &self.space_id)
            .field("region", &self.region)
            .field("api_version", &self.api_version)
            .field("base_url", &self.base_url)
            .field("endpoint_access", &self.endpoint_access)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("models", &self.models)
            .finish()
    }
}

impl WatsonxConfig {
    fn validate(&self) -> Result<(), ProviderError> {
        if self.access_token.trim().is_empty() {
            return Err(ProviderError::configuration(
                "watsonx",
                "access_token is required",
            ));
        }
        if self.project_id.as_deref().is_some_and(str::is_empty)
            || self.space_id.as_deref().is_some_and(str::is_empty)
        {
            return Err(ProviderError::configuration(
                "watsonx",
                "project_id and space_id cannot be empty",
            ));
        }
        if self.project_id.is_some() == self.space_id.is_some() {
            return Err(ProviderError::configuration(
                "watsonx",
                "exactly one of project_id or space_id is required",
            ));
        }
        if self.api_version.trim().is_empty() {
            return Err(ProviderError::configuration(
                "watsonx",
                "api_version is required",
            ));
        }
        Ok(())
    }
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
                "watsonx",
                "region contains invalid characters",
            ));
        }
        Ok(format!("https://{region}.ml.cloud.ibm.com"))
    }
    fn identity(&self, body: &mut serde_json::Map<String, Value>) {
        if let Some(project) = &self.project_id {
            body.insert("project_id".to_string(), Value::String(project.clone()));
        }
        if let Some(space) = &self.space_id {
            body.insert("space_id".to_string(), Value::String(space.clone()));
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatsonxProvider {
    config: WatsonxConfig,
    base_url: String,
    client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl WatsonxProvider {
    pub fn new(config: WatsonxConfig) -> Result<Self, ProviderError> {
        config.validate()?;
        let base_url = config.api_base()?;
        let client = BaseHttpClient::new_for_provider(
            "watsonx",
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
                provider: "watsonx".to_string(),
                capabilities: CAPABILITIES.to_vec(),
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

    fn native_body(&self, model: String, mut body: serde_json::Map<String, Value>) -> Value {
        body.insert("model_id".to_string(), Value::String(model));
        self.config.identity(&mut body);
        Value::Object(body)
    }
    async fn post(&self, path: &str, body: Value) -> Result<Value, ProviderError> {
        let url = format!("{}{path}", self.base_url);
        let response = self
            .client
            .post(url)?
            .bearer_auth(&self.config.access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    ProviderError::timeout("watsonx", "request timed out")
                } else {
                    ProviderError::network("watsonx", "request failed")
                }
            })?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|_| ProviderError::network("watsonx", "failed to read response"))?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "watsonx",
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderError::response_parsing("watsonx", error.to_string()))
    }
    pub fn rerank_adapter(&self) -> WatsonxRerankProvider {
        WatsonxRerankProvider(self.clone())
    }
}

impl LLMProvider for WatsonxProvider {
    fn name(&self) -> &str {
        "watsonx"
    }
    fn error_provider_name(&self) -> &'static str {
        "watsonx"
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        CAPABILITIES
    }
    fn models(&self) -> &[ModelInfo] {
        &self.models
    }
    fn supports_model(&self, model: &str) -> bool {
        self.models.is_empty()
            || self
                .models
                .iter()
                .any(|item| item.id == model || format!("watsonx/{}", item.id) == model)
    }
    async fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        let model = request
            .model
            .strip_prefix("watsonx/")
            .unwrap_or(&request.model)
            .to_string();
        let mut body = serde_json::to_value(request)
            .map_err(|error| ProviderError::serialization("watsonx", error.to_string()))?
            .as_object()
            .cloned()
            .ok_or_else(|| {
                ProviderError::serialization("watsonx", "chat request must be an object")
            })?;
        body.remove("model");
        let mut response = self
            .post(
                &format!("/ml/v1/text/chat?version={}", self.config.api_version),
                self.native_body(model.clone(), body),
            )
            .await?;
        let object = response.as_object_mut().ok_or_else(|| {
            ProviderError::response_parsing("watsonx", "chat response must be an object")
        })?;
        let returned_model = object.remove("model_id").ok_or_else(|| {
            ProviderError::response_parsing("watsonx", "chat response missing model_id")
        })?;
        object.insert("model".to_string(), returned_model);
        object
            .entry("object".to_string())
            .or_insert(Value::String("chat.completion".to_string()));
        serde_json::from_value(response)
            .map_err(|error| ProviderError::response_parsing("watsonx", error.to_string()))
    }
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let model = request
            .model
            .strip_prefix("watsonx/")
            .unwrap_or(&request.model)
            .to_string();
        let mut body = serde_json::Map::new();
        body.insert(
            "inputs".to_string(),
            serde_json::to_value(request.input)
                .map_err(|error| ProviderError::serialization("watsonx", error.to_string()))?,
        );
        let response = self
            .post(
                &format!("/ml/v1/text/embeddings?version={}", self.config.api_version),
                self.native_body(model.clone(), body),
            )
            .await?;
        let results = response
            .get("results")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ProviderError::response_parsing("watsonx", "embedding response missing results")
            })?;
        let mut data = Vec::with_capacity(results.len());
        for (index, result) in results.iter().enumerate() {
            let embedding = serde_json::from_value::<Vec<f32>>(
                result.get("embedding").cloned().ok_or_else(|| {
                    ProviderError::response_parsing("watsonx", "embedding result missing embedding")
                })?,
            )
            .map_err(|error| ProviderError::response_parsing("watsonx", error.to_string()))?;
            data.push(EmbeddingData {
                object: "embedding".to_string(),
                index: index as u32,
                embedding,
            });
        }
        let prompt_tokens = response
            .get("input_token_count")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok());
        Ok(EmbeddingResponse {
            object: "list".to_string(),
            data,
            model,
            usage: prompt_tokens.map(|tokens| Usage::new(tokens, 0)),
            embeddings: None,
        })
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
        Err(ProviderError::not_supported("watsonx", "runtime pricing"))
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
            .map_err(|error| ProviderError::serialization("watsonx", error.to_string()))
    }
    async fn transform_response(
        &self,
        raw: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        serde_json::from_slice(raw)
            .map_err(|error| ProviderError::response_parsing("watsonx", error.to_string()))
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }
}

#[derive(Debug, Clone)]
pub struct WatsonxRerankProvider(WatsonxProvider);

#[async_trait]
impl RerankProvider for WatsonxRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, GatewayError> {
        let model = request
            .model
            .strip_prefix("watsonx/")
            .unwrap_or(&request.model)
            .to_string();
        let mut body = serde_json::Map::new();
        body.insert("query".to_string(), Value::String(request.query));
        body.insert(
            "inputs".to_string(),
            Value::Array(
                request
                    .documents
                    .iter()
                    .map(|doc| Value::String(doc.get_text().to_string()))
                    .collect(),
            ),
        );
        if let Some(top_n) = request.top_n {
            body.insert("top_n".to_string(), Value::from(top_n));
        }
        let response = self
            .0
            .post(
                &format!("/ml/v1/text/rerank?version={}", self.0.config.api_version),
                self.0.native_body(model.clone(), body),
            )
            .await
            .map_err(GatewayError::Provider)?;
        parse_rerank(
            "watsonx",
            model,
            response,
            &request.documents,
            request.return_documents.unwrap_or(true),
        )
    }
    fn provider_name(&self) -> &'static str {
        "watsonx"
    }
    fn supports_model(&self, model: &str) -> bool {
        self.0.supports_model(model)
    }
    fn supported_models(&self) -> Vec<&'static str> {
        Vec::new()
    }
}

fn parse_rerank(
    provider: &'static str,
    model: String,
    response: Value,
    documents: &[RerankDocument],
    return_documents: bool,
) -> Result<RerankResponse, GatewayError> {
    let raw = response
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            GatewayError::Validation(format!("{provider} rerank response missing results"))
        })?;
    let mut results = Vec::with_capacity(raw.len());
    for item in raw {
        let index = item
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| {
                GatewayError::Validation(format!("{provider} rerank result missing index"))
            })?;
        let relevance_score = item
            .get("score")
            .or_else(|| item.get("relevance_score"))
            .and_then(Value::as_f64)
            .ok_or_else(|| {
                GatewayError::Validation(format!("{provider} rerank result missing score"))
            })?;
        let document = if return_documents {
            Some(documents.get(index).cloned().ok_or_else(|| {
                GatewayError::Validation(format!("{provider} rerank result index is out of range"))
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
        .ok_or_else(|| GatewayError::Validation("watsonx rerank response missing id".to_string()))?
        .to_string();
    Ok(RerankResponse {
        id,
        results,
        model,
        usage: response
            .get("input_token_count")
            .and_then(Value::as_u64)
            .and_then(|v| u32::try_from(v).ok())
            .map(|tokens| RerankUsage {
                total_tokens: Some(tokens),
                ..Default::default()
            }),
        meta: HashMap::new(),
    })
}

fn default_api_version() -> String {
    "2024-05-31".to_string()
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
    fn requires_exactly_one_governance_identity() {
        let base = WatsonxConfig {
            access_token: "token".to_string(),
            project_id: None,
            space_id: None,
            region: "us-south".to_string(),
            api_version: default_api_version(),
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert!(base.validate().is_err());
        assert!(
            WatsonxConfig {
                project_id: Some("project".to_string()),
                ..base
            }
            .validate()
            .is_ok()
        );
    }
    #[test]
    fn region_builds_official_endpoint_and_rejects_injection() {
        let base = WatsonxConfig {
            access_token: "token".to_string(),
            project_id: Some("project".to_string()),
            space_id: None,
            region: "us-south".to_string(),
            api_version: default_api_version(),
            base_url: None,
            endpoint_access: ProviderEndpointAccess::PublicOnly,
            timeout: 60,
            max_retries: 2,
            models: Vec::new(),
        };
        assert_eq!(
            base.api_base().expect("valid region"),
            "https://us-south.ml.cloud.ibm.com"
        );
        assert!(
            WatsonxConfig {
                region: "us-south/evil".to_string(),
                ..base
            }
            .api_base()
            .is_err()
        );
    }
}
