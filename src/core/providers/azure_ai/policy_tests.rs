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
        Some("azure_ai/Cohere-rerank-v4.0-pro".to_string()),
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
        "stop",
        "tools",
        "tool_choice",
        "stream",
    ] {
        assert!(params.contains(&param), "missing {param}");
    }

    let phi_params = provider.get_supported_openai_params("Phi-4");
    assert!(phi_params.contains(&"temperature"));
    assert!(phi_params.contains(&"stop"));
    assert!(!phi_params.contains(&"tools"));
    assert!(!phi_params.contains(&"tool_choice"));
    assert!(!phi_params.contains(&"stream"));
    assert!(
        provider
            .get_supported_openai_params("customer-chat-deployment")
            .is_empty()
    );
}

#[test]
fn pricing_backed_chat_identity_exposes_catalog_parameters() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let catalog =
        crate::core::providers::registry::model_catalog_authority::CatalogAuthority::from_embedded(
        )
        .expect("embedded catalog should load");
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("azure_ai/Llama-3.3-70B-Instruct".to_string()),
        None,
    );
    let identity = crate::core::providers::model_identity::validate_deployment_identity(
        "review-azure-ai",
        "azure_ai",
        "wire-llama",
        Some(&mapping),
        None,
        &catalog,
        &pricing.snapshot(),
    )
    .expect("Azure AI chat capability identity should validate");
    let mut provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("Azure AI provider should be created");
    provider.model_identity = Some(
        crate::core::providers::model_identity::DeploymentProviderBinding::new(identity, pricing),
    );

    let params = provider.get_supported_openai_params("wire-llama");
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
    assert!(params.contains(&"stream"));
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

#[tokio::test]
async fn phi_4_rejects_unsupported_params_on_live_chat_paths() {
    let provider = AzureAIProvider::new(policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .expect("Phi-4 policy provider should build");
    let tool = Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "lookup".to_string(),
            description: None,
            parameters: None,
        },
    };

    let error = provider
        .chat_completion(
            ChatRequest {
                model: "Phi-4".to_string(),
                tools: Some(vec![tool]),
                ..Default::default()
            },
            RequestContext::default(),
        )
        .await
        .expect_err("live chat must reject unsupported tools before sending");
    assert!(error.to_string().contains("tools"), "{error}");

    let error = match provider
        .chat_completion_stream(
            ChatRequest {
                model: "Phi-4".to_string(),
                ..Default::default()
            },
            RequestContext::default(),
        )
        .await
    {
        Ok(_) => panic!("live streaming must reject a non-streaming model before sending"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("stream"), "{error}");
}

#[tokio::test]
async fn text_embeddings_use_text_endpoint_for_cohere_and_deployment_names() {
    use crate::core::types::embedding::{EmbeddingInput, EmbeddingRequest};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let upstream = tokio::spawn(async move {
        let mut seen = Vec::new();
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            loop {
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).await.unwrap();
                assert_ne!(count, 0);
                bytes.extend_from_slice(&buffer[..count]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&bytes[..end]);
                    let len: usize = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= end + 4 + len {
                        assert_eq!(headers.lines().next().unwrap(), "POST /embeddings HTTP/1.1");
                        let body: serde_json::Value =
                            serde_json::from_slice(&bytes[end + 4..end + 4 + len]).unwrap();
                        assert_eq!(body["input"], serde_json::json!(["text document"]));
                        let model = body["model"].as_str().unwrap().to_owned();
                        let response = serde_json::json!({"data":[{"index":0,"embedding":[0.1,0.2]}],"model":model,"usage":{"prompt_tokens":2,"total_tokens":2}}).to_string();
                        socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", response.len(), response).as_bytes()).await.unwrap();
                        seen.push(model);
                        break;
                    }
                }
            }
        }
        seen
    });
    let provider = AzureAIProvider::new(policy_config(
        &format!("http://{address}"),
        ProviderEndpointAccess::PrivateNetwork,
    ))
    .unwrap();
    for model in [
        "Cohere-embed-v3-multilingual",
        "customer-multimodal-deployment",
    ] {
        let result = provider
            .embeddings(
                EmbeddingRequest {
                    model: model.to_string(),
                    input: EmbeddingInput::Array(vec!["text document".to_string()]),
                    user: None,
                    encoding_format: None,
                    dimensions: None,
                    task_type: None,
                    truncation: None,
                },
                RequestContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.data[0].embedding, vec![0.1, 0.2]);
        assert_eq!(result.usage.unwrap().total_tokens, 2);
    }
    assert_eq!(
        upstream.await.unwrap(),
        [
            "Cohere-embed-v3-multilingual",
            "customer-multimodal-deployment"
        ]
    );
}

#[test]
fn retired_foundry_alias_cannot_reenter_via_historical_pricing() {
    use crate::core::providers::model_identity::{
        ModelIdentityMapping, validate_deployment_identity,
    };
    use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;
    let pricing = crate::core::pricing_service::PricingService::with_embedded_default().unwrap();
    let catalog = CatalogAuthority::from_embedded().unwrap();
    assert!(
        pricing
            .get_model_info_for_provider("azure_ai", "mistral-large-latest")
            .is_some()
    );
    let mapping =
        ModelIdentityMapping::new(Some("azure_ai/mistral-large-latest".to_string()), None);
    assert!(
        validate_deployment_identity(
            "old-mistral",
            "azure_ai",
            "deployment",
            Some(&mapping),
            None,
            &catalog,
            &pricing.snapshot()
        )
        .is_err()
    );
}

#[tokio::test]
async fn configured_router_keeps_current_foundry_modalities_and_rejects_old_alias() {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::router::unified::Router;
    use crate::core::types::model::ProviderCapability;
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default().unwrap(),
    );
    let config = |model: &str| ProviderConfig {
        name: "foundry-audit".to_string(),
        provider_type: "azure_ai".to_string(),
        api_key: "fixture-key".to_string(),
        base_url: Some("https://example.com".to_string()),
        models: vec![model.to_string()],
        settings: HashMap::from([(
            "model_identity_mappings".to_string(),
            serde_json::json!({model: {"capability_catalog_model": format!("azure_ai/{model}")}}),
        )]),
        ..Default::default()
    };
    for (model, capability) in [
        (
            "Cohere-embed-v3-multilingual",
            ProviderCapability::Embeddings,
        ),
        ("FLUX-1.1-pro", ProviderCapability::ImageGeneration),
    ] {
        let router =
            Router::from_gateway_config_with_pricing(&[config(model)], None, pricing.clone())
                .await
                .unwrap();
        assert!(
            router
                .select_deployment_lease_for_capability(model, &capability)
                .is_ok(),
            "{model} must route to {capability:?}"
        );
        assert!(
            router
                .select_deployment_lease_for_capability(model, &ProviderCapability::ChatCompletion)
                .is_err()
        );
    }
    // The native helper exists, but gateway rerank dispatch is not connected.
    // Neither that missing route nor a chat fallback should be advertised.
    let rerank = Router::from_gateway_config_with_pricing(
        &[config("Cohere-rerank-v4.0-pro")],
        None,
        pricing.clone(),
    )
    .await
    .unwrap();
    for capability in [
        ProviderCapability::ChatCompletion,
        ProviderCapability::Rerank,
    ] {
        assert!(
            rerank
                .select_deployment_lease_for_capability("Cohere-rerank-v4.0-pro", &capability)
                .is_err()
        );
    }
    let old =
        Router::from_gateway_config_with_pricing(&[config("mistral-large-latest")], None, pricing)
            .await;
    assert!(
        old.is_err(),
        "historical prices must not grant current routing capabilities"
    );
}
