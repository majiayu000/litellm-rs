//! Embedding request types

use serde::{Deserialize, Serialize};

/// Embedding request (short form)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    /// Model name
    pub model: String,
    /// Input text
    pub input: EmbedInput,
    /// Encoding format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// Dimensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Embedding input (short form)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbedInput {
    /// Single string
    Single(String),
    /// String array
    Multiple(Vec<String>),
    /// Integer array (token IDs)
    TokenIds(Vec<u32>),
    /// Array of integer arrays
    MultipleTokenIds(Vec<Vec<u32>>),
}

/// Embedding request (full form)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Model name
    pub model: String,
    /// Input text or text list
    pub input: EmbeddingInput,
    /// User ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Embedding format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,
    /// Dimensions count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
    /// Task type (for Vertex AI etc)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    /// Whether the provider may truncate over-length input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<bool>,
}

/// Embedding input type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    /// Single text
    Text(String),
    /// Text list
    Array(Vec<String>),
}

impl EmbeddingInput {
    /// Get iterator over texts
    pub fn iter(&self) -> Box<dyn Iterator<Item = &String> + '_> {
        match self {
            EmbeddingInput::Text(text) => Box::new(std::iter::once(text)),
            EmbeddingInput::Array(texts) => Box::new(texts.iter()),
        }
    }

    /// Convert to text vector
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            EmbeddingInput::Text(text) => vec![text.clone()],
            EmbeddingInput::Array(texts) => texts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== EmbedInput Tests ====================

    #[test]
    fn test_embed_input_single_serialization() {
        let input = EmbedInput::Single("hello world".to_string());
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json, "hello world");
    }

    #[test]
    fn test_embed_input_multiple_serialization() {
        let input = EmbedInput::Multiple(vec!["hello".to_string(), "world".to_string()]);
        let json = serde_json::to_value(&input).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0], "hello");
        assert_eq!(json[1], "world");
    }

    #[test]
    fn test_embed_input_token_ids_serialization() {
        let input = EmbedInput::TokenIds(vec![1, 2, 3, 4]);
        let json = serde_json::to_value(&input).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0], 1);
    }

    #[test]
    fn test_embed_input_multiple_token_ids_serialization() {
        let input = EmbedInput::MultipleTokenIds(vec![vec![1, 2], vec![3, 4]]);
        let json = serde_json::to_value(&input).unwrap();
        assert!(json.is_array());
        assert!(json[0].is_array());
    }

    #[test]
    fn test_embed_input_single_deserialization() {
        let input: EmbedInput = serde_json::from_str("\"test string\"").unwrap();
        match input {
            EmbedInput::Single(s) => assert_eq!(s, "test string"),
            _ => panic!("Expected Single variant"),
        }
    }

    #[test]
    fn test_embed_input_multiple_deserialization() {
        let input: EmbedInput = serde_json::from_str("[\"a\", \"b\", \"c\"]").unwrap();
        match input {
            EmbedInput::Multiple(v) => assert_eq!(v.len(), 3),
            _ => panic!("Expected Multiple variant"),
        }
    }

    #[test]
    fn test_embed_input_clone() {
        let input = EmbedInput::Single("clone test".to_string());
        let cloned = input.clone();
        match (input, cloned) {
            (EmbedInput::Single(a), EmbedInput::Single(b)) => assert_eq!(a, b),
            _ => panic!("Clone mismatch"),
        }
    }

    // ==================== EmbedRequest Tests ====================

    #[test]
    fn test_embed_request_structure() {
        let request = EmbedRequest {
            model: "text-embedding-3-small".to_string(),
            input: EmbedInput::Single("test".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
        };

        assert_eq!(request.model, "text-embedding-3-small");
    }

    #[test]
    fn test_embed_request_with_options() {
        let request = EmbedRequest {
            model: "text-embedding-3-large".to_string(),
            input: EmbedInput::Single("test".to_string()),
            encoding_format: Some("float".to_string()),
            dimensions: Some(1536),
            user: Some("user-123".to_string()),
        };

        assert_eq!(request.dimensions, Some(1536));
        assert_eq!(request.encoding_format, Some("float".to_string()));
    }

    #[test]
    fn test_embed_request_serialization_skip_none() {
        let request = EmbedRequest {
            model: "model".to_string(),
            input: EmbedInput::Single("test".to_string()),
            encoding_format: None,
            dimensions: None,
            user: None,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert!(!json.as_object().unwrap().contains_key("encoding_format"));
        assert!(!json.as_object().unwrap().contains_key("dimensions"));
        assert!(!json.as_object().unwrap().contains_key("user"));
    }

    #[test]
    fn test_embed_request_serialization_include_values() {
        let request = EmbedRequest {
            model: "model".to_string(),
            input: EmbedInput::Single("test".to_string()),
            encoding_format: Some("base64".to_string()),
            dimensions: Some(256),
            user: Some("user".to_string()),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["encoding_format"], "base64");
        assert_eq!(json["dimensions"], 256);
        assert_eq!(json["user"], "user");
    }

    // ==================== EmbeddingInput Tests ====================

    #[test]
    fn test_embedding_input_text_serialization() {
        let input = EmbeddingInput::Text("hello".to_string());
        let json = serde_json::to_value(&input).unwrap();
        assert_eq!(json, "hello");
    }

    #[test]
    fn test_embedding_input_array_serialization() {
        let input = EmbeddingInput::Array(vec!["a".to_string(), "b".to_string()]);
        let json = serde_json::to_value(&input).unwrap();
        assert!(json.is_array());
        assert_eq!(json.as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_embedding_input_iter_single() {
        let input = EmbeddingInput::Text("single".to_string());
        let items: Vec<_> = input.iter().collect();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0], "single");
    }

    #[test]
    fn test_embedding_input_iter_array() {
        let input = EmbeddingInput::Array(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        let items: Vec<_> = input.iter().collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0], "a");
        assert_eq!(items[2], "c");
    }

    #[test]
    fn test_embedding_input_to_vec_single() {
        let input = EmbeddingInput::Text("text".to_string());
        let vec = input.to_vec();
        assert_eq!(vec.len(), 1);
        assert_eq!(vec[0], "text");
    }

    #[test]
    fn test_embedding_input_to_vec_array() {
        let input = EmbeddingInput::Array(vec!["x".to_string(), "y".to_string()]);
        let vec = input.to_vec();
        assert_eq!(vec.len(), 2);
        assert_eq!(vec[0], "x");
        assert_eq!(vec[1], "y");
    }

    #[test]
    fn test_embedding_input_to_vec_empty() {
        let input = EmbeddingInput::Array(vec![]);
        let vec = input.to_vec();
        assert!(vec.is_empty());
    }

    #[test]
    fn test_embedding_input_clone() {
        let input = EmbeddingInput::Text("clone".to_string());
        let cloned = input.clone();
        assert_eq!(input.to_vec(), cloned.to_vec());
    }

    // ==================== EmbeddingRequest Tests ====================

    #[test]
    fn test_embedding_request_structure() {
        let request = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: EmbeddingInput::Text("test".to_string()),
            user: None,
            encoding_format: None,
            dimensions: None,
            task_type: None,
            truncation: None,
        };

        assert_eq!(request.model, "text-embedding-ada-002");
    }

    #[test]
    fn test_embedding_request_full() {
        let request = EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: EmbeddingInput::Array(vec!["a".to_string(), "b".to_string()]),
            user: Some("user-456".to_string()),
            encoding_format: Some("float".to_string()),
            dimensions: Some(512),
            task_type: Some("RETRIEVAL_DOCUMENT".to_string()),
            truncation: None,
        };

        assert_eq!(request.task_type, Some("RETRIEVAL_DOCUMENT".to_string()));
    }

    #[test]
    fn test_embedding_request_serialization() {
        let request = EmbeddingRequest {
            model: "model".to_string(),
            input: EmbeddingInput::Text("test".to_string()),
            user: None,
            encoding_format: None,
            dimensions: None,
            task_type: None,
            truncation: None,
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "model");
        assert_eq!(json["input"], "test");
    }

    #[test]
    fn test_embedding_request_deserialization() {
        let json = r#"{
            "model": "text-embedding-ada-002",
            "input": ["hello", "world"]
        }"#;

        let request: EmbeddingRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "text-embedding-ada-002");
        assert_eq!(request.input.to_vec().len(), 2);
    }

    #[test]
    fn test_embedding_request_clone() {
        let request = EmbeddingRequest {
            model: "model".to_string(),
            input: EmbeddingInput::Text("test".to_string()),
            user: Some("user".to_string()),
            encoding_format: None,
            dimensions: None,
            task_type: None,
            truncation: None,
        };

        let cloned = request.clone();
        assert_eq!(request.model, cloned.model);
        assert_eq!(request.user, cloned.user);
    }
}
