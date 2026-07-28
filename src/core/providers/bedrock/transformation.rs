mod openai_invoke_response;

use serde_json::Value;

use super::get_model_config_for_model_id;
use super::model_config::{BedrockApiType, BedrockModelFamily};
use super::model_id::is_runtime_resolved_invoke_model_id;
use crate::core::providers::shared::{strict_openai_chat_usage, strict_token_count, strict_usage};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChoice, ChatResponse, FinishReason, Usage};
use crate::core::types::tools::{FunctionCall, ToolCall};
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

fn safe_f64_to_number(value: f32) -> serde_json::Number {
    let f64_val: f64 = value.into();
    if f64_val.is_finite() {
        serde_json::Number::from_f64(f64_val).unwrap_or_else(|| 0.into())
    } else {
        0.into()
    }
}

pub fn transform_chat_request(
    model: &str,
    messages: &[ChatMessage],
    max_tokens: Option<u32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    messages_to_prompt: impl Fn(&[ChatMessage]) -> Result<String, ProviderError>,
) -> Result<Value, ProviderError> {
    let model_config = get_model_config_for_model_id(model)?;
    match model_config.family {
        BedrockModelFamily::Claude => {
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
        | BedrockModelFamily::StabilityAI => Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Model family {:?} is not supported for chat completion",
                model_config.family
            ),
        )),
    }
}

