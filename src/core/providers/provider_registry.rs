//! Provider Registry
//!
//! Centralized registry for managing Provider enum instances

use super::{Provider, ProviderType};
use std::collections::HashMap;

/// Legacy provider container retained for source compatibility.
///
/// This is not a runtime routing authority. New execution paths must register
/// deployments with [`crate::core::router::UnifiedRouter`] instead.
#[doc(hidden)]
pub struct ProviderRegistry {
    providers: HashMap<String, Provider>,
}

impl ProviderRegistry {
    /// Create new provider registry
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a provider
    pub fn register(&mut self, provider: Provider) {
        let name = provider.name().to_string();
        self.providers.insert(name, provider);
    }

    /// Register a provider with an explicit key.
    ///
    /// Use this when multiple providers of the same type are configured
    /// under different logical names (for example `openai-primary`,
    /// `openai-backup`).
    pub fn register_with_key(&mut self, key: impl Into<String>, provider: Provider) {
        self.providers.insert(key.into(), provider);
    }

    /// Get provider by name
    pub fn get(&self, name: &str) -> Option<&Provider> {
        self.providers.get(name)
    }

    /// Get mutable provider by name
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Provider> {
        self.providers.get_mut(name)
    }

    /// List all registered providers
    pub fn list(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Remove provider
    pub fn remove(&mut self, name: &str) -> Option<Provider> {
        self.providers.remove(name)
    }

    /// Check if provider is registered
    pub fn contains(&self, name: &str) -> bool {
        self.providers.contains_key(name)
    }

    /// Get provider count
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Clear all providers
    pub fn clear(&mut self) {
        self.providers.clear();
    }

    /// Get providers by type
    pub fn get_by_type(&self, provider_type: ProviderType) -> Vec<&Provider> {
        self.providers
            .values()
            .filter(|p| p.provider_type() == provider_type)
            .collect()
    }

    /// Find providers supporting a specific model
    pub fn find_supporting_model(&self, model: &str) -> Vec<&Provider> {
        self.providers
            .values()
            .filter(|p| p.supports_model(model))
            .collect()
    }

    /// Get all providers as a vector
    pub fn all(&self) -> Vec<&Provider> {
        self.providers.values().collect()
    }

    /// Get provider values iterator (for compatibility with HashMap iteration)
    pub fn values(&self) -> impl Iterator<Item = &Provider> {
        self.providers.values()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProviderRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderRegistry")
            .field("provider_count", &self.providers.len())
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::anthropic::{AnthropicConfig, AnthropicProvider};
    use crate::core::providers::registry::entry_for_type;

    fn test_anthropic_provider() -> Provider {
        let provider = match AnthropicProvider::new(AnthropicConfig::new_test("test-key")) {
            Ok(provider) => provider,
            Err(err) => panic!("test Anthropic provider should be constructible: {err}"),
        };
        Provider::Anthropic(provider)
    }

    // ==================== Creation Tests ====================

    #[test]
    fn test_provider_registry_new() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_provider_registry_default() {
        let registry = ProviderRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    // ==================== Empty Registry Tests ====================

    #[test]
    fn test_get_nonexistent_provider() {
        let registry = ProviderRegistry::new();

        let provider = registry.get("nonexistent");
        assert!(provider.is_none());
    }

    #[test]
    fn test_remove_nonexistent_provider() {
        let mut registry = ProviderRegistry::new();

        let removed = registry.remove("nonexistent");
        assert!(removed.is_none());
    }

    #[test]
    fn test_contains_nonexistent() {
        let registry = ProviderRegistry::new();

        assert!(!registry.contains("nonexistent"));
    }

    // ==================== List Tests ====================

    #[test]
    fn test_list_empty() {
        let registry = ProviderRegistry::new();
        let list = registry.list();
        assert!(list.is_empty());
    }

    // ==================== Length/Empty Tests ====================

    #[test]
    fn test_len_empty() {
        let registry = ProviderRegistry::new();
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_is_empty_true() {
        let registry = ProviderRegistry::new();
        assert!(registry.is_empty());
    }

    // ==================== Get By Type Tests ====================

    #[test]
    fn test_get_by_type_empty() {
        let registry = ProviderRegistry::new();

        let providers = registry.get_by_type(ProviderType::OpenAI);
        assert!(providers.is_empty());
    }

    #[test]
    fn test_get_by_type_all_types() {
        let registry = ProviderRegistry::new();

        // Test all provider types return empty for empty registry
        assert!(registry.get_by_type(ProviderType::OpenAI).is_empty());
        assert!(registry.get_by_type(ProviderType::Anthropic).is_empty());
        assert!(registry.get_by_type(ProviderType::Azure).is_empty());
        assert!(registry.get_by_type(ProviderType::Bedrock).is_empty());
        assert!(registry.get_by_type(ProviderType::Mistral).is_empty());
        assert!(registry.get_by_type(ProviderType::DeepSeek).is_empty());
        assert!(registry.get_by_type(ProviderType::OpenRouter).is_empty());
        assert!(registry.get_by_type(ProviderType::VertexAI).is_empty());
        assert!(registry.get_by_type(ProviderType::Groq).is_empty());
    }

    // ==================== Find Supporting Model Tests ====================

    #[test]
    fn test_find_supporting_model_empty() {
        let registry = ProviderRegistry::new();

        let providers = registry.find_supporting_model("gpt-4");
        assert!(providers.is_empty());
    }

    #[test]
    fn test_find_supporting_model_various_models() {
        let registry = ProviderRegistry::new();

        // All should return empty for empty registry
        assert!(registry.find_supporting_model("gpt-4").is_empty());
        assert!(registry.find_supporting_model("claude-3-opus").is_empty());
        assert!(registry.find_supporting_model("gemini-pro").is_empty());
        assert!(registry.find_supporting_model("unknown-model").is_empty());
    }

    // ==================== All Tests ====================

    #[test]
    fn test_all_empty() {
        let registry = ProviderRegistry::new();
        let all = registry.all();
        assert!(all.is_empty());
    }

    #[test]
    fn test_all_alias_behavior_empty() {
        let registry = ProviderRegistry::new();
        let all = registry.all();
        assert!(all.is_empty());
    }

    // ==================== Values Iterator Tests ====================

    #[test]
    fn test_values_iterator_empty() {
        let registry = ProviderRegistry::new();
        let count = registry.values().count();
        assert_eq!(count, 0);
    }

    // ==================== Clear Tests ====================

    #[test]
    fn test_clear_empty() {
        let mut registry = ProviderRegistry::new();
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    // ==================== Debug Tests ====================

    #[test]
    fn test_debug_empty() {
        let registry = ProviderRegistry::new();
        let debug = format!("{:?}", registry);

        assert!(debug.contains("ProviderRegistry"));
        assert!(debug.contains("provider_count"));
        assert!(debug.contains("0"));
    }

    // ==================== ProviderType Tests ====================

    #[test]
    fn test_provider_type_variants() {
        // Ensure all provider types can be referenced
        let _ = ProviderType::OpenAI;
        let _ = ProviderType::Anthropic;
        let _ = ProviderType::Azure;
        let _ = ProviderType::Bedrock;
        let _ = ProviderType::Mistral;
        let _ = ProviderType::DeepSeek;
        let _ = ProviderType::Moonshot;
        let _ = ProviderType::MetaLlama;
        let _ = ProviderType::OpenRouter;
        let _ = ProviderType::VertexAI;
        let _ = ProviderType::V0;
        let _ = ProviderType::DeepInfra;
        let _ = ProviderType::AzureAI;
        let _ = ProviderType::Groq;
        let _ = ProviderType::XAI;
        let _ = ProviderType::Cloudflare;
    }

    #[test]
    fn test_provider_type_debug() {
        let provider_type = ProviderType::OpenAI;
        let debug = format!("{:?}", provider_type);
        assert!(debug.contains("OpenAI"));
    }

    #[test]
    fn test_provider_type_clone() {
        let provider_type = ProviderType::Anthropic;
        let cloned = provider_type.clone();
        assert!(matches!(cloned, ProviderType::Anthropic));
    }

    #[test]
    fn test_provider_type_equality() {
        assert_eq!(ProviderType::OpenAI, ProviderType::OpenAI);
        assert_ne!(ProviderType::OpenAI, ProviderType::Anthropic);
    }

    // ==================== HashMap Behavior Tests ====================

    #[test]
    fn test_internal_hashmap_behavior() {
        // Test that the underlying HashMap operations work correctly
        let registry = ProviderRegistry::new();

        // Multiple gets on empty registry should all return None
        assert!(registry.get("test1").is_none());
        assert!(registry.get("test2").is_none());
        assert!(registry.get("").is_none());
    }

    #[test]
    fn test_empty_string_key() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("").is_none());
        assert!(!registry.contains(""));
    }

    #[test]
    fn test_special_characters_key() {
        let registry = ProviderRegistry::new();
        assert!(registry.get("provider-with-dash").is_none());
        assert!(registry.get("provider_with_underscore").is_none());
        assert!(registry.get("provider.with.dots").is_none());
    }

    #[test]
    fn test_register_uses_provider_canonical_name() {
        let mut registry = ProviderRegistry::new();
        let entry = match entry_for_type(&ProviderType::Anthropic) {
            Some(entry) => entry,
            None => panic!("anthropic registry entry should exist"),
        };

        registry.register(test_anthropic_provider());

        assert!(registry.contains(entry.canonical_name));
        assert!(registry.get("anthropic").is_some());
        let typed = registry.get_by_type(ProviderType::Anthropic);
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].name(), entry.canonical_name);
    }

    #[test]
    fn test_register_with_key_preserves_logical_key_and_provider_identity() {
        let mut registry = ProviderRegistry::new();

        registry.register_with_key("anthropic-primary", test_anthropic_provider());

        assert!(registry.contains("anthropic-primary"));
        assert!(!registry.contains("anthropic"));
        let typed = registry.get_by_type(ProviderType::Anthropic);
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].provider_type(), ProviderType::Anthropic);
        assert_eq!(typed[0].name(), "anthropic");
    }
}
