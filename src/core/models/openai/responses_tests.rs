use super::*;
use crate::core::types::thinking::ThinkingUsage;

// ==================== Usage Tests ====================

#[test]
fn test_usage_default() {
    let usage = Usage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
    assert!(usage.prompt_tokens_details.is_none());
    assert!(usage.completion_tokens_details.is_none());
}

#[test]
fn test_usage_with_values() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    };

    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_usage_with_details() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: Some(PromptTokensDetails {
            cached_tokens: Some(20),
            cache_creation_tokens: None,
            cache_read_tokens: Some(20),
            audio_tokens: None,
        }),
        completion_tokens_details: Some(CompletionTokensDetails {
            reasoning_tokens: Some(10),
            audio_tokens: Some(5),
        }),
        thinking_usage: None,
    };

    assert!(usage.prompt_tokens_details.is_some());
    assert_eq!(
        usage.prompt_tokens_details.as_ref().unwrap().cached_tokens,
        Some(20)
    );
    assert_eq!(
        usage
            .completion_tokens_details
            .as_ref()
            .unwrap()
            .reasoning_tokens,
        Some(10)
    );
}

#[test]
fn test_usage_serialize() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    };

    let json = serde_json::to_string(&usage).unwrap();
    assert!(json.contains("\"prompt_tokens\":100"));
    assert!(json.contains("\"completion_tokens\":50"));
    assert!(json.contains("\"total_tokens\":150"));
    assert!(!json.contains("prompt_tokens_details"));
    assert!(!json.contains("completion_tokens_details"));
    assert!(!json.contains("thinking_usage"));
}

#[test]
fn test_usage_serialize_thinking_usage() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        prompt_tokens_details: None,
        completion_tokens_details: Some(CompletionTokensDetails {
            reasoning_tokens: Some(25),
            audio_tokens: None,
        }),
        thinking_usage: Some(ThinkingUsage::new(25).with_budget(1000)),
    };

    let json = serde_json::to_value(&usage).unwrap();
    assert_eq!(json["thinking_usage"]["thinking_tokens"], 25);
    assert_eq!(json["thinking_usage"]["budget_tokens"], 1000);
    assert_eq!(json["completion_tokens_details"]["reasoning_tokens"], 25);
}

#[test]
fn test_usage_deserialize() {
    let json = r#"{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}"#;
    let usage: Usage = serde_json::from_str(json).unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
}

// ==================== PromptTokensDetails Tests ====================

#[test]
fn test_prompt_tokens_details_full() {
    let details = PromptTokensDetails {
        cached_tokens: Some(50),
        cache_creation_tokens: Some(5),
        cache_read_tokens: Some(45),
        audio_tokens: Some(10),
    };

    assert_eq!(details.cached_tokens, Some(50));
    assert_eq!(details.cache_creation_tokens, Some(5));
    assert_eq!(details.cache_read_tokens, Some(45));
    assert_eq!(details.audio_tokens, Some(10));
}

#[test]
fn test_prompt_tokens_details_partial() {
    let details = PromptTokensDetails {
        cached_tokens: Some(30),
        cache_creation_tokens: None,
        cache_read_tokens: Some(30),
        audio_tokens: None,
    };

    assert_eq!(details.cached_tokens, Some(30));
    assert!(details.audio_tokens.is_none());
}

// ==================== CompletionTokensDetails Tests ====================

#[test]
fn test_completion_tokens_details_full() {
    let details = CompletionTokensDetails {
        reasoning_tokens: Some(25),
        audio_tokens: Some(15),
    };

    assert_eq!(details.reasoning_tokens, Some(25));
    assert_eq!(details.audio_tokens, Some(15));
}

#[test]
fn test_completion_tokens_details_partial() {
    let details = CompletionTokensDetails {
        reasoning_tokens: None,
        audio_tokens: Some(20),
    };

    assert!(details.reasoning_tokens.is_none());
    assert_eq!(details.audio_tokens, Some(20));
}

// ==================== ChatCompletionResponse Tests ====================

