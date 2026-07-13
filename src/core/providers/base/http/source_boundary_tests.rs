use std::fs;
use std::path::Path as FsPath;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{ExprMethodCall, ItemExternCrate, ItemMacro, ItemMod, ItemUse, Path, UseTree};

const ALLOWED_BASE_SYMBOLS: &[&str] = &[
    "BaseConfig",
    "BaseHttpClient",
    "HeaderPair",
    "HttpErrorMapper",
    "HttpMethod",
    "OpenAIRequestTransformer",
    "ProviderRequestBuilder",
    "SSETransformer",
    "UnifiedSSEStream",
    "UrlBuilder",
    "apply_provider_headers",
    "create_provider_sse_stream",
    "get_pricing_db",
    "header",
    "header_owned",
    "header_static",
    "read_streaming_error_body",
];
const BASE_PREFIX: &[&str] = &["crate", "core", "providers", "base"];
const RAW_PREFIXES: &[&[&str]] = &[
    &["crate", "core", "http"],
    &["crate", "utils", "net"],
    &["crate", "core", "providers", "base", "connection_pool"],
    &["reqwest", "Client"],
    &["reqwest", "ClientBuilder"],
    &["reqwest", "get"],
    &["reqwest", "request"],
];

fn ident_text(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
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

fn path_violation(path: &[String], is_import: bool) -> Option<String> {
    if RAW_PREFIXES
        .iter()
        .any(|prefix| path_starts_with(path, prefix))
    {
        return Some(format!("raw HTTP path {}", path.join("::")));
    }
    if path_starts_with(path, BASE_PREFIX) {
        let symbol = path.get(BASE_PREFIX.len()).map(String::as_str);
        if symbol.is_none() || !ALLOWED_BASE_SYMBOLS.contains(&symbol.unwrap_or_default()) {
            return Some(format!("unapproved base path {}", path.join("::")));
        }
    }
    if is_import
        && (RAW_PREFIXES
            .iter()
            .any(|prefix| path_is_prefix_of(path, prefix))
            || path_is_prefix_of(path, BASE_PREFIX))
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

struct BoundaryVisitor {
    module_path: Vec<String>,
    violations: Vec<String>,
}

impl BoundaryVisitor {
    fn check_segments(&mut self, segments: Vec<String>, is_import: bool) {
        if let Some(path) = canonicalize_path(&self.module_path, &segments)
            && let Some(violation) = path_violation(&path, is_import)
        {
            self.violations.push(violation);
        }
    }
}

impl<'ast> Visit<'ast> for BoundaryVisitor {
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut paths = Vec::new();
        flatten_use_tree(&item.tree, &mut Vec::new(), &mut paths);
        for path in paths {
            self.check_segments(path, true);
        }
    }

    fn visit_path(&mut self, path: &'ast Path) {
        self.check_segments(
            path.segments
                .iter()
                .map(|segment| ident_text(&segment.ident))
                .collect(),
            false,
        );
        visit::visit_path(self, path);
    }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        if ident_text(&expression.method) == "client" && expression.args.is_empty() {
            self.violations
                .push("raw HTTP client accessor .client()".to_string());
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_item_macro(&mut self, item: &'ast ItemMacro) {
        let tokens = item.mac.tokens.to_string();
        for forbidden in [
            "reqwest :: Client",
            "reqwest :: ClientBuilder",
            "GlobalPoolManager",
            "default_outbound_client",
            "get_client_with_timeout",
            "get_ssrf_safe_client",
            "use_ssrf_safe_client",
        ] {
            if tokens.contains(forbidden) {
                self.violations
                    .push(format!("raw HTTP macro token {forbidden}"));
            }
        }
        visit::visit_item_macro(self, item);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        let ident = ident_text(&item.ident);
        if matches!(ident.as_str(), "self" | "reqwest" | "litellm_rs") {
            self.violations
                .push(format!("forbidden extern crate alias for {ident}"));
        }
    }

    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        let is_test = item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident_text(&ident) == "test")
        });
        if is_test {
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
        violations: Vec::new(),
    };
    visitor.visit_file(&file);
    Ok(visitor.violations)
}

fn collect_provider_sources(
    root: &FsPath,
    directory: &FsPath,
    module_path: &[String],
    output: &mut Vec<(String, Vec<String>, String)>,
) -> std::io::Result<()> {
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if entry.file_name() == "provider_tests" {
                continue;
            }
            let mut child_module = module_path.to_vec();
            child_module.push(entry.file_name().to_string_lossy().into_owned());
            collect_provider_sources(root, &path, &child_module, output)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem == "tests" || stem == "provider_tests" || stem.ends_with("_tests") {
            continue;
        }
        let mut file_module = module_path.to_vec();
        if stem != "mod" {
            file_module.push(stem.to_string());
        }
        let relative = path
            .strip_prefix(root)
            .map_err(std::io::Error::other)?
            .display()
            .to_string();
        output.push((relative, file_module, fs::read_to_string(path)?));
    }
    Ok(())
}

