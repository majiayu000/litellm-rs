//! Runtime wiring decisions for exported `core` modules.
//!
//! This registry is the guardrail for declaration-execution drift: a module
//! exported from `src/core/mod.rs` must either be referenced by the gateway
//! runtime or have an explicit non-runtime classification here.

/// Current gateway-runtime decision for a `core` module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubsystemDecision {
    /// The gateway server or binary constructs the module, mounts routes, or
    /// registers middleware for it.
    Wired,
    /// Library API or shared domain code intentionally has no direct gateway
    /// route, middleware, or startup path.
    LibraryOnly,
    /// Shared support code used below wired modules rather than exposed as its
    /// own gateway subsystem.
    InternalDependency,
    /// Deprecated 0.6 source-compatibility variant. No active registry entry
    /// uses it; removal is scheduled for 0.7.
    TemporaryExemption,
    /// Module is hidden from the default build behind a default-off feature.
    FeatureGated,
    /// Parsed configuration exists but validation rejects enabling it until the
    /// runtime path lands.
    ConfigRejected,
}

impl CoreSubsystem {
    /// Returns true when a module can be absent from direct server/main
    /// references without creating a new declaration-execution gap.
    pub fn has_gateway_reference_exemption(&self) -> bool {
        match self.decision {
            SubsystemDecision::Wired => false,
            SubsystemDecision::LibraryOnly
            | SubsystemDecision::InternalDependency
            | SubsystemDecision::TemporaryExemption
            | SubsystemDecision::FeatureGated
            | SubsystemDecision::ConfigRejected => true,
        }
    }
}

/// Deprecated issue number retained for 0.6 source compatibility.
#[deprecated(
    since = "0.6.0",
    note = "GH838 is resolved; this compatibility constant is removed in 0.7.0"
)]
pub const GH838_TEMPORARY_EXEMPTION_ISSUE: u32 = 838;

/// Deprecated exemption names retained for 0.6 source compatibility only.
#[deprecated(
    since = "0.6.0",
    note = "GH838 has explicit final dispositions; this compatibility constant is removed in 0.7.0"
)]
pub const GH838_TEMPORARY_EXEMPTIONS: &[&str] = &[
    "a2a",
    "batch",
    "integrations",
    "mcp",
    "observability",
    "user_management",
    "virtual_keys",
    "webhooks",
];

/// Explicit runtime status for one exported `core` module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSubsystem {
    pub name: &'static str,
    pub decision: SubsystemDecision,
    pub runtime_path: Option<&'static str>,
    pub note: &'static str,
}