#[test]
fn test_chat_completion_response_creation() {
    let response = ChatCompletionResponse {
        id: "chatcmpl-123".to_string(),
        object: "chat.completion".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        system_fingerprint: Some("fp_abc123".to_string()),
        choices: vec![],
        usage: Some(Usage::default()),
    };

    assert_eq!(response.id, "chatcmpl-123");
    assert_eq!(response.object, "chat.completion");
    assert_eq!(response.model, "gpt-4");
    assert!(response.system_fingerprint.is_some());
}

#[test]
fn test_chat_completion_response_serialize() {
    let response = ChatCompletionResponse {
        id: "chatcmpl-456".to_string(),
        object: "chat.completion".to_string(),
        created: 1677652288,
        model: "gpt-3.5-turbo".to_string(),
        system_fingerprint: None,
        choices: vec![],
        usage: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("chatcmpl-456"));
    assert!(json.contains("chat.completion"));
    assert!(json.contains("gpt-3.5-turbo"));
}

// ==================== ChatChoice Tests ====================

#[test]
fn test_chat_choice_creation() {
    let choice = ChatChoice {
        index: 0,
        message: ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("Hello!".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        },
        logprobs: None,
        finish_reason: Some("stop".to_string()),
    };

    assert_eq!(choice.index, 0);
    assert_eq!(choice.finish_reason, Some("stop".to_string()));
}

#[test]
fn test_chat_choice_serialize() {
    let choice = ChatChoice {
        index: 1,
        message: ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("Test response".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        },
        logprobs: None,
        finish_reason: Some("length".to_string()),
    };

    let json = serde_json::to_string(&choice).unwrap();
    assert!(json.contains("\"index\":1"));
    assert!(json.contains("length"));
}

// ==================== Logprobs Tests ====================

#[test]
fn test_logprobs_empty() {
    let logprobs = Logprobs { content: None };
    assert!(logprobs.content.is_none());
}

#[test]
fn test_logprobs_with_content() {
    let logprobs = Logprobs {
        content: Some(vec![ContentLogprob {
            token: "hello".to_string(),
            logprob: -0.5,
            bytes: Some(vec![104, 101, 108, 108, 111]),
            top_logprobs: None,
        }]),
    };

    assert!(logprobs.content.is_some());
    assert_eq!(logprobs.content.as_ref().unwrap().len(), 1);
}

// ==================== ContentLogprob Tests ====================

#[test]
fn test_content_logprob() {
    let logprob = ContentLogprob {
        token: "world".to_string(),
        logprob: -1.2,
        bytes: Some(vec![119, 111, 114, 108, 100]),
        top_logprobs: Some(vec![TopLogprob {
            token: "world".to_string(),
            logprob: -1.2,
            bytes: None,
        }]),
    };

    assert_eq!(logprob.token, "world");
    assert!((logprob.logprob - (-1.2)).abs() < f64::EPSILON);
    assert!(logprob.top_logprobs.is_some());
}

// ==================== TopLogprob Tests ====================

#[test]
fn test_top_logprob() {
    let top = TopLogprob {
        token: "test".to_string(),
        logprob: -0.8,
        bytes: Some(vec![116, 101, 115, 116]),
    };

    assert_eq!(top.token, "test");
    assert!((top.logprob - (-0.8)).abs() < f64::EPSILON);
}

// ==================== ChatCompletionChunk Tests ====================

#[test]
fn test_chat_completion_chunk_creation() {
    let chunk = ChatCompletionChunk {
        id: "chatcmpl-stream-123".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        system_fingerprint: Some("fp_stream".to_string()),
        choices: vec![],
        usage: None,
    };

    assert_eq!(chunk.id, "chatcmpl-stream-123");
    assert_eq!(chunk.object, "chat.completion.chunk");
}

#[test]
fn test_chat_completion_chunk_with_usage() {
    let chunk = ChatCompletionChunk {
        id: "chatcmpl-final".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 1677652288,
        model: "gpt-4".to_string(),
        system_fingerprint: None,
        choices: vec![],
        usage: Some(Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }),
    };

    assert!(chunk.usage.is_some());
    assert_eq!(chunk.usage.as_ref().unwrap().total_tokens, 30);
}

