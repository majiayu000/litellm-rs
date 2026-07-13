//! Bedrock HTTP Client
//!
//! Wrapper around base HTTP client with Bedrock-specific functionality
//! including AWS SigV4 signing and request routing.

mod target;

use reqwest::Response;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error};

use super::config::BedrockConfig;
use super::error::BedrockErrorMapper;
use super::sigv4::SigV4Signer;
use super::utils::{AwsAuth, validate_region};
use crate::core::providers::base::{BaseConfig, BaseHttpClient};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;

use self::target::{BedrockRequestTarget, BedrockService, request_target as build_request_target};

/// Bedrock HTTP client wrapper
#[derive(Debug, Clone)]
pub struct BedrockClient {
    runtime_client: Arc<BaseHttpClient>,
    control_client: Arc<BaseHttpClient>,
    agent_runtime_client: Arc<BaseHttpClient>,
    auth: AwsAuth,
    signer: SigV4Signer,
    error_mapper: BedrockErrorMapper,
}

impl BedrockClient {
    /// Create a new Bedrock client
    pub fn new(config: BedrockConfig) -> Result<Self, ProviderError> {
        // Validate region
        validate_region(&config.aws_region)?;

        // Keep the runtime and control-plane authorities isolated in private mode.
        let runtime_base = BedrockService::Runtime.base_url(&config.aws_region);
        let control_base = BedrockService::Control.base_url(&config.aws_region);
        let agent_runtime_base = BedrockService::AgentRuntime.base_url(&config.aws_region);
        let runtime_config = BaseConfig {
            api_base: Some(runtime_base),
            endpoint_access: config.endpoint_access,
            timeout: config.timeout_seconds,
            max_retries: config.max_retries,
            ..Default::default()
        };
        let control_config = BaseConfig {
            api_base: Some(control_base),
            ..runtime_config.clone()
        };
        let agent_runtime_config = BaseConfig {
            api_base: Some(agent_runtime_base),
            ..runtime_config.clone()
        };

        let runtime_client = Arc::new(BaseHttpClient::new_for_provider_no_redirect(
            "bedrock",
            runtime_config,
        )?);
        let control_client = Arc::new(BaseHttpClient::new_for_provider_no_redirect(
            "bedrock",
            control_config,
        )?);
        let agent_runtime_client = Arc::new(BaseHttpClient::new_for_provider_no_redirect(
            "bedrock",
            agent_runtime_config,
        )?);

        // Create AWS auth
        let auth = AwsAuth::new(
            config.aws_access_key_id.clone(),
            config.aws_secret_access_key.clone(),
            config.aws_session_token.clone(),
            config.aws_region.clone(),
        );

        // Validate auth
        auth.validate()?;

        // Create SigV4 signer
        let signer = SigV4Signer::new(
            config.aws_access_key_id,
            config.aws_secret_access_key,
            config.aws_session_token,
            config.aws_region,
        );

        Ok(Self {
            runtime_client,
            control_client,
            agent_runtime_client,
            auth,
            signer,
            error_mapper: BedrockErrorMapper,
        })
    }

    /// Get AWS auth reference
    pub fn auth(&self) -> &AwsAuth {
        &self.auth
    }

    fn client_for_service(&self, service: BedrockService) -> &BaseHttpClient {
        match service {
            BedrockService::Runtime => &self.runtime_client,
            BedrockService::Control => &self.control_client,
            BedrockService::AgentRuntime => &self.agent_runtime_client,
        }
    }

    fn request_target(
        &self,
        model_id: &str,
        operation: &str,
    ) -> Result<BedrockRequestTarget, ProviderError> {
        let region = &self.auth.credentials().region;
        build_request_target(region, model_id, operation)
    }

    /// Build the Bedrock API URL for a supported operation.
    pub fn build_url(&self, model_id: &str, operation: &str) -> Result<String, ProviderError> {
        self.request_target(model_id, operation)
            .map(|target| target.url)
    }

