//! Native Voyage AI embedding provider.

use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient, HttpErrorMapper};
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
use crate::core::types::health::HealthStatus;
use crate::core::types::model::{ModelInfo, ProviderCapability};
use crate::core::types::responses::{ChatResponse, EmbeddingData, EmbeddingResponse, Usage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
#[path = "voyage/tests.rs"]
mod tests;

const DEFAULT_API_BASE: &str = "https://api.voyageai.com/v1";
const VOYAGE_CAPABILITIES: &[ProviderCapability] =
    &[ProviderCapability::Embeddings, ProviderCapability::Rerank];

#[derive(Clone)]
struct VoyageModelSpec {
    id: &'static str,
    context: u32,
    capability: ProviderCapability,
    flexible_dimensions: bool,
}

const VOYAGE_MODELS: &[VoyageModelSpec] = &[
    embedding_model("voyage-4-large", 32_000, true),
    embedding_model("voyage-4", 32_000, true),
    embedding_model("voyage-4-lite", 32_000, true),
    embedding_model("voyage-code-4", 32_000, true),
    embedding_model("voyage-3-large", 32_000, true),
    embedding_model("voyage-3.5", 32_000, true),
    embedding_model("voyage-3.5-lite", 32_000, true),
    embedding_model("voyage-3", 32_000, false),
    embedding_model("voyage-3-lite", 32_000, false),
    embedding_model("voyage-code-3", 32_000, true),
    embedding_model("voyage-finance-2", 32_000, false),
    embedding_model("voyage-law-2", 16_000, false),
    embedding_model("voyage-multilingual-2", 32_000, false),
    embedding_model("voyage-large-2-instruct", 16_000, false),
    embedding_model("voyage-large-2", 16_000, false),
    embedding_model("voyage-2", 4_000, false),
    rerank_model("rerank-2.5", 32_000),
    rerank_model("rerank-2.5-lite", 32_000),
    rerank_model("rerank-2", 16_000),
    rerank_model("rerank-2-lite", 8_000),
    rerank_model("rerank-1", 8_000),
    rerank_model("rerank-lite-1", 4_000),
];

const fn embedding_model(
    id: &'static str,
    context: u32,
    flexible_dimensions: bool,
) -> VoyageModelSpec {
    VoyageModelSpec {
        id,
        context,
        capability: ProviderCapability::Embeddings,
        flexible_dimensions,
    }
}

const fn rerank_model(id: &'static str, context: u32) -> VoyageModelSpec {
    VoyageModelSpec {
        id,
        context,
        capability: ProviderCapability::Rerank,
        flexible_dimensions: false,
    }
}

fn model_spec(model: &str) -> Option<&'static VoyageModelSpec> {
    VOYAGE_MODELS.iter().find(|spec| spec.id == model)
}

#[derive(Debug, Clone)]
pub struct VoyageProvider {
    api_key: String,
    api_base: String,
    client: BaseHttpClient,
    models: Vec<ModelInfo>,
}