#[test]
fn migrated_shared_providers_have_no_raw_client_escape() {
    let base_source = include_str!("../http.rs");
    let provider_sources: [(&str, &[&str], &str); 8] = [
        (
            "mistral/mod.rs",
            &["crate", "core", "providers", "mistral"],
            include_str!("../../mistral/mod.rs"),
        ),
        (
            "cohere/provider.rs",
            &["crate", "core", "providers", "cohere", "provider"],
            include_str!("../../cohere/provider.rs"),
        ),
        (
            "macros/http_hooks.rs",
            &["crate", "core", "providers", "macros", "http_hooks"],
            include_str!("../../macros/http_hooks.rs"),
        ),
        (
            "macros/pooled_hooks.rs",
            &["crate", "core", "providers", "macros", "pooled_hooks"],
            include_str!("../../macros/pooled_hooks.rs"),
        ),
        (
            "custom_api/provider.rs",
            &["crate", "core", "providers", "custom_api", "provider"],
            include_str!("../../custom_api/provider.rs"),
        ),
        (
            "amazon_nova/provider.rs",
            &["crate", "core", "providers", "amazon_nova", "provider"],
            include_str!("../../amazon_nova/provider.rs"),
        ),
        (
            "openai/api_methods.rs",
            &["crate", "core", "providers", "openai", "api_methods"],
            include_str!("../../openai/api_methods.rs"),
        ),
        (
            "router/health_probe.rs",
            &["crate", "core", "router", "health_probe"],
            include_str!("../../../router/health_probe.rs"),
        ),
    ];
    let allowed = boundary_violations(
        "use crate::core::providers::base::{BaseConfig, r#BaseHttpClient, header};",
        &["crate", "core", "providers", "mistral"],
    )
    .unwrap_or_else(|error| panic!("allowed fixture must parse: {error}"));
    assert!(allowed.is_empty());
    for bypass in [
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
    ] {
        let violations = boundary_violations(bypass, &["crate", "core", "providers", "mistral"])
            .unwrap_or_else(|error| panic!("bypass fixture must parse: {error}"));
        assert!(!violations.is_empty(), "bypass was not rejected: {bypass}");
    }

    for (path, module_path, source) in provider_sources {
        let violations = boundary_violations(source, module_path)
            .unwrap_or_else(|error| panic!("{path} must parse: {error}"));
        assert!(violations.is_empty(), "{path}: {}", violations.join("; "));
    }
    let bedrock_root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers/bedrock");
    let mut bedrock_sources = Vec::new();
    collect_provider_sources(
        &bedrock_root,
        &bedrock_root,
        &[
            "crate".to_string(),
            "core".to_string(),
            "providers".to_string(),
            "bedrock".to_string(),
        ],
        &mut bedrock_sources,
    )
    .unwrap_or_else(|error| panic!("Bedrock source inventory failed: {error}"));
    assert!(!bedrock_sources.is_empty());
    for (path, module_path, source) in bedrock_sources {
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path)
            .unwrap_or_else(|error| panic!("bedrock/{path} must parse: {error}"));
        assert!(
            violations.is_empty(),
            "bedrock/{path}: {}",
            violations.join("; ")
        );
    }
    let azure_root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers/azure");
    let mut azure_sources = Vec::new();
    collect_provider_sources(
        &azure_root,
        &azure_root,
        &[
            "crate".to_string(),
            "core".to_string(),
            "providers".to_string(),
            "azure".to_string(),
        ],
        &mut azure_sources,
    )
    .unwrap_or_else(|error| panic!("Azure source inventory failed: {error}"));
    assert!(!azure_sources.is_empty());
    for (path, module_path, source) in azure_sources {
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path)
            .unwrap_or_else(|error| panic!("azure/{path} must parse: {error}"));
        assert!(
            violations.is_empty(),
            "azure/{path}: {}",
            violations.join("; ")
        );
    }
    let azure_ai_root = FsPath::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers/azure_ai");
    let mut azure_ai_sources = Vec::new();
    collect_provider_sources(
        &azure_ai_root,
        &azure_ai_root,
        &[
            "crate".to_string(),
            "core".to_string(),
            "providers".to_string(),
            "azure_ai".to_string(),
        ],
        &mut azure_ai_sources,
    )
    .unwrap_or_else(|error| panic!("AzureAI source inventory failed: {error}"));
    assert!(!azure_ai_sources.is_empty());
    for (path, module_path, source) in azure_ai_sources {
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path)
            .unwrap_or_else(|error| panic!("azure_ai/{path} must parse: {error}"));
        assert!(
            violations.is_empty(),
            "azure_ai/{path}: {}",
            violations.join("; ")
        );
    }
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
