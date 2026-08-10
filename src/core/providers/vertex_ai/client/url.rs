use super::VertexAIProvider;
use crate::core::providers::vertex_ai::VertexAIModel;

impl VertexAIProvider {
    pub(super) fn build_google_model_url(&self, model: &str, endpoint: &str) -> String {
        if let Some(api_base) = self.config.api_base.as_deref() {
            return format!("{}/{}:{}", api_base.trim_end_matches('/'), model, endpoint);
        }
        format!(
            "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}/publishers/google/models/{}:{}",
            self.config.location,
            self.config.api_version,
            self.config.project_id,
            self.config.location,
            model,
            endpoint
        )
    }

    pub(super) fn build_google_catalog_model_url(
        &self,
        model: &str,
        endpoint: &str,
        stream: bool,
    ) -> String {
        let url = self.build_google_model_url(model, endpoint);
        if stream {
            format!("{url}?alt=sse")
        } else {
            url
        }
    }

    /// Build the API URL for a given model and endpoint
    pub(super) fn build_url(&self, model: &VertexAIModel, endpoint: &str, stream: bool) -> String {
        let model_id = model.model_id();
        let location = &self.config.location;
        let project_id = &self.config.project_id;
        let api_version = &self.config.api_version;

        // Handle custom API base
        if let Some(ref api_base) = self.config.api_base {
            return format!("{}/{}:{}", api_base, model_id, endpoint);
        }

        // Special handling for global models
        let use_global = location == "global" || model_id.contains("imagen");

        let base_url = if use_global {
            format!(
                "https://aiplatform.googleapis.com/{}/projects/{}/locations/global",
                api_version, project_id
            )
        } else {
            format!(
                "https://{}-aiplatform.googleapis.com/{}/projects/{}/locations/{}",
                location, api_version, project_id, location
            )
        };

        // Build full URL based on model type
        let url = if model.is_gemini() {
            format!(
                "{}/publishers/google/models/{}:{}",
                base_url, model_id, endpoint
            )
        } else if model.is_partner_model() {
            // Partner models have different URL structure
            let publisher = self.get_publisher_for_model(&model_id);
            format!(
                "{}/publishers/{}/models/{}:{}",
                base_url, publisher, model_id, endpoint
            )
        } else {
            // Custom models
            format!("{}/endpoints/{}:{}", base_url, model_id, endpoint)
        };

        // Add streaming parameter if needed
        if stream {
            format!("{}?alt=sse", url)
        } else {
            url
        }
    }

    /// Get publisher for partner models
    fn get_publisher_for_model(&self, model_id: &str) -> &str {
        if model_id.contains("claude") {
            "anthropic"
        } else if model_id.contains("llama") {
            "meta"
        } else if model_id.contains("jamba") {
            "ai21"
        } else {
            "google"
        }
    }
}
