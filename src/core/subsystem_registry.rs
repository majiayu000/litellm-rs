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
    /// Retained module-only implementation recorded in the explicit GH838
    /// temporary exemption baseline. It is not a feature gate or runtime
    /// capability.
    TemporaryExemption,
    /// Parsed configuration exists but validation rejects enabling it until the
    /// runtime path lands.
    ConfigRejected,
}

/// Issue backing the temporary unwired-subsystem baseline.
pub const GH838_TEMPORARY_EXEMPTION_ISSUE: u32 = 838;

/// Gateway-facing modules that remain exported only because GH838 tracks their
/// later wire/gate/remove tranche.
pub const GH838_TEMPORARY_EXEMPTIONS: &[&str] = &[
    "a2a",
    "analytics",
    "batch",
    "guardrails",
    "integrations",
    "ip_access",
    "mcp",
    "observability",
    "realtime",
    "user_management",
    "virtual_keys",
    "webhooks",
];

impl CoreSubsystem {
    /// Returns true when a module can be absent from direct server/main
    /// references without creating a new declaration-execution gap.
    pub fn has_gateway_reference_exemption(&self) -> bool {
        match self.decision {
            SubsystemDecision::Wired => false,
            SubsystemDecision::LibraryOnly
            | SubsystemDecision::InternalDependency
            | SubsystemDecision::ConfigRejected => true,
            SubsystemDecision::TemporaryExemption => {
                GH838_TEMPORARY_EXEMPTIONS.contains(&self.name)
            }
        }
    }
}

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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "A2A gateway types compile, but no server route or AppState entry mounts them.",
    },
    CoreSubsystem {
        name: "analytics",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "Analytics types and engine are feature-gated; no runtime collector or route is wired.",
    },
    CoreSubsystem {
        name: "audio",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("/v1/audio/* routes"),
        note: "Audio request paths are mounted by server AI routes.",
    },
    CoreSubsystem {
        name: "audit",
        decision: SubsystemDecision::ConfigRejected,
        runtime_path: None,
        note: "enterprise.audit_logging is rejected until audit middleware/logger are registered by the gateway server.",
    },
    CoreSubsystem {
        name: "batch",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: Some("/v1/batches provider proxy only"),
        note: "The HTTP batch API is a provider proxy; core::batch::BatchProcessor is not constructed.",
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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "GuardrailEngine is not configured or executed on completion requests.",
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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "Langfuse/OpenTelemetry integration managers are not initialized by the binary.",
    },
    CoreSubsystem {
        name: "ip_access",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "IP access middleware exists but is not registered in the Actix middleware stack.",
    },
    CoreSubsystem {
        name: "keys",
        decision: SubsystemDecision::Wired,
        runtime_path: Some("AppState key_manager and /v1/keys routes"),
        note: "Key management is constructed in AppState and exposed through server routes.",
    },
    CoreSubsystem {
        name: "mcp",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "MCP gateway is not mounted; Responses API only passes MCP tool descriptors through.",
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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "Basic tracing/metrics are wired elsewhere; core observability exporters are not.",
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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "Realtime WebSocket types exist but no gateway route is mounted.",
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
        note: "GatewayConfig::validate rejects cache.semantic_cache until runtime handling is wired.",
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
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "User-management domain code is not constructed by the gateway server.",
    },
    CoreSubsystem {
        name: "virtual_keys",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "Gateway key routes use core::keys; VirtualKeyManager is not in AppState.",
    },
    CoreSubsystem {
        name: "webhooks",
        decision: SubsystemDecision::TemporaryExemption,
        runtime_path: None,
        note: "WebhookManager is not configured or constructed by the gateway runtime.",
    },
];

