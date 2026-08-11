//! Realtime API
//!
//! This module provides WebSocket-based real-time communication for LLM interactions,
//! compatible with OpenAI's Realtime API.
//!
//! # Features
//!
//! - WebSocket connection management
//! - Real-time streaming responses
//! - Audio input/output support
//! - Session management
//! - Event-based communication
//!
//! # Example
//!
//! ```rust,ignore
//! use litellm_rs::core::realtime::{RealtimeClient, RealtimeConfig};
//!
//! let config = RealtimeConfig::new()
//!     .model("gpt-4o-realtime-preview")
//!     .voice("alloy");
//!
//! let client = RealtimeClient::connect(config).await?;
//!
//! // Send a message
//! client.send_text("Hello!").await?;
//!
//! // Receive events
//! while let Some(event) = client.recv().await {
//!     match event {
//!         RealtimeEvent::ResponseText { text, .. } => println!("{}", text),
//!         RealtimeEvent::ResponseAudio { audio, .. } => play_audio(audio),
//!         _ => {}
//!     }
//! }
//! ```

pub mod config;
pub mod events;
pub mod session;

#[deprecated(
    since = "0.6.0",
    note = "core::realtime is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use config::RealtimeConfig;
#[deprecated(
    since = "0.6.0",
    note = "core::realtime is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use events::{
    ClientEvent, ContentPart, RealtimeError, RealtimeEvent, RealtimeResult, ResponseStatus,
    ServerEvent, SessionConfig, TurnDetection, Voice,
};
#[deprecated(
    since = "0.6.0",
    note = "core::realtime is a default-off compatibility surface scheduled for removal in 0.7.0"
)]
pub use session::{RealtimeSession, SessionState};