    /// Create signed headers for AWS SigV4
    pub async fn create_signed_headers(
        &self,
        url: &str,
        body: &str,
        method: &str,
    ) -> Result<reqwest::header::HeaderMap, ProviderError> {
        self.create_signed_headers_with_extra(url, body, method, HashMap::new())
            .await
    }

    async fn create_signed_headers_with_extra(
        &self,
        url: &str,
        body: &str,
        method: &str,
        headers: HashMap<String, String>,
    ) -> Result<reqwest::header::HeaderMap, ProviderError> {
        let timestamp = chrono::Utc::now();

        let signed_headers = self
            .signer
            .sign_request(method, url, &headers, body, timestamp)
            .map_err(|e| {
                ProviderError::configuration("bedrock", format!("Signing failed: {}", e))
            })?;

        // Convert to reqwest HeaderMap
        let mut header_map = reqwest::header::HeaderMap::new();
        for (key, value) in signed_headers {
            if let (Ok(header_name), Ok(header_value)) = (
                reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                reqwest::header::HeaderValue::from_str(&value),
            ) {
                header_map.insert(header_name, header_value);
            }
        }

        Ok(header_map)
    }

    /// Send a request to Bedrock API
    pub async fn send_request(
        &self,
        model_id: &str,
        operation: &str,
        body: &Value,
    ) -> Result<Response, ProviderError> {
        let target = self.request_target(model_id, operation)?;
        let url = target.url;
        let body_str = serde_json::to_string(body)
            .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;

        debug!(
            operation,
            url,
            body_bytes = body_str.len(),
            "Bedrock request prepared"
        );

        // Create signed headers
        let headers = self
            .create_signed_headers_with_extra(
                &url,
                &body_str,
                "POST",
                request_headers_for_operation(operation),
            )
            .await?;

        // Send request
        let response = self
            .client_for_service(target.service)
            .post(&url)?
            .headers(headers)
            .body(body_str)
            .send()
            .await
            .map_err(|e| self.error_mapper.map_network_error(&e))?;

        // Check for errors
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error_body_bytes = error_body.len();
            error!(status, body_bytes = error_body_bytes, "Bedrock API error");
            return Err(self.error_mapper.map_http_error(status, &error_body));
        }

        Ok(response)
    }

    /// Send a streaming request to Bedrock API
    pub async fn send_streaming_request(
        &self,
        model_id: &str,
        operation: &str,
        body: &Value,
    ) -> Result<Response, ProviderError> {
        let target = self.request_target(model_id, operation)?;
        if target.service != BedrockService::Runtime {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "streaming requests require a Bedrock runtime operation",
            ));
        }
        let url = target.url;
        let body_str = serde_json::to_string(body)
            .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;

        debug!("Bedrock streaming request to {}", url);

        // Create signed headers
        let headers = self
            .create_signed_headers_with_extra(
                &url,
                &body_str,
                "POST",
                request_headers_for_operation(operation),
            )
            .await?;

        // Send streaming request
        let response = self
            .client_for_service(target.service)
            .post(&url)?
            .headers(headers)
            .body(body_str)
            .send()
            .await
            .map_err(|e| self.error_mapper.map_network_error(&e))?;

        // Check for errors
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error_body_bytes = error_body.len();
            error!(
                status,
                body_bytes = error_body_bytes,
                "Bedrock streaming API error"
            );
            return Err(self.error_mapper.map_http_error(status, &error_body));
        }

        Ok(response)
    }

    /// Send a GET request (for operations like listing models)
    pub async fn send_get_request(&self, operation: &str) -> Result<Response, ProviderError> {
        let target = self.request_target("", operation)?;
        let url = target.url;
        let body = ""; // Empty body for GET

        debug!("Bedrock GET request to {}", url);

        // Create signed headers
        let headers = self.create_signed_headers(&url, body, "GET").await?;

        // Send GET request
        let response = self
            .client_for_service(target.service)
            .get(&url)?
            .headers(headers)
            .send()
            .await
            .map_err(|e| self.error_mapper.map_network_error(&e))?;

        // Check for errors
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            let error_body_bytes = error_body.len();
            error!(
                status,
                body_bytes = error_body_bytes,
                "Bedrock GET API error"
            );
            return Err(self.error_mapper.map_http_error(status, &error_body));
        }

        Ok(response)
    }

    /// Health check by listing foundation models
    pub async fn health_check(&self) -> Result<bool, ProviderError> {
        self.send_get_request("list-foundation-models")
            .await
            .map(|_| true)
    }
}