/// Exhaustive matrix for modules exported from `src/core/mod.rs`.
pub const CORE_SUBSYSTEMS: &[CoreSubsystem] = &[
    CoreSubsystem {
        name: "a2a",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: a2a"),
        note: "A2A library types are excluded from the default build; enabling the feature does not mount HTTP routes.",
    },
    CoreSubsystem {
        name: "analytics",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: analytics"),
        note: "Deprecated analytics types remain default-off for the 0.6 migration window and are scheduled for removal in 0.7.",
    },
    CoreSubsystem {
        name: "audio",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("/v1/audio/* routes"),
        note: "Audio request paths are mounted by server AI routes.",
    },
    CoreSubsystem {
        name: "audit",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState AuditLogger and AuditMiddleware"),
        note: "enterprise.audit_logging explicitly enables request lifecycle audit events; the default remains disabled.",
    },
    CoreSubsystem {
        name: "batch",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "The HTTP /v1/batches surface is a provider proxy; the legacy BatchProcessor is deprecated for 0.6 and scheduled for 0.7 removal.",
    },
    CoreSubsystem {
        name: "budget",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState budget limits and budget routes"),
        note: "Budget state and reservation paths are constructed by the server.",
    },
    CoreSubsystem {
        name: "cache",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState response_cache and /admin/cache"),
        note: "Deterministic response cache is constructed when cache.enabled=true.",
    },
    CoreSubsystem {
        name: "completion",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Library completion API is separate from gateway route handlers.",
    },
    CoreSubsystem {
        name: "cost",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "Cost helpers back providers and spend calculation rather than a standalone route.",
    },
    CoreSubsystem {
        name: "embedding",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Library embedding API is separate from gateway route handlers.",
    },
    CoreSubsystem {
        name: "fine_tuning",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("/v1/fine_tuning/jobs routes"),
        note: "Fine-tuning route handlers use the core provider adapters.",
    },
    CoreSubsystem {
        name: "function_calling",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Function-calling helpers are library/provider support code.",
    },
    CoreSubsystem {
        name: "guardrails",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState GuardrailEngine and canonical chat request/response paths"),
        note: "Prompt-injection checks run before provider execution and on non-streaming output.",
    },
    CoreSubsystem {
        name: "health",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Server health routes are implemented under src/server/routes/health.rs.",
    },
    CoreSubsystem {
        name: "http",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "Shared outbound HTTP profile used by providers and integrations.",
    },
    CoreSubsystem {
        name: "integrations",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState callback dispatcher and LLM request lifecycle"),
        note: "Configured Langfuse, OpenTelemetry, and Datadog callbacks are initialized and receive real request lifecycle events.",
    },
    CoreSubsystem {
        name: "ip_access",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("IpAccessMiddleware outer HTTP policy layer"),
        note: "Configured IP policies short-circuit before auth, handlers, or providers.",
    },
    CoreSubsystem {
        name: "keys",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState key_manager and /v1/keys routes"),
        note: "Key management is constructed in AppState and exposed through server routes.",
    },
    CoreSubsystem {
        name: "mcp",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: mcp"),
        note: "MCP library types are excluded from the default build; enabling the feature does not mount HTTP routes.",
    },
    CoreSubsystem {
        name: "models",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("server route request/response models"),
        note: "OpenAI-compatible models are used by gateway routes and auth context.",
    },
    CoreSubsystem {
        name: "net",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "Network safety helpers are consumed by config/provider support code.",
    },
    CoreSubsystem {
        name: "observability",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState RuntimeObservability callback dispatcher"),
        note: "RuntimeObservability is wired to real LLM callbacks; all other legacy exports are deprecated library-only compatibility surfaces for 0.6.",
    },
    CoreSubsystem {
        name: "pricing",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: Some("spend route pricing normalization"),
        note: "Shared pricing helpers support runtime spend calculations.",
    },
    CoreSubsystem {
        name: "pricing_service",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState pricing service and pricing routes"),
        note: "PricingService is initialized at startup and shared with handlers.",
    },
    CoreSubsystem {
        name: "providers",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("UnifiedRouter provider factory"),
        note: "Gateway startup builds providers through the unified router.",
    },
    CoreSubsystem {
        name: "rate_limiter",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("RateLimitMiddleware"),
        note: "Global rate limiter is initialized by the server app factory.",
    },
    CoreSubsystem {
        name: "realtime",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: websockets"),
        note: "Realtime module is excluded from the default build behind the websockets feature.",
    },
    CoreSubsystem {
        name: "rerank",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("/v1/rerank route"),
        note: "Rerank requests are mounted in the AI route scope.",
    },
    CoreSubsystem {
        name: "router",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState UnifiedRouter"),
        note: "Gateway startup constructs UnifiedRouter from provider config.",
    },
    CoreSubsystem {
        name: "secret_managers",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Secret manager adapters are available as library support code.",
    },
    CoreSubsystem {
        name: "security",
        decision: SubsystemDecision::LibraryOnly,
        runtime_path: None,
        note: "Core security filters are library support; server security middleware is separate.",
    },
    CoreSubsystem {
        name: "semantic_cache",
        decision: SubsystemDecision::ConfigRejected,
        runtime_path: None,
        note: "Deprecated types remain available during the 0.6 compatibility window, while cache.semantic_cache stays rejected before 0.7 removal.",
    },
    CoreSubsystem {
        name: "streaming",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("chat/completions/responses SSE routes"),
        note: "Streaming event types are used by mounted SSE handlers.",
    },
    CoreSubsystem {
        name: "subsystem_registry",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "This guardrail registry classifies the exported core modules.",
    },
    CoreSubsystem {
        name: "teams",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState team_manager and team routes"),
        note: "Team management is constructed in AppState and exposed through server routes.",
    },
    CoreSubsystem {
        name: "traits",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "Trait definitions support providers, integrations, and storage abstractions.",
    },
    CoreSubsystem {
        name: "types",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("server route request context and response types"),
        note: "Gateway handlers use shared context, response, model, and media types.",
    },
    CoreSubsystem {
        name: "user_management",
        decision: SubsystemDecision::InternalDependency,
        runtime_path: None,
        note: "Compatibility user/team records back current auth and storage paths; the optional UserManager implementation is default-off behind user-management.",
    },
    CoreSubsystem {
        name: "virtual_keys",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState canonical RuntimeVirtualKeyManager"),
        note: "The virtual-keys runtime facade resolves to the canonical KeyManager used by auth and /v1/keys; the duplicate legacy manager is deprecated.",
    },
    CoreSubsystem {
        name: "webhooks",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: webhooks"),
        note: "Webhook library types are excluded from the default build and are not advertised as a gateway runtime capability.",
    },
];

/// Look up the recorded decision for an exported core module.
pub fn subsystem_for(name: &str) -> Option<&'static CoreSubsystem> {
    CORE_SUBSYSTEMS
        .iter()
        .find(|subsystem| subsystem.name == name)
}

#[cfg(test)]
mod tests;