impl VoyageProvider {
    pub fn new(
        api_key: String,
        api_base: Option<&str>,
        endpoint_access: ProviderEndpointAccess,
        timeout: u64,
        max_retries: u32,
        configured_models: &[String],
    ) -> Result<Self, ProviderError> {
        if api_key.trim().is_empty() {
            return Err(ProviderError::configuration(
                "voyage",
                "api_key is required",
            ));
        }
        let api_base = api_base
            .unwrap_or(DEFAULT_API_BASE)
            .trim_end_matches('/')
            .to_string();
        let client = BaseHttpClient::new_for_provider(
            "voyage",
            BaseConfig {
                api_key: Some(api_key.clone()),
                api_base: Some(api_base.clone()),
                endpoint_access,
                timeout,
                max_retries,
                ..BaseConfig::default()
            },
        )?;
        let selected_specs = if configured_models.is_empty() {
            VOYAGE_MODELS.iter().collect::<Vec<_>>()
        } else {
            configured_models
                .iter()
                .map(|model| {
                    model_spec(model).ok_or_else(|| {
                        ProviderError::model_not_found(
                            "voyage",
                            format!("Unknown Voyage model '{model}'"),
                        )
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let models = selected_specs
            .into_iter()
            .map(|spec| ModelInfo {
                id: spec.id.to_string(),
                name: spec.id.to_string(),
                provider: "voyage".to_string(),
                max_context_length: spec.context,
                capabilities: vec![spec.capability.clone()],
                metadata: HashMap::from([(
                    "supports_flexible_dimensions".to_string(),
                    Value::Bool(spec.flexible_dimensions),
                )]),
                ..ModelInfo::default()
            })
            .collect();
        Ok(Self {
            api_key,
            api_base,
            client,
            models,
        })
    }

    pub(crate) fn supports_capability_for_model(
        &self,
        model: &str,
        capability: &ProviderCapability,
    ) -> bool {
        self.models
            .iter()
            .find(|info| info.id == model)
            .is_some_and(|info| info.capabilities.contains(capability))
    }

    pub(crate) fn rerank_provider(&self) -> crate::core::rerank::VoyageRerankProvider {
        crate::core::rerank::VoyageRerankProvider::from_transport(
            self.api_key.clone(),
            self.api_base.clone(),
            self.client.clone(),
        )
    }

    fn embedding_request_body(
        request: &EmbeddingRequest,
    ) -> Result<VoyageEmbeddingRequest<'_>, ProviderError> {
        let spec = model_spec(&request.model).ok_or_else(|| {
            ProviderError::model_not_found(
                "voyage",
                format!("Unknown Voyage model '{}'", request.model),
            )
        })?;
        if spec.capability != ProviderCapability::Embeddings {
            return Err(ProviderError::not_supported(
                "voyage",
                format!("model '{}' does not support embeddings", request.model),
            ));
        }
        let input_type = request
            .task_type
            .as_deref()
            .map(VoyageInputType::try_from)
            .transpose()?;
        if let Some(dimensions) = request.dimensions {
            if !spec.flexible_dimensions {
                return Err(ProviderError::invalid_request(
                    "voyage",
                    format!(
                        "model '{}' does not support output_dimension",
                        request.model
                    ),
                ));
            }
            if !matches!(dimensions, 256 | 512 | 1024 | 2048) {
                return Err(ProviderError::invalid_request(
                    "voyage",
                    "output_dimension must be one of 256, 512, 1024, or 2048",
                ));
            }
        }
        if request
            .encoding_format
            .as_deref()
            .is_some_and(|format| format != "float")
        {
            return Err(ProviderError::not_supported(
                "voyage",
                "only float embedding responses are supported",
            ));
        }
        Ok(VoyageEmbeddingRequest {
            model: &request.model,
            input: &request.input,
            input_type,
            output_dimension: request.dimensions,
            truncation: request.truncation,
        })
    }

    async fn post_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, ProviderError> {
        let response = self
            .client
            .post(format!("{}{path}", self.api_base))?
            .bearer_auth(&self.api_key)
            .json(body)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| ProviderError::network("voyage", error.to_string()))?;
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "voyage",
                status.as_u16(),
                &String::from_utf8_lossy(&bytes),
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| ProviderError::response_parsing("voyage", error.to_string()))
    }

    fn transform_embedding_response(
        response: VoyageEmbeddingResponse,
        input_count: usize,
    ) -> Result<EmbeddingResponse, ProviderError> {
        if response.data.len() != input_count {
            return Err(ProviderError::response_parsing(
                "voyage",
                format!(
                    "expected {input_count} embeddings, received {}",
                    response.data.len()
                ),
            ));
        }
        let mut indexes = HashSet::with_capacity(response.data.len());
        let mut data = response
            .data
            .into_iter()
            .map(|item| {
                if item.index >= input_count || !indexes.insert(item.index) {
                    return Err(ProviderError::response_parsing(
                        "voyage",
                        format!("invalid embedding index {}", item.index),
                    ));
                }
                Ok(EmbeddingData {
                    object: item.object,
                    embedding: item.embedding,
                    index: item.index as u32,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        data.sort_unstable_by_key(|item| item.index);
        let usage = Usage {
            prompt_tokens: response.usage.total_tokens,
            completion_tokens: 0,
            total_tokens: response.usage.total_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        };
        Ok(EmbeddingResponse {
            object: response.object,
            data,
            model: response.model,
            usage: Some(usage),
            embeddings: None,
        })
    }
}

#[derive(Debug, Serialize)]
struct VoyageEmbeddingRequest<'a> {
    model: &'a str,
    input: &'a EmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<VoyageInputType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_dimension: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncation: Option<bool>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum VoyageInputType {
    Query,
    Document,
}

impl TryFrom<&str> for VoyageInputType {
    type Error = ProviderError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "query" => Ok(Self::Query),
            "document" => Ok(Self::Document),
            _ => Err(ProviderError::invalid_request(
                "voyage",
                "input_type must be 'query' or 'document'",
            )),
        }
    }
}

#[derive(Debug, Deserialize)]
struct VoyageEmbeddingResponse {
    object: String,
    data: Vec<VoyageEmbeddingData>,
    model: String,
    usage: VoyageUsage,
}

#[derive(Debug, Deserialize)]
struct VoyageEmbeddingData {
    object: String,
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Deserialize)]
pub(crate) struct VoyageUsage {
    pub(crate) total_tokens: u32,
}

impl LLMProvider for VoyageProvider {
    fn name(&self) -> &'static str {
        "voyage"
    }

    fn error_provider_name(&self) -> &'static str {
        "voyage"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        VOYAGE_CAPABILITIES
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        match model_spec(model) {
            Some(spec)
                if spec.capability == ProviderCapability::Embeddings
                    && spec.flexible_dimensions =>
            {
                &["input_type", "dimensions", "truncation"]
            }
            Some(spec) if spec.capability == ProviderCapability::Embeddings => {
                &["input_type", "truncation"]
            }
            _ => &[],
        }
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
        Err(ProviderError::not_supported("voyage", "chat completion"))
    }

    async fn transform_response(
        &self,
        _raw_response: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported("voyage", "chat completion"))
    }

    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
    }

    async fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported("voyage", "chat completion"))
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        let input_count = request.input.iter().count();
        let body = Self::embedding_request_body(&request)?;
        let response = self.post_json("/embeddings", &body).await?;
        Self::transform_embedding_response(response, input_count)
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
            "voyage",
            "cost requires the shared runtime pricing authority",
        ))
    }
}
