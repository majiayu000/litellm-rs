use litellm_rs::core::providers::openai::{OpenAIConfig, OpenAIProvider};
use litellm_rs::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
use litellm_rs::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use litellm_rs::core::types::chat::{ChatMessage, ChatRequest};
use litellm_rs::core::types::context::RequestContext;
use litellm_rs::core::types::message::{MessageContent, MessageRole};
use litellm_rs::core::types::tools::{FunctionDefinition, Tool, ToolChoice, ToolType};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

type TestResult<T = ()> = Result<T, Box<dyn Error + Send + Sync>>;

const CHAT_RESPONSE: &str = r#"{"id":"chatcmpl-test","object":"chat.completion","created":1,"model":"gpt-4","choices":[{"index":0,"message":{"role":"assistant","content":"ok"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;

struct MockUpstream {
    api_base: String,
    body: oneshot::Receiver<Value>,
    task: JoinHandle<io::Result<()>>,
}

impl MockUpstream {
    async fn captured_body(self) -> TestResult<Value> {
        let body = self.body.await;
        self.task.await??;
        Ok(body?)
    }
}

async fn read_json_body(socket: &mut TcpStream) -> io::Result<Value> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 2048];

    loop {
        let bytes_read = socket.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "mock upstream received an incomplete HTTP request",
            ));
        }
        request.extend_from_slice(&buffer[..bytes_read]);

        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "mock upstream request is missing content-length",
                )
            })?;
        let body_start = header_end + 4;
        if request.len() < body_start + content_length {
            continue;
        }

        return serde_json::from_slice(&request[body_start..body_start + content_length])
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error));
    }
}

async fn start_mock_upstream() -> io::Result<MockUpstream> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let address = listener.local_addr()?;
    let (body_sender, body) = oneshot::channel();

    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await?;
        let request_body = read_json_body(&mut socket).await?;
        body_sender.send(request_body).map_err(|_| {
            io::Error::new(
                io::ErrorKind::BrokenPipe,
                "mock upstream request receiver was dropped",
            )
        })?;

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            CHAT_RESPONSE.len(),
            CHAT_RESPONSE
        );
        socket.write_all(response.as_bytes()).await
    });

    Ok(MockUpstream {
        api_base: format!("http://{address}/v1"),
        body,
        task,
    })
}

fn legacy_functions() -> Vec<Value> {
    vec![json!({
        "name": "get_weather",
        "description": "Get weather for a city",
        "parameters": {
            "type": "object",
            "properties": {"city": {"type": "string"}},
            "required": ["city"]
        }
    })]
}

fn modern_weather_tool() -> Tool {
    Tool {
        tool_type: ToolType::Function,
        function: FunctionDefinition {
            name: "get_forecast".to_string(),
            description: Some("Get a weather forecast".to_string()),
            parameters: Some(json!({
                "type": "object",
                "properties": {"city": {"type": "string"}}
            })),
        },
    }
}

fn legacy_function_request(function_call: Value) -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Weather?".to_string())),
            ..Default::default()
        }],
        functions: Some(legacy_functions()),
        function_call: Some(function_call),
        tools: Some(vec![modern_weather_tool()]),
        tool_choice: Some(ToolChoice::String("auto".to_string())),
        extra_params: HashMap::from([
            ("functions".to_string(), json!([{"name": "wrong"}])),
            ("function_call".to_string(), json!("none")),
        ]),
        ..Default::default()
    }
}

fn request_without_legacy_fields() -> ChatRequest {
    ChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        }],
        ..Default::default()
    }
}

async fn openai_request_body(request: ChatRequest) -> TestResult<Value> {
    let upstream = start_mock_upstream().await?;
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test-legacy-functions".to_string());
    config.base.api_base = Some(upstream.api_base.clone());
    let provider = OpenAIProvider::new(config).await?;

    LLMProvider::chat_completion(&provider, request, RequestContext::default()).await?;
    upstream.captured_body().await
}

async fn openai_like_request_body(request: ChatRequest) -> TestResult<Value> {
    let upstream = start_mock_upstream().await?;
    let config = OpenAILikeConfig::new(upstream.api_base.clone()).with_skip_api_key(true);
    let provider = OpenAILikeProvider::new(config).await?;

    LLMProvider::chat_completion(&provider, request, RequestContext::default()).await?;
    upstream.captured_body().await
}

#[tokio::test]
async fn openai_forwards_legacy_functions_and_string_function_call() -> TestResult {
    let body = openai_request_body(legacy_function_request(json!("auto"))).await?;

    assert_eq!(body["functions"], json!(legacy_functions()));
    assert_eq!(body["function_call"], json!("auto"));
    assert_eq!(body["tools"][0]["function"]["name"], "get_forecast");
    assert_eq!(body["tool_choice"], "auto");
    Ok(())
}

#[tokio::test]
async fn openai_like_forwards_legacy_functions_and_object_function_call() -> TestResult {
    let function_call = json!({"name": "get_weather"});
    let body = openai_like_request_body(legacy_function_request(function_call.clone())).await?;

    assert_eq!(body["functions"], json!(legacy_functions()));
    assert_eq!(body["function_call"], function_call);
    assert_eq!(body["tools"][0]["function"]["name"], "get_forecast");
    assert_eq!(body["tool_choice"], "auto");
    Ok(())
}

#[tokio::test]
async fn legacy_fields_remain_absent_when_not_provided() -> TestResult {
    let openai_body = openai_request_body(request_without_legacy_fields()).await?;
    let openai_like_body = openai_like_request_body(request_without_legacy_fields()).await?;

    for body in [openai_body, openai_like_body] {
        assert!(body.get("functions").is_none());
        assert!(body.get("function_call").is_none());
    }
    Ok(())
}

#[tokio::test]
async fn explicit_empty_functions_remain_present() -> TestResult {
    let mut openai_request = request_without_legacy_fields();
    openai_request.functions = Some(Vec::new());
    let mut openai_like_request = request_without_legacy_fields();
    openai_like_request.functions = Some(Vec::new());

    let openai_body = openai_request_body(openai_request).await?;
    let openai_like_body = openai_like_request_body(openai_like_request).await?;

    for body in [openai_body, openai_like_body] {
        assert_eq!(body["functions"], json!([]));
        assert!(body.get("function_call").is_none());
    }
    Ok(())
}
