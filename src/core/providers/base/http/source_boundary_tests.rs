use std::fs;
use std::path::Path as FsPath;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprMethodCall, ImplItemFn, ItemExternCrate, ItemFn, ItemMacro, ItemMod, ItemUse, Local,
    Macro, Pat, Path, Type, UseTree,
};

const RAW_PREFIXES: &[&[&str]] = &[
    &["crate", "core", "http"],
    &["crate", "utils", "net"],
    &["crate", "core", "providers", "base", "connection_pool"],
    &["crate", "core", "providers", "base", "ConnectionPool"],
    &["crate", "core", "providers", "base", "apply_headers"],
    &["crate", "core", "providers", "base", "global_client"],
    &[
        "crate",
        "core",
        "providers",
        "base",
        "send_streaming_request",
    ],
    &[
        "crate",
        "core",
        "providers",
        "base",
        "send_streaming_request_with_timeout",
    ],
    &["crate", "core", "providers", "base", "streaming_client"],
    &[
        "crate",
        "core",
        "providers",
        "base",
        "streaming_unbounded_client",
    ],
    &["reqwest", "Client"],
    &["reqwest", "ClientBuilder"],
    &["reqwest", "get"],
    &["reqwest", "request"],
];

struct BoundaryException {
    path: &'static str,
    violations: &'static [&'static str],
    purpose: &'static str,
}

const UNIFIED_HTTP_IMPLEMENTATION: &str = "src/core/providers/base/http.rs";
const BOUNDARY_EXCEPTIONS: &[BoundaryException] = &[
    BoundaryException {
        path: "src/core/providers/base/connection_pool.rs",
        violations: &[
            "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client",
            "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client",
            "<module>: raw HTTP path crate::utils::net::http::HttpClientPoolConfig",
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client_with_config",
            "<module>: raw HTTP path crate::utils::net::http::create_streaming_client",
            "<module>: raw HTTP path reqwest::Client",
            "client: raw HTTP client accessor .client()",
            "execute_request: raw HTTP client accessor .client()",
        ],
        purpose: "unified pool implementation retains legacy fixed-endpoint clients beside the policy manager",
    },
    BoundaryException {
        path: "src/core/providers/base/mod.rs",
        violations: &["<module>: raw HTTP path crate::utils::net::http::ProviderRequestBuilder"],
        purpose: "unified provider HTTP wrapper re-export",
    },
    BoundaryException {
        path: "src/core/providers/cloudflare/provider.rs",
        violations: &["new: legacy GlobalPoolManager::new() constructor"],
        purpose: "native runtime is restricted by the factory to its account-scoped official endpoint",
    },
    BoundaryException {
        path: "src/core/providers/codestral/provider.rs",
        violations: &[
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request",
            "new: legacy GlobalPoolManager::new() constructor",
        ],
        purpose: "unwired lifecycle stub with no Gateway factory owner",
    },
    BoundaryException {
        path: "src/core/providers/fal_ai/provider.rs",
        violations: &[
            "<module>: raw HTTP path crate::core::providers::base::apply_headers",
            "execute_image_request: raw HTTP client accessor .client()",
            "new: legacy GlobalPoolManager::new() constructor",
        ],
        purpose: "native runtime is restricted by the factory to the fixed https://fal.run endpoint",
    },
    BoundaryException {
        path: "src/core/providers/github/provider.rs",
        violations: &[
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::read_streaming_error_body",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request",
            "new: legacy GlobalPoolManager::new() constructor",
        ],
        purpose: "unwired lifecycle stub; Gateway uses the policy-wired catalog route",
    },
    BoundaryException {
        path: "src/core/providers/github_copilot/authenticator.rs",
        violations: &[
            "perform_device_flow: raw HTTP path crate::core::http::outbound::default_outbound_client",
            "refresh_api_key: raw HTTP path crate::core::http::outbound::default_outbound_client",
        ],
        purpose: "fixed GitHub OAuth and token exchange endpoints, outside provider API base configuration",
    },
    BoundaryException {
        path: "src/core/providers/github_copilot/provider.rs",
        violations: &[
            "chat_completion: raw HTTP path crate::core::http::outbound::default_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::read_streaming_error_body",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request",
            "embeddings: raw HTTP path crate::core::http::outbound::default_outbound_client",
        ],
        purpose: "service-discovered Copilot endpoint; factory rejects caller-configured endpoint access",
    },
    BoundaryException {
        path: "src/core/providers/macros/openai_compatible.rs",
        violations: &[
            "define_openai_compatible_provider: raw HTTP macro token get_client_with_timeout",
            "define_openai_compatible_provider: raw HTTP macro token reqwest :: Client",
        ],
        purpose: "retained but uninvoked legacy lifecycle macro",
    },
    BoundaryException {
        path: "src/core/providers/macros/provider_definitions.rs",
        violations: &[
            "standard_provider: raw HTTP macro token create_custom_client",
            "standard_provider: raw HTTP macro token reqwest :: Client",
        ],
        purpose: "retained but uninvoked legacy lifecycle macro",
    },
    BoundaryException {
        path: "src/core/providers/meta_llama/common_utils.rs",
        violations: &[
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client",
            "<module>: raw HTTP path reqwest::Client",
        ],
        purpose: "unwired lifecycle stub; Gateway uses the policy-wired catalog route",
    },
    BoundaryException {
        path: "src/core/providers/ollama/provider.rs",
        violations: &[
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::read_streaming_error_body",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request",
            "new: legacy GlobalPoolManager::new() constructor",
        ],
        purpose: "unwired lifecycle stub; Gateway uses the policy-wired catalog route",
    },
    BoundaryException {
        path: "src/core/providers/replicate/provider.rs",
        violations: &[
            "<module>: raw HTTP path crate::core::providers::base::apply_headers",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::read_streaming_error_body",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request_with_timeout",
            "new: legacy GlobalPoolManager::new() constructor",
        ],
        purpose: "native runtime is restricted by the factory to the fixed Replicate API endpoint",
    },
    BoundaryException {
        path: "src/core/providers/v0/mod.rs",
        violations: &[
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client",
            "<module>: raw HTTP path reqwest::Client",
            "new_or_default: raw HTTP path crate::core::http::outbound::default_outbound_client",
        ],
        purpose: "unwired lifecycle stub with no Gateway factory owner",
    },
    BoundaryException {
        path: "src/core/providers/vertex_ai/auth.rs",
        violations: &[
            "<module>: raw HTTP path reqwest::Client",
            "new: raw HTTP path crate::core::http::outbound::default_outbound_client",
        ],
        purpose: "fixed Google service-account token exchange, not the configurable Vertex API base",
    },
];