pub fn transform_chat_response(
    raw_response: &[u8],
    model: &str,
) -> Result<ChatResponse, ProviderError> {
    let response: Value = serde_json::from_slice(raw_response)
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

    if is_runtime_resolved_invoke_model_id(model) {
        return Ok(ChatResponse {
            id: format!("bedrock-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: parse_runtime_invoke_response(&response),
            usage: parse_runtime_invoke_usage(&response),
            system_fingerprint: None,
        });
    }

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

    Ok(ChatResponse {
        id: format!("bedrock-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".to_string(),
        created: chrono::Utc::now().timestamp(),
        model: model.to_string(),
        choices,
        usage,
        system_fingerprint: None,
    })
}

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
    openai_invoke_response::parse_response(response)
}

fn parse_runtime_invoke_response(response: &Value) -> Vec<ChatChoice> {
    if response.get("choices").is_some() {
        return parse_openai_compatible_response(response);
    }

    let content = response
        .get("completion")
        .or_else(|| response.get("generation"))
        .or_else(|| response.get("text"))
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            response
                .get("outputs")
                .and_then(Value::as_array)
                .and_then(|outputs| outputs.first())
                .and_then(|output| output.get("text"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| {
            response
                .get("results")
                .and_then(Value::as_array)
                .and_then(|results| results.first())
                .and_then(|result| result.get("outputText"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();

    vec![create_chat_choice(content)]
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
        "model_context_window_exceeded" => FinishReason::Length,
        "stop_sequence" => FinishReason::StopSequence,
        "content_filtered" | "guardrail_intervened" => FinishReason::ContentFilter,
        "malformed_model_output" | "malformed_tool_use" => FinishReason::Refusal,
        _ => FinishReason::Stop,
    }
}

fn parse_openai_compatible_usage(response: &Value) -> Option<Usage> {
    strict_openai_chat_usage(response.get("usage")?)
}

fn parse_runtime_invoke_usage(response: &Value) -> Option<Usage> {
    if response.get("usage").is_some() {
        return parse_openai_compatible_usage(response);
    }
    if response.get("inputTextTokenCount").is_some() || response.get("results").is_some() {
        return parse_titan_usage(response);
    }
    parse_llama_invoke_usage(response)
}

fn parse_llama_invoke_usage(response: &Value) -> Option<Usage> {
    let prompt = strict_token_count(response.get("prompt_token_count"))?;
    let completion = strict_token_count(response.get("generation_token_count"))?;
    strict_usage(&[prompt], &[completion], None, None)
}

fn parse_claude_usage(response: &Value) -> Option<Usage> {
    let usage = response.get("usage")?;
    let prompt = strict_token_count(usage.get("input_tokens"))?;
    let completion = strict_token_count(usage.get("output_tokens"))?;
    strict_usage(&[prompt], &[completion], None, None)
}

fn parse_converse_usage(response: &Value) -> Option<Usage> {
    let usage = response.get("usage")?;
    let prompt = strict_token_count(usage.get("inputTokens"))?;
    let completion = strict_token_count(usage.get("outputTokens"))?;
    let total = strict_token_count(usage.get("totalTokens"))?;
    strict_usage(
        &[prompt],
        &[completion],
        Some((total, &[prompt, completion])),
        None,
    )
}

fn parse_titan_usage(response: &Value) -> Option<Usage> {
    let prompt = strict_token_count(response.get("inputTextTokenCount"))?;
    let completion = strict_token_count(
        response
            .get("results")
            .and_then(Value::as_array)?
            .first()?
            .get("tokenCount"),
    )?;
    strict_usage(&[prompt], &[completion], None, None)
}

#[cfg(test)]
mod tests {
    use super::{
        parse_claude_usage, parse_converse_usage, parse_llama_invoke_usage,
        parse_openai_compatible_usage, parse_runtime_invoke_usage, parse_titan_usage,
        transform_chat_response,
    };
    use crate::core::types::message::MessageContent;
    use crate::core::types::responses::{FinishReason, Usage};
    use serde_json::json;

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
                "outputTokens": 4,
                "totalTokens": 15
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
    fn maps_converse_context_window_stop_reason_to_length() {
        let raw_response = serde_json::json!({
            "output": {
                "message": {
                    "role": "assistant",
                    "content": [{ "text": "partial answer" }]
                }
            },
            "stopReason": "model_context_window_exceeded"
        });
        let raw_response = serde_json::to_vec(&raw_response)
            .unwrap_or_else(|err| panic!("Converse response should serialize: {err}"));

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/my-team-profile",
        )
        .unwrap_or_else(|err| panic!("Converse response should parse: {err}"));

        assert_eq!(
            response.choices[0].finish_reason,
            Some(FinishReason::Length)
        );
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

    #[test]
    fn parses_native_response_for_runtime_resolved_invoke_arn() {
        let raw_response = serde_json::json!({
            "completion": "hello from native invoke"
        });
        let raw_response = serde_json::to_vec(&raw_response)
            .unwrap_or_else(|err| panic!("native response should serialize: {err}"));

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:imported-model/ABC123",
        )
        .unwrap_or_else(|err| panic!("native response should parse: {err}"));

        let content = response.choices[0]
            .message
            .content
            .as_ref()
            .unwrap_or_else(|| panic!("response should include assistant content"));
        assert!(
            matches!(content, MessageContent::Text(text) if text == "hello from native invoke")
        );
    }

    #[test]
    fn parses_bedrock_completion_usage_for_runtime_resolved_invoke_arn() {
        let raw_response = serde_json::json!({
            "completion": "hello with usage",
            "prompt_token_count": 13,
            "generation_token_count": 8
        });
        let raw_response = serde_json::to_vec(&raw_response)
            .unwrap_or_else(|err| panic!("native response should serialize: {err}"));

        let response = transform_chat_response(
            &raw_response,
            "arn:aws:bedrock:us-east-1:123456789012:unknown-resource/ABC123",
        )
        .unwrap_or_else(|err| panic!("native response should parse: {err}"));

        let usage = response
            .usage
            .unwrap_or_else(|| panic!("response should include BedrockCompletion usage"));
        assert_eq!(usage.prompt_tokens, 13);
        assert_eq!(usage.completion_tokens, 8);
        assert_eq!(usage.total_tokens, 21);
    }

    #[test]
    fn usage_parsers_fail_closed_without_alias_splicing() {
        let rejected = [
            parse_openai_compatible_usage(
                &json!({"usage": {"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 4}}),
            ),
            parse_openai_compatible_usage(
                &json!({"usage": {"prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0}}),
            ),
            parse_openai_compatible_usage(
                &json!({"usage": {"prompt_tokens": "2", "completion_tokens": 1, "total_tokens": 3}}),
            ),
            parse_llama_invoke_usage(
                &json!({"prompt_token_count": 0, "generation_token_count": 0}),
            ),
            parse_llama_invoke_usage(
                &json!({"prompt_token_count": 2, "generation_token_count": []}),
            ),
            parse_claude_usage(&json!({"usage": {"input_tokens": 2}})),
            parse_claude_usage(&json!({"usage": {"input_tokens": 0, "output_tokens": 0}})),
            parse_claude_usage(&json!({"usage": {"input_tokens": {}, "output_tokens": 1}})),
            parse_converse_usage(&json!({"usage": {"inputTokens": 2, "outputTokens": 1}})),
            parse_converse_usage(
                &json!({"usage": {"inputTokens": 2, "outputTokens": 1, "totalTokens": 4}}),
            ),
            parse_converse_usage(
                &json!({"usage": {"inputTokens": 0, "outputTokens": 0, "totalTokens": 0}}),
            ),
            parse_converse_usage(
                &json!({"usage": {"inputTokens": 2, "outputTokens": 1.0, "totalTokens": 3}}),
            ),
            parse_titan_usage(&json!({"inputTextTokenCount": 2, "results": []})),
            parse_titan_usage(&json!({"inputTextTokenCount": 0, "results": [{"tokenCount": 0}]})),
            parse_titan_usage(
                &json!({"inputTextTokenCount": null, "results": [{"tokenCount": 1}]}),
            ),
            parse_runtime_invoke_usage(
                &json!({"inputTextTokenCount": 2, "generation_token_count": 1}),
            ),
            parse_runtime_invoke_usage(
                &json!({"inputTextTokenCount": 2, "prompt_token_count": 2, "generation_token_count": 1}),
            ),
            parse_runtime_invoke_usage(&json!({
                "usage": {"prompt_tokens": "2", "completion_tokens": 1, "total_tokens": 3},
                "prompt_token_count": 2, "generation_token_count": 1
            })),
        ];
        assert!(rejected.into_iter().all(|usage| usage.is_none()));
    }

    #[test]
    fn usage_parsers_saturate_after_raw_total_validation() {
        let titan = parse_runtime_invoke_usage(
            &json!({"inputTextTokenCount": 2, "results": [{"tokenCount": 1}]}),
        )
        .unwrap();
        assert_eq!(titan.prompt_tokens, 2);
        assert_eq!(titan.completion_tokens, 1);
        assert_eq!(titan.total_tokens, 3);
        let usages: [Usage; 5] = [
            parse_openai_compatible_usage(&json!({"usage": {
                "prompt_tokens": u64::MAX, "completion_tokens": 0, "total_tokens": u64::MAX
            }}))
            .unwrap(),
            parse_llama_invoke_usage(
                &json!({"prompt_token_count": u64::MAX, "generation_token_count": 0}),
            )
            .unwrap(),
            parse_claude_usage(&json!({"usage": {"input_tokens": u64::MAX, "output_tokens": 0}}))
                .unwrap(),
            parse_converse_usage(&json!({"usage": {
                "inputTokens": u64::MAX, "outputTokens": 0, "totalTokens": u64::MAX
            }}))
            .unwrap(),
            parse_titan_usage(
                &json!({"inputTextTokenCount": u64::MAX, "results": [{"tokenCount": 1}]}),
            )
            .unwrap(),
        ];
        for usage in usages {
            assert_eq!(usage.total_tokens, u32::MAX);
        }
    }
}
