//! MCP Tools
//!
//! Tool definitions and invocation types for MCP.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    /// Tool name
    pub name: String,

    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Input schema (JSON Schema format)
    #[serde(rename = "inputSchema")]
    pub input_schema: ToolInputSchema,
}

impl Tool {
    /// Create a new tool with minimal information
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: ToolInputSchema::default(),
        }
    }

    /// Set description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set input schema
    pub fn with_schema(mut self, schema: ToolInputSchema) -> Self {
        self.input_schema = schema;
        self
    }

    /// Convert to OpenAI function format
    pub fn to_openai_function(&self) -> Value {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema.to_json_schema()
            }
        })
    }
}

/// Tool input schema (JSON Schema subset)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolInputSchema {
    /// Schema type (usually "object")
    #[serde(rename = "type", default = "default_object_type")]
    pub schema_type: String,

    /// Properties for object type
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, PropertySchema>,

    /// Required property names
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<String>,

    /// Additional properties allowed
    #[serde(
        rename = "additionalProperties",
        skip_serializing_if = "Option::is_none"
    )]
    pub additional_properties: Option<bool>,
}

fn default_object_type() -> String {
    "object".to_string()
}

impl ToolInputSchema {
    /// Create a new object schema
    pub fn object() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: HashMap::new(),
            required: Vec::new(),
            additional_properties: Some(false),
        }
    }

    /// Add a property
    pub fn with_property(
        mut self,
        name: impl Into<String>,
        schema: PropertySchema,
        required: bool,
    ) -> Self {
        let name = name.into();
        self.properties.insert(name.clone(), schema);
        if required {
            self.required.push(name);
        }
        self
    }

    /// Convert to JSON Schema Value
    pub fn to_json_schema(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Object(serde_json::Map::new()))
    }

    /// Validate arguments against this schema.
    ///
    /// When the `mcp-validation` feature is enabled, delegates to full
    /// JSON Schema validation via the `jsonschema` crate. Otherwise falls
    /// back to the built-in lightweight checks (required fields, types,
    /// additional properties, enum constraints).
    pub fn validate_arguments(&self, args: &Value) -> Result<(), Vec<String>> {
        #[cfg(feature = "mcp-validation")]
        {
            super::validation::validate_jsonschema(self, args)
        }
        #[cfg(not(feature = "mcp-validation"))]
        {
            super::validation::validate_builtin(self, args)
        }
    }
}

/// Property schema definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertySchema {
    /// Property type
    #[serde(rename = "type")]
    pub property_type: String,

    /// Property description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Enum values (for string type)
    #[serde(rename = "enum", skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// Default value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,

    /// Array item schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Box<PropertySchema>>,

    /// Minimum value (for number)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum: Option<f64>,

    /// Maximum value (for number)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum: Option<f64>,

    /// Pattern (for string)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
}

impl PropertySchema {
    /// Create a string property
    pub fn string() -> Self {
        Self {
            property_type: "string".to_string(),
            description: None,
            enum_values: None,
            default: None,
            items: None,
            minimum: None,
            maximum: None,
            pattern: None,
        }
    }

    /// Create a number property
    pub fn number() -> Self {
        Self {
            property_type: "number".to_string(),
            description: None,
            enum_values: None,
            default: None,
            items: None,
            minimum: None,
            maximum: None,
            pattern: None,
        }
    }

    /// Create an integer property
    pub fn integer() -> Self {
        Self {
            property_type: "integer".to_string(),
            description: None,
            enum_values: None,
            default: None,
            items: None,
            minimum: None,
            maximum: None,
            pattern: None,
        }
    }

    /// Create a boolean property
    pub fn boolean() -> Self {
        Self {
            property_type: "boolean".to_string(),
            description: None,
            enum_values: None,
            default: None,
            items: None,
            minimum: None,
            maximum: None,
            pattern: None,
        }
    }

    /// Create an array property
    pub fn array(items: PropertySchema) -> Self {
        Self {
            property_type: "array".to_string(),
            description: None,
            enum_values: None,
            default: None,
            items: Some(Box::new(items)),
            minimum: None,
            maximum: None,
            pattern: None,
        }
    }

    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set enum values
    pub fn with_enum(mut self, values: Vec<String>) -> Self {
        self.enum_values = Some(values);
        self
    }

    /// Set default value
    pub fn with_default(mut self, value: Value) -> Self {
        self.default = Some(value);
        self
    }

    /// Set range for number
    pub fn with_range(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.minimum = min;
        self.maximum = max;
        self
    }
}

/// Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool name to call
    pub name: String,

    /// Arguments to pass to the tool
    #[serde(default)]
    pub arguments: Value,
}

impl ToolCall {
    /// Create a new tool call
    pub fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }

    /// Create a tool call with no arguments
    pub fn no_args(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arguments: Value::Object(serde_json::Map::new()),
        }
    }
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Result content
    pub content: Vec<ToolContent>,

    /// Whether the tool call resulted in an error
    #[serde(rename = "isError", default)]
    pub is_error: bool,
}

impl ToolResult {
    /// Create a successful text result
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text { text: text.into() }],
            is_error: false,
        }
    }

    /// Create an error result
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::Text {
                text: message.into(),
            }],
            is_error: true,
        }
    }

    /// Create a result with multiple content items
    pub fn multi(content: Vec<ToolContent>) -> Self {
        Self {
            content,
            is_error: false,
        }
    }

    /// Mark this result as an error
    pub fn as_error(mut self) -> Self {
        self.is_error = true;
        self
    }
}

/// Tool content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Text content
    Text { text: String },

    /// Image content
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },

    /// Resource reference
    Resource {
        uri: String,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

impl ToolContent {
    /// Create text content
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create image content
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    /// Create resource content
    pub fn resource(uri: impl Into<String>) -> Self {
        Self::Resource {
            uri: uri.into(),
            mime_type: None,
            text: None,
        }
    }
}

/// List of tools from MCP server
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolList {
    /// Available tools
    pub tools: Vec<Tool>,

    /// Cursor for pagination
    #[serde(rename = "nextCursor", skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl ToolList {
    /// Create an empty tool list
    pub fn empty() -> Self {
        Self::default()
    }

    /// Check if more pages are available
    pub fn has_more(&self) -> bool {
        self.next_cursor.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_new() {
        let tool = Tool::new("get_weather").with_description("Get weather for a location");
        assert_eq!(tool.name, "get_weather");
        assert_eq!(
            tool.description.as_deref(),
            Some("Get weather for a location")
        );
    }

    #[test]
    fn test_tool_to_openai_function() {
        let tool = Tool::new("get_weather")
            .with_description("Get weather")
            .with_schema(ToolInputSchema::object().with_property(
                "location",
                PropertySchema::string().with_description("City name"),
                true,
            ));

        let func = tool.to_openai_function();
        assert_eq!(func["type"], "function");
        assert_eq!(func["function"]["name"], "get_weather");
    }

    #[test]
    fn test_property_schema_string() {
        let schema = PropertySchema::string()
            .with_description("User name")
            .with_enum(vec!["alice".to_string(), "bob".to_string()]);

        assert_eq!(schema.property_type, "string");
        assert!(schema.description.is_some());
        assert!(schema.enum_values.is_some());
    }

    #[test]
    fn test_property_schema_number_with_range() {
        let schema = PropertySchema::number().with_range(Some(0.0), Some(100.0));

        assert_eq!(schema.minimum, Some(0.0));
        assert_eq!(schema.maximum, Some(100.0));
    }

    #[test]
    fn test_property_schema_array() {
        let schema = PropertySchema::array(PropertySchema::string());
        assert_eq!(schema.property_type, "array");
        assert!(schema.items.is_some());
    }

    #[test]
    fn test_input_schema_builder() {
        let schema = ToolInputSchema::object()
            .with_property("name", PropertySchema::string(), true)
            .with_property("age", PropertySchema::integer(), false);

        assert!(schema.properties.contains_key("name"));
        assert!(schema.properties.contains_key("age"));
        assert!(schema.required.contains(&"name".to_string()));
        assert!(!schema.required.contains(&"age".to_string()));
    }

    #[test]
    fn test_tool_call_new() {
        let call = ToolCall::new("get_weather", serde_json::json!({"city": "London"}));
        assert_eq!(call.name, "get_weather");
        assert_eq!(call.arguments["city"], "London");
    }

    #[test]
    fn test_tool_call_no_args() {
        let call = ToolCall::no_args("list_all");
        assert_eq!(call.name, "list_all");
        assert!(call.arguments.is_object());
    }

    #[test]
    fn test_tool_result_text() {
        let result = ToolResult::text("Success!");
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn test_tool_result_error() {
        let result = ToolResult::error("Something went wrong");
        assert!(result.is_error);
    }

    #[test]
    fn test_tool_content_types() {
        let text = ToolContent::text("hello");
        let image = ToolContent::image("base64data", "image/png");
        let resource = ToolContent::resource("file:///path/to/file");

        match text {
            ToolContent::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("Expected text"),
        }

        match image {
            ToolContent::Image { mime_type, .. } => assert_eq!(mime_type, "image/png"),
            _ => panic!("Expected image"),
        }

        match resource {
            ToolContent::Resource { uri, .. } => assert!(uri.starts_with("file://")),
            _ => panic!("Expected resource"),
        }
    }

    #[test]
    fn test_tool_list_empty() {
        let list = ToolList::empty();
        assert!(list.tools.is_empty());
        assert!(!list.has_more());
    }

    #[test]
    fn test_tool_serialization() {
        let tool = Tool::new("test_tool")
            .with_description("A test tool")
            .with_schema(ToolInputSchema::object());

        let json = serde_json::to_string(&tool).unwrap();
        assert!(json.contains("test_tool"));
        assert!(json.contains("inputSchema"));
    }

    // --- validate_arguments tests ---
    // Tests that only check is_ok/is_err work with both builtin and jsonschema backends.
    // Tests that assert specific error message strings are gated to the builtin backend.

    #[test]
    fn test_validate_valid_arguments() {
        let schema = ToolInputSchema::object()
            .with_property("name", PropertySchema::string(), true)
            .with_property("age", PropertySchema::integer(), false);

        let args = serde_json::json!({"name": "Alice", "age": 30});
        assert!(schema.validate_arguments(&args).is_ok());
    }

    #[test]
    fn test_validate_missing_required_field() {
        let schema = ToolInputSchema::object()
            .with_property("name", PropertySchema::string(), true)
            .with_property("city", PropertySchema::string(), true);

        let args = serde_json::json!({"name": "Alice"});
        let err = schema.validate_arguments(&args).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_validate_wrong_type() {
        let schema =
            ToolInputSchema::object().with_property("count", PropertySchema::integer(), true);

        let args = serde_json::json!({"count": "not_a_number"});
        let err = schema.validate_arguments(&args).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_validate_additional_properties_forbidden() {
        let schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);

        let args = serde_json::json!({"name": "Alice", "extra": "field"});
        let err = schema.validate_arguments(&args).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_validate_additional_properties_allowed() {
        let mut schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);
        schema.additional_properties = Some(true);

        let args = serde_json::json!({"name": "Alice", "extra": "field"});
        assert!(schema.validate_arguments(&args).is_ok());
    }

    #[test]
    fn test_validate_additional_properties_unset_allows() {
        let mut schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);
        schema.additional_properties = None;

        let args = serde_json::json!({"name": "Alice", "extra": 42});
        assert!(schema.validate_arguments(&args).is_ok());
    }

    #[test]
    fn test_validate_enum_values() {
        let schema = ToolInputSchema::object().with_property(
            "color",
            PropertySchema::string().with_enum(vec![
                "red".to_string(),
                "green".to_string(),
                "blue".to_string(),
            ]),
            true,
        );

        let valid = serde_json::json!({"color": "red"});
        assert!(schema.validate_arguments(&valid).is_ok());

        let invalid = serde_json::json!({"color": "purple"});
        assert!(schema.validate_arguments(&invalid).is_err());
    }

    #[test]
    fn test_validate_non_object_args() {
        let schema = ToolInputSchema::object();

        let args = serde_json::json!("a string");
        let err = schema.validate_arguments(&args).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn test_validate_multiple_errors() {
        let schema = ToolInputSchema::object()
            .with_property("name", PropertySchema::string(), true)
            .with_property("age", PropertySchema::integer(), true);

        let args = serde_json::json!({"age": "not_int"});
        let err = schema.validate_arguments(&args).unwrap_err();
        assert!(err.len() >= 2);
    }

    #[test]
    fn test_validate_boolean_type() {
        let schema =
            ToolInputSchema::object().with_property("flag", PropertySchema::boolean(), true);

        let valid = serde_json::json!({"flag": true});
        assert!(schema.validate_arguments(&valid).is_ok());

        let invalid = serde_json::json!({"flag": "yes"});
        assert!(schema.validate_arguments(&invalid).is_err());
    }

    #[test]
    fn test_validate_array_type() {
        let schema = ToolInputSchema::object().with_property(
            "tags",
            PropertySchema::array(PropertySchema::string()),
            true,
        );

        let valid = serde_json::json!({"tags": ["a", "b"]});
        assert!(schema.validate_arguments(&valid).is_ok());

        let invalid = serde_json::json!({"tags": "not_array"});
        assert!(schema.validate_arguments(&invalid).is_err());
    }

    #[test]
    fn test_validate_empty_args_with_no_required() {
        let schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), false);

        let args = serde_json::json!({});
        assert!(schema.validate_arguments(&args).is_ok());
    }
}
