//! Opaque identity for one logical provider construction.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

/// Identity shared by deployments backed by one logical provider instance.
///
/// Pointer identity is used only in process memory. No provider configuration
/// or credential material is retained or exposed by this value.
#[derive(Clone)]
pub(crate) struct ProviderInstanceIdentity(Arc<()>);

impl ProviderInstanceIdentity {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl fmt::Debug for ProviderInstanceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProviderInstanceIdentity")
    }
}

impl PartialEq for ProviderInstanceIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ProviderInstanceIdentity {}

impl Hash for ProviderInstanceIdentity {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
