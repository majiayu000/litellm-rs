//! Provider Registry - data-driven Tier 1 provider system
//!
//! Instead of maintaining 50+ separate provider implementations that are
//! essentially OpenAI-compatible with different base URLs, this module
//! defines them as static data entries in a catalog.
//!
//! A Tier 1 provider needs zero code — just a `ProviderDefinition` entry.

pub mod catalog;
pub mod definition;
pub mod lifecycle;
pub mod support_matrix;
pub mod types;

#[cfg(test)]
mod readme_tests;
#[cfg(test)]
mod support_matrix_tests;

pub use catalog::{PROVIDER_CATALOG, canonical_catalog_name, get_definition, is_tier1_provider};
pub use definition::{AuthType, ProviderDefinition};
pub use lifecycle::{
    PROVIDER_MODULE_LIFECYCLE, PROVIDER_ORPHAN_BASELINE, ProviderModuleLifecycle,
    ProviderModuleLifecycleEntry, ProviderOrphanBaselineEntry, provider_module_lifecycle,
    provider_orphan_baseline,
};
pub use support_matrix::{
    ProviderRouteSurface, ProviderSurfaceSupport, SurfaceSupport, canonical_selector,
    provider_surface_matrix, selector_has_matrix_entry, support_state_for_surface,
    supports_provider_surface,
};
pub use types::{
    DEFAULT_CATALOG_RUNTIME_PROVIDERS, PROVIDER_TYPE_REGISTRY, ProviderDispatchKind,
    ProviderRegistryEntry, catalog_dispatch_entries, catalog_dispatch_entry_for_type,
    default_catalog_runtime_provider_names, dispatchable_provider_types,
    dispatchable_provider_types_slice, entry_for_name, entry_for_type, provider_type_registry,
};

pub fn catalog_definition_for_provider_type(
    provider_type: &super::provider_type::ProviderType,
) -> Option<&'static ProviderDefinition> {
    match provider_type {
        super::provider_type::ProviderType::Custom(name) => get_definition(name),
        _ => catalog_dispatch_entry_for_type(provider_type)
            .and_then(|entry| get_definition(entry.canonical_name)),
    }
}
