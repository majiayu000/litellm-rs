//! Macros for provider implementation
//!
//! Split into sub-modules for maintainability:
//! - `config_helpers`: Configuration extraction functions and macros
//! - `basic_macros`: Basic provider implementation macros (error conversion, health check, etc.)
//! - `provider_definitions`: `standard_provider!` macro
//! - `openai_compatible`: `define_openai_compatible_provider!` macro
//! - `http_hooks`: `define_http_provider_with_hooks!` macro
//! - `pooled_hooks`: `define_pooled_http_provider_with_hooks!` macro
mod config_helpers;

// All macro modules contain #[macro_export] macros, which are hoisted to crate root.
// We still need to include the modules so the macros are compiled.
mod basic_macros;
mod http_hooks;
mod openai_compatible;
mod pooled_hooks;
mod provider_definitions;

// Re-export config helper functions at the same path as before
pub use config_helpers::*;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
