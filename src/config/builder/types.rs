//! Type definitions for configuration builders

use crate::utils::data::type_utils::{NonEmptyString, PositiveF64};
use std::collections::HashMap;
use std::time::Duration;

/// Builder for creating gateway configurations
#[derive(Debug, Clone)]
pub struct GatewayConfigBuilder {
    pub(super) server: Option<super::super::ServerConfig>,
    pub(super) auth: Option<super::super::AuthConfig>,
    pub(super) storage: Option<super::super::StorageConfig>,
    pub(super) providers: Vec<super::super::ProviderConfig>,
    pub(super) features: HashMap<String, bool>,
}

/// Builder for server configuration
#[derive(Debug, Clone)]
pub struct ServerConfigBuilder {
    pub(super) host: Option<String>,
    pub(super) port: Option<u16>,
    pub(super) workers: Option<usize>,
    pub(super) timeout: Option<Duration>,
    pub(super) max_connections: Option<usize>,
    pub(super) enable_cors: bool,
    pub(super) cors_origins: Vec<String>,
}

/// Builder for provider configuration
#[derive(Debug, Clone)]
pub struct ProviderConfigBuilder {
    pub(super) name: Option<NonEmptyString>,
    pub(super) provider_type: Option<NonEmptyString>,
    pub(super) api_key: Option<String>,
    pub(super) base_url: Option<String>,
    pub(super) endpoint_access: crate::core::net::ProviderEndpointAccess,
    pub(super) models: Vec<String>,
    pub(super) model_identity_mappings:
        HashMap<String, crate::core::providers::model_identity::ModelIdentityMapping>,
    pub(super) max_requests_per_minute: Option<u32>,
    pub(super) timeout: Option<Duration>,
    pub(super) enabled: bool,
    pub(super) weight: Option<PositiveF64>,
}

/// Backward-compatible alias.
pub type ConfigBuilder = GatewayConfigBuilder;
