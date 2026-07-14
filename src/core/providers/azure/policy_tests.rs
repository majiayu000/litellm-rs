use super::assistants::{AssistantApiConfig, BaseAssistantHandler, CreateAssistantRequest};
use super::batches::{BaseBatchHandler, CreateBatchRequest};
use super::image::ImageEditRequest;
use super::{
    AzureAssistantHandler, AzureBatchHandler, AzureChatHandler, AzureConfig, AzureEmbeddingHandler,
    AzureImageHandler, ProviderError,
};
use crate::core::net::ProviderEndpointAccess;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use crate::core::types::embedding::EmbeddingRequest;
use crate::core::types::image::ImageGenerationRequest;
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

const REJECTION: &[u8] =
    b"HTTP/1.1 400 Bad Request\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}";

fn private_config(endpoint: String) -> AzureConfig {
    AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint(endpoint)
        .with_endpoint_access(ProviderEndpointAccess::PrivateNetwork)
        .with_deployment_name("deployment".to_string())
}

fn create_batch_request() -> CreateBatchRequest {
    CreateBatchRequest {
        input_file_id: "file-1".to_string(),
        endpoint: "/v1/chat/completions".to_string(),
        completion_window: "24h".to_string(),
    }
}

fn create_assistant_request() -> CreateAssistantRequest {
    CreateAssistantRequest {
        model: "gpt-4".to_string(),
        name: None,
        description: None,
        instructions: None,
    }
}

async fn rejecting_endpoint(count: usize) -> (String, tokio::task::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("listener should have an address");
    let capture = tokio::spawn(async move {
        let mut requests = Vec::with_capacity(count);
        for _ in 0..count {
            let (mut socket, _) = listener.accept().await.expect("request should connect");
            let mut request = String::new();
            BufReader::new(&mut socket)
                .read_line(&mut request)
                .await
                .expect("request line should be readable");
            socket
                .write_all(REJECTION)
                .await
                .expect("response should be writable");
            requests.push(request);
        }
        requests
    });
    (format!("http://{address}"), capture)
}

fn policy_chat_request() -> ChatRequest {
    serde_json::from_value(json!({
        "model": "gpt-4",
        "messages": [{"role": "user", "content": "hello"}]
    }))
    .expect("chat request should deserialize")
}

fn assert_request_error<T>(result: Result<T, ProviderError>) {
    assert!(result.is_err());
}

macro_rules! assert_errors {
    ($assertion:ident; $($request:expr),+ $(,)?) => {
        $($assertion($request.await);)+
    };
}

#[tokio::test]
async fn azure_core_operation_matrix_uses_policy_client() {
    let (endpoint, capture) = rejecting_endpoint(7).await;
    let config = private_config(endpoint.clone());
    let chat = AzureChatHandler::new(config.clone()).expect("chat should build");
    let embeddings = AzureEmbeddingHandler::new(config.clone()).expect("embeddings should build");
    let request: EmbeddingRequest = serde_json::from_value(json!({
        "model": "embedding", "input": "hello"
    }))
    .expect("embedding request should deserialize");
    let images = AzureImageHandler::new(config.clone()).expect("images should build");
    let generation: ImageGenerationRequest = serde_json::from_value(json!({
        "prompt": "sunset", "model": "dall-e-3", "n": 1, "size": "1024x1024"
    }))
    .expect("image request should deserialize");
    let edit = ImageEditRequest {
        model: "dall-e-2".to_string(),
        image: reqwest::multipart::Part::bytes(vec![1, 2, 3]).file_name("image.png"),
        mask: None,
        prompt: "edit".to_string(),
        n: Some(1),
        size: Some("1024x1024".to_string()),
    };
    let batch = AzureBatchHandler::new(config.clone()).expect("batch should build");
    let assistant = AzureAssistantHandler::new(config).expect("assistant should build");
    let assistant_api = AssistantApiConfig::new(None, Some(endpoint.as_str()), None);
    let requests = tokio::time::timeout(Duration::from_secs(5), async {
        assert_errors!(assert_request_error;
            chat.create_chat_completion(policy_chat_request(), RequestContext::default()),
            chat.create_chat_completion_stream(policy_chat_request(), RequestContext::default()),
            embeddings.create_embeddings(request, RequestContext::default()),
            images.generate_image(generation, RequestContext::default()),
            images.edit_image(edit, RequestContext::default()),
            batch.create_batch(create_batch_request(), None, Some(endpoint.as_str()), None),
            assistant.create_assistant(create_assistant_request(), &assistant_api),
        );
        capture.await.expect("capture task should finish")
    })
    .await
    .expect("policy operation matrix should finish within five seconds");
    let paths = [
        "/openai/deployments/deployment/chat/completions",
        "/openai/deployments/deployment/chat/completions",
        "/openai/deployments/deployment/embeddings",
        "/openai/deployments/deployment/images/generations",
        "/openai/deployments/deployment/images/edits",
        "/openai/batches",
        "/openai/assistants",
    ];
    for (request, path) in requests.iter().zip(paths) {
        assert!(request.starts_with(&format!("POST {path}")), "{request}");
    }
}

