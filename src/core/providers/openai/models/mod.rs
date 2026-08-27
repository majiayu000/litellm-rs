//! OpenAI Model Registry and API Types
//!
//! Split into sub-modules for maintainability:
//! - `registry_types`: Enums and structs (OpenAIModelFeature, OpenAIModelFamily, etc.)
//! - `static_models`: Hardcoded model catalog data
//! - `registry`: Model discovery, feature detection, and capability classification
//! - `api_types`: OpenAI API request/response serialization types

mod api_types;
mod registry;
mod registry_types;
mod static_models;

// Re-export everything at the same path as before
pub use api_types::*;
pub use registry::*;
pub use registry_types::*;
