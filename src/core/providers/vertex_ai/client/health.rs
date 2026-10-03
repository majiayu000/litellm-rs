use super::VertexAIProvider;
use crate::core::providers::vertex_ai::error::VertexAIError;

impl VertexAIProvider {
    /// Internal health check
    pub(super) async fn check_health(&self) -> Result<(), VertexAIError> {
        // This synthetic authentication/service probe contains no customer data.
        // Gemini 3.7 is available in global/us/eu, not the default us-central1.
        let mut probe = self.clone();
        probe.config.location = "global".to_string();
        let url = probe.build_google_model_url("gemini-3.7-flash", "countTokens");

        let body = serde_json::json!({
            "contents": [{
                "parts": [{"text": "test"}]
            }]
        });

        self.make_request(&url, body).await?;
        Ok(())
    }
}
