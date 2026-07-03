//! Core functionality for the Gateway
//!
//! This module contains the core business logic and data structures.

pub mod a2a; // Experimental module-only A2A gateway; see subsystem_registry.
#[cfg(feature = "analytics")]
pub mod analytics;
pub mod audio; // Audio API (transcription, translation, speech)
pub mod audit; // Audit logging system
// pub mod base_provider;  // Removed: unused dead code
#[cfg(feature = "storage")]
pub mod batch;
pub mod budget; // Budget management system
#[cfg(feature = "storage")]
pub mod cache; // Canonical deterministic cache subsystem (DualCache / LLMCache)
pub mod completion; // Core completion API
pub mod cost; // Unified cost calculation system
pub mod embedding; // Core embedding API (Python LiteLLM compatible)
pub mod fine_tuning; // Fine-tuning API
pub mod function_calling; // Function calling support for AI providers
pub mod guardrails; // Experimental module-only guardrails; see subsystem_registry.
pub mod health; // Health monitoring system
pub mod http; // Shared outbound HTTP client utilities
pub mod integrations; // Experimental module-only integrations; see subsystem_registry.
pub mod ip_access; // Experimental module-only IP access control; see subsystem_registry.
pub mod keys; // API Key Management System
pub mod mcp; // Experimental module-only MCP gateway; see subsystem_registry.
pub mod models;
pub mod net; // Network validation and safety utilities
pub mod observability; // Experimental module-only observability; see subsystem_registry.
pub mod pricing; // Shared pricing data types
pub mod pricing_service; // Runtime pricing service
pub mod providers;
pub mod rate_limiter; // Rate limiting system
#[cfg(feature = "websockets")]
pub mod realtime; // Experimental module-only realtime API; see subsystem_registry.
pub mod rerank; // Rerank API for RAG systems
pub mod router;
pub mod secret_managers; // Secret management system
pub mod security;
#[cfg(feature = "storage")]
pub mod semantic_cache; // Semantic similarity cache (vector-based)
pub mod streaming;
pub mod subsystem_registry; // Runtime wiring decisions for exported core modules.
pub mod teams; // Team management module
pub mod traits;
pub mod types;
#[cfg(feature = "storage")]
pub mod user_management; // Experimental module-only user management; see subsystem_registry.
#[cfg(feature = "gateway")]
pub mod virtual_keys; // Experimental module-only virtual keys; see subsystem_registry.
pub mod webhooks; // Experimental module-only webhooks; see subsystem_registry.