fn ident_text(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
}

fn has_test_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        let syn::Meta::List(meta) = &attribute.meta else {
            return false;
        };
        let cfg = meta.tokens.to_string().replace(' ', "");
        attribute.path().is_ident("cfg")
            && (cfg == "test"
                || cfg
                    .strip_prefix("all(")
                    .and_then(|cfg| cfg.strip_suffix(')'))
                    .is_some_and(|cfg| cfg.split(',').any(|term| term == "test")))
    })
}

fn path_starts_with(path: &[String], prefix: &[&str]) -> bool {
    path.len() >= prefix.len()
        && path
            .iter()
            .zip(prefix)
            .all(|(segment, expected)| segment == expected)
}

fn path_is_prefix_of(path: &[String], target: &[&str]) -> bool {
    path.len() <= target.len()
        && path
            .iter()
            .zip(target)
            .all(|(segment, expected)| segment == expected)
}

fn canonicalize_path(module_path: &[String], path: &[String]) -> Option<Vec<String>> {
    let first = path.first()?;
    if first == "crate" || first == "reqwest" {
        return Some(path.to_vec());
    }

    let mut canonical = module_path.to_vec();
    let mut index = 0;
    if first == "self" {
        index = 1;
    } else if first == "super" {
        while path.get(index).is_some_and(|segment| segment == "super") {
            if canonical.len() <= 1 {
                return None;
            }
            canonical.pop();
            index += 1;
        }
    } else {
        return None;
    }
    canonical.extend(path[index..].iter().cloned());
    Some(canonical)
}

