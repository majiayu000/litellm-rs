//! Redis storage implementation
//!
//! This module provides Redis connectivity and caching operations.
//!
//! ## Module Structure
//!
//! - `pool` - Connection pool and core connection management
//! - `cache` - Basic cache operations (get, set, delete, exists, expire, ttl)
//! - `batch` - Batch operations (mget, mset)
//! - `collections` - List and Set operations
//! - `hash` - Hash and Sorted Set operations
//! - `pubsub` - Pub/Sub operations (temporarily disabled)
//! - `atomic` - Atomic operations and utilities
//! - `budget` - Cluster-safe single-key Lua budget leases
//! - `admission` - Cluster-safe single-key Lua deployment admission
//! - `circuit` - Cluster-safe single-key Lua deployment circuit state
//! - `tests` - Module tests

#![allow(dead_code)]

// Module declarations
pub(crate) mod admission;
mod atomic;
mod batch;
pub(crate) mod budget;
mod cache;
pub(crate) mod circuit;
mod collections;
mod hash;
mod pool;
mod pubsub;
mod rate_limit;
#[cfg(test)]
mod tests;

// Re-export public types
pub use pool::{RedisConnection, RedisPool, cluster_hash_tag};
pub use pubsub::Subscription;
