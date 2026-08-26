//! Observability Integrations
//!
//! This module provides integrations with observability platforms for metrics,
//! tracing, and monitoring.
//!
//! # Available Integrations
//!
//! - **Prometheus** (`PrometheusIntegration`): Export metrics to Prometheus
//! - **OpenTelemetry** (`OpenTelemetryIntegration`): Distributed tracing with OTLP
//! - **DataDog** (`DataDogIntegration`): DataDog APM integration
//! - **Arize** (`ArizeIntegration`): Arize AI observability
//! - **Helicone** (`HeliconeIntegration`): Helicone logging
//!
//! # Usage
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::observability::{
//!     PrometheusIntegration, OpenTelemetryIntegration,
//!     DataDogIntegration, DataDogConfig,
//!     HeliconeIntegration, HeliconeConfig,
//!     ArizeIntegration, ArizeConfig,
//! };
//!
//! // DataDog
//! let datadog = DataDogIntegration::new(
//!     DataDogConfig::new("dd-api-key")
//!         .service("my-service")
//!         .env("production")
//! )?;
//!
//! // Helicone
//! let helicone = HeliconeIntegration::new(
//!     HeliconeConfig::new("helicone-api-key")
//!         .with_cache(3600)
//! )?;
//!
//! // Arize
//! let arize = ArizeIntegration::new(
//!     ArizeConfig::new("arize-api-key", "space-key")
//!         .model_id("my-model")
//! )?;
//! ```

pub mod arize;
pub mod datadog;
mod durable_batch;
pub mod helicone;
pub mod opentelemetry;
pub mod prometheus;

pub use self::arize::{ArizeConfig, ArizeIntegration};
pub use self::datadog::{DataDogConfig, DataDogIntegration};
pub use self::helicone::{HeliconeConfig, HeliconeIntegration};
pub use self::opentelemetry::{OpenTelemetryConfig, OpenTelemetryIntegration};
pub use self::prometheus::{PrometheusConfig, PrometheusIntegration};
