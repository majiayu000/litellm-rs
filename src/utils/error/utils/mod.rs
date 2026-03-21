//! Error utility sub-modules.
//!
//! Split from the original monolithic utils.rs (1435 lines) into focused files:
//! - `types`      — ErrorContext, ErrorCategory, ErrorUtils struct
//! - `http_status` — HTTP status code → ProviderError mapping
//! - `retry`      — retry-after extraction, should_retry, get_retry_delay
//! - `parsers`    — provider-specific error body parsers (OpenAI, Anthropic, Google)
//! - `format`     — user-facing formatting and error categorisation

pub mod format;
pub mod http_status;
pub mod parsers;
pub mod retry;
pub mod types;

pub use types::{ErrorCategory, ErrorContext, ErrorUtils};
