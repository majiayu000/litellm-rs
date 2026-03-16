//! Authentication endpoints
//!
//! This module provides authentication-related API endpoints.

#![allow(dead_code)]

mod email;
mod login;
mod models;
mod password;
mod register;
mod session;
mod token;
mod user;

// Re-export public items for backward compatibility
pub use email::verify_email;
pub use login::login;
pub use models::{
    AuthResponse, ChangePasswordRequest, ForgotPasswordRequest, LoginRequest, LoginResponse,
    RefreshTokenRequest, RefreshTokenResponse, RegisterRequest, RegisterResponse,
    ResetPasswordRequest, UserInfo, UserResponse, VerifyEmailRequest,
};
pub use password::{change_password, forgot_password, reset_password};
pub use register::register;
pub use session::{extract_session_token, logout};
pub use token::refresh_token;
pub use user::{get_authenticated_user, get_current_user};

use actix_web::web;

/// Configure authentication routes
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/auth")
            .route("/register", web::post().to(register))
            .route("/login", web::post().to(login))
            .route("/logout", web::post().to(logout))
            .route("/refresh", web::post().to(refresh_token))
            .route("/forgot-password", web::post().to(forgot_password))
            .route("/reset-password", web::post().to(reset_password))
            .route("/verify-email", web::post().to(verify_email))
            .route("/change-password", web::post().to(change_password))
            .route("/me", web::get().to(get_current_user)),
    );
}
