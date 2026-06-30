use crate::core::providers::Provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;

impl Provider {
    /// Check whether a deployment's concrete model supports `capability`.
    ///
    /// Provider-level capabilities answer whether a provider family has an
    /// implementation. When the provider has a registry entry for the concrete
    /// deployment model, route selection also respects that model-specific
    /// capability list.
    pub fn supports_capability_for_model(
        &self,
        model: &str,
        capability: &ProviderCapability,
    ) -> bool {
        match self {
            Provider::OpenAI(provider) => {
                if provider.get_model_config(model).is_some() {
                    provider.model_supports_capability(model, capability)
                } else {
                    LLMProvider::supports_capability(provider, capability)
                }
            }
            Provider::OpenAILike(provider) if capability == &ProviderCapability::Rerank => {
                openai_like_provider_supports_rerank(provider.name())
            }
            _ => self.supports_capability(capability),
        }
    }
}

fn openai_like_provider_supports_rerank(provider_name: &str) -> bool {
    let normalized = provider_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();

    normalized.contains("cohere") || normalized.contains("jina")
}