fn request_headers_for_operation(operation: &str) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    headers.insert("content-type".to_string(), "application/json".to_string());

    if operation == "invoke-with-response-stream" {
        headers.insert(
            "x-amzn-bedrock-accept".to_string(),
            "application/json".to_string(),
        );
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> BedrockConfig {
        BedrockConfig {
            aws_access_key_id: "AKIATEST123456789012".to_string(),
            aws_secret_access_key: "test-secret-key-1234567890".to_string(),
            ..Default::default()
        }
    }

    fn create_test_client() -> BedrockClient {
        BedrockClient::new(create_test_config()).unwrap()
    }

    fn build_test_url(client: &BedrockClient, model_id: &str, operation: &str) -> String {
        client
            .build_url(model_id, operation)
            .unwrap_or_else(|error| panic!("test URL should build: {error}"))
    }

    // ==================== Client Creation Tests ====================

    #[tokio::test]
    async fn test_client_creation() {
        let client = BedrockClient::new(create_test_config());
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.auth().credentials().region, "us-east-1");
        assert!(!client.auth().is_temporary_credentials());
    }

    #[test]
    fn test_client_creation_with_session_token() {
        let config = BedrockConfig {
            aws_session_token: Some("session-token-12345".to_string()),
            aws_region: "us-west-2".to_string(),
            timeout_seconds: 60,
            max_retries: 5,
            ..create_test_config()
        };

        let client = BedrockClient::new(config);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert!(client.auth().is_temporary_credentials());
    }

    #[test]
    fn test_invalid_region() {
        let config = BedrockConfig {
            aws_region: "invalid-region".to_string(),
            ..create_test_config()
        };

        let client = BedrockClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_empty_access_key() {
        let config = BedrockConfig {
            aws_access_key_id: "".to_string(),
            ..create_test_config()
        };

        let client = BedrockClient::new(config);
        assert!(client.is_err());
    }

    #[test]
    fn test_empty_secret_key() {
        let config = BedrockConfig {
            aws_secret_access_key: "".to_string(),
            ..create_test_config()
        };

        let client = BedrockClient::new(config);
        assert!(client.is_err());
    }

    // ==================== URL Building Tests ====================

    #[test]
    fn test_url_building() {
        let client = create_test_client();

        // Test invoke URL
        let url = build_test_url(&client, "anthropic.claude-3-opus-20240229", "invoke");
        assert_eq!(
            url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-opus-20240229/invoke"
        );

        // Test streaming URL
        let url = build_test_url(
            &client,
            "amazon.titan-text-express-v1",
            "invoke-with-response-stream",
        );
        assert_eq!(
            url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/amazon.titan-text-express-v1/invoke-with-response-stream"
        );

        // Test converse URL
        let url = build_test_url(&client, "anthropic.claude-3-sonnet-20240229", "converse");
        assert_eq!(
            url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-sonnet-20240229/converse"
        );
    }

    #[test]
    fn test_url_building_converse_stream() {
        let client = create_test_client();

        let url = build_test_url(
            &client,
            "anthropic.claude-3-haiku-20240307",
            "converse-stream",
        );
        assert_eq!(
            url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-haiku-20240307/converse-stream"
        );
    }

    #[test]
    fn test_url_building_list_foundation_models() {
        let client = create_test_client();

        let url = build_test_url(&client, "", "list-foundation-models");
        assert_eq!(
            url,
            "https://bedrock.us-east-1.amazonaws.com/foundation-models"
        );
    }

    #[test]
    fn test_url_building_custom_operation() {
        let client = create_test_client();

        let error = client
            .build_url("some-model", "custom-operation")
            .err()
            .unwrap_or_else(|| panic!("unknown operation must fail"));
        assert!(error.to_string().contains("unsupported Bedrock operation"));
    }

    #[test]
    fn test_url_building_agent_and_arn_paths() {
        let client = create_test_client();
        let agent = build_test_url(&client, "", "agents/a/agentAliases/b/sessions/c/text");
        assert!(agent.starts_with("https://bedrock-agent-runtime.us-east-1.amazonaws.com/"));

        let batch_arn = "arn:aws:bedrock:us-east-1:123:model-invocation-job/job-1";
        let batch = build_test_url(
            &client,
            "",
            &format!("model-invocation-job/{batch_arn}/stop"),
        );
        assert!(batch.contains("model-invocation-job/arn%3A"));
        assert!(batch.contains("%2Fjob-1/stop"));

        let guardrail_arn = "arn:aws:bedrock:us-east-1:123:guardrail/guard-1";
        let guardrail = build_test_url(
            &client,
            "",
            &format!("guardrail/{guardrail_arn}/version/1/apply"),
        );
        assert!(guardrail.contains("guardrail/arn%3A"));
        assert!(guardrail.contains("%2Fguard-1/version/1/apply"));
    }

    #[test]
    fn test_url_building_different_regions() {
        // us-west-2
        let config = BedrockConfig {
            aws_region: "us-west-2".to_string(),
            ..create_test_config()
        };
        let client = BedrockClient::new(config).unwrap();
        let url = build_test_url(&client, "anthropic.claude-3-opus-20240229", "invoke");
        assert!(url.contains("us-west-2"));

        // eu-west-1
        let config = BedrockConfig {
            aws_region: "eu-west-1".to_string(),
            ..create_test_config()
        };
        let client = BedrockClient::new(config).unwrap();
        let url = build_test_url(&client, "anthropic.claude-3-opus-20240229", "invoke");
        assert!(url.contains("eu-west-1"));
    }

    // ==================== Auth Access Tests ====================

    #[test]
    fn test_auth_access() {
        let client = create_test_client();

        let auth = client.auth();
        assert_eq!(auth.credentials().region, "us-east-1");
        assert_eq!(auth.credentials().access_key_id, "AKIATEST123456789012");
    }

    #[test]
    fn private_clients_reject_every_cross_service_authority() {
        let client = BedrockClient::new(BedrockConfig {
            endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
            ..create_test_config()
        })
        .unwrap_or_else(|error| panic!("private Bedrock client should build: {error}"));
        let clients = [
            &client.runtime_client,
            &client.control_client,
            &client.agent_runtime_client,
        ];
        let urls = [
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/invoke",
            "https://bedrock.us-east-1.amazonaws.com/foundation-models",
            "https://bedrock-agent-runtime.us-east-1.amazonaws.com/knowledgebases/kb/retrieve",
        ];

        for (client_index, client) in clients.into_iter().enumerate() {
            for (url_index, url) in urls.into_iter().enumerate() {
                if client_index != url_index {
                    assert!(
                        client.get(url).is_err(),
                        "client {client_index} allowed {url}"
                    );
                }
            }
        }
    }

    // ==================== Signed Headers Tests ====================

    #[tokio::test]
    async fn test_create_signed_headers() {
        let client = create_test_client();

        let headers = client
            .create_signed_headers(
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/invoke",
                r#"{"test": "body"}"#,
                "POST",
            )
            .await;

        assert!(headers.is_ok());
        let headers = headers.unwrap();

        // Should have authorization header
        assert!(headers.contains_key("authorization"));
        // Should have x-amz-date header
        assert!(headers.contains_key("x-amz-date"));
        // Should have host header
        assert!(headers.contains_key("host"));
    }

    #[tokio::test]
    async fn test_create_signed_headers_get() {
        let client = create_test_client();

        let headers = client
            .create_signed_headers(
                "https://bedrock.us-east-1.amazonaws.com/foundation-models",
                "",
                "GET",
            )
            .await;

        assert!(headers.is_ok());
    }

    #[tokio::test]
    async fn test_operation_headers_are_signed() {
        let client = create_test_client();
        let headers = client
            .create_signed_headers_with_extra(
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/invoke",
                r#"{"test":"body"}"#,
                "POST",
                request_headers_for_operation("invoke"),
            )
            .await
            .unwrap_or_else(|err| panic!("signed invoke headers should build: {err}"));

        assert_eq!(
            headers
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        let authorization = headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_else(|| panic!("authorization header should be present"));
        assert!(authorization.contains("content-type"));
    }

    #[test]
    fn test_invoke_stream_headers_include_bedrock_accept() {
        let headers = request_headers_for_operation("invoke-with-response-stream");

        assert_eq!(
            headers.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            headers.get("x-amzn-bedrock-accept"),
            Some(&"application/json".to_string())
        );
    }

    #[test]
    fn test_all_json_post_operations_include_content_type() {
        for operation in [
            "model-invocation-job",
            "agents/a/agentAliases/b/sessions/c/text",
            "knowledgebases/kb/retrieve",
            "guardrail/g/version/1/apply",
        ] {
            assert_eq!(
                request_headers_for_operation(operation).get("content-type"),
                Some(&"application/json".to_string())
            );
        }
    }

    // ==================== Clone/Debug Tests ====================

    #[test]
    fn test_client_clone() {
        let client = create_test_client();
        let cloned = client.clone();

        assert_eq!(
            client.auth().credentials().region,
            cloned.auth().credentials().region
        );
        assert_eq!(
            client.auth().credentials().access_key_id,
            cloned.auth().credentials().access_key_id
        );
    }

    #[test]
    fn test_client_debug() {
        let client = create_test_client();
        let debug_str = format!("{:?}", client);

        assert!(debug_str.contains("BedrockClient"));
    }

    // ==================== Multiple Region Tests ====================

    #[test]
    fn test_supported_regions() {
        let regions = vec![
            "us-east-1",
            "us-west-2",
            "eu-west-1",
            "eu-central-1",
            "ap-northeast-1",
            "ap-southeast-1",
        ];

        for region in regions {
            let config = BedrockConfig {
                aws_region: region.to_string(),
                ..create_test_config()
            };

            let client = BedrockClient::new(config);
            assert!(client.is_ok(), "Region {} should be supported", region);
        }
    }

    // ==================== URL Building Edge Cases ====================

    #[test]
    fn test_url_building_special_model_ids() {
        let client = create_test_client();

        // Model with version suffix
        let url = build_test_url(&client, "meta.llama3-70b-instruct-v1:0", "invoke");
        assert!(url.contains("meta.llama3-70b-instruct-v1%3A0"));

        // Model with dots
        let url = build_test_url(&client, "ai21.jamba-1-5-large-v1:0", "invoke");
        assert!(url.contains("ai21.jamba-1-5-large-v1%3A0"));
    }

    #[test]
    fn test_url_building_encodes_arn_model_ids() {
        let client = create_test_client();
        let arn = "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-3-5-sonnet-20241022-v2:0";

        let url = build_test_url(&client, arn, "invoke");

        assert_eq!(
            url,
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Ainference-profile%2Fus.anthropic.claude-3-5-sonnet-20241022-v2%3A0/invoke"
        );
        assert!(!url.contains("/inference-profile/"));
    }
}