// ==================== ChatChoiceDelta Tests ====================

#[test]
fn test_chat_choice_delta() {
    let delta = ChatChoiceDelta {
        index: 0,
        delta: ChatMessageDelta {
            role: Some(MessageRole::Assistant),
            content: Some("Hello".to_string()),
            function_call: None,
            tool_calls: None,
            audio: None,
        },
        logprobs: None,
        finish_reason: None,
    };

    assert_eq!(delta.index, 0);
    assert!(delta.delta.role.is_some());
    assert!(delta.finish_reason.is_none());
}

#[test]
fn test_chat_choice_delta_final() {
    let delta = ChatChoiceDelta {
        index: 0,
        delta: ChatMessageDelta {
            role: None,
            content: None,
            function_call: None,
            tool_calls: None,
            audio: None,
        },
        logprobs: None,
        finish_reason: Some("stop".to_string()),
    };

    assert_eq!(delta.finish_reason, Some("stop".to_string()));
}

// ==================== ChatMessageDelta Tests ====================

#[test]
fn test_chat_message_delta_first_chunk() {
    let delta = ChatMessageDelta {
        role: Some(MessageRole::Assistant),
        content: None,
        function_call: None,
        tool_calls: None,
        audio: None,
    };

    assert!(delta.role.is_some());
    assert!(delta.content.is_none());
}

#[test]
fn test_chat_message_delta_content_chunk() {
    let delta = ChatMessageDelta {
        role: None,
        content: Some("partial content".to_string()),
        function_call: None,
        tool_calls: None,
        audio: None,
    };

    assert!(delta.role.is_none());
    assert_eq!(delta.content, Some("partial content".to_string()));
}

// ==================== CompletionResponse Tests ====================

#[test]
fn test_completion_response_creation() {
    let response = CompletionResponse {
        id: "cmpl-123".to_string(),
        object: "text_completion".to_string(),
        created: 1677652288,
        model: "text-davinci-003".to_string(),
        choices: vec![],
        usage: Some(Usage::default()),
    };

    assert_eq!(response.id, "cmpl-123");
    assert_eq!(response.object, "text_completion");
}

#[test]
fn test_completion_response_serialize() {
    let response = CompletionResponse {
        id: "cmpl-456".to_string(),
        object: "text_completion".to_string(),
        created: 1677652288,
        model: "gpt-3.5-turbo-instruct".to_string(),
        choices: vec![CompletionChoice {
            text: "Generated text".to_string(),
            index: 0,
            logprobs: None,
            finish_reason: Some("stop".to_string()),
        }],
        usage: None,
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("Generated text"));
    assert!(json.contains("text_completion"));
}

// ==================== CompletionChoice Tests ====================

#[test]
fn test_completion_choice() {
    let choice = CompletionChoice {
        text: "Hello, world!".to_string(),
        index: 0,
        logprobs: None,
        finish_reason: Some("stop".to_string()),
    };

    assert_eq!(choice.text, "Hello, world!");
    assert_eq!(choice.index, 0);
    assert_eq!(choice.finish_reason, Some("stop".to_string()));
}

// ==================== EmbeddingResponse Tests ====================

#[test]
fn test_embedding_response_creation() {
    let response = EmbeddingResponse {
        object: "list".to_string(),
        data: vec![EmbeddingObject {
            object: "embedding".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            index: 0,
        }],
        model: "text-embedding-ada-002".to_string(),
        usage: EmbeddingUsage::default(),
    };

    assert_eq!(response.object, "list");
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].embedding.len(), 3);
}

#[test]
fn test_embedding_response_serialize() {
    let response = EmbeddingResponse {
        object: "list".to_string(),
        data: vec![],
        model: "text-embedding-3-small".to_string(),
        usage: EmbeddingUsage {
            prompt_tokens: 10,
            total_tokens: 10,
        },
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("text-embedding-3-small"));
}

// ==================== EmbeddingObject Tests ====================