fn path_violation(path: &[String], is_import: bool, module_path: &[String]) -> Option<String> {
    let connection_pool = &["crate", "core", "providers", "base", "connection_pool"];
    let internal_connection_pool =
        path_starts_with(module_path, connection_pool) && path_starts_with(path, connection_pool);
    if RAW_PREFIXES
        .iter()
        .any(|prefix| path_starts_with(path, prefix))
        && !internal_connection_pool
    {
        return Some(format!("raw HTTP path {}", path.join("::")));
    }
    if is_import
        && (RAW_PREFIXES
            .iter()
            .any(|prefix| path_is_prefix_of(path, prefix)))
    {
        return Some(format!("raw HTTP ancestor import {}", path.join("::")));
    }
    None
}

fn flatten_use_tree(tree: &UseTree, prefix: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
    match tree {
        UseTree::Path(path) => {
            prefix.push(ident_text(&path.ident));
            flatten_use_tree(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            let ident = ident_text(&name.ident);
            if ident == "self" {
                paths.push(prefix.clone());
            } else {
                let mut path = prefix.clone();
                path.push(ident);
                paths.push(path);
            }
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            let ident = ident_text(&rename.ident);
            if ident != "self" {
                path.push(ident);
            }
            paths.push(path);
        }
        UseTree::Glob(_) => paths.push(prefix.clone()),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, prefix, paths);
            }
        }
    }
}

fn collect_manager_aliases(tree: &UseTree, matched: bool, aliases: &mut Vec<String>) {
    match tree {
        UseTree::Path(path) => collect_manager_aliases(
            &path.tree,
            matched || ident_text(&path.ident) == "GlobalPoolManager",
            aliases,
        ),
        UseTree::Name(name) if matched || ident_text(&name.ident) == "GlobalPoolManager" => {
            aliases.push(ident_text(&name.ident));
        }
        UseTree::Rename(rename) if matched || ident_text(&rename.ident) == "GlobalPoolManager" => {
            aliases.push(ident_text(&rename.rename));
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_manager_aliases(item, matched, aliases);
            }
        }
        _ => {}
    }
}

struct BoundaryVisitor {
    module_path: Vec<String>,
    context: Vec<String>,
    violations: Vec<String>,
    manager_aliases: Vec<String>,
}

impl BoundaryVisitor {
    fn record(&mut self, violation: String) {
        let context = self
            .context
            .last()
            .map(String::as_str)
            .unwrap_or("<module>");
        self.violations.push(format!("{context}: {violation}"));
    }

    fn check_segments(&mut self, segments: Vec<String>, is_import: bool) {
        if let Some(path) = canonicalize_path(&self.module_path, &segments)
            && let Some(violation) = path_violation(&path, is_import, &self.module_path)
        {
            self.record(violation);
        }
    }
}

