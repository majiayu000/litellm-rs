//! Core type definition module
//!
//! Contains all core data structures and type definitions

// Split from common.rs (new modules)
pub mod context;
pub mod health;
pub mod metrics;
pub mod model;
pub mod model_id;
#[cfg(test)]
mod model_id_tests;
pub mod pagination;
pub mod service;

// Split from requests.rs (new modules)
pub mod anthropic;
pub mod chat;
pub mod codex;
pub mod content;
pub mod embedding;
pub mod image;
pub mod message;
pub mod thinking;
pub mod tools;

// Original files kept as-is
pub mod errors;
pub mod responses;

// No top-level type re-exports: use explicit module paths from call sites.