#[tokio::test]
async fn azure_streaming_uses_the_shared_header_timeout() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener
        .local_addr()
        .expect("listener should have an address");
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should connect");
        let mut request = String::new();
        BufReader::new(&mut socket)
            .read_line(&mut request)
            .await
            .expect("request line should be readable");
        tokio::time::sleep(Duration::from_millis(1_250)).await;
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .expect("response headers should be writable");
    });
    let mut config = private_config(format!("http://{address}"));
    config.timeout = 1;
    let chat = AzureChatHandler::new(config).expect("chat should build");

    drop(
        tokio::time::timeout(
            Duration::from_secs(3),
            chat.create_chat_completion_stream(policy_chat_request(), RequestContext::default()),
        )
        .await
        .expect("shared streaming header timeout must exceed the ordinary provider timeout")
        .expect("delayed response headers must remain valid"),
    );
    server.await.expect("server should finish");
}

fn assert_private_endpoint_rejected<T>(result: Result<T, ProviderError>) {
    let Err(error) = result else {
        panic!("public-only Azure handler accepted a private endpoint");
    };
    assert!(error.to_string().contains("private or reserved address"));
}

#[test]
fn azure_handlers_reject_private_endpoints_under_public_policy() {
    let config = AzureConfig::new()
        .with_api_key("test-key".to_string())
        .with_azure_endpoint("http://127.0.0.1:9".to_string())
        .with_endpoint_access(ProviderEndpointAccess::PublicOnly)
        .with_deployment_name("deployment".to_string());
    assert_private_endpoint_rejected(AzureChatHandler::new(config.clone()));
    assert_private_endpoint_rejected(AzureEmbeddingHandler::new(config.clone()));
    assert_private_endpoint_rejected(AzureImageHandler::new(config.clone()));
    assert_private_endpoint_rejected(AzureBatchHandler::new(config.clone()));
    assert_private_endpoint_rejected(AzureAssistantHandler::new(config));
}

fn assert_override_rejected<T: std::fmt::Debug>(result: Result<T, ProviderError>) {
    let error = result.expect_err("mismatched api_base must fail before network access");
    assert!(error.to_string().contains("policy-bound Azure endpoint"));
}

#[tokio::test]
async fn azure_batch_and_assistant_operations_reject_mismatched_api_base() {
    let config = private_config("https://azure.example.test".to_string());
    let batch = AzureBatchHandler::new(config.clone()).expect("batch should build");
    let bad_base = Some("https://other.example.test");
    assert_errors!(assert_override_rejected;
        batch.create_batch(create_batch_request(), None, bad_base, None),
        batch.list_batches(None, None, None, bad_base, None),
        batch.retrieve_batch("batch-1", None, bad_base, None),
        batch.cancel_batch("batch-1", None, bad_base, None),
    );

    let assistant = AzureAssistantHandler::new(config).expect("assistant should build");
    let api = AssistantApiConfig::new(None, bad_base, None);
    let modify = serde_json::from_value(json!({"name": "updated"}))
        .expect("assistant update should deserialize");
    assert_errors!(assert_override_rejected;
        assistant.create_assistant(create_assistant_request(), &api),
        assistant.list_assistants(None, None, None, None, &api),
        assistant.retrieve_assistant("assistant-1", &api),
        assistant.modify_assistant("assistant-1", modify, &api),
        assistant.delete_assistant("assistant-1", &api),
    );
}