impl<'ast> Visit<'ast> for BoundaryVisitor {
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        collect_manager_aliases(&item.tree, false, &mut self.manager_aliases);
        let mut paths = Vec::new();
        flatten_use_tree(&item.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.check_segments(path, true);
        }
    }

    fn visit_path(&mut self, path: &'ast Path) {
        let segments: Vec<_> = path
            .segments
            .iter()
            .map(|segment| ident_text(&segment.ident))
            .collect();
        self.check_segments(segments.clone(), false);
        if let Some(window) = segments.windows(2).find(|window| {
            self.manager_aliases.contains(&window[0])
                && matches!(window[1].as_str(), "new" | "shared" | "default")
        }) {
            self.record(format!(
                "legacy GlobalPoolManager::{}() constructor",
                window[1]
            ));
        }
        visit::visit_path(self, path);
    }

    fn visit_local(&mut self, local: &'ast Local) {
        if let Pat::Type(pattern) = &local.pat
            && let Type::Path(ty) = &*pattern.ty
            && ty
                .path
                .segments
                .last()
                .is_some_and(|segment| self.manager_aliases.contains(&ident_text(&segment.ident)))
            && let Some(initializer) = &local.init
            && let Expr::Call(call) = &*initializer.expr
            && let Expr::Path(function) = &*call.func
        {
            let segments: Vec<_> = function
                .path
                .segments
                .iter()
                .map(|segment| ident_text(&segment.ident))
                .collect();
            if segments.ends_with(&["Default".to_string(), "default".to_string()]) {
                self.record("legacy GlobalPoolManager Default::default() constructor".to_string());
            }
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        if ident_text(&expression.method) == "client" && expression.args.is_empty() {
            self.record("raw HTTP client accessor .client()".to_string());
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        self.context.push(ident_text(&item.sig.ident));
        visit::visit_item_fn(self, item);
        self.context.pop();
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        self.context.push(ident_text(&item.sig.ident));
        visit::visit_impl_item_fn(self, item);
        self.context.pop();
    }

    fn visit_item_macro(&mut self, item: &'ast ItemMacro) {
        self.context.push(
            item.ident
                .as_ref()
                .map(ident_text)
                .unwrap_or_else(|| "<macro>".to_string()),
        );
        visit::visit_item_macro(self, item);
        self.context.pop();
    }

    fn visit_macro(&mut self, item: &'ast Macro) {
        let tokens = item.tokens.to_string();
        for forbidden in [
            "reqwest :: Client",
            "reqwest :: ClientBuilder",
            "GlobalPoolManager",
            "create_custom_client",
            "create_streaming_client",
            "default_outbound_client",
            "get_client_with_timeout",
            "get_ssrf_safe_client",
            "use_ssrf_safe_client",
        ] {
            if tokens.contains(forbidden) {
                self.record(format!("raw HTTP macro token {forbidden}"));
            }
        }
        visit::visit_macro(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        let ident = ident_text(&item.ident);
        if matches!(ident.as_str(), "self" | "reqwest" | "litellm_rs") {
            self.record(format!("forbidden extern crate alias for {ident}"));
        }
    }

    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        if has_test_cfg(&item.attrs) {
            return;
        }
        if let Some((_, items)) = &item.content {
            self.module_path.push(ident_text(&item.ident));
            for child in items {
                self.visit_item(child);
            }
            self.module_path.pop();
        }
    }
}

fn boundary_violations(source: &str, module_path: &[&str]) -> Result<Vec<String>, syn::Error> {
    let file = syn::parse_file(source)?;
    let mut visitor = BoundaryVisitor {
        module_path: module_path
            .iter()
            .map(|segment| (*segment).to_string())
            .collect(),
        context: Vec::new(),
        violations: Vec::new(),
        manager_aliases: vec!["GlobalPoolManager".to_string()],
    };
    visitor.visit_file(&file);
    Ok(visitor.violations)
}

fn is_test_only_module(path: &FsPath) -> std::io::Result<bool> {
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let name = if path.is_dir() {
        path.file_name()
    } else {
        path.file_stem()
    }
    .and_then(|name| name.to_str())
    .unwrap_or_default();
    let Some(owner) = [parent.join("mod.rs"), parent.with_extension("rs")]
        .into_iter()
        .find(|candidate| candidate.is_file() && candidate != path)
    else {
        return Ok(false);
    };
    let source = fs::read_to_string(owner)?;
    let file = syn::parse_file(&source)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))?;
    Ok(file.items.iter().any(|item| {
        matches!(item, syn::Item::Mod(module)
            if ident_text(&module.ident) == name
                && module.content.is_none()
                && has_test_cfg(&module.attrs))
    }))
}

fn collect_production_sources(
    repository_root: &FsPath,
    directory: &FsPath,
    module_path: &[String],
    output: &mut Vec<(String, Vec<String>, String)>,
) -> std::io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            let directory_name = entry.file_name();
            let directory_name = directory_name.to_string_lossy();
            if (directory_name == "tests"
                || directory_name == "provider_tests"
                || directory_name.ends_with("_tests"))
                && is_test_only_module(&path)?
            {
                continue;
            }
            let mut child_module = module_path.to_vec();
            child_module.push(directory_name.into_owned());
            collect_production_sources(repository_root, &path, &child_module, output)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if (stem == "tests" || stem == "provider_tests" || stem.ends_with("_tests"))
            && is_test_only_module(&path)?
        {
            continue;
        }
        let mut file_module = module_path.to_vec();
        if stem != "mod" {
            file_module.push(stem.to_string());
        }
        let relative = path
            .strip_prefix(repository_root)
            .map_err(std::io::Error::other)?
            .display()
            .to_string();
        output.push((relative, file_module, fs::read_to_string(path)?));
    }
    Ok(())
}

