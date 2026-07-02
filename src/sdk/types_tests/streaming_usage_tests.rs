use crate::sdk::types::*;

// ==================== ChatChunk Tests ====================

#[test]
fn test_chat_chunk_creation() {
    let chunk = ChatChunk {
        id: "chunk_123".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![],
    };
    assert_eq!(chunk.id, "chunk_123");
    assert_eq!(chunk.model, "gpt-4");
}

#[test]
fn test_chat_chunk_with_choices() {
    let chunk = ChatChunk {
        id: "chunk_456".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![ChunkChoice {
            index: 0,
            delta: MessageDelta {
                role: Some(Role::Assistant),
                content: Some("Hello".to_string()),
                tool_calls: None,
            },
            finish_reason: None,
        }],
    };
    assert_eq!(chunk.choices.len(), 1);
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

// ==================== ChunkChoice Tests ====================

#[test]
fn test_chunk_choice_creation() {
    let choice = ChunkChoice {
        index: 0,
        delta: MessageDelta {
            role: None,
            content: Some("text".to_string()),
            tool_calls: None,
        },
        finish_reason: None,
    };
    assert_eq!(choice.index, 0);
    assert!(choice.finish_reason.is_none());
}

#[test]
fn test_chunk_choice_with_finish_reason() {
    let choice = ChunkChoice {
        index: 0,
        delta: MessageDelta {
            role: None,
            content: None,
            tool_calls: None,
        },
        finish_reason: Some("stop".to_string()),
    };
    assert_eq!(choice.finish_reason, Some("stop".to_string()));
}

// ==================== MessageDelta Tests ====================

#[test]
fn test_message_delta_creation() {
    let delta = MessageDelta {
        role: Some(Role::Assistant),
        content: Some("Hello".to_string()),
        tool_calls: None,
    };
    assert_eq!(delta.role, Some(Role::Assistant));
    assert_eq!(delta.content, Some("Hello".to_string()));
}

#[test]
fn test_message_delta_content_only() {
    let delta = MessageDelta {
        role: None,
        content: Some(" world".to_string()),
        tool_calls: None,
    };
    assert!(delta.role.is_none());
    assert_eq!(delta.content, Some(" world".to_string()));
}

#[test]
fn test_message_delta_empty() {
    let delta = MessageDelta {
        role: None,
        content: None,
        tool_calls: None,
    };
    assert!(delta.role.is_none());
    assert!(delta.content.is_none());
    assert!(delta.tool_calls.is_none());
}

// ==================== Usage Tests ====================

#[test]
fn test_usage_default() {
    let usage = Usage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
}

#[test]
fn test_usage_creation() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

#[test]
fn test_usage_clone() {
    let usage = Usage {
        prompt_tokens: 10,
        completion_tokens: 20,
        total_tokens: 30,
    };
    let cloned = usage.clone();
    assert_eq!(usage.prompt_tokens, cloned.prompt_tokens);
    assert_eq!(usage.completion_tokens, cloned.completion_tokens);
    assert_eq!(usage.total_tokens, cloned.total_tokens);
}

#[test]
fn test_usage_serialization() {
    let usage = Usage {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
    };
    let json = serde_json::to_string(&usage).unwrap();
    assert!(json.contains("\"prompt_tokens\":100"));
    assert!(json.contains("\"completion_tokens\":50"));
    assert!(json.contains("\"total_tokens\":150"));
}

#[test]
fn test_usage_deserialization() {
    let json = r#"{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}"#;
    let usage: Usage = serde_json::from_str(json).unwrap();
    assert_eq!(usage.prompt_tokens, 100);
    assert_eq!(usage.completion_tokens, 50);
    assert_eq!(usage.total_tokens, 150);
}

// ==================== Cost Tests ====================

#[test]
fn test_cost_creation() {
    let cost = Cost {
        amount: 0.05,
        currency: "USD".to_string(),
        breakdown: CostBreakdown {
            input_cost: 0.03,
            output_cost: 0.02,
            total_cost: 0.05,
        },
    };
    assert_eq!(cost.amount, 0.05);
    assert_eq!(cost.currency, "USD");
}

#[test]
fn test_cost_clone() {
    let cost = Cost {
        amount: 1.0,
        currency: "EUR".to_string(),
        breakdown: CostBreakdown {
            input_cost: 0.6,
            output_cost: 0.4,
            total_cost: 1.0,
        },
    };
    let cloned = cost.clone();
    assert_eq!(cost.amount, cloned.amount);
    assert_eq!(cost.currency, cloned.currency);
}

#[test]
fn test_cost_zero() {
    let cost = Cost {
        amount: 0.0,
        currency: "USD".to_string(),
        breakdown: CostBreakdown {
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost: 0.0,
        },
    };
    assert_eq!(cost.amount, 0.0);
    assert_eq!(cost.breakdown.total_cost, 0.0);
}

// ==================== CostBreakdown Tests ====================

#[test]
fn test_cost_breakdown_creation() {
    let breakdown = CostBreakdown {
        input_cost: 0.01,
        output_cost: 0.02,
        total_cost: 0.03,
    };
    assert_eq!(breakdown.input_cost, 0.01);
    assert_eq!(breakdown.output_cost, 0.02);
    assert_eq!(breakdown.total_cost, 0.03);
}

#[test]
fn test_cost_breakdown_clone() {
    let breakdown = CostBreakdown {
        input_cost: 0.5,
        output_cost: 0.5,
        total_cost: 1.0,
    };
    let cloned = breakdown.clone();
    assert_eq!(breakdown.input_cost, cloned.input_cost);
    assert_eq!(breakdown.output_cost, cloned.output_cost);
    assert_eq!(breakdown.total_cost, cloned.total_cost);
}

#[test]
fn test_cost_breakdown_debug() {
    let breakdown = CostBreakdown {
        input_cost: 0.1,
        output_cost: 0.2,
        total_cost: 0.3,
    };
    let debug_str = format!("{:?}", breakdown);
    assert!(debug_str.contains("input_cost"));
    assert!(debug_str.contains("output_cost"));
    assert!(debug_str.contains("total_cost"));
}