/// Look up the recorded decision for an exported core module.
pub fn subsystem_for(name: &str) -> Option<&'static CoreSubsystem> {
    CORE_SUBSYSTEMS
        .iter()
        .find(|subsystem| subsystem.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn manifest_path(relative: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
    }

    fn exported_core_modules() -> BTreeSet<String> {
        let source = std::fs::read_to_string(manifest_path("src/core/mod.rs"))
            .expect("read src/core/mod.rs");
        source
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("pub mod ")?;
                let name = rest
                    .split(|ch: char| ch == ';' || ch.is_ascii_whitespace())
                    .next()?;
                (!name.is_empty()).then(|| name.to_string())
            })
            .collect()
    }

    fn registry_names() -> BTreeSet<String> {
        CORE_SUBSYSTEMS
            .iter()
            .map(|subsystem| subsystem.name.to_string())
            .collect()
    }

    fn append_runtime_rust_sources(dir: &Path, buffer: &mut String) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if path.is_dir() {
                append_runtime_rust_sources(&path, buffer);
                continue;
            }

            let is_rust = path.extension().is_some_and(|extension| extension == "rs");
            if !is_rust {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if file_name.ends_with("_tests.rs") || file_name == "tests.rs" {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(path) {
                buffer.push_str(&content);
                buffer.push('\n');
            }
        }
    }

    fn gateway_runtime_source() -> String {
        let mut source =
            std::fs::read_to_string(manifest_path("src/main.rs")).expect("read src/main.rs");
        source.push('\n');
        append_runtime_rust_sources(&manifest_path("src/server"), &mut source);
        append_runtime_rust_sources(&manifest_path("src/bin"), &mut source);
        source
    }

    fn runtime_source_has_core_module_reference(source: &str, module: &str) -> bool {
        let pattern = format!("core::{module}");
        source.match_indices(&pattern).any(|(index, _)| {
            let after_match = index + pattern.len();
            source[after_match..]
                .chars()
                .next()
                .is_none_or(|ch| !is_rust_identifier_char(ch))
        })
    }

    fn is_rust_identifier_char(ch: char) -> bool {
        ch == '_' || ch.is_ascii_alphanumeric()
    }

    #[test]
    fn registry_is_sorted_by_module_name() {
        let mut previous = "";
        for subsystem in CORE_SUBSYSTEMS {
            assert!(
                subsystem.name > previous,
                "CORE_SUBSYSTEMS must stay sorted by module name"
            );
            previous = subsystem.name;
        }
    }

    #[test]
    fn every_exported_core_module_has_decision() {
        assert_eq!(exported_core_modules(), registry_names());
    }

    #[test]
    fn exported_modules_are_referenced_or_exempted() {
        let runtime_source = gateway_runtime_source();
        for module in exported_core_modules() {
            let subsystem = subsystem_for(&module).expect("registry is exhaustive");
            if runtime_source_has_core_module_reference(&runtime_source, &module) {
                continue;
            }

            assert!(
                subsystem.has_gateway_reference_exemption(),
                "core::{module} is exported but not referenced by server/main/bin and has no exemption"
            );
        }
    }

    #[test]
    fn gh838_temporary_exemptions_match_explicit_issue_baseline() {
        assert_eq!(GH838_TEMPORARY_EXEMPTION_ISSUE, 838);

        let baseline: BTreeSet<&str> = GH838_TEMPORARY_EXEMPTIONS.iter().copied().collect();
        for name in &baseline {
            let Some(subsystem) = subsystem_for(name) else {
                panic!("temporary exemption {name} exists");
            };
            assert_eq!(
                subsystem.decision,
                SubsystemDecision::TemporaryExemption,
                "{name} must use the GH838 temporary exemption decision"
            );
        }

        for subsystem in CORE_SUBSYSTEMS {
            if subsystem.decision == SubsystemDecision::TemporaryExemption {
                assert!(
                    baseline.contains(subsystem.name),
                    "{} must be added to the explicit GH838 temporary exemption baseline",
                    subsystem.name
                );
            }
        }
    }

    #[test]
    fn issue_838_subsystems_have_explicit_non_silent_decisions() {
        let expected = [
            ("a2a", SubsystemDecision::TemporaryExemption),
            ("analytics", SubsystemDecision::TemporaryExemption),
            ("audit", SubsystemDecision::ConfigRejected),
            ("batch", SubsystemDecision::TemporaryExemption),
            ("guardrails", SubsystemDecision::TemporaryExemption),
            ("integrations", SubsystemDecision::TemporaryExemption),
            ("ip_access", SubsystemDecision::TemporaryExemption),
            ("mcp", SubsystemDecision::TemporaryExemption),
            ("observability", SubsystemDecision::TemporaryExemption),
            ("realtime", SubsystemDecision::TemporaryExemption),
            ("semantic_cache", SubsystemDecision::ConfigRejected),
            ("virtual_keys", SubsystemDecision::TemporaryExemption),
            ("webhooks", SubsystemDecision::TemporaryExemption),
        ];

        for (name, decision) in expected {
            let subsystem = subsystem_for(name).expect("subsystem decision exists");
            assert_eq!(
                subsystem.decision, decision,
                "unexpected decision for {name}"
            );
            assert!(
                !subsystem.note.trim().is_empty(),
                "{name} must explain the decision"
            );
        }
    }
}
