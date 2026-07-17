//! Core router for AI provider management and request routing
//!
//! This module provides intelligent routing, load balancing, and failover
//! across multiple AI providers.
//!
//! ## Module Structure
//!
//! The router is organized into modular components following the single-responsibility principle:
//!
//! - `config` - Router configuration and routing strategy definitions
//! - `error` - Error types and cooldown reasons
//! - `fallback` - Fallback configuration and execution results
//! - `deployment` - Deployment management and health tracking
//! - `unified` - Core Router struct and deployment management
//! - `selection` - Deployment selection logic
//! - `strategy_impl` - Routing strategy implementations
//! - `execution` - Execution helpers and error conversion
//! - `execute_impl` - Execute methods with retry and fallback support
//! - `gateway_config` - Gateway configuration integration

use crate::core::providers::unified_provider::ProviderError;
use crate::utils::sync::AtomicValue;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use url::Url;

// New modular router components
pub mod budget_routing;
pub mod config;
pub mod deployment;
pub mod error;
pub mod execute_impl;
pub mod execution;
pub mod fallback;
pub mod gateway_config;
mod health_probe;
pub mod retry_policy;
pub mod selection;
pub mod strategy_impl;
pub mod unified;

#[cfg(test)]
mod tests;

// Re-exports from deployment module
pub use deployment::{
    Deployment, DeploymentConfig, DeploymentId, DeploymentState, HealthCheckPolicy, HealthStatus,
    RetrySchedule,
};

// Re-exports from new modular router (UnifiedRouter)
pub use budget_routing::{BudgetAwareRouter, BudgetAwareRouting, RequestBudgetCheck};
pub use config::{RouterConfig, RoutingStrategy as UnifiedRoutingStrategy};
pub use error::{CooldownReason, RouterError};
pub use fallback::{ExecutionResult, FallbackConfig, FallbackType};
pub use unified::{Router as UnifiedRouter, RoutingMetrics, RoutingSnapshot};

/// Refreshable owner token for a canonical router runtime.
#[derive(Clone)]
pub struct RuntimeBinding {
    router: Arc<UnifiedRouter>,
}

impl RuntimeBinding {
    pub fn new(router: Arc<UnifiedRouter>) -> Self {
        Self { router }
    }

    pub fn bind(&self) -> RuntimeHandle {
        RuntimeHandle {
            binding: self.clone(),
            snapshot: self.router.load_routing_snapshot(),
        }
    }
}

#[derive(Clone)]
pub struct RuntimeHandle {
    binding: RuntimeBinding,
    snapshot: Arc<RoutingSnapshot>,
}

impl RuntimeHandle {
    pub fn generation(&self) -> u64 {
        self.snapshot.generation()
    }

    pub fn snapshot(&self) -> &RoutingSnapshot {
        self.snapshot.as_ref()
    }
}

pub struct DefaultRuntimeBinding {
    current: AtomicValue<RuntimeBinding>,
}

impl DefaultRuntimeBinding {
    pub fn new(initial: RuntimeBinding) -> Self {
        initial.router.publish_current_snapshot();
        Self {
            current: AtomicValue::new(initial),
        }
    }

    pub fn load(&self) -> RuntimeHandle {
        self.current.load().bind()
    }

    pub fn replace(&self, next: RuntimeBinding) -> RuntimeBinding {
        next.router.publish_current_snapshot();
        self.current.swap(next).as_ref().clone()
    }
}

static DEFAULT_RUNTIME: OnceLock<DefaultRuntimeBinding> = OnceLock::new();

pub fn install_default_runtime(runtime: RuntimeBinding) -> Result<RuntimeHandle, ProviderError> {
    let default_runtime = DefaultRuntimeBinding::new(runtime);
    let handle = default_runtime.load();
    DEFAULT_RUNTIME.set(default_runtime).map_err(|_| {
        ProviderError::configuration(
            "router",
            "default runtime is already installed; use replace_default_runtime",
        )
    })?;
    Ok(handle)
}

pub fn default_runtime() -> Result<RuntimeHandle, ProviderError> {
    DEFAULT_RUNTIME
        .get()
        .map(DefaultRuntimeBinding::load)
        .ok_or_else(|| ProviderError::configuration("router", "default runtime is not installed"))
}