#[test]
fn test_embedding_object() {
    let obj = EmbeddingObject {
        object: "embedding".to_string(),
        embedding: vec![0.5, -0.3, 0.8, 0.1],
        index: 2,
    };

    assert_eq!(obj.object, "embedding");
    assert_eq!(obj.embedding.len(), 4);
    assert_eq!(obj.index, 2);
}

// ==================== EmbeddingUsage Tests ====================

#[test]
fn test_embedding_usage_default() {
    let usage = EmbeddingUsage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
}

#[test]
fn test_embedding_usage_with_values() {
    let usage = EmbeddingUsage {
        prompt_tokens: 50,
        total_tokens: 50,
    };

    assert_eq!(usage.prompt_tokens, 50);
    assert_eq!(usage.total_tokens, 50);
}

// ==================== ImageGenerationResponse Tests ====================

#[test]
fn test_image_generation_response() {
    let response = ImageGenerationResponse {
        created: 1677652288,
        data: vec![ImageObject {
            url: Some("https://example.com/image.png".to_string()),
            b64_json: None,
        }],
    };

    assert_eq!(response.created, 1677652288);
    assert_eq!(response.data.len(), 1);
    assert!(response.data[0].url.is_some());
}

#[test]
fn test_image_generation_response_b64() {
    let response = ImageGenerationResponse {
        created: 1677652288,
        data: vec![ImageObject {
            url: None,
            b64_json: Some("base64encodeddata".to_string()),
        }],
    };

    assert!(response.data[0].url.is_none());
    assert!(response.data[0].b64_json.is_some());
}

// ==================== ImageObject Tests ====================

#[test]
fn test_image_object_url() {
    let obj = ImageObject {
        url: Some("https://cdn.example.com/img.jpg".to_string()),
        b64_json: None,
    };

    assert_eq!(obj.url, Some("https://cdn.example.com/img.jpg".to_string()));
    assert!(obj.b64_json.is_none());
}

#[test]
fn test_image_object_b64() {
    let obj = ImageObject {
        url: None,
        b64_json: Some("SGVsbG8gV29ybGQ=".to_string()),
    };

    assert!(obj.url.is_none());
    assert!(obj.b64_json.is_some());
}

// ==================== Model Tests ====================

#[test]
fn test_model_creation() {
    let model = Model {
        id: "gpt-4".to_string(),
        object: "model".to_string(),
        created: 1687882411,
        owned_by: "openai".to_string(),
    };

    assert_eq!(model.id, "gpt-4");
    assert_eq!(model.object, "model");
    assert_eq!(model.owned_by, "openai");
}

#[test]
fn test_model_serialize() {
    let model = Model {
        id: "claude-3-opus".to_string(),
        object: "model".to_string(),
        created: 1709596800,
        owned_by: "anthropic".to_string(),
    };

    let json = serde_json::to_string(&model).unwrap();
    assert!(json.contains("claude-3-opus"));
    assert!(json.contains("anthropic"));
}

// ==================== ModelListResponse Tests ====================

#[test]
fn test_model_list_response() {
    let response = ModelListResponse {
        object: "list".to_string(),
        data: vec![
            Model {
                id: "gpt-4".to_string(),
                object: "model".to_string(),
                created: 1687882411,
                owned_by: "openai".to_string(),
            },
            Model {
                id: "gpt-3.5-turbo".to_string(),
                object: "model".to_string(),
                created: 1677610602,
                owned_by: "openai".to_string(),
            },
        ],
    };

    assert_eq!(response.object, "list");
    assert_eq!(response.data.len(), 2);
}

#[test]
fn test_model_list_response_empty() {
    let response = ModelListResponse {
        object: "list".to_string(),
        data: vec![],
    };

    assert!(response.data.is_empty());
}

#[test]
fn test_model_list_response_serialize() {
    let response = ModelListResponse {
        object: "list".to_string(),
        data: vec![Model {
            id: "test-model".to_string(),
            object: "model".to_string(),
            created: 1700000000,
            owned_by: "test-org".to_string(),
        }],
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("test-model"));
    assert!(json.contains("\"object\":\"list\""));
}
