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
    exception!("src/core/providers/base/http.rs", "unified policy-enforcing HTTP implementation owns the approved request-builder abstraction", ["<module>: raw HTTP path crate::core::providers::base::connection_pool::HeaderPair", "<module>: raw HTTP path crate::utils::net::http::ProviderHttpClient", "<module>: raw HTTP path crate::utils::net::http::ProviderRequestBuilder"]),
    exception!("src/core/providers/base/connection_pool.rs", "unified pool implementation retains legacy fixed-endpoint clients beside the policy manager", [
            "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client", "<module>: raw HTTP path crate::core::http::outbound::default_outbound_client", "<module>: raw HTTP path crate::utils::net::http::HttpClientPoolConfig", "<module>: raw HTTP path crate::utils::net::http::create_custom_client_with_config", "<module>: raw HTTP path crate::utils::net::http::create_streaming_client", "<module>: raw HTTP path reqwest::Client", "client: raw HTTP client accessor .client()", "execute_request: raw HTTP client accessor .client()",
    ]),
    exception!("src/core/providers/base/mod.rs", "unified provider HTTP wrapper re-export", ["<module>: raw HTTP path crate::utils::net::http::ProviderRequestBuilder"]),
    exception!("src/core/providers/cloudflare/provider.rs", "native runtime is restricted by the factory to its account-scoped official endpoint", ["new: legacy GlobalPoolManager::new() constructor"]),
    exception!("src/core/providers/fal_ai/provider.rs", "native runtime is restricted by the factory to the fixed https://fal.run endpoint", ["<module>: raw HTTP path crate::core::providers::base::apply_headers", "execute_image_request: raw HTTP client accessor .client()", "new: legacy GlobalPoolManager::new() constructor"]),
    exception!("src/core/providers/github/provider.rs", "unwired lifecycle stub; Gateway uses the policy-wired catalog route", ["chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request", "new: legacy GlobalPoolManager::new() constructor"]),
    exception!("src/core/providers/github_copilot/authenticator.rs", "fixed GitHub OAuth and token exchange endpoints, outside provider API base configuration", ["perform_device_flow: raw HTTP path crate::core::http::outbound::default_outbound_client", "refresh_api_key: raw HTTP path crate::core::http::outbound::default_outbound_client"]),
    exception!("src/core/providers/github_copilot/provider.rs", "service-discovered Copilot endpoint; factory rejects caller-configured endpoint access", [
            "chat_completion: raw HTTP path crate::core::http::outbound::default_outbound_client", "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request", "embeddings: raw HTTP path crate::core::http::outbound::default_outbound_client",
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
    exception!("src/core/providers/replicate/provider.rs", "native runtime is restricted by the factory to the fixed Replicate API endpoint", [
            "<module>: raw HTTP path crate::core::providers::base::apply_headers", "build_request: raw HTTP client accessor .client()", "build_request: raw HTTP client accessor .client()", "build_request: raw HTTP client accessor .client()", "build_request: raw HTTP client accessor .client()", "chat_completion_stream: raw HTTP path crate::core::http::outbound::streaming_outbound_client", "chat_completion_stream: raw HTTP path crate::core::providers::base::connection_pool::send_streaming_request_with_timeout", "new: legacy GlobalPoolManager::new() constructor",
    ]),
    exception!("src/core/providers/v0/mod.rs", "unwired lifecycle stub with no Gateway factory owner", [
            "<module>: raw HTTP path crate::utils::net::http::create_custom_client", "<module>: raw HTTP path reqwest::Client", "new_or_default: raw HTTP path crate::core::http::outbound::default_outbound_client",
    ]),
    exception!("src/core/providers/vertex_ai/auth.rs", "fixed Google service-account token exchange, not the configurable Vertex API base", [
            "<module>: raw HTTP path reqwest::Client", "new: raw HTTP path crate::core::http::outbound::default_outbound_client",
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
#[rustfmt::skip]
fn production_macro_tokens(tokens: &str) -> String {
    let parts: Vec<_> = tokens.split_whitespace().collect(); let (mut output, mut index) = (Vec::new(), 0);
    while index < parts.len() {
        if parts[index] == "#" { let mut end = index + 1; let mut attribute = String::new(); while end < parts.len() { attribute.push_str(parts[end]); end += 1; if attribute.ends_with(']') { break; } } let test_cfg = attribute == "[cfg(test)]" || (attribute.starts_with("[cfg(all(") && attribute.ends_with(")]" ) && attribute.trim_start_matches("[cfg(all(").trim_end_matches(")]" ).split(',').any(|term| term == "test")); if test_cfg { let item_gate = parts[end..].iter().take_while(|token| !token.contains('{') && **token != ";").any(|token| matches!(*token, "fn" | "struct" | "enum" | "impl" | "mod" | "trait" | "union" | "type" | "static" | "const" | "use" | "extern" | "macro_rules")); let block_gate = parts.get(end).is_some_and(|token| matches!(*token, "if" | "match" | "loop" | "while" | "for" | "async" | "unsafe")); let (mut depth, mut saw_group, mut saw_brace) = (0_i32, false, false); index = end; while index < parts.len() { let token = parts[index]; for character in token.chars() { match character { '(' | '[' | '{' => { depth += 1; saw_group = true; saw_brace |= character == '{'; } ')' | ']' | '}' => depth -= 1, _ => {} } } index += 1; if depth <= 0 && (matches!(token, ";" | ",") || (!item_gate && !block_gate && saw_group) || ((item_gate || block_gate) && saw_brace && token.contains('}'))) { if block_gate && token.contains('}') && parts.get(index) == Some(&"else") { saw_brace = false; continue; } break; } } continue; } }
        output.push(parts[index]); index += 1;
    } output.join(" ")
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
        Type::Array(ty) => is_manager_type(&ty.elem), Type::Slice(ty) => is_manager_type(&ty.elem), Type::Ptr(ty) => is_manager_type(&ty.elem), Type::Reference(ty) => is_manager_type(&ty.elem), Type::Tuple(ty) => ty.elems.iter().any(is_manager_type),
        Type::Macro(ty) => ty.mac.tokens.to_string().contains("GlobalPoolManager"), _ => false,
    }
}
struct DefaultFinder<'a>(bool, &'a [String], &'a [String]);
#[rustfmt::skip]
impl<'ast> Visit<'ast> for DefaultFinder<'_> {
        fn visit_expr(&mut self, expr: &'ast Expr) { if !expr_attrs(expr).is_some_and(has_test_cfg) { visit::visit_expr(self, expr); } }
        fn visit_path(&mut self, path: &'ast Path) {
            let names: Vec<_> = path.segments.iter().map(|segment| ident_text(&segment.ident)).collect();
            self.0 |= (names.len() > 1 && names.last().is_some_and(|name| name == "default")) || (names.len() == 1 && self.1.contains(&names[0])); visit::visit_path(self, path);
        } fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) { let segments = match &*expression.func { Expr::Path(path) => path.path.segments.iter().map(|segment| ident_text(&segment.ident)).collect::<Vec<_>>(), _ => Vec::new() }; if segments.last().is_some_and(|name| name == "default" || self.2.contains(name)) || is_take_path(&segments, self.2) { self.0 = true; } else if !segments.windows(2).any(|window| window == ["GlobalPoolManager", "new_for_provider"]) { self.visit_expr(&expression.func); expression.args.iter().for_each(|argument| self.visit_expr(argument)); } } fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) { let method = ident_text(&expression.method); if matches!(method.as_str(), "unwrap_or_default" | "or_default" | "get_or_insert_default") { self.0 = true; } else { self.visit_expr(&expression.receiver); expression.args.iter().for_each(|argument| self.visit_expr(argument)); } } fn visit_macro(&mut self, item: &'ast Macro) { let name = item.path.segments.last().map(|segment| ident_text(&segment.ident)); let tokens = item.tokens.to_string(); let identifiers = || tokens.split(|character: char| !(character == '_' || character.is_alphanumeric())); self.0 |= name.is_some_and(|name| self.2.contains(&name)) || macro_uses_default(&tokens, self.2) || self.1.iter().any(|local| identifiers().any(|token| token == local)); } }
#[rustfmt::skip] fn uses_default_or_local(expr: &Expr, locals: &[String], macros: &[String]) -> bool { let mut finder = DefaultFinder(false, locals, macros); finder.visit_expr(expr); finder.0 }
#[allow(clippy::possible_missing_else)]
#[rustfmt::skip] fn manager_return_variants(output: &ReturnType) -> Option<Vec<String>> { fn paths(ty: &Type) -> Vec<Vec<String>> { if let Type::Path(path) = ty && let Some(segment) = path.path.segments.last() && let PathArguments::AngleBracketed(args) = &segment.arguments { let types: Vec<_> = args.args.iter().filter_map(|arg| match arg { GenericArgument::Type(ty) => Some(ty), _ => None }).collect(); if segment.ident == "Result" { return types.iter().enumerate().flat_map(|(index, ty)| paths(ty).into_iter().map(move |mut path| { path.insert(0, if index == 0 { "Ok".into() } else { "Err".into() }); path })).collect(); } if segment.ident == "Option" { return types.first().into_iter().flat_map(|ty| paths(ty).into_iter().map(|mut path| { path.insert(0, "Some".into()); path })).collect(); } } if is_manager_type(ty) { vec![Vec::new()] } else { Vec::new() } } let ReturnType::Type(_, ty) = output else { return None; }; let paths = paths(ty); (!paths.is_empty()).then(|| if paths.len() == 1 && paths[0].is_empty() { Vec::new() } else { paths.into_iter().map(|path| path.join("/")).collect() }) }
#[rustfmt::skip] fn canonical_variant(name: String, aliases: &[String]) -> String { aliases.iter().find_map(|alias| alias.strip_prefix(&format!("@variant:{name}:"))).map_or(name, str::to_string) }
#[rustfmt::skip] fn root_container(expr: &Expr, aliases: &[String]) -> bool { match expr { Expr::Call(call) => matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| matches!(canonical_variant(ident_text(&segment.ident), aliases).as_str(), "Ok" | "Err" | "Some" | "None"))), Expr::MethodCall(call) => matches!(ident_text(&call.method).as_str(), "map" | "map_err" | "and_then" | "or_else" | "inspect" | "inspect_err" | "filter" | "ok" | "err" | "ok_or" | "ok_or_else" | "transpose"), Expr::Group(group) => root_container(&group.expr, aliases), Expr::Paren(paren) => root_container(&paren.expr, aliases), Expr::Try(value) => root_container(&value.expr, aliases), Expr::Block(block) => block.block.stmts.last().is_some_and(|stmt| matches!(stmt, syn::Stmt::Expr(expr, None) if root_container(expr, aliases))), _ => false } }
#[rustfmt::skip] #[allow(clippy::possible_missing_else)] fn path_uses_default(expr: &Expr, path: &[&str], locals: &[String], aliases: &[String]) -> bool { fn value<'a>(expr: &'a Expr, expected: &str, aliases: &[String]) -> Option<&'a Expr> { match expr { Expr::Group(group) => value(&group.expr, expected, aliases), Expr::Paren(paren) => value(&paren.expr, expected, aliases), Expr::Block(block) => block.block.stmts.last().and_then(|stmt| match stmt { syn::Stmt::Expr(expr, None) => value(expr, expected, aliases), _ => None }), Expr::Call(call) if matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| canonical_variant(ident_text(&segment.ident), aliases) == expected)) => call.args.first(), _ => None } } if path.is_empty() { return uses_default_or_local(expr, locals, aliases); } if let Some(value) = value(expr, path[0], aliases) { return path_uses_default(value, &path[1..], locals, aliases); } if let Expr::MethodCall(call) = expr && let Some(Expr::Closure(closure)) = call.args.first() { let method = ident_text(&call.method); if (method == "map" && matches!(path[0], "Some" | "Ok")) || (method == "map_err" && path[0] == "Err") { return path_uses_default(&closure.body, &path[1..], locals, aliases); } if method == "and_then" && matches!(path[0], "Some" | "Ok") || method == "or_else" && path[0] == "Err" { return path_uses_default(&closure.body, path, locals, aliases); } } false }
#[rustfmt::skip] fn transpose_uses_default(expr: &Expr, variants: &[String], locals: &[String], aliases: &[String]) -> bool { variants.iter().any(|variant| { let output: Vec<_> = variant.split('/').filter(|part| !part.is_empty()).collect(); let input: Vec<_> = match output.as_slice() { ["Some", "Ok", rest @ ..] => [vec!["Ok", "Some"], rest.to_vec()].concat(), ["Some", "Err", rest @ ..] => [vec!["Err"], rest.to_vec()].concat(), ["Ok", "Some", rest @ ..] => [vec!["Some", "Ok"], rest.to_vec()].concat(), ["Err", rest @ ..] => [vec!["Some", "Err"], rest.to_vec()].concat(), _ => Vec::new() }; path_uses_default(expr, &input, locals, aliases) }) }
#[rustfmt::skip] struct ReturnDefaultFinder<'a>(bool, &'a [String], &'a [String], &'a [String]);
#[rustfmt::skip] #[allow(clippy::possible_missing_else)] impl<'ast> Visit<'ast> for ReturnDefaultFinder<'_> { fn visit_expr(&mut self, expr: &'ast Expr) { if !expr_attrs(expr).is_some_and(has_test_cfg) { visit::visit_expr(self, expr); } } fn visit_path(&mut self, path: &'ast Path) { if path.segments.len() == 1 { self.0 |= path.segments.first().is_some_and(|segment| self.2.contains(&ident_text(&segment.ident))); } } fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) { let variant = match &*expression.func { Expr::Path(path) => path.path.segments.last().map(|segment| canonical_variant(ident_text(&segment.ident), self.3)), _ => None }; if variant.as_ref().is_some_and(|name| matches!(name.as_str(), "Ok" | "Err" | "Some" | "None")) { if variant.is_some_and(|name| self.1.iter().any(|path| path.split('/').next() == Some(&name))) { self.0 |= expression.args.iter().any(|argument| uses_default_or_local(argument, self.2, self.3)); } } else { self.visit_expr(&expression.func); expression.args.iter().for_each(|argument| self.visit_expr(argument)); } } fn visit_expr_method_call(&mut self, expression: &'ast ExprMethodCall) { let method = ident_text(&expression.method); if method == "transpose" { self.0 |= transpose_uses_default(&expression.receiver, self.1, self.2, self.3); return; } let has_root = |name| self.1.iter().any(|path| path.split('/').next() == Some(name)); let alternate = if matches!(method.as_str(), "ok_or" | "ok_or_else") && has_root("Ok") { Some(vec!["Some".into()]) } else if method == "ok" && has_root("Some") { Some(vec!["Ok".into()]) } else if method == "err" && has_root("Some") { Some(vec!["Err".into()]) } else { None }; if let Some(variants) = alternate { let mut finder = ReturnDefaultFinder(false, &variants, self.2, self.3); finder.visit_expr(&expression.receiver); self.0 |= finder.0; } else { self.visit_expr(&expression.receiver); } if matches!(method.as_str(), "map" | "and_then" | "or_else") { self.0 |= expression.args.iter().any(|argument| uses_default_or_local(argument, self.2, self.3)); } } fn visit_macro(&mut self, item: &'ast Macro) { let name = item.path.segments.last().map(|segment| ident_text(&segment.ident)); let tokens = item.tokens.to_string(); self.0 |= name.is_some_and(|name| self.3.contains(&name)) || tokens.split(|character: char| !(character == '_' || character.is_alphanumeric())).any(|token| self.3.iter().any(|name| name == token)); } }
#[rustfmt::skip] fn return_uses_default(expr: &Expr, variants: &[String], locals: &[String], macros: &[String]) -> bool { if variants.is_empty() { uses_default_or_local(expr, locals, macros) } else { let mut finder = ReturnDefaultFinder(false, variants, locals, macros); finder.visit_expr(expr); finder.0 } }
#[rustfmt::skip] fn item_attrs(item: &syn::Item) -> Option<&[syn::Attribute]> { match item { syn::Item::Const(item) => Some(&item.attrs), syn::Item::Enum(item) => Some(&item.attrs), syn::Item::ExternCrate(item) => Some(&item.attrs), syn::Item::Fn(item) => Some(&item.attrs), syn::Item::ForeignMod(item) => Some(&item.attrs), syn::Item::Impl(item) => Some(&item.attrs), syn::Item::Macro(item) => Some(&item.attrs), syn::Item::Mod(item) => Some(&item.attrs), syn::Item::Static(item) => Some(&item.attrs), syn::Item::Struct(item) => Some(&item.attrs), syn::Item::Trait(item) => Some(&item.attrs), syn::Item::TraitAlias(item) => Some(&item.attrs), syn::Item::Type(item) => Some(&item.attrs), syn::Item::Union(item) => Some(&item.attrs), syn::Item::Use(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn impl_item_attrs(item: &syn::ImplItem) -> Option<&[syn::Attribute]> { match item { syn::ImplItem::Const(item) => Some(&item.attrs), syn::ImplItem::Fn(item) => Some(&item.attrs), syn::ImplItem::Macro(item) => Some(&item.attrs), syn::ImplItem::Type(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn trait_item_attrs(item: &syn::TraitItem) -> Option<&[syn::Attribute]> { match item { syn::TraitItem::Const(item) => Some(&item.attrs), syn::TraitItem::Fn(item) => Some(&item.attrs), syn::TraitItem::Macro(item) => Some(&item.attrs), syn::TraitItem::Type(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn foreign_item_attrs(item: &syn::ForeignItem) -> Option<&[syn::Attribute]> { match item { syn::ForeignItem::Fn(item) => Some(&item.attrs), syn::ForeignItem::Macro(item) => Some(&item.attrs), syn::ForeignItem::Static(item) => Some(&item.attrs), syn::ForeignItem::Type(item) => Some(&item.attrs), _ => None } }
#[rustfmt::skip] fn expr_attrs(expr: &Expr) -> Option<&[syn::Attribute]> { match expr { Expr::Array(e) => Some(&e.attrs), Expr::Assign(e) => Some(&e.attrs), Expr::Async(e) => Some(&e.attrs), Expr::Await(e) => Some(&e.attrs), Expr::Binary(e) => Some(&e.attrs), Expr::Block(e) => Some(&e.attrs), Expr::Break(e) => Some(&e.attrs), Expr::Call(e) => Some(&e.attrs), Expr::Cast(e) => Some(&e.attrs), Expr::Closure(e) => Some(&e.attrs), Expr::Const(e) => Some(&e.attrs), Expr::Continue(e) => Some(&e.attrs), Expr::Field(e) => Some(&e.attrs), Expr::ForLoop(e) => Some(&e.attrs), Expr::Group(e) => Some(&e.attrs), Expr::If(e) => Some(&e.attrs), Expr::Index(e) => Some(&e.attrs), Expr::Infer(e) => Some(&e.attrs), Expr::Let(e) => Some(&e.attrs), Expr::Lit(e) => Some(&e.attrs), Expr::Loop(e) => Some(&e.attrs), Expr::Macro(e) => Some(&e.attrs), Expr::Match(e) => Some(&e.attrs), Expr::MethodCall(e) => Some(&e.attrs), Expr::Paren(e) => Some(&e.attrs), Expr::Path(e) => Some(&e.attrs), Expr::Range(e) => Some(&e.attrs), Expr::RawAddr(e) => Some(&e.attrs), Expr::Reference(e) => Some(&e.attrs), Expr::Repeat(e) => Some(&e.attrs), Expr::Return(e) => Some(&e.attrs), Expr::Struct(e) => Some(&e.attrs), Expr::Try(e) => Some(&e.attrs), Expr::TryBlock(e) => Some(&e.attrs), Expr::Tuple(e) => Some(&e.attrs), Expr::Unary(e) => Some(&e.attrs), Expr::Unsafe(e) => Some(&e.attrs), Expr::While(e) => Some(&e.attrs), Expr::Yield(e) => Some(&e.attrs), _ => None } }
#[rustfmt::skip] fn pat_names(pat: &Pat) -> Vec<String> { struct Names(Vec<String>); impl<'ast> Visit<'ast> for Names { fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) { self.0.push(ident_text(&pat.ident)); visit::visit_pat_ident(self, pat); } } let mut names = Names(Vec::new()); names.visit_pat(pat); names.0 }
#[rustfmt::skip] fn typed_manager_names(pat: &Pat, ty: &Type, fields: &[String], module: &[String]) -> Vec<String> { let names = pat_names(pat); if is_manager_type(ty) { return names; } let Type::Path(ty) = ty else { return Vec::new(); }; let path: Vec<_> = ty.path.segments.iter().map(|segment| ident_text(&segment.ident)).collect(); let mut prefixes = vec![format!("{}#", path.join("::"))]; if let Some(path) = resolve_relative_path(module, &path) { prefixes.push(format!("{}#", path.join("::"))); } names.into_iter().flat_map(|name| fields.iter().filter_map({ let prefixes = prefixes.clone(); move |field| prefixes.iter().find_map(|prefix| field.strip_prefix(prefix)).map(|index| format!("{name}#{index}")) })).collect() }
#[rustfmt::skip] fn manager_place(expr: &Expr, fields: &[String], locals: &[String]) -> bool { fn local(expr: &Expr) -> Option<String> { match expr { Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => path.path.segments.first().map(|segment| ident_text(&segment.ident)), Expr::Paren(paren) => local(&paren.expr), Expr::Group(group) => local(&group.expr), Expr::Reference(reference) => local(&reference.expr), Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => local(&unary.expr), _ => None } } match expr { Expr::Reference(reference) => manager_place(&reference.expr, fields, locals), Expr::Paren(paren) => manager_place(&paren.expr, fields, locals), Expr::Group(group) => manager_place(&group.expr, fields, locals), Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => manager_place(&unary.expr, fields, locals), Expr::Try(value) => manager_place(&value.expr, fields, locals), Expr::Await(value) => manager_place(&value.base, fields, locals), Expr::Block(block) => block.block.stmts.last().is_some_and(|stmt| matches!(stmt, syn::Stmt::Expr(expr, None) if manager_place(expr, fields, locals))), Expr::Path(_) => local(expr).is_some_and(|name| locals.contains(&name)), Expr::Field(field) => match &field.member { syn::Member::Named(name) => fields.contains(&ident_text(name)), syn::Member::Unnamed(index) => local(&field.base).is_some_and(|name| locals.contains(&format!("{name}#{}", index.index))) }, Expr::MethodCall(call) if call.args.is_empty() && matches!(ident_text(&call.method).as_str(), "clone" | "to_owned") => manager_place(&call.receiver, fields, locals), Expr::Call(call) if matches!(&*call.func, Expr::Path(path) if path.path.segments.last().is_some_and(|segment| matches!(ident_text(&segment.ident).as_str(), "identity" | "clone"))) => call.args.iter().any(|arg| manager_place(arg, fields, locals)), Expr::Tuple(tuple) => tuple.elems.iter().any(|value| manager_place(value, fields, locals)), Expr::Match(value) => value.arms.iter().any(|arm| manager_place(&arm.body, fields, locals)), Expr::If(value) => value.then_branch.stmts.last().is_some_and(|stmt| matches!(stmt, syn::Stmt::Expr(expr, None) if manager_place(expr, fields, locals))) || value.else_branch.as_ref().is_some_and(|(_, expr)| manager_place(expr, fields, locals)), _ => false } }
#[rustfmt::skip] fn binding_names(pat: &Pat, expr: &Expr, tainted: &impl Fn(&Expr) -> bool) -> Vec<String> { match (pat, expr) { (Pat::Type(pat), expr) => binding_names(&pat.pat, expr, tainted), (Pat::Paren(pat), expr) => binding_names(&pat.pat, expr, tainted), (Pat::Reference(pat), expr) => binding_names(&pat.pat, expr, tainted), (Pat::Tuple(pat), Expr::Tuple(expr)) => pat.elems.iter().zip(&expr.elems).flat_map(|(pat, expr)| binding_names(pat, expr, tainted)).collect(), (Pat::TupleStruct(pat), Expr::Call(expr)) => pat.elems.iter().zip(&expr.args).flat_map(|(pat, expr)| binding_names(pat, expr, tainted)).collect(), _ if tainted(expr) => pat_names(pat), _ => Vec::new() } }
#[rustfmt::skip] fn alias_name(alias: &str) -> &str { if let Some(alias) = alias.strip_prefix("@variant:") { alias.split(':').next().unwrap_or(alias) } else if alias.starts_with('@') { alias.rsplit_once(':').map_or(alias, |(_, name)| name) } else { alias } }
#[rustfmt::skip] #[allow(clippy::possible_missing_else)] fn retarget_alias(entry: &str, original: &str, alias: &str) -> Option<String> { if alias_name(entry) != original { return None; } if let Some(value) = entry.strip_prefix("@variant:") { return value.split_once(':').map(|(_, canonical)| format!("@variant:{alias}:{canonical}")); } entry.rsplit_once(':').filter(|_| entry.starts_with('@')).map_or_else(|| Some(alias.into()), |(kind, _)| Some(format!("{kind}:{alias}"))) }
#[rustfmt::skip] fn push_canonical_alias(path: &[String], alias: &str, aliases: &mut Vec<String>) { let system = path.first().is_some_and(|root| matches!(root.as_str(), "std" | "core")); let value = if system && path.len() == 3 && path[1] == "default" && path[2] == "default" { Some(alias.into()) } else if system && path.len() == 3 && path[1] == "default" && path[2] == "Default" { Some(format!("@trait:{alias}")) } else if system && path.len() == 3 && path[1] == "mem" && path[2] == "take" { Some(format!("@take:{alias}")) } else if system && path.len() == 2 && path[1] == "mem" { Some(format!("@mem:{alias}")) } else if path.len() >= 2 && matches!(path[path.len()-2].as_str(), "Result" | "Option") && matches!(path[path.len()-1].as_str(), "Ok" | "Err" | "Some" | "None") { Some(format!("@variant:{alias}:{}", path[path.len()-1])) } else { None }; if let Some(value) = value && !aliases.contains(&value) { aliases.push(value); } }
#[rustfmt::skip] fn collect_default_aliases(tree: &UseTree, prefix: &mut Vec<String>, aliases: &mut Vec<String>) { match tree { UseTree::Path(path) => { prefix.push(ident_text(&path.ident)); collect_default_aliases(&path.tree, prefix, aliases); prefix.pop(); }, UseTree::Name(name) => { let mut path = prefix.clone(); path.push(ident_text(&name.ident)); push_canonical_alias(&path, &ident_text(&name.ident), aliases); }, UseTree::Rename(rename) => { let mut path = prefix.clone(); path.push(ident_text(&rename.ident)); push_canonical_alias(&path, &ident_text(&rename.rename), aliases); }, UseTree::Group(group) => group.items.iter().for_each(|tree| collect_default_aliases(tree, prefix, aliases)), _ => {} } }
#[rustfmt::skip] fn collect_renames(tree: &UseTree, prefix: &mut Vec<String>, renames: &mut Vec<(Vec<String>, String)>) { match tree { UseTree::Path(path) => { prefix.push(ident_text(&path.ident)); collect_renames(&path.tree, prefix, renames); prefix.pop(); }, UseTree::Name(name) => { let name = ident_text(&name.ident); let mut path = prefix.clone(); if name != "self" { path.push(name.clone()); } renames.push((path, name)); }, UseTree::Rename(rename) => { let mut path = prefix.clone(); if rename.ident != "self" { path.push(ident_text(&rename.ident)); } renames.push((path, ident_text(&rename.rename))); }, UseTree::Group(group) => group.items.iter().for_each(|tree| collect_renames(tree, prefix, renames)), _ => {} } }
#[rustfmt::skip] fn resolve_relative_path(module: &[String], path: &[String]) -> Option<Vec<String>> { canonicalize_path(module, path).or_else(|| path.first().filter(|root| !matches!(root.as_str(), "std" | "core" | "reqwest")).map(|_| module.iter().chain(path).cloned().collect())) }
#[rustfmt::skip] fn macro_uses_default(tokens: &str, aliases: &[String]) -> bool { let compact = tokens.replace(' ', ""); let identifiers = || tokens.split(|character: char| !(character == '_' || character.is_alphanumeric())); compact.contains("Default::default") || compact.contains("std::mem::take") || compact.contains("core::mem::take") || aliases.iter().any(|entry| { let name = alias_name(entry); if entry.starts_with("@variant:") { false } else if entry.starts_with("@trait:") { compact.contains(&format!("{name}::default")) } else if entry.starts_with("@mem:") { compact.contains(&format!("{name}::take")) } else { identifiers().any(|token| token == name) } }) }
#[rustfmt::skip] fn is_take_path(path: &[String], aliases: &[String]) -> bool { path.ends_with(&["std".into(), "mem".into(), "take".into()]) || path.ends_with(&["core".into(), "mem".into(), "take".into()]) || (path.len() == 1 && aliases.contains(&format!("@take:{}", path[0]))) || (path.len() == 2 && path[1] == "take" && aliases.contains(&format!("@mem:{}", path[0]))) }
#[rustfmt::skip] fn extend_aliases(tree: &UseTree, aliases: &mut Vec<String>) { collect_default_aliases(tree, &mut Vec::new(), aliases); let mut renames = Vec::new(); collect_renames(tree, &mut Vec::new(), &mut renames); loop { let before = aliases.len(); for (path, alias) in &renames { if path.len() == 1 { let original = &path[0]; for entry in aliases.clone() { if let Some(entry) = retarget_alias(&entry, original, alias) && !aliases.contains(&entry) { aliases.push(entry); } } } } if aliases.len() == before { break; } } }
#[rustfmt::skip] fn default_macro_names(file: &syn::File) -> Vec<String> { struct Defaults(Vec<(String, String)>); impl<'ast> Visit<'ast> for Defaults { fn visit_item_macro(&mut self, item: &'ast ItemMacro) { if !has_test_cfg(&item.attrs) && let Some(name) = &item.ident { self.0.push((ident_text(name), production_macro_tokens(&item.mac.tokens.to_string()))); } } } let mut found = Defaults(Vec::new()); found.visit_file(file); let uses: Vec<_> = file.items.iter().filter_map(|item| match item { syn::Item::Use(item) if !has_test_cfg(&item.attrs) => Some(&item.tree), _ => None }).collect(); let mut defaults = Vec::new(); loop { let before = defaults.len(); uses.iter().for_each(|tree| extend_aliases(tree, &mut defaults)); for (name, tokens) in &found.0 { if macro_uses_default(tokens, &defaults) && !defaults.contains(name) { defaults.push(name.clone()); } } if defaults.len() == before { return defaults; } } }
#[rustfmt::skip]
fn manager_fields(source: &str, module_path: &[String]) -> Result<Vec<String>, syn::Error> {
    struct Fields(Vec<String>, Vec<String>); impl<'ast> Visit<'ast> for Fields { fn visit_item(&mut self, item: &'ast syn::Item) { if !item_attrs(item).is_some_and(has_test_cfg) { visit::visit_item(self, item); } } fn visit_item_struct(&mut self, item: &'ast ItemStruct) { self.0.extend(item.fields.iter().enumerate().filter(|(_, field)| !has_test_cfg(&field.attrs) && is_manager_type(&field.ty)).map(|(index, field)| field.ident.as_ref().map_or_else(|| format!("{}::{}#{index}", self.1.join("::"), item.ident), ident_text))); visit::visit_item_struct(self, item); } fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) { for variant in &item.variants { self.0.extend(variant.fields.iter().enumerate().filter(|(_, field)| !has_test_cfg(&field.attrs) && is_manager_type(&field.ty)).map(|(index, field)| field.ident.as_ref().map_or_else(|| format!("{}::{}::{}#{index}", self.1.join("::"), item.ident, variant.ident), ident_text))); } visit::visit_item_enum(self, item); } fn visit_item_mod(&mut self, item: &'ast ItemMod) { if let Some((_, items)) = &item.content { self.1.push(ident_text(&item.ident)); items.iter().for_each(|item| self.visit_item(item)); self.1.pop(); } } }
    let mut fields = Fields(Vec::new(), module_path.to_vec()); fields.visit_file(&syn::parse_file(source)?); Ok(fields.0)
}
#[rustfmt::skip] struct BoundaryVisitor {
    module_path: Vec<String>, context: Vec<String>, violations: Vec<String>, manager_fields: Vec<String>, default_locals: Vec<String>, manager_locals: Vec<String>, local_scopes: Vec<Vec<(String, bool, bool)>>,
    manager_returning: bool, return_variants: Vec<String>, default_macros: Vec<String>, alias_scopes: Vec<Vec<String>>,
}
#[allow(clippy::possible_missing_else)]
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
    #[rustfmt::skip]    fn push_pattern(&mut self, pat: &Pat, defaults: &[String], managers: &[String]) { self.local_scopes.push(Vec::new()); self.alias_scopes.push(Vec::new()); let names = pat_names(pat); self.shadow_aliases(&names); for name in names { let (was_default, was_manager) = (self.default_locals.contains(&name), self.manager_locals.contains(&name)); let Some(scope) = self.local_scopes.last_mut() else { panic!("pattern scope must exist") }; scope.push((name.clone(), was_default, was_manager)); self.default_locals.retain(|local| local != &name); self.manager_locals.retain(|local| local != &name); if defaults.contains(&name) { self.default_locals.push(name.clone()); } if managers.contains(&name) { self.manager_locals.push(name); } } }
    #[rustfmt::skip]    fn pop_scope(&mut self) { for (name, was_default, was_manager) in self.local_scopes.pop().unwrap_or_default().into_iter().rev() { self.default_locals.retain(|local| local != &name); self.manager_locals.retain(|local| local != &name); if was_default { self.default_locals.push(name.clone()); } if was_manager { self.manager_locals.push(name); } } }
    #[rustfmt::skip]    fn add_alias(&mut self, alias: String) { if !self.default_macros.contains(&alias) { if let Some(scope) = self.alias_scopes.last_mut() { scope.push(alias.clone()); } self.default_macros.push(alias); } }
    #[rustfmt::skip]    fn add_aliases(&mut self, tree: &UseTree) { let mut aliases = self.default_macros.clone(); extend_aliases(tree, &mut aliases); for alias in aliases { self.add_alias(alias); } let mut imports = Vec::new(); collect_renames(tree, &mut Vec::new(), &mut imports); for (path, alias) in imports { let Some(original) = path.last() else { continue; }; let target = resolve_relative_path(&self.module_path, &path).map(|path| path.join("::")); for marker in self.manager_fields.clone().into_iter() { let derived = if (path.len() == 1 || (path.len() == 2 && path[0] == "self")) && marker.starts_with(&format!("{original}#")) { marker.strip_prefix(original).map(|suffix| format!("{alias}{suffix}")) } else if (path.len() == 1 || (path.len() == 2 && path[0] == "self")) && marker.starts_with(&format!("{original}::")) { marker.strip_prefix(original).map(|suffix| format!("{alias}{suffix}")) } else { target.as_ref().and_then(|target| marker.strip_prefix(&format!("{target}#")).map(|index| format!("{alias}#{index}")).or_else(|| marker.strip_prefix(&format!("{target}::")).map(|suffix| format!("{alias}::{suffix}")))) }; if let Some(marker) = derived && !self.manager_fields.contains(&marker) { if let Some(scope) = self.alias_scopes.last_mut() { scope.push(format!("@manager:{marker}")); } self.manager_fields.push(marker); } } } }
    #[rustfmt::skip]    fn add_macro(&mut self, item: &ItemMacro) { if !has_test_cfg(&item.attrs) && let Some(name) = &item.ident && macro_uses_default(&production_macro_tokens(&item.mac.tokens.to_string()), &self.default_macros) { self.add_alias(ident_text(name)); } }
    #[rustfmt::skip]    fn shadow_aliases(&mut self, names: &[String]) { for alias in self.default_macros.clone().into_iter().filter(|alias| names.iter().any(|name| name == alias_name(alias))) { self.default_macros.retain(|current| current != &alias); if let Some(scope) = self.alias_scopes.last_mut() { scope.push(format!("@restore:{alias}")); } } }
    #[rustfmt::skip]    fn pop_alias_scope(&mut self) { for alias in self.alias_scopes.pop().unwrap_or_default().into_iter().rev() { if let Some(marker) = alias.strip_prefix("@manager:") { self.manager_fields.retain(|current| current != marker); } else if let Some(alias) = alias.strip_prefix("@restore:") { if !self.default_macros.contains(&alias.to_string()) { self.default_macros.push(alias.into()); } } else { self.default_macros.retain(|current| current != &alias); } } }
    #[rustfmt::skip]    fn push_alias_scope(&mut self, statements: &[syn::Stmt]) { self.alias_scopes.push(Vec::new()); loop { let before = self.default_macros.len() + self.manager_fields.len(); for stmt in statements { if let syn::Stmt::Item(syn::Item::Use(item)) = stmt && !has_test_cfg(&item.attrs) { self.add_aliases(&item.tree); } } for stmt in statements { if let syn::Stmt::Item(syn::Item::Macro(item)) = stmt { self.add_macro(item); } } if self.default_macros.len() + self.manager_fields.len() == before { break; } } }
    #[rustfmt::skip]    fn push_item_alias_scope(&mut self, items: &[syn::Item]) { self.alias_scopes.push(Vec::new()); loop { let before = self.default_macros.len() + self.manager_fields.len(); for item in items { if let syn::Item::Use(item) = item && !has_test_cfg(&item.attrs) { self.add_aliases(&item.tree); } } for item in items { if let syn::Item::Macro(item) = item { self.add_macro(item); } } if self.default_macros.len() + self.manager_fields.len() == before { break; } } }
}

#[rustfmt::skip] #[allow(clippy::possible_missing_else)]
impl<'ast> Visit<'ast> for BoundaryVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) { if item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_item(self, item); }
    fn visit_expr(&mut self, expr: &'ast Expr) { if !expr_attrs(expr).is_some_and(has_test_cfg) { visit::visit_expr(self, expr); } }
    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) { if impl_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_impl_item(self, item); }
    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) { if trait_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_trait_item(self, item); }
    fn visit_foreign_item(&mut self, item: &'ast syn::ForeignItem) { if foreign_item_attrs(item).is_some_and(has_test_cfg) { return; } visit::visit_foreign_item(self, item); }
    fn visit_field(&mut self, field: &'ast syn::Field) { if !has_test_cfg(&field.attrs) { visit::visit_field(self, field); } }
    fn visit_variant(&mut self, variant: &'ast syn::Variant) { if !has_test_cfg(&variant.attrs) { visit::visit_variant(self, variant); } }
    fn visit_arm(&mut self, arm: &'ast syn::Arm) { if !has_test_cfg(&arm.attrs) { visit::visit_arm(self, arm); } }
    fn visit_stmt_macro(&mut self, item: &'ast syn::StmtMacro) { if !has_test_cfg(&item.attrs) { visit::visit_stmt_macro(self, item); } }
    fn visit_block(&mut self, block: &'ast syn::Block) { self.local_scopes.push(Vec::new()); self.push_alias_scope(&block.stmts); visit::visit_block(self, block); self.pop_alias_scope(); self.pop_scope(); }
    fn visit_item_use(&mut self, item: &'ast ItemUse) {
        self.add_aliases(&item.tree);
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
            if self.manager_fields.contains(&name) && uses_default_or_local(&field.expr, &self.default_locals, &self.default_macros) {
                self.record(format!("policy-less Default construction for GlobalPoolManager field {name}"));
            }
        }
        visit::visit_expr_struct(self, expression);
    }
    fn visit_expr_call(&mut self, expression: &'ast syn::ExprCall) { if let Expr::Path(path) = &*expression.func { let segments = path.path.segments.iter().map(|segment| ident_text(&segment.ident)).collect::<Vec<_>>(); if let Some(constructor) = segments.last() { if is_take_path(&segments, &self.default_macros) && expression.args.iter().any(|argument| manager_place(argument, &self.manager_fields, &self.manager_locals)) { self.record("implicit Default construction for GlobalPoolManager via mem::take".into()); } let direct = segments.join("::"); let canonical = resolve_relative_path(&self.module_path, &segments).map(|path| path.join("::")); for (index, argument) in expression.args.iter().enumerate() { if (self.manager_fields.contains(&format!("{constructor}#{index}")) || self.manager_fields.contains(&format!("{direct}#{index}")) || canonical.as_ref().is_some_and(|path| self.manager_fields.contains(&format!("{path}#{index}")))) && uses_default_or_local(argument, &self.default_locals, &self.default_macros) { self.record(format!("policy-less Default construction for GlobalPoolManager tuple field {constructor}#{index}")); } } } } visit::visit_expr_call(self, expression); }
    fn visit_local(&mut self, local: &'ast Local) { if has_test_cfg(&local.attrs) { return; } let names = pat_names(&local.pat); let defaults = local.init.as_ref().map_or_else(Vec::new, |init| binding_names(&local.pat, &init.expr, &|expr| if self.manager_returning && root_container(expr, &self.default_macros) { return_uses_default(expr, &self.return_variants, &self.default_locals, &self.default_macros) } else { uses_default_or_local(expr, &self.default_locals, &self.default_macros) })); let mut managers = local.init.as_ref().map_or_else(Vec::new, |init| binding_names(&local.pat, &init.expr, &|expr| manager_place(expr, &self.manager_fields, &self.manager_locals))); let typed = match &local.pat { Pat::Type(pat) => typed_manager_names(&pat.pat, &pat.ty, &self.manager_fields, &self.module_path), _ => Vec::new() }; managers.extend(typed); for name in &names { let (was_default, was_manager) = (self.default_locals.contains(name), self.manager_locals.contains(name)); if let Some(scope) = self.local_scopes.last_mut() { scope.push((name.clone(), was_default, was_manager)); } self.default_locals.retain(|local| local != name); self.manager_locals.retain(|local| local != name && !local.starts_with(&format!("{name}#"))); } self.shadow_aliases(&names); managers.sort(); managers.dedup(); if defaults.iter().any(|name| managers.contains(name)) { self.record("policy-less Default construction for GlobalPoolManager local".into()); } self.default_locals.extend(defaults); self.manager_locals.extend(managers); visit::visit_local(self, local); }
    fn visit_expr_assign(&mut self, expression: &'ast syn::ExprAssign) { let inferred_default = uses_default_or_local(&expression.right, &self.default_locals, &self.default_macros); let inferred_manager = manager_place(&expression.right, &self.manager_fields, &self.manager_locals); match &*expression.left { Expr::Field(_) if inferred_default && manager_place(&expression.left, &self.manager_fields, &self.manager_locals) => self.record("policy-less Default construction for GlobalPoolManager field assignment".into()), Expr::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 => if let Some(segment) = path.path.segments.first() { let name = ident_text(&segment.ident); self.default_locals.retain(|local| local != &name); self.manager_locals.retain(|local| local != &name); if inferred_default { self.default_locals.push(name.clone()); } if inferred_manager { self.manager_locals.push(name); } }, _ => {} } visit::visit_expr_assign(self, expression); }
    fn visit_item_const(&mut self, item: &'ast ItemConst) { if is_manager_type(&item.ty) && uses_default_or_local(&item.expr, &[], &self.default_macros) { self.record("policy-less Default construction for GlobalPoolManager const".into()); } visit::visit_item_const(self, item); }
    fn visit_item_static(&mut self, item: &'ast ItemStatic) { if is_manager_type(&item.ty) && uses_default_or_local(&item.expr, &[], &self.default_macros) { self.record("policy-less Default construction for GlobalPoolManager static".into()); } visit::visit_item_static(self, item); }
    fn visit_expr_closure(&mut self, expression: &'ast ExprClosure) { let (outer, outer_managers, outer_aliases) = (self.default_locals.clone(), self.manager_locals.clone(), self.default_macros.clone()); let parameters: Vec<_> = expression.inputs.iter().flat_map(pat_names).collect(); self.default_locals.retain(|name| !parameters.contains(name)); self.manager_locals.retain(|name| !parameters.contains(name)); self.default_macros.retain(|alias| !parameters.iter().any(|name| name == alias_name(alias))); for input in &expression.inputs { if let Pat::Type(input) = input { self.manager_locals.extend(typed_manager_names(&input.pat, &input.ty, &self.manager_fields, &self.module_path)); } } let old_returning = std::mem::replace(&mut self.manager_returning, matches!(&expression.output, ReturnType::Type(_, ty) if is_manager_type(ty))); let old_variants = std::mem::take(&mut self.return_variants); if self.manager_returning && uses_default_or_local(&expression.body, &self.default_locals, &self.default_macros) { self.record("policy-less Default construction for GlobalPoolManager closure".into()); } visit::visit_expr_closure(self, expression); self.default_locals.retain(|name| !parameters.contains(name)); for name in outer { if !self.default_locals.contains(&name) { self.default_locals.push(name); } } self.manager_locals = outer_managers; self.default_macros = outer_aliases; self.manager_returning = old_returning; self.return_variants = old_variants; }
    fn visit_item_fn(&mut self, item: &'ast ItemFn) { self.context.push(ident_text(&item.sig.ident)); let (outer, outer_managers) = (std::mem::take(&mut self.default_locals), std::mem::take(&mut self.manager_locals)); self.manager_locals = item.sig.inputs.iter().flat_map(|argument| match argument { syn::FnArg::Typed(argument) => typed_manager_names(&argument.pat, &argument.ty, &self.manager_fields, &self.module_path), syn::FnArg::Receiver(_) => Vec::new() }).collect(); let variants = manager_return_variants(&item.sig.output); let old_returning = std::mem::replace(&mut self.manager_returning, variants.is_some()); let old_variants = std::mem::replace(&mut self.return_variants, variants.unwrap_or_default()); self.local_scopes.push(Vec::new()); self.push_alias_scope(&item.block.stmts); self.shadow_aliases(&item.sig.inputs.iter().flat_map(|argument| match argument { syn::FnArg::Typed(argument) => pat_names(&argument.pat), syn::FnArg::Receiver(_) => Vec::new() }).collect::<Vec<_>>()); visit::visit_signature(self, &item.sig); item.block.stmts.iter().for_each(|stmt| self.visit_stmt(stmt)); if self.manager_returning && let Some(syn::Stmt::Expr(tail, None)) = item.block.stmts.last() && return_uses_default(tail, &self.return_variants, &self.default_locals, &self.default_macros) { self.record("policy-less Default construction for GlobalPoolManager return".into()); } self.pop_alias_scope(); self.local_scopes.pop(); self.manager_returning = old_returning; self.return_variants = old_variants; self.default_locals = outer; self.manager_locals = outer_managers; self.context.pop(); }
    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) { self.context.push(ident_text(&item.sig.ident)); let (outer, outer_managers) = (std::mem::take(&mut self.default_locals), std::mem::take(&mut self.manager_locals)); self.manager_locals = item.sig.inputs.iter().flat_map(|argument| match argument { syn::FnArg::Typed(argument) => typed_manager_names(&argument.pat, &argument.ty, &self.manager_fields, &self.module_path), syn::FnArg::Receiver(_) => Vec::new() }).collect(); let variants = manager_return_variants(&item.sig.output); let old_returning = std::mem::replace(&mut self.manager_returning, variants.is_some()); let old_variants = std::mem::replace(&mut self.return_variants, variants.unwrap_or_default()); self.local_scopes.push(Vec::new()); self.push_alias_scope(&item.block.stmts); self.shadow_aliases(&item.sig.inputs.iter().flat_map(|argument| match argument { syn::FnArg::Typed(argument) => pat_names(&argument.pat), syn::FnArg::Receiver(_) => Vec::new() }).collect::<Vec<_>>()); visit::visit_signature(self, &item.sig); item.block.stmts.iter().for_each(|stmt| self.visit_stmt(stmt)); if self.manager_returning && let Some(syn::Stmt::Expr(tail, None)) = item.block.stmts.last() && return_uses_default(tail, &self.return_variants, &self.default_locals, &self.default_macros) { self.record("policy-less Default construction for GlobalPoolManager return".into()); } self.pop_alias_scope(); self.local_scopes.pop(); self.manager_returning = old_returning; self.return_variants = old_variants; self.default_locals = outer; self.manager_locals = outer_managers; self.context.pop(); }
    fn visit_expr_return(&mut self, expression: &'ast syn::ExprReturn) { if self.manager_returning && expression.expr.as_ref().is_some_and(|expr| return_uses_default(expr, &self.return_variants, &self.default_locals, &self.default_macros)) { self.record("policy-less Default construction for GlobalPoolManager return".into()); } visit::visit_expr_return(self, expression); }
    fn visit_expr_match(&mut self, expression: &'ast syn::ExprMatch) { self.visit_expr(&expression.expr); for arm in &expression.arms { if has_test_cfg(&arm.attrs) { continue; } let defaults = binding_names(&arm.pat, &expression.expr, &|expr| uses_default_or_local(expr, &self.default_locals, &self.default_macros)); let managers = binding_names(&arm.pat, &expression.expr, &|expr| manager_place(expr, &self.manager_fields, &self.manager_locals)); self.push_pattern(&arm.pat, &defaults, &managers); visit::visit_arm(self, arm); self.pop_alias_scope(); self.pop_scope(); } }
    fn visit_expr_for_loop(&mut self, expression: &'ast syn::ExprForLoop) { let defaults = if uses_default_or_local(&expression.expr, &self.default_locals, &self.default_macros) { pat_names(&expression.pat) } else { Vec::new() }; let managers = if manager_place(&expression.expr, &self.manager_fields, &self.manager_locals) { pat_names(&expression.pat) } else { Vec::new() }; self.visit_expr(&expression.expr); self.push_pattern(&expression.pat, &defaults, &managers); self.visit_block(&expression.body); self.pop_alias_scope(); self.pop_scope(); }
    fn visit_expr_if(&mut self, expression: &'ast syn::ExprIf) { let Expr::Let(binding) = &*expression.cond else { visit::visit_expr_if(self, expression); return; }; let defaults = binding_names(&binding.pat, &binding.expr, &|expr| uses_default_or_local(expr, &self.default_locals, &self.default_macros)); let managers = binding_names(&binding.pat, &binding.expr, &|expr| manager_place(expr, &self.manager_fields, &self.manager_locals)); self.visit_expr(&expression.cond); self.push_pattern(&binding.pat, &defaults, &managers); self.visit_block(&expression.then_branch); self.pop_alias_scope(); self.pop_scope(); if let Some((_, otherwise)) = &expression.else_branch { self.visit_expr(otherwise); } }
    fn visit_expr_while(&mut self, expression: &'ast syn::ExprWhile) { let Expr::Let(binding) = &*expression.cond else { visit::visit_expr_while(self, expression); return; }; let defaults = binding_names(&binding.pat, &binding.expr, &|expr| uses_default_or_local(expr, &self.default_locals, &self.default_macros)); let managers = binding_names(&binding.pat, &binding.expr, &|expr| manager_place(expr, &self.manager_fields, &self.manager_locals)); self.visit_expr(&expression.cond); self.push_pattern(&binding.pat, &defaults, &managers); self.visit_block(&expression.body); self.pop_alias_scope(); self.pop_scope(); }
    fn visit_item_macro(&mut self, item: &'ast ItemMacro) {
        self.add_macro(item);
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
        let tokens = production_macro_tokens(&item.tokens.to_string());
        for path in macro_paths(&tokens.replace("$ crate", "crate")) {
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
            self.push_item_alias_scope(items);
            for child in items {
                self.visit_item(child);
            }
            self.pop_alias_scope();
            self.module_path.pop();
        }
    }
}
#[rustfmt::skip]
fn boundary_violations(source: &str, module_path: &[&str], manager_fields: &[String]) -> Result<Vec<String>, syn::Error> {
    let file = syn::parse_file(source)?; let default_macros = default_macro_names(&file); let mut fields = manager_fields.to_vec(); let prefix = format!("{}::", module_path.join("::")); fields.extend(manager_fields.iter().filter_map(|field| field.strip_prefix(&prefix).map(str::to_string))); fields.sort(); fields.dedup();
    let mut visitor = BoundaryVisitor {
        module_path: module_path.iter().map(|segment| (*segment).to_string()).collect(), context: Vec::new(), violations: Vec::new(), manager_fields: fields, default_locals: Vec::new(), manager_locals: Vec::new(), local_scopes: Vec::new(), manager_returning: false, return_variants: Vec::new(), default_macros, alias_scopes: Vec::new(), };
    visitor.push_item_alias_scope(&file.items); visitor.visit_file(&file); visitor.pop_alias_scope(); Ok(visitor.violations)
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
    assert!(manager_fields("struct Fixture { #[cfg(test)] pool_manager: crate::core::providers::base::GlobalPoolManager }", &[]).unwrap_or_else(|error| panic!("test-only manager field fixture must parse: {error}")).is_empty()); let manager_fields = vec!["pool_manager".to_string(), "TupleProvider#0".to_string(), "Ready#0".to_string(), "Boxed#0".to_string(), "crate::core::providers::mistral::model::ManagerBox#0".to_string()];
    let allowed = boundary_violations(
        "use crate::core::providers::base::{BaseConfig, r#BaseHttpClient, header}; #[cfg(test)] fn mock() { reqwest::Client::new(); } struct Fixture { #[cfg(test)] client: Option<reqwest::Client> } enum Mode { #[cfg(test)] Mock(reqwest::Client), Prod } trait TestOnly { #[cfg(test)] type Pool; #[cfg(test)] fn raw() { reqwest::Client::new(); } } impl TestOnly for Fixture { #[cfg(test)] type Pool = crate::core::providers::base::GlobalPoolManager; #[cfg(test)] passthrough!(reqwest::Client::new()); } extern \"C\" { #[cfg(test)] fn raw(client: reqwest::Client); } fn nested(safe_manager: crate::core::providers::base::GlobalPoolManager) { #[cfg(test)] let client = reqwest::Client::new(); #[cfg(test)] reqwest::Client::new(); #[cfg(test)] passthrough!(reqwest::Client::new()); match false { #[cfg(test)] true => reqwest::Client::new(), false => todo!() }; { let manager = Default::default(); consume(manager); } let manager = safe_manager; let build = |manager| Provider { pool_manager: manager }; build(safe_manager); Provider { pool_manager: passthrough!(safe_manager) }; } macro_rules! test_only { () => { #[cfg(test)] reqwest::Client::new(); safe(); } } macro_rules! test_fn { () => { #[cfg(test)] pub async unsafe fn raw() { reqwest::Client::new(); } pub fn live() { safe(); } } } macro_rules! test_if { () => { #[cfg(test)] if predicate() { reqwest::Client::new(); } safe(); } } macro_rules! test_if_else { () => { #[cfg(test)] if predicate() { reqwest::Client::new(); } else if predicate() { reqwest::Client::new(); } else { reqwest::Client::new(); } safe(); } } macro_rules! policy { () => {{ let _error = Error::default(); policy_manager }} } #[cfg(test)] macro_rules! test_default { () => { Default::default() } } fn result(fail: bool) -> Result<crate::core::providers::base::GlobalPoolManager, Error> { if fail { let result = Err(Error::default()); return result; } let result = Ok(policy_manager).map_err(|_| Error::default()); result } fn policy() -> crate::core::providers::base::GlobalPoolManager { let config = BaseConfig::default(); crate::core::providers::base::GlobalPoolManager::new_for_provider(\"provider\", config) } struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn safe(policy_manager: crate::core::providers::base::GlobalPoolManager) { { use std::default::default as make_error; let _: Error = make_error(); } #[cfg(test)] use std::default::default as test_make; fn take(manager: crate::core::providers::base::GlobalPoolManager) -> crate::core::providers::base::GlobalPoolManager { manager } Provider { pool_manager: take(policy_manager) }; Provider { pool_manager: policy!() }; } use std::default::default as outer_make; fn shadow(outer_make: impl Fn() -> crate::core::providers::base::GlobalPoolManager) { Provider { pool_manager: outer_make() }; } struct SafeTuple(Error); fn safe_tuple(mut value: SafeTuple) { std::mem::take(&mut value.0); } mod other { pub struct TupleProvider(pub Error); } use other::TupleProvider as SafeAlias; fn safe_alias() { SafeAlias(Error::default()); }",
        &["crate", "core", "providers", "mistral"],
        &manager_fields,
    )
    .unwrap_or_else(|error| panic!("allowed fixture must parse: {error}"));
    assert!(allowed.is_empty(), "allowed fixture violations: {allowed:?}"); let precision = boundary_violations("struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn safe(policy: crate::core::providers::base::GlobalPoolManager) { let (mut error, fresh) = (Error::default(), policy); std::mem::take(&mut error); Provider { pool_manager: fresh }; match (Error::default(), policy) { (mut error, fresh) => { std::mem::take(&mut error); Provider { pool_manager: fresh }; } } }", &["crate", "core", "providers", "mistral"], &manager_fields).unwrap_or_else(|error| panic!("precision fixture must parse: {error}")); assert!(precision.is_empty(), "precision fixture violations: {precision:?}");
    for bypass in [
        "fn replace(policy: crate::core::providers::base::GlobalPoolManager) { let mut fresh = Clone::clone(&policy); std::mem::take(&mut fresh); }", "fn replace(policy: crate::core::providers::base::GlobalPoolManager) { if let Some(mut fresh) = Some(policy) { std::mem::take(&mut fresh); } }", "struct Boxed(crate::core::providers::base::GlobalPoolManager); fn replace(policy: crate::core::providers::base::GlobalPoolManager) { let Boxed(mut fresh) = Boxed(policy); std::mem::take(&mut fresh); }", "mod model { pub struct ManagerBox(pub crate::core::providers::base::GlobalPoolManager); } use model as m; fn build() { m::ManagerBox(Default::default()); }", "mod model { pub struct ManagerBox(pub crate::core::providers::base::GlobalPoolManager); } fn replace(mut boxed: model::ManagerBox) { std::mem::take(&mut boxed.0); }", "fn build(policy: crate::core::providers::base::GlobalPoolManager) -> Result<Option<crate::core::providers::base::GlobalPoolManager>, Error> { Some(policy).map(|_| Ok::<_, Error>(Default::default())).transpose() }",
        "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn clone_replace(policy_manager: crate::core::providers::base::GlobalPoolManager) { let mut manager = policy_manager.clone(); std::mem::take(&mut manager); Provider { pool_manager: manager }; }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn identity_replace(policy_manager: crate::core::providers::base::GlobalPoolManager) { let mut manager = std::convert::identity(policy_manager); std::mem::take(&mut manager); Provider { pool_manager: manager }; }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn destructure(policy_manager: crate::core::providers::base::GlobalPoolManager) { let (mut manager,) = (policy_manager,); std::mem::take(&mut manager); Provider { pool_manager: manager }; }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn binding(policy_manager: crate::core::providers::base::GlobalPoolManager) { match policy_manager { mut manager => { std::mem::take(&mut manager); Provider { pool_manager: manager }; } } }", "mod model { pub struct ManagerBox(pub crate::core::providers::base::GlobalPoolManager); } use model::ManagerBox as Alias; fn build() { Alias(Default::default()); }", "fn build() -> Option<Result<(), crate::core::providers::base::GlobalPoolManager>> { Err::<Option<()>, crate::core::providers::base::GlobalPoolManager>(Default::default()).transpose() }", "fn build() -> Result<Option<crate::core::providers::base::GlobalPoolManager>, Error> { ({ Some(Ok::<crate::core::providers::base::GlobalPoolManager, Error>(Default::default())) }).transpose() }",
        "fn probe() { reqwest::Client::new(); }", "fn probe() { reqwest::Client::builder(); }", "fn probe() { reqwest::ClientBuilder::new(); }", "fn probe() { crate::utils::net::http::create_custom_client(timeout); }", "fn probe() { crate::utils::net::http::create_custom_client_with_config(timeout, config); }", "fn probe() { crate::utils::net::http::create_streaming_client(); }", "fn probe() { GlobalPoolManager::new(); }", "fn probe() { GlobalPoolManager::shared(); }", "fn probe() { Pool::new(); } use crate::core::providers::base::GlobalPoolManager as Pool;", "type Pool = crate::core::providers::base::GlobalPoolManager; fn probe() { Pool::shared(); }", "#[allow(unused_parens)] type Pool = (crate::core::providers::base::GlobalPoolManager); fn probe() { Pool::default(); }", "trait HasPool { type Pool; } struct Marker; impl HasPool for Marker { type Pool = crate::core::providers::base::GlobalPoolManager; }",
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
        "fn probe() { passthrough!(reqwest::Client::new()); }", "fn probe() { passthrough!(crate::core::providers::base::send_streaming_request(req, provider)); }",
        "fn probe() { passthrough!(super::base::send_streaming_request(req, provider)); }",
        "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { Self { pool_manager: std::sync::Arc::new(Default::default()) } } }",
        "fn legacy() -> crate::core::providers::base::GlobalPoolManager { Default::default() }",
        "fn legacy() { let pool_manager: crate::core::providers::base::GlobalPoolManager = Default::default(); consume(pool_manager); }",
        "macro_rules! manager_type { () => { crate::core::providers::base::GlobalPoolManager } } type Pool = manager_type!();",
        "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = Default::default(); Self { pool_manager: manager } } }",
        "static FACTORY: fn() -> crate::core::providers::base::GlobalPoolManager = || Default::default();", "use std::default::Default as D; struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = D::default(); Self { pool_manager: manager } } }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new(safe_manager: crate::core::providers::base::GlobalPoolManager) -> Self { let mut manager = safe_manager; manager = Default::default(); Self { pool_manager: manager } } }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { let manager = Default::default(); let build = || Self { pool_manager: manager }; build() } }", "use std::default::Default as D; use D as E; fn legacy() -> crate::core::providers::base::GlobalPoolManager { E::default() }", "impl Provider { fn replace(&mut self) { self.pool_manager = Default::default(); } }", "impl Provider { fn new(safe_manager: crate::core::providers::base::GlobalPoolManager) -> Self { let manager = Default::default(); { let manager = safe_manager; consume(manager); } Self { pool_manager: manager } } }", "impl Provider { fn new(safe_manager: crate::core::providers::base::GlobalPoolManager) -> Self { let mut manager = safe_manager; let mut set = || manager = Default::default(); set(); Self { pool_manager: manager } } }", "macro_rules! legacy { () => { Default::default() } } impl Provider { fn new() -> Self { Self { pool_manager: legacy!() } } }", "static FACTORIES: [fn() -> crate::core::providers::base::GlobalPoolManager; 1] = [|| Default::default()];", "macro_rules! legacy { () => { Default::default() } } static FACTORY: fn() -> crate::core::providers::base::GlobalPoolManager = || legacy!();", "fn legacy() { match <crate::core::providers::base::GlobalPoolManager as Default>::default() { manager => Provider { pool_manager: manager } }; }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } impl Provider { fn new() -> Self { Self { pool_manager: std::convert::identity(Default::default()) } } }", "fn legacy() { for manager in std::iter::once(Default::default()) { Provider { pool_manager: manager }; } while let Some(manager) = Some(Default::default()) { Provider { pool_manager: manager }; } }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { let manager = Default::default(); Ok(manager) }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { let factory = || Default::default(); Ok(factory()) }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { Ok(policy_manager).map(|_| Default::default()) }", "fn build() -> Option<crate::core::providers::base::GlobalPoolManager> { Some(policy_manager).map(|_| Default::default()) }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { Some(Default::default()).ok_or(error) }", "fn build() -> Option<crate::core::providers::base::GlobalPoolManager> { Ok::<_, Error>(Default::default()).ok() }", "use std::result::Result::Ok as Success; fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { Success(Default::default()) }", "fn replace(mut manager: crate::core::providers::base::GlobalPoolManager) { std::mem::take(&mut manager); }", "use std::mem::take as steal; fn replace(mut manager: crate::core::providers::base::GlobalPoolManager) { steal(&mut (manager)); }", "struct TupleProvider(crate::core::providers::base::GlobalPoolManager); fn replace(mut provider: TupleProvider) { std::mem::take(&mut provider.0); provider.0 = Default::default(); }", "struct TupleProvider(crate::core::providers::base::GlobalPoolManager); use self::TupleProvider as Alias; fn build() { Alias(Default::default()); }", "enum State { Ready(crate::core::providers::base::GlobalPoolManager) } fn build() { State::Ready(Default::default()); }", "macro_rules! legacy { () => { Default::default() } } macro_rules! pass { ($value:expr) => { $value } } struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn build() { Provider { pool_manager: pass!(legacy!()) }; }", "struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn replace(provider: &mut Provider) { std::mem::take(&mut provider.pool_manager); }", "use std::default::default as make_manager; struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn build() { Provider { pool_manager: make_manager() }; }", "struct TupleProvider(crate::core::providers::base::GlobalPoolManager); fn build() { TupleProvider(Default::default()); }", "macro_rules! raw { () => { $crate::core::http::default_outbound_client() } }", "macro_rules! hidden { () => { #[cfg(test)] pub fn mock() { reqwest::Client::new(); } pub fn live() { reqwest::Client::new(); } } }", "macro_rules! hidden { () => { #[cfg(test)] if true { reqwest::Client::new(); } reqwest::Client::new(); } }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { let manager = Some(Default::default()).unwrap(); Ok(manager) }", "fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { let manager = { let _ = Some(1); Default::default() }; Ok(manager) }", "fn build() -> Result<Option<crate::core::providers::base::GlobalPoolManager>, Error> { Some(Ok::<_, Error>(Default::default())).transpose() }", "use std::mem::take; fn replace(policy_manager: crate::core::providers::base::GlobalPoolManager) { let mut manager = policy_manager; take(&mut manager); }", "fn replace(mut manager: crate::core::providers::base::GlobalPoolManager) { core::mem::take(&mut manager); }", "use std::mem as memory; fn replace(mut manager: crate::core::providers::base::GlobalPoolManager) { memory::take(&mut manager); }", "use std::mem as memory; use memory as again; fn replace(mut manager: crate::core::providers::base::GlobalPoolManager) { again::take(&mut manager); }", "fn replace(manager_ref: &mut crate::core::providers::base::GlobalPoolManager) { std::mem::take(&mut *manager_ref); }", "use std::result::Result::Ok as Success; use Success as Again; fn build() -> Result<crate::core::providers::base::GlobalPoolManager, Error> { Again(Default::default()) }", "use std::default::default as make; use make as again; struct Provider { pool_manager: crate::core::providers::base::GlobalPoolManager } fn build() { Provider { pool_manager: again() }; }", "struct TupleProvider(crate::core::providers::base::GlobalPoolManager); use self::TupleProvider as Alias; use Alias as Again; fn build() { Again(Default::default()); }", "fn build() { use std::default::default as make; macro_rules! legacy { () => { make() } } Provider { pool_manager: legacy!() }; }", "fn build() { use std::default::Default as D; macro_rules! legacy { () => { D::default() } } Provider { pool_manager: legacy!() }; }",
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
    for (path, module_path, source) in &all_sources { manager_field_names.extend(manager_fields(source, module_path).unwrap_or_else(|error| panic!("{path} must parse: {error}"))); }
    manager_field_names.sort(); manager_field_names.dedup();
    assert!(sources.len() > 300, "provider/runtime inventory is incomplete");
    for exception in BOUNDARY_EXCEPTIONS {
        assert!(sources.iter().any(|(path, _, _)| path == exception.path), "stale boundary exception for {}", exception.path);
    }
    assert!(sources.iter().any(|(path, _, _)| path == UNIFIED_HTTP_IMPLEMENTATION), "unified HTTP implementation is missing from the inventory");
    let mut failures = Vec::new();
    for (path, module_path, source) in sources {
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
    assert!(compact_base.contains("BaseRedirectMode::StreamingDisabled=>{ProviderHttpClient::streaming_no_redirect"));
    assert!(!include_str!("../../../../utils/net/http.rs").contains("get_ssrf_safe_no_redirect_client_with_timeout_fallible"));
    assert_eq!(include_str!("../../bedrock/client.rs").matches("new_for_provider_no_redirect").count(), 3);
    for source in [
        include_str!("../../oci/mod.rs"),
        include_str!("../../sagemaker/mod.rs"),
        include_str!("../../watsonx/mod.rs"),
    ] {
        assert!(source.contains("new_for_provider_no_redirect"));
    }
    assert!(include_str!("../../enterprise.rs").contains("new_for_catalog_no_redirect"));
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
