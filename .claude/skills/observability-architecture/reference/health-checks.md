## Health Checks

### Health Check System

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub checks: Vec<ComponentHealth>,
}

#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub latency_ms: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

pub struct HealthChecker {
    start_time: std::time::Instant,
    providers: Vec<Arc<dyn LLMProvider>>,
    redis_client: Option<Arc<RedisCache>>,
}

impl HealthChecker {
    pub async fn check(&self) -> HealthResponse {
        let mut checks = Vec::new();
        let mut overall_status = HealthStatus::Healthy;

        // Check providers
        for provider in &self.providers {
            let start = std::time::Instant::now();
            let status = provider.health_check().await;
            let latency = start.elapsed().as_millis() as u64;

            let health_status = match status {
                crate::core::types::common::HealthStatus::Healthy => HealthStatus::Healthy,
                crate::core::types::common::HealthStatus::Degraded => HealthStatus::Degraded,
                crate::core::types::common::HealthStatus::Unhealthy => HealthStatus::Unhealthy,
            };

            if health_status != HealthStatus::Healthy && overall_status == HealthStatus::Healthy {
                overall_status = HealthStatus::Degraded;
            }

            checks.push(ComponentHealth {
                name: format!("provider:{}", provider.name()),
                status: health_status,
                latency_ms: Some(latency),
                message: None,
            });
        }

        // Check Redis
        if let Some(redis) = &self.redis_client {
            let start = std::time::Instant::now();
            let status = match redis.ping().await {
                Ok(_) => HealthStatus::Healthy,
                Err(e) => {
                    overall_status = HealthStatus::Degraded;
                    HealthStatus::Unhealthy
                }
            };
            let latency = start.elapsed().as_millis() as u64;

            checks.push(ComponentHealth {
                name: "redis".to_string(),
                status,
                latency_ms: Some(latency),
                message: None,
            });
        }

        HealthResponse {
            status: overall_status,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            checks,
        }
    }
}
```

### Health Endpoints

```rust
use actix_web::{HttpResponse, web};

// Liveness probe - is the service running?
pub async fn liveness_handler() -> HttpResponse {
    HttpResponse::Ok().json(json!({ "status": "alive" }))
}

// Readiness probe - is the service ready to accept requests?
pub async fn readiness_handler(
    health_checker: web::Data<HealthChecker>,
) -> HttpResponse {
    let health = health_checker.check().await;

    match health.status {
        HealthStatus::Healthy | HealthStatus::Degraded => {
            HttpResponse::Ok().json(health)
        }
        HealthStatus::Unhealthy => {
            HttpResponse::ServiceUnavailable().json(health)
        }
    }
}

// Detailed health - full system status
pub async fn health_handler(
    health_checker: web::Data<HealthChecker>,
) -> HttpResponse {
    let health = health_checker.check().await;

    let status_code = match health.status {
        HealthStatus::Healthy => 200,
        HealthStatus::Degraded => 200,  // Still operational
        HealthStatus::Unhealthy => 503,
    };

    HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
        .json(health)
}

pub fn configure_health(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health_handler))
       .route("/health/live", web::get().to(liveness_handler))
       .route("/health/ready", web::get().to(readiness_handler));
}
```
