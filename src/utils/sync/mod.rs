//! Concurrent-safe value helpers for the LiteLLM Gateway
//!
//! This module keeps only shared synchronization helpers that are used by
//! production code.
//!
//! ## Available Helpers
//!
//! - [`AtomicValue`] - A concurrent-safe single value container using arc-swap
//!
//! ## Example Usage
//!
//! ```rust
//! use litellm_rs::utils::sync::AtomicValue;
//!
//! // AtomicValue
//! let value: AtomicValue<String> = AtomicValue::new("initial".to_string());
//! value.store("updated".to_string());
//! assert_eq!(value.load().as_ref(), "updated");
//! ```

mod atomic_value;

pub use atomic_value::AtomicValue;
