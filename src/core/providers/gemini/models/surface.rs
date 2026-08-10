use super::{GeminiModelFamily, ModelInfo, ModelSpec};

/// Google Gemini API surface for provider-specific catalog overlays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GoogleGeminiApiSurface {
    /// Google AI Studio / Gemini Developer API.
    DeveloperApi,
    /// Google Cloud Vertex AI publisher endpoint.
    VertexAi,
    /// Vertex AI publisher endpoint with experimental models enabled.
    VertexAiExperimental,
}

impl GoogleGeminiApiSurface {
    fn provider_name(self) -> &'static str {
        match self {
            Self::DeveloperApi => "gemini",
            Self::VertexAi | Self::VertexAiExperimental => "vertex_ai",
        }
    }

    fn surface_name(self) -> &'static str {
        match self {
            Self::DeveloperApi => "developer_api",
            Self::VertexAi | Self::VertexAiExperimental => "vertex_ai",
        }
    }

    fn auth_boundary(self) -> &'static str {
        match self {
            Self::DeveloperApi => "api_key",
            Self::VertexAi | Self::VertexAiExperimental => "bearer_token",
        }
    }

    fn endpoint_family(self) -> &'static str {
        match self {
            Self::DeveloperApi => "generativelanguage",
            Self::VertexAi | Self::VertexAiExperimental => "aiplatform_publishers_google",
        }
    }

    pub(crate) fn includes(self, spec: &ModelSpec) -> bool {
        match self {
            Self::DeveloperApi => true,
            Self::VertexAi => {
                !matches!(
                    spec.family,
                    GeminiModelFamily::Gemini10Pro
                        | GeminiModelFamily::Gemini10ProVision
                        | GeminiModelFamily::GeminiExperimental
                        | GeminiModelFamily::Gemini20FlashThinking
                ) && spec.model_info.id != "gemini-2.0-flash-exp"
            }
            Self::VertexAiExperimental => !matches!(
                spec.family,
                GeminiModelFamily::Gemini10Pro | GeminiModelFamily::Gemini10ProVision
            ),
        }
    }

    pub(super) fn overlay_model_info(self, spec: &ModelSpec) -> ModelInfo {
        let mut model_info = spec.model_info.clone();
        model_info.provider = self.provider_name().to_string();
        model_info.metadata.insert(
            "google_model_catalog_surface".to_string(),
            serde_json::json!(self.surface_name()),
        );
        model_info.metadata.insert(
            "google_auth_boundary".to_string(),
            serde_json::json!(self.auth_boundary()),
        );
        model_info.metadata.insert(
            "google_endpoint_family".to_string(),
            serde_json::json!(self.endpoint_family()),
        );
        model_info.metadata.insert(
            "google_model_source_provider".to_string(),
            serde_json::json!("gemini"),
        );
        model_info
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::gemini::get_gemini_registry;

    #[test]
    fn google_api_surface_model_overlays_are_stable() {
        let registry = get_gemini_registry();

        let developer_models =
            registry.list_model_infos_for_surface(GoogleGeminiApiSurface::DeveloperApi);
        let vertex_models = registry.list_model_infos_for_surface(GoogleGeminiApiSurface::VertexAi);
        let experimental_vertex_models =
            registry.list_model_infos_for_surface(GoogleGeminiApiSurface::VertexAiExperimental);

        assert_eq!(developer_models.len(), registry.list_models().len());
        assert!(
            developer_models
                .iter()
                .any(|model| model.id == "gemini-1.0-pro")
        );
        assert!(
            vertex_models
                .iter()
                .any(|model| model.id == "gemini-3.5-flash")
        );
        assert!(
            !vertex_models
                .iter()
                .any(|model| model.id == "gemini-1.0-pro")
        );
        for experimental_id in ["gemini-2.0-flash-exp", "gemini-2.0-flash-thinking-exp"] {
            assert!(
                !vertex_models
                    .iter()
                    .any(|model| model.id == experimental_id)
            );
            assert!(
                experimental_vertex_models
                    .iter()
                    .any(|model| model.id == experimental_id)
            );
        }

        for models in [&developer_models, &vertex_models] {
            let ids = models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>();
            let mut sorted_ids = ids.clone();
            sorted_ids.sort_unstable();
            assert_eq!(ids, sorted_ids);
        }

        let developer_model = developer_models
            .iter()
            .find(|model| model.id == "gemini-3.5-flash")
            .unwrap();
        assert_eq!(developer_model.provider, "gemini");
        assert_eq!(
            developer_model.metadata["google_auth_boundary"],
            serde_json::json!("api_key")
        );

        let vertex_model = vertex_models
            .iter()
            .find(|model| model.id == "gemini-3.5-flash")
            .unwrap();
        assert_eq!(vertex_model.provider, "vertex_ai");
        assert_eq!(
            vertex_model.metadata["google_auth_boundary"],
            serde_json::json!("bearer_token")
        );
        assert_eq!(
            vertex_model.metadata["google_endpoint_family"],
            serde_json::json!("aiplatform_publishers_google")
        );
    }
}
