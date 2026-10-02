use super::VertexAIProvider;
use crate::core::providers::vertex_ai::error::VertexAIError;

impl VertexAIProvider {
    /// Internal health check
    pub(super) async fn check_health(&self) -> Result<(), VertexAIError> {
        // Simple health check by calling countTokens
        let url = self.build_google_model_url("gemini-3.7-flash", "countTokens");

        let body = serde_json::json!({
            "contents": [{
                "parts": [{"text": "test"}]
            }]
        });

        self.make_request(&url, body).await?;
        Ok(())
    }
}