#[test]
fn provider_runtime_http_boundary_guard_rejects_forbidden_spellings() {
    let allowed = boundary_violations(
        "use crate::core::providers::base::{BaseConfig, r#BaseHttpClient, header};",
        &["crate", "core", "providers", "mistral"],
    )
    .unwrap_or_else(|error| panic!("allowed fixture must parse: {error}"));
    assert!(allowed.is_empty());
    for bypass in [
        "fn probe() { reqwest::Client::new(); }",
        "fn probe() { reqwest::Client::builder(); }",
        "fn probe() { reqwest::ClientBuilder::new(); }",
        "fn probe() { crate::utils::net::http::create_custom_client(timeout); }",
        "fn probe() { crate::utils::net::http::create_custom_client_with_config(timeout, config); }",
        "fn probe() { crate::utils::net::http::create_streaming_client(); }",
        "fn probe() { GlobalPoolManager::new(); }",
        "fn probe() { GlobalPoolManager::shared(); }",
        "use crate::core::providers::base::GlobalPoolManager as Pool; fn probe() { Pool::new(); }",
        "fn probe() { let pool: GlobalPoolManager = Default::default(); }",
        "use crate::core as raw_core; fn probe() { raw_core::http::default_outbound_client(); }",
        "use crate::utils as raw_utils; fn probe() { raw_utils::net::http::get_shared_client(); }",
        "use crate::core::{http as raw_http}; fn probe() { raw_http::default_outbound_client(); }",
        "use crate::core::providers::base::{BaseConfig, ConnectionPool};",
        "fn probe() { crate::core::providers::base::ConnectionPool::client(&pool); }",
        "fn probe() { super::super::http::default_outbound_client(); }",
        "use reqwest as raw_http; fn probe() { raw_http::Client::new(); }",
        "use crate::core::r#http::default_outbound_client;",
        "use crate::utils::r#net::http::get_shared_client;",
        "use r#reqwest::Client;",
        "use crate::core::providers::r#base::ConnectionPool;",
        "extern crate r#reqwest as raw_http;",
        "fn probe(pool: Pool) { pool.client(); }",
        "macro_rules! raw { () => { reqwest::Client::new() } }",
        "fn probe() { passthrough!(reqwest::Client::new()); }",
    ] {
        let violations = boundary_violations(bypass, &["crate", "core", "providers", "mistral"])
            .unwrap_or_else(|error| panic!("bypass fixture must parse: {error}"));
        assert!(!violations.is_empty(), "bypass was not rejected: {bypass}");
    }
}

fn source_exception_violations(path: &str) -> Vec<String> {
    let matches: Vec<_> = BOUNDARY_EXCEPTIONS
        .iter()
        .filter(|exception| exception.path == path)
        .collect();
    assert!(
        matches.len() <= 1,
        "{path}: duplicate boundary exception entry"
    );
    let mut violations: Vec<_> = matches
        .into_iter()
        .flat_map(|exception| {
            assert!(
                !exception.purpose.trim().is_empty(),
                "{path}: boundary exception must document a fixed purpose"
            );
            exception
                .violations
                .iter()
                .map(|violation| violation.to_string())
        })
        .collect();
    violations.sort();
    violations
}

