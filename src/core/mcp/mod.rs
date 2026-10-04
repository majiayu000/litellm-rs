//! MCP configuration and shared protocol/schema support.
//!
//! `gateway,mcp` exposes the authenticated stateless Streamable HTTP proxy in
//! `server::routes::mcp`; requests can move between instances. See
//! `docs/gateway/mcp.md` for supported transport and authentication.
//! Protocol/schema helpers are library support, not a separate runtime client.

pub mod config;
pub mod error;
pub mod protocol;
pub mod tools;
pub mod transport;
pub mod validation;