pub fn replace_default_runtime(runtime: RuntimeBinding) -> Result<RuntimeBinding, ProviderError> {
    DEFAULT_RUNTIME
        .get()
        .map(|current| current.replace(runtime))
        .ok_or_else(|| ProviderError::configuration("router", "default runtime is not installed"))
}

#[derive(Default)]
pub struct RuntimeRequestOptions {
    pub headers: Option<HashMap<String, String>>,
    pub timeout: Option<Duration>,
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub api_version: Option<String>,
    pub organization: Option<String>,
}

#[allow(dead_code)] // Populated in D1; consumed by the staged completion migration.
pub struct LegacyRuntimeSelector {
    pub(crate) api_key: Option<String>,
    pub(crate) api_base: Option<Url>,
    pub(crate) api_version: Option<String>,
    pub(crate) organization: Option<String>,
}

/// Validated request context; fields cannot bypass [`Self::validate`].
/// ```compile_fail
/// use litellm_rs::core::router::RuntimeRequestContext;
/// let _ = RuntimeRequestContext {
///     headers: Default::default(),
///     timeout: None,
///     legacy_selector: None,
/// };
/// ```
pub struct RuntimeRequestContext {
    headers: HeaderMap,
    timeout: Option<Duration>,
    legacy_selector: Option<LegacyRuntimeSelector>,
}

impl RuntimeRequestContext {
    pub fn validate(
        options: RuntimeRequestOptions,
        policy: &RouterConfig,
    ) -> Result<Self, ProviderError> {
        let mut headers = HeaderMap::new();
        for (raw_name, raw_value) in options.headers.unwrap_or_default() {
            let name = HeaderName::from_bytes(raw_name.as_bytes())
                .map_err(|_| invalid_request(format!("invalid header name: {raw_name}")))?;
            if is_forbidden_runtime_header(&name) {
                return Err(invalid_request(format!(
                    "request header override is forbidden: {name}"
                )));
            }
            let value = HeaderValue::from_bytes(raw_value.as_bytes()).map_err(|_| {
                invalid_request(format!("invalid value for request header: {name}"))
            })?;
            headers.insert(name, value);
        }

        if let Some(timeout) = options.timeout {
            let max_timeout = Duration::from_secs(policy.timeout_secs);
            if timeout.is_zero() || timeout > max_timeout {
                return Err(invalid_request(format!(
                    "request timeout must be greater than zero and at most {} seconds",
                    policy.timeout_secs
                )));
            }
        }

        let api_key = options.api_key;
        let api_version = options.api_version;
        let organization = options.organization;
        let api_base = options
            .api_base
            .map(|raw| {
                if raw.trim().is_empty() {
                    return Err(invalid_request("legacy selector api_base cannot be empty"));
                }
                let url = Url::parse(&raw)
                    .map_err(|_| invalid_request("legacy selector api_base is invalid"))?;
                if !matches!(url.scheme(), "http" | "https") {
                    return Err(invalid_request(
                        "legacy selector api_base must use http or https",
                    ));
                }
                Ok(url)
            })
            .transpose()?;

        let legacy_selector = if api_key.is_some()
            || api_base.is_some()
            || api_version.is_some()
            || organization.is_some()
        {
            Some(LegacyRuntimeSelector {
                api_key,
                api_base,
                api_version,
                organization,
            })
        } else {
            None
        };

        Ok(Self {
            headers,
            timeout: options.timeout,
            legacy_selector,
        })
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    pub fn has_legacy_selector(&self) -> bool {
        self.legacy_selector.is_some()
    }
}

fn invalid_request(message: impl Into<String>) -> ProviderError {
    ProviderError::invalid_request("router", message)
}

fn is_forbidden_runtime_header(name: &HeaderName) -> bool {
    matches!(
        name.as_str(),
        "authorization"
            | "proxy-authorization"
            | "proxy-authenticate"
            | "cookie"
            | "set-cookie"
            | "host"
            | "content-length"
            | "connection"
            | "keep-alive"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}