fn collect_boundary_inventory() -> Vec<(String, Vec<String>, String)> {
    let repository_root = FsPath::new(env!("CARGO_MANIFEST_DIR"));
    let roots: [(&str, &[&str]); 2] = [
        ("src/core/providers", &["crate", "core", "providers"]),
        ("src/server/routes/ai", &["crate", "server", "routes", "ai"]),
    ];
    let mut sources = Vec::new();
    for (relative, module_path) in roots {
        collect_production_sources(
            repository_root,
            &repository_root.join(relative),
            &module_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect::<Vec<_>>(),
            &mut sources,
        )
        .unwrap_or_else(|error| panic!("{relative} source inventory failed: {error}"));
    }
    for (relative, module_path) in [
        (
            "src/core/router/health_probe.rs",
            &["crate", "core", "router", "health_probe"][..],
        ),
        (
            "src/core/fine_tuning/providers/openai.rs",
            &["crate", "core", "fine_tuning", "providers", "openai"][..],
        ),
        (
            "src/core/rerank/providers/cohere.rs",
            &["crate", "core", "rerank", "providers", "cohere"][..],
        ),
        (
            "src/core/rerank/providers/jina.rs",
            &["crate", "core", "rerank", "providers", "jina"][..],
        ),
    ] {
        sources.push((
            relative.to_string(),
            module_path
                .iter()
                .map(|segment| (*segment).to_string())
                .collect(),
            fs::read_to_string(repository_root.join(relative))
                .unwrap_or_else(|error| panic!("cannot read {relative}: {error}")),
        ));
    }
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

#[test]
fn provider_runtime_http_boundary_has_no_unapproved_bypass() {
    let base_source = include_str!("../http.rs");
    let sources = collect_boundary_inventory();
    assert!(
        sources.len() > 300,
        "provider/runtime inventory is incomplete"
    );
    for exception in BOUNDARY_EXCEPTIONS {
        assert!(
            sources.iter().any(|(path, _, _)| path == exception.path),
            "stale boundary exception for {}",
            exception.path
        );
    }
    assert!(
        sources
            .iter()
            .any(|(path, _, _)| path == UNIFIED_HTTP_IMPLEMENTATION),
        "unified HTTP implementation is missing from the inventory"
    );
    let mut failures = Vec::new();
    for (path, module_path, source) in sources {
        if path == UNIFIED_HTTP_IMPLEMENTATION {
            continue;
        }
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path)
            .unwrap_or_else(|error| panic!("{path} must parse: {error}"));
        let mut violations = violations;
        violations.sort();
        let expected = source_exception_violations(&path);
        if violations != expected {
            failures.push(format!(
                "{path}: expected [{}], found [{}]",
                expected.join("; "),
                violations.join("; ")
            ));
        }
        if matches!(
            path.as_str(),
            "src/core/providers/openai/client.rs" | "src/core/providers/openai_like/provider.rs"
        ) {
            let compact = source.split_whitespace().collect::<String>();
            assert!(compact.contains("GlobalPoolManager::new_for_provider"));
            for forbidden in ["GlobalPoolManager::new()", ".client()"] {
                assert!(!compact.contains(forbidden), "{path}: {forbidden}");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    for pattern in [
        ["pub fn ", "inner("].concat(),
        ["pub fn ", "into_inner("].concat(),
        ["pub fn ", "client("].concat(),
        ["impl Deref for ", "BaseHttpClient"].concat(),
        ["pub ", "client:"].concat(),
    ] {
        assert!(
            !base_source.contains(&pattern),
            "BaseHttpClient exposes policy-bound internals through {pattern}"
        );
    }
    let compact_base: String = base_source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert!(compact_base.contains("BaseRedirectMode::Policy=>ProviderHttpClient::new"));
    assert!(compact_base.contains("BaseRedirectMode::Disabled=>ProviderHttpClient::no_redirect"));
    assert!(compact_base.contains("BaseRedirectMode::Streaming=>ProviderHttpClient::streaming"));
    assert!(
        !include_str!("../../../../utils/net/http.rs")
            .contains("get_ssrf_safe_no_redirect_client_with_timeout_fallible")
    );
    assert_eq!(
        include_str!("../../bedrock/client.rs")
            .matches("new_for_provider_no_redirect")
            .count(),
        3
    );
}

#[test]
fn provider_runtime_http_boundary_guard_is_wired_to_pr_and_main_ci() {
    for (path, workflow) in [
        (
            ".github/workflows/ci.yml",
            include_str!("../../../../../.github/workflows/ci.yml"),
        ),
        (
            ".github/workflows/ci-main-full.yml",
            include_str!("../../../../../.github/workflows/ci-main-full.yml"),
        ),
    ] {
        assert!(
            workflow.contains("bash scripts/guards/check_outbound_http_clients.sh"),
            "{path} must run the provider/runtime HTTP boundary guard"
        );
    }
}
