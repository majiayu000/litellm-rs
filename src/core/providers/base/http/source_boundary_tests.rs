use syn::visit::{self, Visit};
use syn::{ItemExternCrate, ItemMod, ItemUse, Path, UseTree};

const ALLOWED_BASE_SYMBOLS: &[&str] = &[
    "BaseConfig",
    "BaseHttpClient",
    "HttpErrorMapper",
    "OpenAIRequestTransformer",
    "UrlBuilder",
    "apply_provider_headers",
    "create_provider_sse_stream",
    "get_pricing_db",
    "header",
    "header_owned",
    "header_static",
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
            prefix.push(path.ident.to_string());
            flatten_use_tree(&path.tree, prefix, paths);
            prefix.pop();
        }
        UseTree::Name(name) => {
            if name.ident == "self" {
                paths.push(prefix.clone());
            } else {
                let mut path = prefix.clone();
                path.push(name.ident.to_string());
                paths.push(path);
            }
        }
        UseTree::Rename(rename) => {
            let mut path = prefix.clone();
            if rename.ident != "self" {
                path.push(rename.ident.to_string());
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
                .map(|segment| segment.ident.to_string())
                .collect(),
            false,
        );
        visit::visit_path(self, path);
    }

    fn visit_item_extern_crate(&mut self, item: &'ast ItemExternCrate) {
        if item.ident == "self" || item.ident == "reqwest" || item.ident == "litellm_rs" {
            self.violations
                .push(format!("forbidden extern crate alias for {}", item.ident));
        }
    }

    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        let is_test = item.attrs.iter().any(|attribute| {
            attribute.path().is_ident("cfg")
                && attribute
                    .parse_args::<syn::Ident>()
                    .is_ok_and(|ident| ident == "test")
        });
        if is_test {
            return;
        }
        if let Some((_, items)) = &item.content {
            self.module_path.push(item.ident.to_string());
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
            "bedrock/client.rs",
            &["crate", "core", "providers", "bedrock", "client"],
            include_str!("../../bedrock/client.rs"),
        ),
        (
            "bedrock/client/target.rs",
            &["crate", "core", "providers", "bedrock", "client", "target"],
            include_str!("../../bedrock/client/target.rs"),
        ),
        (
            "bedrock/agents/mod.rs",
            &["crate", "core", "providers", "bedrock", "agents"],
            include_str!("../../bedrock/agents/mod.rs"),
        ),
        (
            "bedrock/batch/mod.rs",
            &["crate", "core", "providers", "bedrock", "batch"],
            include_str!("../../bedrock/batch/mod.rs"),
        ),
        (
            "bedrock/guardrails/mod.rs",
            &["crate", "core", "providers", "bedrock", "guardrails"],
            include_str!("../../bedrock/guardrails/mod.rs"),
        ),
        (
            "bedrock/knowledge_bases/mod.rs",
            &["crate", "core", "providers", "bedrock", "knowledge_bases"],
            include_str!("../../bedrock/knowledge_bases/mod.rs"),
        ),
    ];
    let allowed = boundary_violations(
        "use crate::core::providers::base::{BaseConfig, BaseHttpClient, header};",
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
    let no_redirect_constructor = ["ProviderHttpClient::", "no_redirect"].concat();
    assert!(base_source.contains(&no_redirect_constructor));
    assert_eq!(
        include_str!("../../bedrock/client.rs")
            .matches("new_for_provider_no_redirect")
            .count(),
        3
    );
}
