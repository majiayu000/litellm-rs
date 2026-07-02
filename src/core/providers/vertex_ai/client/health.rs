use super::VertexAIProvider;
use crate::core::providers::vertex_ai::error::VertexAIError;

impl VertexAIProvider {
    /// Internal health check
    pub(super) async fn check_health(&self) -> Result<(), VertexAIError> {
        // Simple health check by calling countTokens
        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/gemini-1.5-flash:countTokens",
            self.config.location, self.config.project_id, self.config.location
        );

        let body = serde_json::json!({
            "contents": [{
                "parts": [{"text": "test"}]
            }]
        });

        self.make_request(&url, body).await?;
        Ok(())
    }
}
