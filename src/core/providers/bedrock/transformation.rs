//! Request and Response Transformation for Bedrock Provider
//!
//! Contains the logic for transforming requests and responses between
//! OpenAI-compatible format and Bedrock model-specific formats.

use serde_json::Value;

use super::get_model_config_for_model_id;
use super::model_config::{BedrockApiType, BedrockModelFamily};
use super::model_id::is_runtime_resolved_invoke_model_id;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChoice, ChatResponse, FinishReason, Usage};
use crate::core::types::tools::{FunctionCall, ToolCall};
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

/// Safely convert an f32 to a serde_json::Number, defaulting to 0 for NaN/Inf values
fn safe_f64_to_number(value: f32) -> serde_json::Number {
    let f64_val: f64 = value.into();
    if f64_val.is_finite() {
        serde_json::Number::from_f64(f64_val).unwrap_or_else(|| 0.into())
    } else {
        0.into()
    }
}

/// Transform a chat request to Bedrock format based on model family
pub fn transform_chat_request(
    model: &str,
    messages: &[ChatMessage],
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    messages_to_prompt: impl Fn(&[ChatMessage]) -> Result<String, ProviderError>,
) -> Result<Value, ProviderError> {
    // Get model configuration
    let model_config = get_model_config_for_model_id(model)?;

    // Route based on model family
    match model_config.family {
        BedrockModelFamily::Claude => {
            // Claude models on Bedrock use anthropic messages format
            let mut body = serde_json::json!({
                "messages": messages,
                "max_tokens": max_tokens.unwrap_or(4096),
                "anthropic_version": "bedrock-2023-05-20"
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            if let Some(top_p_val) = top_p {
                body["top_p"] = Value::Number(safe_f64_to_number(top_p_val));
            }

            Ok(body)
        }
        BedrockModelFamily::TitanText => {
            // Titan models use different format
            let prompt = messages_to_prompt(messages)?;
            let mut body = serde_json::json!({
                "inputText": prompt,
                "textGenerationConfig": {
                    "maxTokenCount": max_tokens.unwrap_or(4096),
                }
            });

            if let Some(temp) = temperature {
                body["textGenerationConfig"]["temperature"] =
                    Value::Number(safe_f64_to_number(temp));
            }

            if let Some(top_p_val) = top_p {
                body["textGenerationConfig"]["topP"] = Value::Number(safe_f64_to_number(top_p_val));
            }

            Ok(body)
        }
        BedrockModelFamily::Nova => {
            // Nova models use converse API format similar to Claude
            let mut body = serde_json::json!({
                "messages": messages,
                "max_tokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::Llama => {
            // Meta Llama models use similar format to Claude
            let mut body = serde_json::json!({
                "messages": messages,
                "max_tokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::Mistral => {
            // Mistral models use their own format
            let prompt = messages_to_prompt(messages)?;
            let mut body = serde_json::json!({
                "prompt": prompt,
                "max_tokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::AI21 => {
            // AI21 models use their own format
            let prompt = messages_to_prompt(messages)?;
            let mut body = serde_json::json!({
                "prompt": prompt,
                "maxTokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::Cohere => {
            // Cohere models use their own format
            let prompt = messages_to_prompt(messages)?;
            let mut body = serde_json::json!({
                "prompt": prompt,
                "max_tokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::DeepSeek => {
            // DeepSeek models use their own format
            let prompt = messages_to_prompt(messages)?;
            let mut body = serde_json::json!({
                "prompt": prompt,
                "max_tokens": max_tokens.unwrap_or(4096),
            });

            if let Some(temp) = temperature {
                body["temperature"] = Value::Number(safe_f64_to_number(temp));
            }

            Ok(body)
        }
        BedrockModelFamily::TitanEmbedding
        | BedrockModelFamily::TitanImage
        | BedrockModelFamily::StabilityAI => {
            // These are not chat models
            Err(ProviderError::invalid_request(
                "bedrock",
                format!(
                    "Model family {:?} is not supported for chat completion",
                    model_config.family
                ),
            ))
        }
    }
}

/// Transform a Bedrock response to ChatResponse format based on model family
pub fn transform_chat_response(
    raw_response: &[u8],
    model: &str,
) -> Result<ChatResponse, ProviderError> {
    let response: Value = serde_json::from_slice(raw_response)
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

    if is_runtime_resolved_invoke_model_id(model) {
        let mut usage = parse_openai_compatible_usage(&response);
        if let Some(ref mut usage) = usage
            && usage.total_tokens == 0
        {
            usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
        }

        return Ok(ChatResponse {
            id: format!("bedrock-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: parse_openai_compatible_response(&response),
            usage,
            system_fingerprint: None,
        });
    }

    // Get model configuration
    let model_config = get_model_config_for_model_id(model)?;

    let (choices, usage) = match model_config.api_type {
        BedrockApiType::Converse | BedrockApiType::ConverseStream => (
            parse_converse_response(&response),
            parse_converse_usage(&response),
        ),
        BedrockApiType::Invoke | BedrockApiType::InvokeStream => {
            let choices = match model_config.family {
                BedrockModelFamily::Claude => parse_claude_response(&response),
                BedrockModelFamily::TitanText => parse_titan_response(&response),
                BedrockModelFamily::Nova | BedrockModelFamily::Llama => {
                    parse_nova_llama_response(&response)
                }
                BedrockModelFamily::Mistral => parse_mistral_response(&response),
                BedrockModelFamily::AI21 => parse_ai21_response(&response),
                BedrockModelFamily::Cohere => parse_cohere_response(&response),
                BedrockModelFamily::DeepSeek => parse_deepseek_response(&response),
                _ => {
                    return Err(ProviderError::invalid_request(
                        "bedrock",
                        format!(
                            "Model family {:?} is not supported for response parsing",
                            model_config.family
                        ),
                    ));
                }
            };

            let usage = match model_config.family {
                BedrockModelFamily::Claude
                | BedrockModelFamily::Nova
                | BedrockModelFamily::Llama => parse_claude_usage(&response),
                BedrockModelFamily::TitanText => parse_titan_usage(&response),
                _ => None,
            };

            (choices, usage)
        }
    };

    let mut final_usage = usage;
    if let Some(ref mut usage) = final_usage {
        usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;
    }

    Ok(ChatResponse {
        id: format!("bedrock-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices,
        usage: final_usage,
        system_fingerprint: None,
    })
}

// ==================== Response Parsing Helpers ====================

fn create_chat_choice(content: String) -> ChatChoice {
    ChatChoice {
        index: 0,
        message: ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text(content)),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        },
        finish_reason: Some(FinishReason::Stop),
        logprobs: None,
    }
}

fn parse_claude_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_titan_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("results")
        .and_then(|r| r.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("outputText"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_nova_llama_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_mistral_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("outputs")
        .and_then(|o| o.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_ai21_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("completions")
        .and_then(|c| c.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("text"))
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_cohere_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("text")
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_deepseek_response(response: &Value) -> Vec<ChatChoice> {
    let content = response
        .get("completion")
        .and_then(|text| text.as_str())
        .unwrap_or("")
        .to_string();

    vec![create_chat_choice(content)]
}

fn parse_openai_compatible_response(response: &Value) -> Vec<ChatChoice> {
    let first_choice = response
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first());

    let content = first_choice
        .and_then(|choice| {
            choice
                .get("message")
                .and_then(|message| message.get("content"))
                .and_then(Value::as_str)
                .or_else(|| choice.get("text").and_then(Value::as_str))
                .or_else(|| {
                    choice
                        .get("delta")
                        .and_then(|delta| delta.get("content"))
                        .and_then(Value::as_str)
                })
        })
        .unwrap_or("")
        .to_string();

    let finish_reason = first_choice
        .and_then(|choice| choice.get("finish_reason"))
        .and_then(Value::as_str)
        .map(parse_openai_finish_reason)
        .unwrap_or(FinishReason::Stop);

    let mut choice = create_chat_choice(content);
    choice.finish_reason = Some(finish_reason);
    vec![choice]
}

fn parse_converse_response(response: &Value) -> Vec<ChatChoice> {
    let (text_parts, tool_calls) = response
        .get("output")
        .and_then(|output| output.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .map(|blocks| {
            let text_parts = blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>();
            let tool_calls = blocks
                .iter()
                .filter_map(parse_converse_tool_call)
                .collect::<Vec<_>>();
            (text_parts, tool_calls)
        })
        .unwrap_or_default();

    let content = if text_parts.is_empty() {
        None
    } else {
        Some(MessageContent::Text(text_parts.join("")))
    };
    let tool_calls = if tool_calls.is_empty() {
        None
    } else {
        Some(tool_calls)
    };

    vec![ChatChoice {
        index: 0,
        message: ChatMessage {
            role: MessageRole::Assistant,
            content,
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls,
            tool_call_id: None,
        },
        finish_reason: Some(
            response
                .get("stopReason")
                .and_then(Value::as_str)
                .map(parse_converse_finish_reason)
                .unwrap_or(FinishReason::Stop),
        ),
        logprobs: None,
    }]
}

fn parse_converse_tool_call(block: &Value) -> Option<ToolCall> {
    let tool_use = block.get("toolUse")?;
    let tool_use = tool_use.get("tool_use").unwrap_or(tool_use);
    let id = tool_use.get("toolUseId").and_then(Value::as_str)?;
    let name = tool_use.get("name").and_then(Value::as_str)?;
    let arguments = tool_use
        .get("input")
        .map(Value::to_string)
        .unwrap_or_else(|| "{}".to_string());

    Some(ToolCall {
        id: id.to_string(),
        tool_type: "function".to_string(),
        function: FunctionCall {
            name: name.to_string(),
            arguments,
        },
    })
}

fn parse_converse_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "end_turn" => FinishReason::Stop,
        "tool_use" => FinishReason::ToolCalls,
        "max_tokens" => FinishReason::Length,
        "stop_sequence" => FinishReason::StopSequence,
        "content_filtered" | "guardrail_intervened" => FinishReason::ContentFilter,
        _ => FinishReason::Stop,
    }
}

fn parse_openai_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "content_filter" => FinishReason::ContentFilter,
        "stop_sequence" => FinishReason::StopSequence,
        _ => FinishReason::Stop,
    }
}

// ==================== Usage Parsing Helpers ====================

fn parse_openai_compatible_usage(response: &Value) -> Option<Usage> {
    response.get("usage").map(|u| Usage {
        prompt_tokens: u.get("prompt_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u
            .get("completion_tokens")
            .and_then(|t| t.as_u64())
            .unwrap_or(0) as u32,
        total_tokens: u.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    })
}

fn parse_claude_usage(response: &Value) -> Option<Usage> {
    response.get("usage").map(|u| Usage {
        prompt_tokens: u.get("input_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u.get("output_tokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        total_tokens: 0, // Will be calculated by caller
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    })
}

fn parse_converse_usage(response: &Value) -> Option<Usage> {
    response.get("usage").map(|u| Usage {
        prompt_tokens: u.get("inputTokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        completion_tokens: u.get("outputTokens").and_then(|t| t.as_u64()).unwrap_or(0) as u32,
        total_tokens: 0,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    })
}

fn parse_titan_usage(response: &Value) -> Option<Usage> {
    response.get("inputTextTokenCount").and_then(|input| {
        response.get("results").and_then(|results| {
            results.as_array().and_then(|arr| {
                arr.first().and_then(|r| {
                    r.get("tokenCount").map(|output| Usage {
                        prompt_tokens: input.as_u64().unwrap_or(0) as u32,
                        completion_tokens: output.as_u64().unwrap_or(0) as u32,
                        total_tokens: 0, // Will be calculated by caller
                        prompt_tokens_details: None,
                        completion_tokens_details: None,
                        thinking_usage: None,
                    })
                })
            })
        })
    })
}

#[cfg(test)]
mod tests {
    use super::transform_chat_response;
    use crate::core::types::message::MessageContent;
    use crate::core::types::responses::FinishReason;

    #[test]
    fn parses_converse_response_for_runtime_resolved_profile_arn() {
        let raw_response = serde_json::json!({
            "output": {
                "message": {
                    "role": "assistant",
                    "content": [
                        { "text": "hello " },
                        { "text": "world" }
                    ]
                }
            },
            "usage": {
                "inputTokens": 7,
                "outputTokens": 3,
                "totalTokens": 10
            },
            "stopReason": "end_turn"
        });
        let raw_response = serde_json::to_vec(&raw_response).unwrap();

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
        )
        .unwrap();

        let content = response.choices[0].message.content.as_ref().unwrap();
        assert!(matches!(content, MessageContent::Text(text) if text == "hello world"));
        let usage = response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 7);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 10);
    }

    #[test]
    fn preserves_converse_tool_use_response_blocks() {
        let raw_response = serde_json::json!({
            "output": {
                "message": {
                    "role": "assistant",
                    "content": [
                        {
                            "toolUse": {
                                "toolUseId": "tool-123",
                                "name": "get_weather",
                                "input": { "city": "Paris", "unit": "celsius" }
                            }
                        }
                    ]
                }
            },
            "usage": {
                "inputTokens": 11,
                "outputTokens": 4
            },
            "stopReason": "tool_use"
        });
        let raw_response = serde_json::to_vec(&raw_response).unwrap();

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
        )
        .unwrap();

        let choice = &response.choices[0];
        assert_eq!(choice.finish_reason, Some(FinishReason::ToolCalls));
        assert!(choice.message.content.is_none());

        let tool_calls = choice.message.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        let tool_call = &tool_calls[0];
        assert_eq!(tool_call.id, "tool-123");
        assert_eq!(tool_call.tool_type, "function");
        assert_eq!(tool_call.function.name, "get_weather");

        let arguments: serde_json::Value =
            serde_json::from_str(&tool_call.function.arguments).unwrap();
        assert_eq!(arguments["city"], "Paris");
        assert_eq!(arguments["unit"], "celsius");
    }

    #[test]
    fn parses_openai_compatible_response_for_runtime_resolved_invoke_arn() {
        let raw_response = serde_json::json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "hello from imported"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 5,
                "completion_tokens": 3,
                "total_tokens": 8
            }
        });
        let raw_response = serde_json::to_vec(&raw_response)
            .unwrap_or_else(|err| panic!("OpenAI-compatible response should serialize: {err}"));

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:imported-model/ABC123",
        )
        .unwrap_or_else(|err| panic!("OpenAI-compatible response should parse: {err}"));

        let content = response.choices[0]
            .message
            .content
            .as_ref()
            .unwrap_or_else(|| panic!("response should include assistant content"));
        assert!(matches!(content, MessageContent::Text(text) if text == "hello from imported"));
        let usage = response
            .usage
            .unwrap_or_else(|| panic!("response should include usage"));
        assert_eq!(usage.prompt_tokens, 5);
        assert_eq!(usage.completion_tokens, 3);
        assert_eq!(usage.total_tokens, 8);
    }
}
