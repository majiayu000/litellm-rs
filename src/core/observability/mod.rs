//! Gateway observability and sensitive-data redaction.
//!
//! Runtime events use the canonical callback dispatcher. Redaction is also
//! consumed by provider configuration formatting.

mod redaction;

pub use redaction::{RedactionConfig, redact_headers, redact_json_value, redact_value};

/// Canonical gateway observability handle.
///
/// The gateway stores this dispatcher in `AppState`; configured integrations
/// receive request start/success/failure events from the real LLM lifecycle.
pub type RuntimeObservability = crate::core::integrations::CallbackDispatcher;
