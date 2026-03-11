use serde_json::{Map, Value};
use std::collections::HashMap;

use super::types::{MessageContent, RequestUtils};

impl RequestUtils {
    pub fn add_dummy_tool(provider: &str) -> Vec<Value> {
        match provider.to_lowercase().as_str() {
            "anthropic" | "claude" => {
                vec![serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": "dummy_function",
                        "description": "A dummy function for testing",
                        "parameters": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "A dummy query parameter"
                                }
                            },
                            "required": ["query"]
                        }
                    }
                })]
            }
            _ => vec![],
        }
    }

    pub fn convert_messages_to_dict(messages: &[MessageContent]) -> Vec<Map<String, Value>> {
        messages
            .iter()
            .map(|msg| {
                let mut map = Map::new();
                map.insert("role".to_string(), Value::String(msg.role.clone()));
                map.insert("content".to_string(), Value::String(msg.content.clone()));
                map
            })
            .collect()
    }

    pub fn has_tool_call_blocks(messages: &[MessageContent]) -> bool {
        messages.iter().any(|msg| {
            msg.content.contains("tool_calls")
                || msg.content.contains("function_call")
                || msg.role == "tool"
        })
    }

    pub fn get_standard_openai_params(params: &HashMap<String, Value>) -> HashMap<String, Value> {
        let standard_params = [
            "model",
            "messages",
            "temperature",
            "max_tokens",
            "top_p",
            "frequency_penalty",
            "presence_penalty",
            "stop",
            "stream",
            "tools",
            "tool_choice",
            "response_format",
        ];

        params
            .iter()
            .filter(|(key, _)| standard_params.contains(&key.as_str()))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn get_non_default_completion_params(
        params: &HashMap<String, Value>,
    ) -> HashMap<String, Value> {
        let mut non_default = HashMap::new();

        if let Some(temp) = params.get("temperature")
            && temp.as_f64() != Some(1.0)
        {
            non_default.insert("temperature".to_string(), temp.clone());
        }

        if let Some(max_tokens) = params.get("max_tokens")
            && max_tokens.as_u64().is_some()
        {
            non_default.insert("max_tokens".to_string(), max_tokens.clone());
        }

        if let Some(top_p) = params.get("top_p")
            && top_p.as_f64() != Some(1.0)
        {
            non_default.insert("top_p".to_string(), top_p.clone());
        }

        non_default
    }
}
