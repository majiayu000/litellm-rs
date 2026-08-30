use reqwest::Method;

use super::{
    AzureAIChatHandler, AzureAIEmbeddingHandler, AzureAIImageHandler, AzureAIProvider,
    AzureAIRerankHandler, client::AzureAIClient, config::AzureAIConfig,
};
use crate::core::net::ProviderEndpointAccess;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    tools::{FunctionDefinition, Tool, ToolChoice, ToolType},
};
use std::collections::HashMap;

#[test]
fn mapped_rerank_deployment_exposes_no_chat_parameters() {
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    let catalog =
        crate::core::providers::registry::model_catalog_authority::CatalogAuthority::from_embedded(
        )
        .expect("embedded catalog should load");
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("azure_ai/cohere-rerank-v3.5".to_string()),
        None,
    );
    let identity = crate::core::providers::model_identity::validate_deployment_identity(
        "review-azure-ai",
        "azure_ai",
        "wire-rerank",
        Some(&mapping),
        None,
        &catalog,
        &pricing.snapshot(),
    )
    .expect("Azure AI rerank capability identity should validate");
    let mut provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("Azure AI provider should be created");
    provider.model_identity = Some(
        crate::core::providers::model_identity::DeploymentProviderBinding::new(identity, pricing),
    );

    assert!(
        provider
            .get_supported_openai_params("wire-rerank")
            .is_empty()
    );
}

#[test]
fn supported_parameters_follow_exact_model_capabilities() {
    let provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("policy provider should build");
    let params = provider.get_supported_openai_params("gpt-4o");

    for param in [
        "temperature",
        "max_tokens",
        "max_completion_tokens",
        "top_p",
        "frequency_penalty",
        "presence_penalty",
        "tools",
        "tool_choice",
        "stream",
    ] {
        assert!(params.contains(&param), "missing {param}");
    }

    let phi_params = provider.get_supported_openai_params("Phi-4");
    assert!(phi_params.contains(&"temperature"));
    assert!(!phi_params.contains(&"tools"));
    assert!(!phi_params.contains(&"tool_choice"));
    assert!(!phi_params.contains(&"stream"));
    assert!(
        provider
            .get_supported_openai_params("customer-chat-deployment")
            .is_empty()
    );
}

#[tokio::test]
async fn supported_openai_params_pass_through_unchanged() {
    let provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("policy provider should build");
    let params = HashMap::from([
        ("temperature".to_string(), serde_json::json!(0.7)),
        ("max_tokens".to_string(), serde_json::json!(100)),
    ]);

    let mapped = provider
        .map_openai_params(params.clone(), "gpt-4o")
        .await
        .expect("supported parameters should pass through");
    assert_eq!(mapped, params);
}

fn policy_config(endpoint: &str, access: ProviderEndpointAccess) -> AzureAIConfig {
    let mut config = AzureAIConfig::new("azure_ai");
    config.base.api_key = Some("test-key".to_string());
    config.base.api_base = Some(endpoint.to_string());
    config.base.endpoint_access = access;
    config
}

#[test]
fn public_loopback_is_rejected_by_every_native_consumer() {
    let config = policy_config("http://127.0.0.1:18080", ProviderEndpointAccess::PublicOnly);

    assert!(AzureAIChatHandler::new(config.clone()).is_err());
    assert!(AzureAIEmbeddingHandler::new(config.clone()).is_err());
    assert!(AzureAIImageHandler::new(config.clone()).is_err());
    assert!(AzureAIRerankHandler::new(config.clone()).is_err());
    assert!(AzureAIProvider::new(config).is_err());
}

#[test]
fn private_loopback_is_accepted_by_every_native_consumer() {
    let config = policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    );

    assert!(AzureAIChatHandler::new(config.clone()).is_ok());
    assert!(AzureAIEmbeddingHandler::new(config.clone()).is_ok());
    assert!(AzureAIImageHandler::new(config.clone()).is_ok());
    assert!(AzureAIRerankHandler::new(config.clone()).is_ok());
    assert!(AzureAIProvider::new(config).is_ok());
}

#[test]
fn metadata_is_rejected_even_with_private_access() {
    let config = policy_config(
        "http://169.254.169.254",
        ProviderEndpointAccess::PrivateNetwork,
    );

    assert!(AzureAIClient::new(config).is_err());
}

#[test]
fn private_client_rejects_cross_authority_requests() {
    let config = policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    );
    let client = match AzureAIClient::new(config) {
        Ok(client) => client,
        Err(error) => panic!("private policy client should initialize: {error}"),
    };

    assert!(
        client
            .request(
                Method::POST,
                "http://127.0.0.1:18080/models/chat/completions"
            )
            .is_ok()
    );
    assert!(
        client
            .streaming_request(
                Method::POST,
                "http://127.0.0.1:18080/models/chat/completions",
            )
            .is_ok()
    );
    assert!(
        client
            .request(Method::GET, "http://127.0.0.1:18081/models")
            .is_err()
    );
    assert!(
        client
            .streaming_request(
                Method::POST,
                "http://127.0.0.1:18081/models/chat/completions",
            )
            .is_err()
    );
}

#[tokio::test]
async fn phi_4_rejects_unsupported_params_at_map_and_transform_boundaries() {
    let provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("Phi-4 policy provider should build");

    for field in [
        "tools",
        "tool_choice",
        "stream",
        "max_completion_tokens",
        "unknown_field",
    ] {
        let params = HashMap::from([(field.to_string(), serde_json::json!(true))]);
        let error = provider
            .map_openai_params(params, "Phi-4")
            .await
            .expect_err("unsupported mapped parameter must fail closed");
        assert!(error.to_string().contains(field), "{error}");
    }

    let tool = Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "lookup".to_string(),
            description: None,
            parameters: None,
        },
    };
    let requests = [
        (
            "tools",
            ChatRequest {
                model: "Phi-4".to_string(),
                tools: Some(vec![tool]),
                ..Default::default()
            },
        ),
        (
            "tool_choice",
            ChatRequest {
                model: "Phi-4".to_string(),
                tool_choice: Some(ToolChoice::String("auto".to_string())),
                ..Default::default()
            },
        ),
        (
            "stream",
            ChatRequest {
                model: "Phi-4".to_string(),
                stream: true,
                ..Default::default()
            },
        ),
        (
            "max_completion_tokens",
            ChatRequest {
                model: "Phi-4".to_string(),
                max_completion_tokens: Some(128),
                ..Default::default()
            },
        ),
    ];
    for (field, request) in requests {
        let error = provider
            .transform_request(request, RequestContext::default())
            .await
            .expect_err("unsupported typed parameter must fail before serialization");
        assert!(error.to_string().contains(field), "{error}");
    }
}
