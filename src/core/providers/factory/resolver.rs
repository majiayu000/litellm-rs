//! Provider selector resolution
//!
//! Resolves a provider selector string to determine if it can be instantiated.

use crate::core::providers::Provider;
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry;

/// Returns true if a provider selector can be instantiated by the current runtime.
///
/// The selector is resolved using the same precedence as `create_provider`:
/// 1. Tier-1 data-driven catalog names
/// 2. Built-in factory provider types
pub fn is_provider_selector_supported(selector: &str) -> bool {
    let normalized = selector.trim().to_lowercase();
    if normalized.is_empty() {
        return false;
    }

    if registry::get_definition(&normalized).is_some() {
        return true;
    }

    // Catalog selectors are already handled above; use strict FromStr for enum variants.
    match normalized.parse::<ProviderType>() {
        Ok(t) => Provider::factory_supported_provider_types().contains(&t),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::registry;

    #[test]
    fn test_provider_selector_support_detection() {
        assert!(is_provider_selector_supported("openai"));
        assert!(is_provider_selector_supported("openai_compatible"));
        assert!(is_provider_selector_supported("groq")); // Tier-1 catalog
        assert!(!is_provider_selector_supported("totally_unknown_provider"));
    }

    #[test]
    fn test_catalog_entries_are_supported_selectors() {
        for name in registry::PROVIDER_CATALOG.keys() {
            assert!(
                is_provider_selector_supported(name),
                "Catalog provider '{}' must be a supported selector",
                name
            );
        }
    }
}
