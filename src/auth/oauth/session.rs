//! Session management for OAuth authentication

mod memory_store;
mod model;
#[cfg(feature = "redis")]
mod redis_store;
mod store;

#[cfg(test)]
mod tests;

pub use memory_store::InMemorySessionStore;
pub use model::OAuthSession;
#[cfg(feature = "redis")]
pub use redis_store::RedisSessionStore;
pub use store::{SessionError, SessionStore};
