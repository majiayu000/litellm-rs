//! Core functionality for the Gateway
//!
//! This module contains the core business logic and data structures.

#[cfg(feature = "a2a")]
pub mod a2a; // Opt-in A2A gateway configuration and domain types.
pub mod audio; // Audio API (transcription, translation, speech)
pub mod audit; // Audit logging system
pub mod request_ledger; // Metadata-only terminal request ledger
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
pub mod guardrails; // Gateway request/output guardrails.
pub mod health; // Health monitoring system
pub mod http; // Shared outbound HTTP client utilities
pub mod integrations; // Configured callback integrations.
pub mod ip_access; // Gateway IP access middleware.
pub mod keys; // API Key Management System
#[cfg(feature = "mcp")]
pub mod mcp; // MCP configuration/schema types and optional Streamable HTTP gateway.
pub mod models;
pub mod net; // Network validation and safety utilities
pub mod observability; // Canonical callback dispatcher and shared redaction helpers.
pub mod pricing; // Shared pricing data types
pub mod pricing_service; // Runtime pricing service
pub mod providers;
pub mod rate_limiter; // Rate limiting system
pub mod rerank; // Rerank API for RAG systems
pub mod router;
pub mod secret_managers; // Secret management system
pub mod security;
pub mod streaming;
pub mod subsystem_registry; // Runtime wiring decisions for exported core modules.
pub mod teams; // Team management module
pub mod traits;
pub mod types;
#[cfg(feature = "storage")]
pub mod user_management; // User/team domain records used by auth and storage.
#[cfg(feature = "gateway")]
pub mod virtual_keys; // Canonical KeyManager runtime facade.
