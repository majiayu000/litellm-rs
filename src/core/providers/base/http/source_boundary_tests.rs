use std::fs;
use std::path::Path as FsPath;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{
    Expr, ExprClosure, ExprMethodCall, ExprStruct, GenericArgument, ImplItemFn, ItemConst,
    ItemExternCrate, ItemFn, ItemMacro, ItemMod, ItemStatic, ItemStruct, ItemType, ItemUse, Local,
    Macro, Pat, Path, PathArguments, ReturnType, Type, UseTree,
};

#[rustfmt::skip]
const RAW_PREFIXES: &[&[&str]] = &[
    &["crate", "core", "http"],
    &["crate", "utils", "net"],
    &["crate", "core", "providers", "base", "connection_pool"],
    &["crate", "core", "providers", "base", "ConnectionPool"],
    &["crate", "core", "providers", "base", "apply_headers"],
    &["crate", "core", "providers", "base", "global_client"],
    &["crate", "core", "providers", "base", "send_streaming_request"],
    &["crate", "core", "providers", "base", "send_streaming_request_with_timeout"],
    &["crate", "core", "providers", "base", "streaming_client"],
    &["crate", "core", "providers", "base", "streaming_unbounded_client"],
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
macro_rules! exception {
    ($path:literal, $purpose:literal, [$($violation:literal),* $(,)?]) => {
        BoundaryException { path: $path, violations: &[$($violation),*], purpose: $purpose }
    };
}

#[rustfmt::skip]
const BOUNDARY_EXCEPTIONS: &[BoundaryException] = &[
    exception!("src/core/providers/base/connection_pool.rs", "unified pool implementation retains legacy fixed-endpoint clients beside the policy manager", [
            "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client", "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client",
            "<module>: raw HTTP path crate::utils::net::http::HttpClientPoolConfig", "<module>: raw HTTP path crate::utils::net::http::create_custom_client_with_config",
            "<module>: raw HTTP path crate::utils::net::http::create_streaming_client", "<module>: raw HTTP path reqwest::Client",
            "client: raw HTTP client accessor .client()", "execute_request: raw HTTP client accessor .client()",
    ]),
    exception!("src/core/providers/base/mod.rs", "unified provider HTTP wrapper re-export", ["<module>: raw HTTP path crate::utils::net::http::ProviderRequestBuilder"]),
    exception!("src/core/providers/cloudflare/provider.rs", "native runtime is restricted by the factory to its account-scoped official endpoint", ["new: legacy GlobalPoolManager::new() constructor"]),
    exception!("src/core/providers/codestral/provider.rs", "unwired lifecycle stub with no Gateway factory owner", [
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request", "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/fal_ai/provider.rs", "native runtime is restricted by the factory to the fixed https://fal.run endpoint", [
            "<module>: raw HTTP path crate::core::providers::base::apply_headers", "execute_image_request: raw HTTP client accessor .client()", "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/github/provider.rs", "unwired lifecycle stub; Gateway uses the policy-wired catalog route", [
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request", "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/github_copilot/authenticator.rs", "fixed GitHub OAuth and token exchange endpoints, outside provider API base configuration", [
            "perform_device_flow: raw HTTP path crate::core::http::outbound::default_outbound_client", "refresh_api_key: raw HTTP path crate::core::http::outbound::default_outbound_client",
    ]),
    exception!("src/core/providers/github_copilot/provider.rs", "service-discovered Copilot endpoint; factory rejects caller-configured endpoint access", [
            "chat_completion: raw HTTP path crate::core::http::outbound::default_outbound_client", "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request", "embeddings: raw HTTP path crate::core::http::outbound::default_outbound_client",
    ]),
    exception!("src/core/providers/macros/openai_compatible.rs", "retained but uninvoked legacy lifecycle macro", [
            "define_openai_compatible_provider: raw HTTP macro token crate :: utils :: net :: http :: get_client_with_timeout_fallible", "define_openai_compatible_provider: raw HTTP macro token get_client_with_timeout", "define_openai_compatible_provider: raw HTTP macro token reqwest :: Client",
    ]),
    exception!("src/core/providers/macros/provider_definitions.rs", "retained but uninvoked legacy lifecycle macro", [
            "standard_provider: raw HTTP macro token crate :: utils :: net :: http :: create_custom_client", "standard_provider: raw HTTP macro token create_custom_client", "standard_provider: raw HTTP macro token reqwest :: Client",
    ]),
    exception!("src/core/providers/meta_llama/common_utils.rs", "unwired lifecycle stub; Gateway uses the policy-wired catalog route", [
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client", "<module>: raw HTTP path reqwest::Client",
    ]),
    exception!("src/core/providers/ollama/provider.rs", "unwired lifecycle stub; Gateway uses the policy-wired catalog route", [
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request",
            "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/replicate/provider.rs", "native runtime is restricted by the factory to the fixed Replicate API endpoint", [
            "<module>: raw HTTP path crate::core::providers::base::apply_headers",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "build_request: raw HTTP client accessor .client()",
            "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client",
            "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request_with_timeout",
            "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/v0/mod.rs", "unwired lifecycle stub with no Gateway factory owner", [
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client",
            "<module>: raw HTTP path reqwest::Client",
            "new_or_default: raw HTTP path crate::core::http::outbound::default_outbound_client",
    ]),
    exception!("src/core/providers/vertex_ai/auth.rs", "fixed Google service-account token exchange, not the configurable Vertex API base", [
            "<module>: raw HTTP path reqwest::Client",
            "new: raw HTTP path crate::core::http::outbound::default_outbound_client",
    ]),
];

fn ident_text(ident: &syn::Ident) -> String {
    ident.unraw().to_string()
}

#[rustfmt::skip] fn has_test_cfg(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        let syn::Meta::List(meta) = &attribute.meta else { return false; };
        let cfg = meta.tokens.to_string().replace(' ', "");
        attribute.path().is_ident("cfg") && (cfg == "test" || cfg.strip_prefix("all(").and_then(|cfg| cfg.strip_suffix(')')).is_some_and(|cfg| cfg.split(',').any(|term| term == "test")))
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

#[rustfmt::skip]
fn macro_paths(tokens: &str) -> Vec<Vec<String>> {
    let parts: Vec<_> = tokens.split_whitespace().collect();
    let mut paths = Vec::new();
    for start in 0..parts.len() {
        let first = parts[start].trim_start_matches("r#");
        if !matches!(first, "crate" | "reqwest" | "self" | "super") { continue; }
        let (mut path, mut index) = (vec![first.to_string()], start + 1);
        while parts.get(index) == Some(&"::") {
            let Some(segment) = parts.get(index + 1) else { break; };
            let segment = segment.trim_start_matches("r#");
            if !segment.chars().all(|character| character == '_' || character.is_alphanumeric()) { break; }
            path.push(segment.to_string()); index += 2; }
        if path.len() > 1 { paths.push(path); }
    } paths
}

fn path_violation(path: &[String], is_import: bool, module_path: &[String]) -> Option<String> {
    let connection_pool = &["crate", "core", "providers", "base", "connection_pool"];
    let internal_connection_pool =
        path_starts_with(module_path, connection_pool) && path_starts_with(path, connection_pool);
    let response_reader = path_starts_with(path, connection_pool)
        && path.get(connection_pool.len()).is_some_and(|name| {
            matches!(
                name.as_str(),
                "read_streaming_error_body" | "read_streaming_error_body_with_limits"
            )
        });
    if RAW_PREFIXES
        .iter()
        .any(|prefix| path_starts_with(path, prefix))
        && !internal_connection_pool
        && !response_reader
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

#[rustfmt::skip]
fn collect_aliases(tree: &UseTree, target: &str, matched: bool, aliases: &mut Vec<String>) {
    match tree {
        UseTree::Path(path) => collect_aliases(&path.tree, target, matched || ident_text(&path.ident) == target, aliases),
        UseTree::Name(name) if matched || ident_text(&name.ident) == target => aliases.push(ident_text(&name.ident)),
        UseTree::Rename(rename) if matched || ident_text(&rename.ident) == target => aliases.push(ident_text(&rename.rename)),
        UseTree::Group(group) => group.items.iter().for_each(|item| collect_aliases(item, target, matched, aliases)),
        _ => {}
    }
}

fn is_manager_type(ty: &Type) -> bool {
    match ty {
        Type::Path(ty) => ty.path.segments.iter().any(|segment| segment.ident == "GlobalPoolManager" || matches!(&segment.arguments, PathArguments::AngleBracketed(args) if args.args.iter().any(|arg| matches!(arg, GenericArgument::Type(ty) if is_manager_type(ty))))),
        Type::BareFn(ty) => matches!(&ty.output, ReturnType::Type(_, ty) if is_manager_type(ty)), Type::Group(ty) => is_manager_type(&ty.elem), Type::Paren(ty) => is_manager_type(&ty.elem),
        Type::Reference(ty) => is_manager_type(&ty.elem), Type::Tuple(ty) => ty.elems.iter().any(is_manager_type),
        Type::Macro(ty) => ty.mac.tokens.to_string().contains("GlobalPoolManager"), _ => false,
    }
}

struct DefaultFinder<'a>(bool, &'a [String], &'a [String]);
#[rustfmt::skip]
impl<'ast> Visit<'ast> for DefaultFinder<'_> {
        fn visit_path(&mut self, path: &'ast Path) {
            let names: Vec<_> = path.segments.iter().map(|segment| ident_text(&segment.ident)).collect();
            self.0 |= (names.last().is_some_and(|name| name == "default") && names.iter().rev().nth(1).is_some_and(|name| name == "Default" || self.1.contains(name))) || (names.len() == 1 && self.2.contains(&names[0])); visit::visit_path(self, path);
        } }
#[rustfmt::skip] fn uses_default_or_local(expr: &Expr, aliases: &[String], locals: &[String]) -> bool { let mut finder = DefaultFinder(false, aliases, locals); finder.visit_expr(expr); finder.0 }
#[rustfmt::skip] fn uses_default(expr: &Expr, aliases: &[String]) -> bool { uses_default_or_local(expr, aliases, &[]) }
#[rustfmt::skip]
fn returns_manager(output: &ReturnType) -> bool { matches!(output, ReturnType::Type(_, ty) if is_manager_type(ty)) }

#[rustfmt::skip] fn item_attrs(item: &syn::Item) -> Option<&[syn::Attribute]> { match item { syn::Item::Const(item) => Some(&item.attrs), syn::Item::Enum(item) => Some(&item.attrs), syn::Item::ExternCrate(item) => Some(&item.attrs), syn::Item::Fn(item) => Some(&item.attrs), syn::Item::ForeignMod(item) => Some(&item.attrs), syn::Item::Impl(item) => Some(&item.attrs), syn::Item::Macro(item) => Some(&item.attrs), syn::Item::Mod(item) => Some(&item.attrs), syn::Item::Static(item) => Some(&item.attrs), syn::Item::Struct(item) => Some(&item.attrs), syn::Item::Trait(item) => Some(&item.attrs), syn::Item::TraitAlias(item) => Some(&item.attrs), syn::Item::Type(item) => Some(&item.attrs), syn::Item::Union(item) => Some(&item.attrs), syn::Item::Use(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn impl_item_attrs(item: &syn::ImplItem) -> Option<&[syn::Attribute]> { match item { syn::ImplItem::Const(item) => Some(&item.attrs), syn::ImplItem::Fn(item) => Some(&item.attrs), syn::ImplItem::Macro(item) => Some(&item.attrs), syn::ImplItem::Type(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn trait_item_attrs(item: &syn::TraitItem) -> Option<&[syn::Attribute]> { match item { syn::TraitItem::Const(item) => Some(&item.attrs), syn::TraitItem::Fn(item) => Some(&item.attrs), syn::TraitItem::Macro(item) => Some(&item.attrs), syn::TraitItem::Type(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn foreign_item_attrs(item: &syn::ForeignItem) -> Option<&[syn::Attribute]> { match item { syn::ForeignItem::Fn(item) => Some(&item.attrs), syn::ForeignItem::Macro(item) => Some(&item.attrs), syn::ForeignItem::Static(item) => Some(&item.attrs), syn::ForeignItem::Type(item) => Some(&item.attrs), _ => None } }

#[rustfmt::skip]
fn default_aliases(file: &syn::File) -> Vec<String> { struct Aliases(Vec<String>); impl<'ast> Visit<'ast> for Aliases { fn visit_item(&mut self, item: &'ast syn::Item) { if !item_attrs(item).is_some_and(has_test_cfg) { visit::visit_item(self, item); } } fn visit_item_use(&mut self, item: &'ast ItemUse) { collect_aliases(&item.tree, "Default", false, &mut self.0); } } let mut aliases = Aliases(Vec::new()); aliases.visit_file(file); aliases.0 }

#[rustfmt::skip]
fn manager_fields(source: &str) -> Result<Vec<String>, syn::Error> {
    struct Fields(Vec<String>);
    impl<'ast> Visit<'ast> for Fields {
        fn visit_item(&mut self, item: &'ast syn::Item) { if !item_attrs(item).is_some_and(has_test_cfg) { visit::visit_item(self, item); } }
        fn visit_item_struct(&mut self, item: &'ast ItemStruct) {
            self.0.extend(item.fields.iter().filter(|field| !has_test_cfg(&field.attrs) && is_manager_type(&field.ty)).filter_map(|field| field.ident.as_ref().map(ident_text)));
            visit::visit_item_struct(self, item); } }
    let mut fields = Fields(Vec::new()); fields.visit_file(&syn::parse_file(source)?); Ok(fields.0)
}

#[rustfmt::skip] struct BoundaryVisitor {
    module_path: Vec<String>,
    context: Vec<String>,
    violations: Vec<String>,
    manager_fields: Vec<String>, default_aliases: Vec<String>,
    default_locals: Vec<String>,
}

impl BoundaryVisitor {
    fn record(&mut self, violation: String) {
        let context = self.context.last().map_or("<module>", String::as_str);
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

#[rustfmt::skip]
impl<'ast> Visit<'ast> for BoundaryVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) { if item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_item(self, item); }
    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) { if impl_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_impl_item(self, item); }
    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) { if trait_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_trait_item(self, item); }
    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) { if foreign_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_foreign_item(self, item); }
    fn visit_field(&mut self, field: &'ast syn::Field) { if !has_test_cfg(&field.attrs) { visit::visit_field(self, field); } }
    fn visit_variant(&mut self, variant: &'ast syn::Variant) { if !has_test_cfg(&variant.attrs) { visit::visit_variant(self, variant); } }
    fn visit_arm(&mut self, arm: &'ast syn::Arm) { if !has_test_cfg(&arm.attrs) { visit::visit_arm(self, arm); } }
    fn visit_stmt_macro(&mut self, item: &'ast syn::StmtMacro) { if !has_test_cfg(&item.attrs) { visit::visit_stmt_macro(self, item); } }
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        let mut aliases = Vec::new();
        collect_aliases(&item.tree, "GlobalPoolManager", false, &mut aliases);
        aliases.retain(|alias| alias != "GlobalPoolManager");
        for alias in aliases {
            self.record(format!("legacy GlobalPoolManager alias {alias}"));
        }
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
            window[0] == "GlobalPoolManager"
                && matches!(window[1].as_str(), "new" | "shared" | "default")
        }) {
            self.record(format!(
                "legacy GlobalPoolManager::{}() constructor",
                window[1]
            ));
        }
        visit::visit_path(self, path);
    }

    fn visit_item_type(&mut self, item: &'ast ItemType) { if is_manager_type(&item.ty) { self.record(format!("legacy GlobalPoolManager alias {}", item.ident)); } visit::visit_item_type(self, item); }

    fn visit_impl_item_type(&mut self, item: &'ast syn::ImplItemType) { if is_manager_type(&item.ty) { self.record(format!("legacy GlobalPoolManager alias {}", item.ident)); } visit::visit_impl_item_type(self, item); }

    fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) {
        if ident_text(&expression.method) == "client" && expression.args.is_empty() {
            self.record("raw HTTP client accessor .client()".to_string());
        }
        visit::visit_expr_method_call(self, expression);
    }

    fn visit_expr_struct(&mut self, expression: &'ast ExprStruct) {
        for field in &expression.fields {
            let name = match &field.member { syn::Member::Named(name) => ident_text(name), syn::Member::Unnamed(_) => continue };
            if self.manager_fields.contains(&name) && uses_default_or_local(&field.expr, &self.default_aliases, &self.default_locals) {
                self.record(format!("policy-less Default construction for GlobalPoolManager field {name}"));
            }
        }
        visit::visit_expr_struct(self, expression);
    }

    fn visit_local(&mut self, local: &'ast Local) { if has_test_cfg(&local.attrs) { return; } let name = match &local.pat { Pat::Ident(pat) => Some(ident_text(&pat.ident)), Pat::Type(pat) => match &*pat.pat { Pat::Ident(pat) => Some(ident_text(&pat.ident)), _ => None }, _ => None }; if let Some(name) = &name { self.default_locals.retain(|local| local != name); } let inferred_default = local.init.as_ref().is_some_and(|init| uses_default_or_local(&init.expr, &self.default_aliases, &self.default_locals)); let manager = match &local.pat { Pat::Type(pat) => is_manager_type(&pat.ty), Pat::Ident(pat) => self.manager_fields.contains(&ident_text(&pat.ident)), _ => false }; if inferred_default { if manager { self.record("policy-less Default construction for GlobalPoolManager local".into()); } self.default_locals.extend(name); } visit::visit_local(self, local); }

    fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) { let name = match &*expression.left { Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => path.path.segments.first().map(|segment| ident_text(&segment.ident)), _ => None }; if let Some(name) = name { let inferred_default = uses_default_or_local(&expression.right, &self.default_aliases, &self.default_locals); self.default_locals.retain(|local| local != &name); if inferred_default { self.default_locals.push(name); } } visit::visit_expr_assign(self, expression); }

    fn visit_item_const(&mut self, item: &'ast ItemConst) { if is_manager_type(&item.ty) && uses_default(&item.expr, &self.default_aliases) { self.record("policy-less Default construction for GlobalPoolManager const".into()); } visit::visit_item_const(self, item); }

    fn visit_item_static(&mut self, item: &'ast ItemStatic) { if is_manager_type(&item.ty) && uses_default(&item.expr, &self.default_aliases) { self.record("policy-less Default construction for GlobalPoolManager static".into()); } visit::visit_item_static(self, item); }

    fn visit_expr_closure(&mut self, expression: &'ast ExprClosure) { if returns_manager(&expression.output) && uses_default(&expression.body, &self.default_aliases) { self.record("policy-less Default construction for GlobalPoolManager closure".into()); } let outer = self.default_locals.clone(); visit::visit_expr_closure(self, expression); self.default_locals = outer; }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) { self.context.push(ident_text(&item.sig.ident)); let mut defaults = DefaultFinder(false, &self.default_aliases, &[]); defaults.visit_block(&item.block); if returns_manager(&item.sig.output) && defaults.0 { self.record("policy-less Default construction for GlobalPoolManager return".into()); } let outer = std::mem::take(&mut self.default_locals); visit::visit_item_fn(self, item); self.default_locals = outer; self.context.pop(); }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) { self.context.push(ident_text(&item.sig.ident)); let mut defaults = DefaultFinder(false, &self.default_aliases, &[]); defaults.visit_block(&item.block); if returns_manager(&item.sig.output) && defaults.0 { self.record("policy-less Default construction for GlobalPoolManager return".into()); } let outer = std::mem::take(&mut self.default_locals); visit::visit_impl_item_fn(self, item); self.default_locals = outer; self.context.pop(); }

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
        for path in macro_paths(&tokens) {
            if let Some(path) = canonicalize_path(&self.module_path, &path)
                && path_violation(&path, false, &self.module_path).is_some()
            {
                self.record(format!("raw HTTP macro token {}", path.join(" :: ")));
            }
        }
        for forbidden in [
            "GlobalPoolManager", "create_custom_client", "create_streaming_client",
            "default_outbound_client", "get_client_with_timeout", "get_ssrf_safe_client",
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

#[rustfmt::skip]
fn boundary_violations(source: &str, module_path: &[&str], manager_fields: &[String]) -> Result<Vec<String>, syn::Error> {
    let file = syn::parse_file(source)?;
    let default_aliases = default_aliases(&file);
    let mut visitor = BoundaryVisitor {
        module_path: module_path.iter().map(|segment| (*segment).to_string()).collect(),
        context: Vec::new(), violations: Vec::new(), manager_fields: manager_fields.to_vec(), default_aliases, default_locals: Vec::new(),
    };
    visitor.visit_file(&file);
    Ok(visitor.violations)
}

#[rustfmt::skip]
fn declared_module(path: &FsPath, module_path: &[String]) -> std::io::Result<Option<(Vec<String>, bool)>> {
    let Some(parent) = path.parent() else { return Ok(None); }; let target = fs::canonicalize(path)?;
    let mut owners = fs::read_dir(parent)?.filter_map(Result::ok).map(|entry| entry.path()).filter(|candidate| candidate.extension().and_then(|value| value.to_str()) == Some("rs")).collect::<Vec<_>>();
    owners.extend([parent.join("mod.rs"), parent.with_extension("rs")]); owners.sort(); owners.dedup();
    for owner in owners.into_iter().filter(|owner| owner.is_file() && owner != path) {
        let file = syn::parse_file(&fs::read_to_string(&owner)?).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?; for item in file.items {
            let syn::Item::Mod(module) = item else { continue; }; let declared = module.attrs.iter().find_map(|attr| match &attr.meta {
                syn::Meta::NameValue(value) if attr.path().is_ident("path") => match &value.value { Expr::Lit(value) => match &value.lit { syn::Lit::Str(path) => Some(path.value()), _ => None }, _ => None }, _ => None });
            if declared.and_then(|declared| fs::canonicalize(owner.parent()?.join(declared)).ok()).as_ref() != Some(&target) { continue; }
            let mut resolved = module_path.to_vec();
            if owner != parent.with_extension("rs") && owner.file_name().and_then(|name| name.to_str()) != Some("mod.rs")
                && let Some(stem) = owner.file_stem().and_then(|stem| stem.to_str()) { resolved.push(stem.to_string()); }
            resolved.push(ident_text(&module.ident)); return Ok(Some((resolved, has_test_cfg(&module.attrs)))); } }
    Ok(None)
}

#[rustfmt::skip]
fn is_test_only_module(path: &FsPath, module_path: &[String]) -> std::io::Result<bool> {
    if let Some((_, test_only)) = declared_module(path, module_path)? { return Ok(test_only); }
    let Some(parent) = path.parent() else { return Ok(false); };
    let name = if path.is_dir() { path.file_name() } else { path.file_stem() }
        .and_then(|name| name.to_str()).unwrap_or_default();
    let Some(owner) = [parent.join("mod.rs"), parent.with_extension("rs")]
        .into_iter().find(|candidate| candidate.is_file() && candidate != path)
    else { return Ok(false); };
    let source = fs::read_to_string(owner)?;
    let file = syn::parse_file(&source).map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(file.items.iter().any(|item| matches!(item, syn::Item::Mod(module) if ident_text(&module.ident) == name && module.content.is_none() && has_test_cfg(&module.attrs))))
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
                && is_test_only_module(&path, module_path)?
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
            && is_test_only_module(&path, module_path)?
        {
            continue;
        }
        let declared = declared_module(&path, module_path)?;
        if declared.as_ref().is_some_and(|(_, test_only)| *test_only) {
            continue;
        }
        let mut file_module = declared
            .map(|(module, _)| module)
            .unwrap_or_else(|| module_path.to_vec());
        if stem != "mod" && file_module == module_path {
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
#[rustfmt::skip] fn provider_runtime_http_boundary_guard_rejects_forbidden_spellings() {
    assert!(manager_fields("struct Fixture { #[cfg(test)] pool_manager: crate::core::providers::base::GlobalPoolManager }").unwrap_or_else(|error| panic!("test-only manager field fixture must parse: {error}")).is_empty()); let manager_fields = vec!["pool_manager".to_string()];
    let allowed = boundary_violations(
        "use crate::core::providers::base::{BaseConfig, r#BaseHttpClient, header}; #[cfg(test)] fn mock() { reqwest::Client::new(); } struct Fixture { #[cfg(test)] client: Option<reqwest::Client> } enum Mode { #[cfg(test)] Mock(reqwest::Client), Prod } trait TestOnly { #[cfg(test)] type Pool; #[cfg(test)] fn raw() { reqwest::Client::new(); } } impl TestOnly for Fixture { #[cfg(test)] type Pool = crate::core::providers::base::GlobalPoolManager; #[cfg(test)] passthrough!(reqwest::Client::new()); } extern \"C\" { #[cfg(test)] fn raw(client: reqwest::Client); } fn nested() { #[cfg(test)] let client = reqwest::Client::new(); #[cfg(test)] passthrough!(reqwest::Client::new()); match false { #[cfg(test)] true => reqwest::Client::new(), false => todo!() }; }",
        &["crate", "core", "providers", "mistral"],
        &manager_fields,
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
        "fn probe() { Pool::new(); } use crate::core::providers::base::GlobalPoolManager as Pool;",
        "type Pool = crate::core::providers::base::GlobalPoolManager; fn probe() { Pool::shared(); }",
        "#[allow(unused_parens)] type Pool = (crate::core::providers::base::GlobalPoolManager); fn probe() { Pool::default(); }",
        "trait HasPool { type Pool; } struct Marker; impl HasPool for Marker { type Pool = crate::core::providers::base::GlobalPoolManager; }",
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
        "fn probe() { passthrough!(crate::core::providers::base::send_streaming_request(req, provider)); }",
        "fn probe() { passthrough!(super::base::send_streaming_request(req, provider)); }",
        "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { Self { pool_manager: std::sync::Arc::new(Default::default()) } } }",
        "fn legacy() -> crate::core::providers::base::GlobalPoolManager { Default::default() }",
        "fn legacy() { let pool_manager: crate::core::providers::base::GlobalPoolManager = Default::default(); consume(pool_manager); }",
        "macro_rules! manager_type { () => { crate::core::providers::base::GlobalPoolManager } } type Pool = manager_type!();",
        "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = Default::default(); Self { pool_manager: manager } } }",
        "static FACTORY: fn() -> crate::core::providers::base::GlobalPoolManager = || Default::default();", "use std::default::Default as D; struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = D::default(); Self { pool_manager: manager } } }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new(safe_manager: crate::core::providers::base::GlobalPoolManager) -> Self { let mut manager = safe_manager; manager = Default::default(); Self { pool_manager: manager } } }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = Default::default(); let build = || Self { pool_manager: manager }; build() } }",
    ] {
        let violations = boundary_violations(
            bypass,
            &["crate", "core", "providers", "mistral"],
            &manager_fields,
        )
        .unwrap_or_else(|error| panic!("bypass fixture must parse: {error}"));
        assert!(!violations.is_empty(), "bypass was not rejected: {bypass}");
    }
}

#[rustfmt::skip]
fn source_exception_violations(path: &str) -> Vec<String> {
    let matches: Vec<_> = BOUNDARY_EXCEPTIONS.iter().filter(|exception| exception.path == path).collect();
    assert!(matches.len() <= 1, "{path}: duplicate boundary exception entry");
    let mut violations: Vec<_> = matches.into_iter().flat_map(|exception| {
            assert!(!exception.purpose.trim().is_empty(), "{path}: boundary exception must document a fixed purpose");
            exception.violations.iter().map(|violation| violation.to_string()) }).collect();
    violations.sort();
    violations
}

#[rustfmt::skip]
fn collect_boundary_inventory() -> Vec<(String, Vec<String>, String)> {
    let repository_root = FsPath::new(env!("CARGO_MANIFEST_DIR"));
    #[rustfmt::skip]
    let roots: [(&str, &[&str]); 4] = [
        ("src/core/providers", &["crate", "core", "providers"]),
        ("src/server/routes/ai", &["crate", "server", "routes", "ai"]),
        ("src/core/fine_tuning/providers", &["crate", "core", "fine_tuning", "providers"]),
        ("src/core/rerank/providers", &["crate", "core", "rerank", "providers"]),
    ];
    let mut sources = Vec::new();
    for (relative, module_path) in roots {
        let module_path = module_path.iter().map(|segment| (*segment).to_string()).collect::<Vec<_>>();
        collect_production_sources(repository_root, &repository_root.join(relative), &module_path, &mut sources)
            .unwrap_or_else(|error| panic!("{relative} source inventory failed: {error}"));
    }
    let relative = "src/core/router/health_probe.rs";
    sources.push((
        relative.to_string(),
        ["crate", "core", "router", "health_probe"]
            .map(str::to_string)
            .to_vec(),
        fs::read_to_string(repository_root.join(relative))
            .unwrap_or_else(|error| panic!("cannot read {relative}: {error}")),
    ));
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    sources
}

#[test]
#[rustfmt::skip]
fn provider_runtime_http_boundary_has_no_unapproved_bypass() {
    let base_source = include_str!("../http.rs");
    let repository_root = FsPath::new(env!("CARGO_MANIFEST_DIR"));
    let sources = collect_boundary_inventory();
    assert!(!sources.iter().any(|(path, _, _)| path == "src/server/routes/ai/chat_tests.rs"), "#[path] test module entered production inventory");
    let unified = sources.iter().find(|(path, _, _)| path == "src/core/providers/unified_provider_methods.rs").expect("#[path] production module missing");
    assert_eq!(unified.1.join("::"), "crate::core::providers::unified_provider::methods");
    let mut all_sources = Vec::new();
    collect_production_sources(repository_root, &repository_root.join("src"), &["crate".into()], &mut all_sources)
        .unwrap_or_else(|error| panic!("repository alias inventory failed: {error}"));
    let mut manager_field_names = Vec::new();
    for (path, _, source) in &all_sources { manager_field_names.extend(manager_fields(source).unwrap_or_else(|error| panic!("{path} must parse: {error}"))); }
    manager_field_names.sort(); manager_field_names.dedup();
    assert!(sources.len() > 300, "provider/runtime inventory is incomplete");
    for exception in BOUNDARY_EXCEPTIONS {
        assert!(sources.iter().any(|(path, _, _)| path == exception.path), "stale boundary exception for {}", exception.path);
    }
    assert!(sources.iter().any(|(path, _, _)| path == UNIFIED_HTTP_IMPLEMENTATION), "unified HTTP implementation is missing from the inventory");
    let mut failures = Vec::new();
    for (path, module_path, source) in sources {
        if path == UNIFIED_HTTP_IMPLEMENTATION {
            continue;
        }
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path, &manager_field_names)
            .unwrap_or_else(|error| panic!("{path} must parse: {error}"));
        let mut violations = violations;
        violations.sort();
        let expected = source_exception_violations(&path);
        if violations != expected {
            failures.push(format!("{path}: expected [{}], found [{}]", expected.join("; "), violations.join("; ")));
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
    for (path, module_path, source) in all_sources {
        let module_path: Vec<_> = module_path.iter().map(String::as_str).collect();
        let violations = boundary_violations(&source, &module_path, &manager_field_names)
            .unwrap_or_else(|error| panic!("{path} must parse: {error}"));
        let has_alias = violations
            .iter()
            .any(|violation| violation.contains("GlobalPoolManager alias") || violation.contains("macro token GlobalPoolManager"));
        assert!(
            !has_alias,
            "{path}: GlobalPoolManager aliases are forbidden across production src"
        );
    }
    for pattern in [
        ["pub fn ", "inner("].concat(),
        ["pub fn ", "into_inner("].concat(),
        ["pub fn ", "client("].concat(),
        ["impl Deref for ", "BaseHttpClient"].concat(),
        ["pub ", "client:"].concat(),
    ] {
        assert!(!base_source.contains(&pattern), "BaseHttpClient exposes policy-bound internals through {pattern}");
    }
    let compact_base: String = base_source.chars().filter(|character| !character.is_whitespace()).collect();
    assert!(compact_base.contains("BaseRedirectMode::Policy=>ProviderHttpClient::new"));
    assert!(compact_base.contains("BaseRedirectMode::Disabled=>ProviderHttpClient::no_redirect"));
    assert!(compact_base.contains("BaseRedirectMode::Streaming=>ProviderHttpClient::streaming"));
    assert!(!include_str!("../../../../utils/net/http.rs").contains("get_ssrf_safe_no_redirect_client_with_timeout_fallible"));
    assert_eq!(include_str!("../../bedrock/client.rs").matches("new_for_provider_no_redirect").count(), 3);
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
