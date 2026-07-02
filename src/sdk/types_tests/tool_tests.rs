use crate::sdk::types::*;

// ==================== ToolCall Tests ====================

#[test]
fn test_tool_call_creation() {
    let call = ToolCall {
        id: "call_123".to_string(),
        tool_type: "function".to_string(),
        function: Function {
            name: "get_weather".to_string(),
            description: None,
            parameters: serde_json::json!({}),
            arguments: Some("{\"city\": \"London\"}".to_string()),
        },
    };
    assert_eq!(call.id, "call_123");
    assert_eq!(call.tool_type, "function");
    assert_eq!(call.function.name, "get_weather");
}

#[test]
fn test_tool_call_clone() {
    let call = ToolCall {
        id: "call_456".to_string(),
        tool_type: "function".to_string(),
        function: Function {
            name: "search".to_string(),
            description: Some("Search the web".to_string()),
            parameters: serde_json::json!({"type": "object"}),
            arguments: None,
        },
    };
    let cloned = call.clone();
    assert_eq!(call.id, cloned.id);
    assert_eq!(call.function.name, cloned.function.name);
}

// ==================== Function Tests ====================

#[test]
fn test_function_creation() {
    let func = Function {
        name: "calculate".to_string(),
        description: Some("Perform calculations".to_string()),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {"type": "string"}
            }
        }),
        arguments: None,
    };
    assert_eq!(func.name, "calculate");
    assert!(func.description.is_some());
    assert!(func.arguments.is_none());
}

#[test]
fn test_function_with_arguments() {
    let func = Function {
        name: "greet".to_string(),
        description: None,
        parameters: serde_json::json!({}),
        arguments: Some("{\"name\": \"Alice\"}".to_string()),
    };
    assert_eq!(func.arguments, Some("{\"name\": \"Alice\"}".to_string()));
}

#[test]
fn test_function_clone() {
    let func = Function {
        name: "test".to_string(),
        description: Some("Test function".to_string()),
        parameters: serde_json::json!({}),
        arguments: None,
    };
    let cloned = func.clone();
    assert_eq!(func.name, cloned.name);
    assert_eq!(func.description, cloned.description);
}

// ==================== Tool Tests ====================

#[test]
fn test_tool_creation() {
    let tool = Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "get_time".to_string(),
            description: Some("Get current time".to_string()),
            parameters: serde_json::json!({}),
            arguments: None,
        },
    };
    assert_eq!(tool.tool_type, "function");
    assert_eq!(tool.function.name, "get_time");
}

#[test]
fn test_tool_clone() {
    let tool = Tool {
        tool_type: "function".to_string(),
        function: Function {
            name: "search".to_string(),
            description: None,
            parameters: serde_json::json!({}),
            arguments: None,
        },
    };
    let cloned = tool.clone();
    assert_eq!(tool.tool_type, cloned.tool_type);
    assert_eq!(tool.function.name, cloned.function.name);
}

// ==================== ToolChoice Tests ====================

#[test]
fn test_tool_choice_variants() {
    let none = ToolChoice::None;
    let auto = ToolChoice::Auto;
    let required = ToolChoice::Required;
    let func = ToolChoice::Function {
        name: "my_function".to_string(),
    };

    // Just verify they can be created
    assert!(matches!(none, ToolChoice::None));
    assert!(matches!(auto, ToolChoice::Auto));
    assert!(matches!(required, ToolChoice::Required));
    assert!(matches!(func, ToolChoice::Function { .. }));
}

#[test]
fn test_tool_choice_function() {
    let choice = ToolChoice::Function {
        name: "get_weather".to_string(),
    };
    if let ToolChoice::Function { name } = choice {
        assert_eq!(name, "get_weather");
    } else {
        panic!("Expected Function variant");
    }
}

#[test]
fn test_tool_choice_clone() {
    let choice = ToolChoice::Function {
        name: "test".to_string(),
    };
    let cloned = choice.clone();
    if let (ToolChoice::Function { name: a }, ToolChoice::Function { name: b }) = (&choice, &cloned)
    {
        assert_eq!(a, b);
    }
}
