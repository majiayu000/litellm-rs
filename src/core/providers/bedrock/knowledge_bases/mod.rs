//! Bedrock Knowledge Bases Module
//!
//! Handles vector store integration and RAG workflows

use crate::core::providers::unified_provider::ProviderError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Knowledge base retrieval request
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBaseRetrievalRequest {
    pub retrieval_query: RetrievalQuery,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retrieval_configuration: Option<RetrievalConfiguration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
}

/// Retrieval query
#[derive(Debug, Serialize, Deserialize)]
pub struct RetrievalQuery {
    pub text: String,
}

/// Retrieval configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalConfiguration {
    pub vector_search_configuration: VectorSearchConfiguration,
}

/// Vector search configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VectorSearchConfiguration {
    pub number_of_results: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_search_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<RetrievalFilter>,
}

/// Retrieval filter
#[derive(Debug, Serialize, Deserialize)]
pub struct RetrievalFilter {
    pub equals: Option<Value>,
    pub not_equals: Option<Value>,
    pub greater_than: Option<Value>,
    pub greater_than_or_equals: Option<Value>,
    pub less_than: Option<Value>,
    pub less_than_or_equals: Option<Value>,
    #[serde(rename = "in")]
    pub in_values: Option<Vec<Value>>,
    pub not_in: Option<Vec<Value>>,
    pub starts_with: Option<String>,
    pub list_contains: Option<Value>,
    pub string_contains: Option<String>,
}

/// Knowledge base retrieval response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeBaseRetrievalResponse {
    pub retrieval_results: Vec<RetrievalResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_token: Option<String>,
}

/// Retrieval result
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub content: RetrievalContent,
    pub location: RetrievalLocation,
    pub score: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

/// Retrieval content
#[derive(Debug, Deserialize)]
pub struct RetrievalContent {
    pub text: String,
}

/// Retrieval location
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalLocation {
    #[serde(rename = "type")]
    pub type_: String,
    pub s3_location: Option<S3Location>,
    pub web_location: Option<WebLocation>,
    pub confluence_location: Option<ConfluenceLocation>,
    pub salesforce_location: Option<SalesforceLocation>,
    pub sharepoint_location: Option<SharepointLocation>,
}

/// S3 location
#[derive(Debug, Deserialize)]
pub struct S3Location {
    pub uri: String,
}

/// Web location
#[derive(Debug, Deserialize)]
pub struct WebLocation {
    pub url: String,
}

/// Confluence location
#[derive(Debug, Deserialize)]
pub struct ConfluenceLocation {
    pub url: String,
}

/// Salesforce location
#[derive(Debug, Deserialize)]
pub struct SalesforceLocation {
    pub url: String,
}

/// SharePoint location
#[derive(Debug, Deserialize)]
pub struct SharepointLocation {
    pub url: String,
}

/// Knowledge base client
pub struct KnowledgeBaseClient<'a> {
    client: &'a crate::core::providers::bedrock::client::BedrockClient,
}

impl<'a> KnowledgeBaseClient<'a> {
    /// Create a new knowledge base client
    pub fn new(client: &'a crate::core::providers::bedrock::client::BedrockClient) -> Self {
        Self { client }
    }

    /// Retrieve from knowledge base
    pub async fn retrieve(
        &self,
        knowledge_base_id: &str,
        query: &str,
        number_of_results: u32,
    ) -> Result<KnowledgeBaseRetrievalResponse, ProviderError> {
        let request = KnowledgeBaseRetrievalRequest {
            retrieval_query: RetrievalQuery {
                text: query.to_string(),
            },
            retrieval_configuration: Some(RetrievalConfiguration {
                vector_search_configuration: VectorSearchConfiguration {
                    number_of_results,
                    override_search_type: None,
                    filter: None,
                },
            }),
            next_token: None,
        };

        let url = format!("knowledgebases/{}/retrieve", knowledge_base_id);
        let response = self
            .client
            .send_request("", &url, &serde_json::to_value(request)?)
            .await?;

        let kb_response: KnowledgeBaseRetrievalResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

        Ok(kb_response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_location_maps_wire_type_field() {
        let location =
            serde_json::from_value::<RetrievalLocation>(serde_json::json!({"type": "S3"}))
                .unwrap_or_else(|error| panic!("retrieval location should parse: {error}"));
        assert_eq!(location.type_, "S3");
    }
}
