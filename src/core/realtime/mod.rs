//! Legacy Realtime library types.
//!
//! These deprecated configuration, event, and session types do not open a
//! WebSocket connection. Their outstanding removal is tracked in issue #1402.
//! The separate `GET /v1/realtime` HTTP gateway uses native GA events and does
//! not use this module; see `docs/gateway/realtime.md` for its supported scope.

pub mod config;
pub mod events;
pub mod session;

#[deprecated(
    since = "0.6.0",
    note = "legacy core::realtime types remain pending removal in issue #1402; use the separate /v1/realtime gateway"
)]
pub use config::RealtimeConfig;
#[deprecated(
    since = "0.6.0",
    note = "legacy core::realtime types remain pending removal in issue #1402; use the separate /v1/realtime gateway"
)]
pub use events::{
    ClientEvent, ContentPart, RealtimeError, RealtimeEvent, RealtimeResult, ResponseStatus,
    ServerEvent, SessionConfig, TurnDetection, Voice,
};
#[deprecated(
    since = "0.6.0",
    note = "legacy core::realtime types remain pending removal in issue #1402; use the separate /v1/realtime gateway"
)]
pub use session::{RealtimeSession, SessionState};
