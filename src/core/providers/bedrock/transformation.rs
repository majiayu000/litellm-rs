//! Request and Response Transformation for Bedrock Provider
//!
//! Contains the logic for transforming requests and responses between
//! OpenAI-compatible format and Bedrock model-specific formats.

use serde_json::Value;

use super::model_config::{BedrockModelFamily, get_model_config};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::responses::{ChatChoice, ChatResponse, FinishReason, Usage};
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
    let model_config = get_model_config(model)?;

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

    // Get model configuration
    let model_config = get_model_config(model)?;

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
            // Unsupported model family
            return Err(ProviderError::invalid_request(
                "bedrock",
                format!(
                    "Model family {:?} is not supported for response parsing",
                    model_config.family
                ),
            ));
        }
    };

    // Extract usage information based on model family
    let usage = match model_config.family {
        BedrockModelFamily::Claude | BedrockModelFamily::Nova | BedrockModelFamily::Llama => {
            parse_claude_usage(&response)
        }
        BedrockModelFamily::TitanText => parse_titan_usage(&response),
        _ => None,
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

// ==================== Usage Parsing Helpers ====================

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
