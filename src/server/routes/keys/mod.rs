//! API Key Management HTTP Routes
//!
//! This module provides HTTP endpoints for managing API keys.
//!
//! ## Endpoints
//!
//! - `POST /v1/keys` - Generate a new API key
//! - `GET /v1/keys` - List all keys (masked)
//! - `GET /v1/keys/{id}` - Get key info
//! - `PUT /v1/keys/{id}` - Update key configuration
//! - `DELETE /v1/keys/{id}` - Revoke key
//! - `POST /v1/keys/{id}/rotate` - Rotate key
//! - `GET /v1/keys/{id}/usage` - Get usage statistics
//! - `POST /v1/keys/verify` - Verify a key

mod access;
mod api_key_budget;
mod handlers;
mod middleware;
mod types;

use actix_web::web;

/// Configure key management routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/keys")
            .route("", web::post().to(handlers::create_key))
            .route("", web::get().to(handlers::list_keys))
            .route("/verify", web::post().to(handlers::verify_key))
            .route("/{id}", web::get().to(handlers::get_key))
            .route("/{id}", web::put().to(handlers::update_key))
            .route("/{id}", web::delete().to(handlers::revoke_key))
            .route("/{id}/rotate", web::post().to(handlers::rotate_key))
            .route("/{id}/usage", web::get().to(handlers::get_key_usage)),
    );
}
