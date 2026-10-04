//! A2A configuration and shared domain/error types.
//!
//! `gateway,a2a` exposes the authenticated A2A 1.0 JSON-RPC gateway in
//! `server::routes::a2a`. Task/context ownership remains process-local; see
//! `docs/gateway/a2a.md`. Domain records do not imply native platform adapters.

pub mod config;
pub mod error;
pub mod message;
