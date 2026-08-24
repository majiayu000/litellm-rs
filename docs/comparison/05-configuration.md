# Configuration System Comparison: litellm-rs vs litellm

This document provides an in-depth analysis of the configuration systems between the Rust implementation (litellm-rs) and the Python implementation (litellm).

## Table of Contents

1. [Configuration File Format](#1-configuration-file-format)
2. [Configuration Loading Mechanism](#2-configuration-loading-mechanism)
3. [Environment Variable Processing](#3-environment-variable-processing)
4. [Configuration Type Safety](#4-configuration-type-safety)
5. [Configuration Items Comparison](#5-configuration-items-comparison)
6. [Summary and Recommendations](#6-summary-and-recommendations)

---

## 1. Configuration File Format

### 1.1 Supported Formats

| Feature | litellm-rs (Rust) | litellm (Python) |
|---------|-------------------|------------------|
| YAML | Yes (primary) | Yes (primary) |
| JSON | Yes (serialization) | Yes (partial) |
| TOML | No | No |
| Remote Config (S3) | No | Yes |
| Remote Config (GCS) | No | Yes |
| Config Include | No | Yes |

### 1.2 Configuration Structure

#### litellm-rs Configuration Structure

```yaml
# Primary Configuration Sections
server:           # Server configuration
  host: "0.0.0.0"
  port: 8000
  workers: 4
  max_connections: 1000 # server-wide; may round down or reduce workers
  timeout: 30 # seconds to receive the first request head
  max_body_size: 10485760
  tls:            # Optional TLS
    cert_file: "/path/to/cert.pem"
    key_file: "/path/to/key.pem"
  cors:           # CORS settings
    enabled: true
    allowed_origins: ["*"]

providers:        # Provider configurations
  - name: "openai-primary"
    provider_type: "openai"
    api_key: "${OPENAI_API_KEY}"
    weight: 100
    limits:
      max_requests_per_minute: 1000
      max_tokens_per_minute: 100000

router:           # Routing configuration
  strategy:
    type: "least_latency"
  circuit_breaker:
    failure_threshold: 5
    recovery_timeout: 30

storage:          # Storage configuration
  database:
    url: "${DATABASE_URL}"
    max_connections: 10
  redis:
    url: "${REDIS_URL}"
    max_connections: 20

auth:             # Authentication configuration
  jwt:
    enabled: true
    secret: "${JWT_SECRET}"
  api_key:
    enabled: true
    header: "Authorization"
  rbac:
    enabled: true
    default_role: "user"

monitoring:       # Monitoring configuration
  metrics:
    enabled: true
    port: 9090
  tracing:
    enabled: false
    endpoint: "${JAEGER_ENDPOINT}"

cache:            # Caching configuration
  enabled: true
  ttl: 300
  semantic_cache: false

rate_limit:       # Rate limiting configuration
  enabled: true

enterprise:       # Enterprise features
  enabled: false
```

#### litellm (Python) Configuration Structure

```yaml
# Primary Configuration Sections
model_list:       # Model deployments list
  - model_name: gpt-4o
    litellm_params:
      model: azure/gpt-4o-eu
      api_base: https://api.openai.azure.com/
      api_key: "os.environ/AZURE_API_KEY"
      rpm: 6
      tpm: 20000
    model_info:
      version: 2

litellm_settings: # LiteLLM module settings
  drop_params: True
  success_callback: ["langfuse"]
  cache: false
  default_internal_user_params:
    user_role: os.environ/DEFAULT_USER_ROLE

general_settings: # Server/proxy settings
  master_key: sk-1234
  alerting: ["slack"]
  alerting_threshold: 300
  database_url: os.environ/DATABASE_URL

router_settings:  # Router settings
  routing_strategy: simple-shuffle
  redis_host: localhost
  redis_port: 6379

environment_variables:  # Environment variables
  REDIS_HOST: localhost
  REDIS_PASSWORD: ""
```

### 1.3 Key Structural Differences

| Aspect | litellm-rs | litellm |
|--------|------------|---------|
| Top-level structure | Hierarchical (server, providers, router, etc.) | Flat sections (model_list, litellm_settings, etc.) |
| Provider definition | `providers` array with detailed config | `model_list` with `litellm_params` |
| Server settings | Dedicated `server` section | Split between `general_settings` and CLI |
| Auth configuration | Dedicated `auth` section with JWT/API key/RBAC | `general_settings.master_key` + external auth |
| Routing | `router.strategy.type` enum | `router_settings.routing_strategy` string |

---

## 2. Configuration Loading Mechanism

### 2.1 Loading Order

#### litellm-rs Loading Order

```
1. Default values (in Rust code via Default trait)
2. Configuration file (gateway.yaml)
3. Environment variables (override config values)
4. Runtime validation
```

**Code Reference** (`/src/config/mod.rs`):
```rust
impl Config {
    /// Load configuration from file
    pub async fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path).await?;
        let gateway: GatewayConfig = serde_yaml::from_str(&content)?;
        let config = Self { gateway };
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let gateway = GatewayConfig::from_env()?;
        let config = Self { gateway };
        config.validate()?;
        Ok(config)
    }

    /// Merge with another configuration (other takes precedence)
    pub fn merge(mut self, other: Self) -> Self {
        self.gateway = self.gateway.merge(other.gateway);
        self
    }
}
```

#### litellm (Python) Loading Order

```
1. CLI arguments (--config, --port, --host, etc.)
2. Environment variables (dotenv loaded in DEV mode)
3. Configuration file (YAML from local, S3, or GCS)
4. Database configuration (if store_model_in_db enabled)
5. Runtime configuration updates via API
```

**Code Reference** (`/litellm/proxy/proxy_cli.py`):
```python
litellm_mode = os.getenv("LITELLM_MODE", "DEV")
if litellm_mode == "DEV":
    load_dotenv()

# CLI options define loading behavior
@click.option("--config", "-c", default=None, help="Path to config.yaml")
@click.option("--host", default="0.0.0.0", envvar="HOST")
@click.option("--port", default=4000, envvar="PORT")
```

### 2.2 Hot Reload Support

| Feature | litellm-rs | litellm |
|---------|------------|---------|
| Hot reload | Not implemented | Yes (via API) |
| Database sync | Not implemented | Yes (`store_model_in_db`) |
| Config file watch | Not implemented | Not implemented |
| API-based update | Not implemented | Yes (`/config/update`) |

### 2.3 Validation Mechanism

#### litellm-rs Validation

The Rust implementation uses a trait-based validation system with compile-time type safety:

```rust
// Validation trait definition
pub trait Validate {
    fn validate(&self) -> Result<(), String>;
}

// Server config validation
impl Validate for ServerConfig {
    fn validate(&self) -> Result<(), String> {
        if self.host.is_empty() {
            return Err("Server host cannot be empty".to_string());
        }
        if self.port == 0 {
            return Err("Server port must be greater than 0".to_string());
        }
        if self.port < 1024 && !cfg!(test) {
            return Err("Server port should be >= 1024".to_string());
        }
        // ... more validations
        Ok(())
    }
}

// Provider config validation with SSRF protection
impl Validate for ProviderConfig {
    fn validate(&self) -> Result<(), String> {
        if self.name.is_empty() {
            return Err("Provider name cannot be empty".to_string());
        }
        // Validate supported provider types
        let supported_types = [
            "openai", "anthropic", "azure", "google",
            "bedrock", "cohere", "huggingface", "ollama", "custom"
        ];
        if !supported_types.contains(&self.provider_type.as_str()) {
            return Err(format!("Unsupported provider type: {}", self.provider_type));
        }
        // SSRF protection for URLs
        if let Some(base_url) = &self.base_url {
            validate_url_against_ssrf(base_url, &format!("Provider {} base URL", self.name))?;
        }
        Ok(())
    }
}

// Auth config validation with security checks
impl AuthConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.enable_jwt {
            if self.jwt_secret.len() < 32 {
                return Err("JWT secret must be at least 32 characters".to_string());
            }
            if self.jwt_secret == "your-secret-key" || self.jwt_secret == "change-me" {
                return Err("JWT secret must not use default values".to_string());
            }
        }
        // ... more security validations
        Ok(())
    }
}
```

#### litellm (Python) Validation

Python uses Pydantic models with validators:

```python
class NewMCPServerRequest(LiteLLMPydanticObjectBase):
    server_id: Optional[str] = None
    transport: MCPTransportType = MCPTransport.sse
    url: Optional[str] = None

    @model_validator(mode="before")
    @classmethod
    def validate_transport_fields(cls, values):
        if isinstance(values, dict):
            transport = values.get("transport")
            if transport == MCPTransport.stdio:
                if not values.get("command"):
                    raise ValueError("command is required for stdio transport")
            elif transport in [MCPTransport.http, MCPTransport.sse]:
                if not values.get("url"):
                    raise ValueError("url is required for HTTP/SSE transport")
        return values
```

---

## 3. Environment Variable Processing

### 3.1 Environment Variable Syntax

| Format | litellm-rs | litellm |
|--------|------------|---------|
| `${VAR_NAME}` | Yes | No |
| `os.environ/VAR_NAME` | No | Yes |
| Direct `os.getenv()` | No | Yes |
| Secret manager integration | No | Yes (AWS KMS, Azure Key Vault, etc.) |

### 3.2 litellm-rs Environment Variable Processing

```yaml
# In config file
server:
  port: ${SERVER_PORT}
providers:
  - name: openai
    api_key: "${OPENAI_API_KEY}"
    api_base: "${OPENAI_API_BASE}"
```

The Rust implementation uses serde for YAML parsing, with environment variable substitution expected at the shell level or through custom deserialization.

### 3.3 litellm (Python) Environment Variable Processing

```yaml
# In config file - using os.environ/ prefix
model_list:
  - model_name: gpt-4
    litellm_params:
      model: azure/gpt-4
      api_key: "os.environ/AZURE_API_KEY"
      api_base: "os.environ/AZURE_API_BASE"

litellm_settings:
  s3_aws_access_key_id: os.environ/AWS_ACCESS_KEY_ID
  s3_aws_secret_access_key: os.environ/AWS_SECRET_ACCESS_KEY
```

**Processing Logic** (`/litellm/proxy/health_endpoints/_health_endpoints.py`):
```python
def resolve_env_vars(dst, src):
    """Resolve os.environ/ environment variables in litellm_params."""
    for key, value in src.items():
        if isinstance(value, str) and value.startswith("os.environ/"):
            dst[key] = get_secret(value)
        elif isinstance(value, dict):
            resolve_env_vars(dst[key], value)
```

### 3.4 Supported Environment Variables

#### litellm-rs Core Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | - |
| `REDIS_URL` | Redis connection string | `redis://localhost:6379` |
| `JWT_SECRET` | JWT signing secret | Auto-generated |
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `AZURE_OPENAI_API_KEY` | Azure OpenAI API key | - |
| `JAEGER_ENDPOINT` | Jaeger tracing endpoint | - |

#### litellm (Python) Core Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | PostgreSQL connection string | - |
| `REDIS_HOST` | Redis host | `localhost` |
| `REDIS_PORT` | Redis port | `6379` |
| `REDIS_PASSWORD` | Redis password | - |
| `LITELLM_MASTER_KEY` | Master API key | - |
| `LITELLM_MODE` | Operation mode (DEV/PRODUCTION) | `DEV` |
| `USE_AWS_KMS` | Enable AWS KMS | - |
| `AZURE_KEY_VAULT_URI` | Azure Key Vault URI | - |
| `IAM_TOKEN_DB_AUTH` | Use IAM for database auth | - |

### 3.5 Secret Management

#### litellm (Python) Secret Managers

```python
# AWS KMS integration
if os.getenv("USE_AWS_KMS") == "True":
    from litellm.secret_managers.aws_kms import decrypt_value
    for k, v in decrypted_values.items():
        os.environ[k] = v

# Azure Key Vault integration
def load_from_azure_key_vault(use_azure_key_vault: bool = False):
    from azure.identity import DefaultAzureCredential
    from azure.keyvault.secrets import SecretClient

    KVUri = os.getenv("AZURE_KEY_VAULT_URI")
    credential = DefaultAzureCredential()
    client = SecretClient(vault_url=KVUri, credential=credential)
    litellm.secret_manager_client = client
```

#### litellm-rs Secret Management

Currently not implemented. Secrets are expected to be passed via environment variables.

---

## 4. Configuration Type Safety

### 4.1 litellm-rs Type Safety (Rust)

The Rust implementation provides **compile-time type safety** through strongly typed structs:

```rust
/// Main gateway configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GatewayConfig {
    pub server: ServerConfig,
    pub providers: Vec<ProviderConfig>,
    pub router: RouterConfig,
    pub storage: StorageConfig,
    pub auth: AuthConfig,
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
    #[serde(default)]
    pub enterprise: EnterpriseConfig,
}

/// Provider configuration with typed fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub provider_type: String,
    pub api_key: String,
    pub base_url: Option<String>,
    #[serde(default = "default_weight")]
    pub weight: f32,               // Typed as f32
    #[serde(default = "default_rpm")]
    pub rpm: u32,                  // Typed as u32
    #[serde(default = "default_tpm")]
    pub tpm: u32,                  // Typed as u32
    #[serde(default = "default_timeout")]
    pub timeout: u64,              // Typed as u64
    #[serde(default)]
    pub retry: RetryConfig,        // Nested typed config
    #[serde(default)]
    pub health_check: HealthCheckConfig,
}

/// Routing strategy as enum with tagged variants
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutingStrategyConfig {
    #[default]
    RoundRobin,
    LeastLatency,
    LeastCost,
    Random,
    Weighted { weights: HashMap<String, f64> },
    Priority { priorities: HashMap<String, u32> },
    ABTest { split_ratio: f64 },
    Custom { logic: String },
}
```

**Benefits:**
- Compile-time error detection for invalid configuration
- IDE autocompletion support
- Refactoring safety
- No runtime type errors

### 4.2 litellm (Python) Configuration (Dynamic)

Python uses Pydantic models for runtime validation:

```python
class GenerateRequestBase(LiteLLMPydanticObjectBase):
    models: Optional[List[str]] = []
    max_budget: Optional[float] = None
    budget_duration: Optional[str] = None
    allowed_cache_controls: Optional[list] = []
    soft_budget: Optional[float] = None
    max_parallel_requests: Optional[int] = None
    metadata: Optional[dict] = {}

    @model_validator(mode="before")
    @classmethod
    def check_potential_json_str(cls, values):
        if isinstance(values.get("litellm_params"), str):
            try:
                values["litellm_params"] = json.loads(values["litellm_params"])
            except json.JSONDecodeError:
                pass
        return values
```

**Configuration loaded as dictionaries:**
```python
# Global variables for configuration
general_settings: dict = {}
llm_model_list: Optional[list] = None
config_passthrough_endpoints: Optional[List[Dict[str, Any]]] = None
```

### 4.3 Type Safety Comparison

| Aspect | litellm-rs | litellm |
|--------|------------|---------|
| Type checking | Compile-time | Runtime (Pydantic) |
| Default values | Typed functions | Optional fields |
| Enum validation | Exhaustive match | String comparison |
| Nested config | Strongly typed | Dict with runtime checks |
| Missing fields | Compile error or explicit Option | Runtime error or None |
| Invalid values | Deserialization error | Pydantic ValidationError |

---

## 5. Configuration Items Comparison

### 5.1 Server Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Host | `server.host` | CLI `--host` / `HOST` env |
| Port | `server.port` | CLI `--port` / `PORT` env |
| Workers | `server.workers` | CLI `--num_workers` / `NUM_WORKERS` env |
| Timeout | `server.timeout` | `general_settings.request_timeout` |
| Max body size | `server.max_body_size` | Not configurable |
| TLS cert | `server.tls.cert_file` | CLI `--ssl_certfile_path` |
| TLS key | `server.tls.key_file` | CLI `--ssl_keyfile_path` |
| CORS enabled | `server.cors.enabled` | Hardcoded enabled |
| CORS origins | `server.cors.allowed_origins` | Hardcoded `["*"]` |

### 5.2 Provider Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Provider name | `providers[].name` | `model_list[].model_name` |
| Model | `providers[].provider_type` + models | `model_list[].litellm_params.model` |
| API key | `providers[].api_key` | `model_list[].litellm_params.api_key` |
| API base | `providers[].base_url` | `model_list[].litellm_params.api_base` |
| API version | `providers[].api_version` | `model_list[].litellm_params.api_version` |
| RPM limit | `providers[].rpm` | `model_list[].litellm_params.rpm` |
| TPM limit | `providers[].tpm` | `model_list[].litellm_params.tpm` |
| Weight | `providers[].weight` | Implicit via routing |
| Timeout | `providers[].timeout` | `model_list[].litellm_params.timeout` |
| Max retries | `providers[].max_retries` | `model_list[].litellm_params.max_retries` |
| Tags | `providers[].tags` | `model_list[].model_info` |

### 5.3 Router Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Strategy | `router.strategy.type` | `router_settings.routing_strategy` |
| Failure threshold | `router.circuit_breaker.failure_threshold` | `router_settings.allowed_fails` |
| Recovery timeout | `router.circuit_breaker.recovery_timeout` | `router_settings.cooldown_time` |
| Health check | `router.load_balancer.health_check_enabled` | Separate health check system |
| Redis host | `storage.redis.url` | `router_settings.redis_host` |
| Redis port | Included in URL | `router_settings.redis_port` |

**Routing Strategies Comparison:**

| litellm-rs | litellm |
|------------|---------|
| `round_robin` | `simple-shuffle` (weighted) |
| `least_latency` | `latency-based-routing` |
| `least_cost` | `usage-based-routing` |
| `random` | N/A |
| `weighted` | Implicit via tpm/rpm |
| `priority` | N/A |
| `a_b_test` | N/A |
| `custom` | N/A |
| N/A | `least-busy` |

### 5.4 Authentication Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Master key | `auth.api_key.enabled` + API keys | `general_settings.master_key` |
| JWT enabled | `auth.jwt.enabled` | External JWT handler |
| JWT secret | `auth.jwt.secret` | `general_settings.jwt_secret` |
| JWT expiration | `auth.jwt.expiration` | JWT handler config |
| API key header | `auth.api_key.header` | Hardcoded `Authorization` |
| RBAC enabled | `auth.rbac.enabled` | `general_settings.enable_rbac` |
| Default role | `auth.rbac.default_role` | `LitellmUserRoles` enum |

### 5.5 Storage Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Database URL | `storage.database.url` | `general_settings.database_url` |
| Max connections | `storage.database.max_connections` | Prisma client config |
| Connection timeout | `storage.database.connection_timeout` | Prisma client config |
| Redis URL | `storage.redis.url` | Separate `redis_host`/`redis_port` |
| Redis cluster | `storage.redis.cluster` | `router_settings.redis_cluster_enabled` |
| Vector DB | `storage.vector_db` | External vector store config |

### 5.6 Caching Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Cache enabled | `cache.enabled` | `litellm_settings.cache` |
| Cache TTL | `cache.ttl` | `litellm_settings.cache_params.ttl` |
| Max size | `cache.max_size` | Redis-based (unlimited) |
| Semantic cache | `cache.semantic_cache` | `litellm_settings.enable_semantic_caching` |
| Similarity threshold | `cache.similarity_threshold` | `litellm_settings.semantic_cache_params` |

### 5.7 Monitoring Configuration

| Setting | litellm-rs | litellm |
|---------|------------|---------|
| Metrics enabled | `monitoring.metrics.enabled` | `litellm_settings.success_callback` includes `prometheus` |
| Metrics port | `monitoring.metrics.port` | Same port as server |
| Metrics path | `monitoring.metrics.path` | `/metrics` |
| Tracing enabled | `monitoring.tracing.enabled` | OpenTelemetry config |
| Tracing endpoint | `monitoring.tracing.endpoint` | `litellm_settings.otel_endpoint` |
| Service name | `monitoring.tracing.service_name` | `litellm_settings.service_name` |
| Alerting | `monitoring.alerting` | `general_settings.alerting` |

---

## 6. Summary and Recommendations

### 6.1 Feature Matrix

| Feature | litellm-rs | litellm |
|---------|------------|---------|
| Type Safety | Compile-time | Runtime (Pydantic) |
| Configuration Format | YAML only | YAML + Remote (S3/GCS) |
| Environment Variables | `${VAR}` syntax | `os.environ/VAR` syntax |
| Hot Reload | No | Yes (API-based) |
| Config Validation | Strongly typed + custom validators | Pydantic validators |
| Secret Management | Environment only | AWS KMS, Azure KV, etc. |
| Database Sync | No | Yes (`store_model_in_db`) |
| Config Includes | No | Yes |
| Builder Pattern | Yes | No |
| Default Values | Typed functions | Optional fields |

### 6.2 Architectural Differences

| Aspect | litellm-rs | litellm |
|--------|------------|---------|
| Configuration model | Hierarchical, gateway-centric | Flat, model-centric |
| Provider abstraction | Unified `ProviderConfig` | Per-model `litellm_params` |
| Routing | Dedicated router config | Embedded in router_settings |
| Auth | Comprehensive auth section | Scattered across settings |
| Extensibility | Via trait implementation | Via callback system |

### 6.3 Recommendations for litellm-rs

1. **Remote Configuration Support**: Consider adding S3/GCS configuration loading for parity with Python version

2. **Hot Reload**: Implement config reload via API or file watching

3. **Secret Manager Integration**: Add support for AWS KMS, Azure Key Vault, HashiCorp Vault

4. **Config Include**: Support `!include` directive for modular configuration

5. **Environment Variable Syntax**: Consider supporting `os.environ/` syntax for Python compatibility

6. **Database Sync**: Implement `store_model_in_db` equivalent for dynamic configuration

### 6.4 Configuration Migration Guide

When migrating from litellm (Python) to litellm-rs:

```yaml
# Python litellm config
model_list:
  - model_name: gpt-4
    litellm_params:
      model: azure/gpt-4
      api_base: "os.environ/AZURE_API_BASE"
      api_key: "os.environ/AZURE_API_KEY"
      rpm: 60
      tpm: 100000

# Equivalent litellm-rs config
providers:
  - name: "gpt-4"
    provider_type: "azure"
    api_key: "${AZURE_API_KEY}"
    base_url: "${AZURE_API_BASE}"
    rpm: 60
    tpm: 100000
    models:
      - "gpt-4"
```

### 6.5 Validation Strength Comparison

| Validation Type | litellm-rs | litellm |
|-----------------|------------|---------|
| Port range | Yes (1024-65535) | No |
| JWT secret strength | Yes (32+ chars, complexity) | No |
| SSRF protection | Yes (URL validation) | No |
| CORS security | Yes (credentials + origins) | No |
| Timeout bounds | Yes (max 1 hour) | No |
| Body size limits | Yes (max 100MB) | No |
| Provider type validation | Yes (whitelist) | No (any string) |

The Rust implementation provides significantly stronger configuration validation with security-focused checks that are missing from the Python implementation.
