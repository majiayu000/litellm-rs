## Cache Key Generation

### Request Hashing

```rust
use sha2::{Sha256, Digest};
use serde::Serialize;

pub struct CacheKeyGenerator;

impl CacheKeyGenerator {
    /// Generate a deterministic cache key from a chat request
    pub fn generate_key(request: &ChatRequest) -> String {
        let mut hasher = Sha256::new();

        // Include model
        hasher.update(request.model.as_bytes());

        // Include messages (normalized)
        for message in &request.messages {
            hasher.update(message.role.to_string().as_bytes());
            if let Some(content) = &message.content {
                hasher.update(content.to_string().as_bytes());
            }
        }

        // Include relevant parameters
        if let Some(temp) = request.temperature {
            hasher.update(temp.to_le_bytes());
        }
        if let Some(top_p) = request.top_p {
            hasher.update(top_p.to_le_bytes());
        }
        if let Some(max_tokens) = request.max_tokens {
            hasher.update(max_tokens.to_le_bytes());
        }

        // Include tools if present
        if let Some(tools) = &request.tools {
            let tools_json = serde_json::to_string(tools).unwrap_or_default();
            hasher.update(tools_json.as_bytes());
        }

        let result = hasher.finalize();
        format!("chat:{}", hex::encode(result))
    }

    /// Generate a semantic cache key (for vector lookup)
    pub fn generate_semantic_key(request: &ChatRequest) -> String {
        // Extract the last user message for semantic matching
        let user_message = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .and_then(|m| m.content.as_ref())
            .map(|c| c.to_string())
            .unwrap_or_default();

        format!("semantic:{}:{}", request.model, user_message)
    }
}
```
