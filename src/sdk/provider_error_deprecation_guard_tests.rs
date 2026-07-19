#[rustfmt::skip]
mod guard {
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use syn::ext::IdentExt;
use syn::visit::{self, Visit};
use syn::{Expr, GenericArgument, ItemImpl, Meta, PathArguments, Type, UseTree};
const MARKER: &str = "SP965-T010 links 0.7 removal follow-up for SDKError::ProviderError";
const META: &str = "#[deprecated(\n        since = \"0.6.0\",\n        note = \"use the existing typed SDK categories returned by ProviderError conversion\"\n    )]\n    ProviderError(String),";
type Finding = (String, String, String); type Sources = BTreeMap<String, String>; type LintFiles = Vec<(String, String)>;
fn id(value: &syn::Ident) -> String { value.unraw().to_string() }
fn lint_suppression(meta: &Meta) -> bool {
    let Meta::List(list) = meta else { return false }; let kind = list.path.get_ident().map(id).unwrap_or_default();
    if matches!(kind.as_str(), "allow" | "expect") { let body = list.tokens.to_string().replace(' ', ""); return body.split(',').any(|name| matches!(name, "deprecated" | "warnings")); }
    let nested = list.parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    kind == "cfg_attr" && nested.map_or(true, |attrs| attrs.iter().skip(1).any(lint_suppression))
}
fn ty_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Path(value) => value.path.segments.last().map(|part| id(&part.ident)),
        Type::Group(value) => ty_name(&value.elem),
        Type::Paren(value) => ty_name(&value.elem),
        _ => None,
    }
}
fn impl_name(item: &ItemImpl) -> String {
    item.trait_.as_ref().and_then(|(_, path, _)| path.segments.last()).and_then(|part| {
        let PathArguments::AngleBracketed(args) = &part.arguments else { return None };
        args.args.iter().find_map(|arg| match arg { GenericArgument::Type(ty) => ty_name(ty), _ => None })
    }).or_else(|| ty_name(&item.self_ty)).unwrap_or_else(|| "unknown".into())
}
fn flatten_use(tree: &UseTree, prefix: &mut Vec<String>, out: &mut Vec<(Vec<String>, String)>) {
    match tree {
        UseTree::Path(value) => { prefix.push(id(&value.ident)); flatten_use(&value.tree, prefix, out); prefix.pop(); }
        UseTree::Name(value) => { let mut path = prefix.clone(); path.push(id(&value.ident)); out.push((path, id(&value.ident))); }
        UseTree::Rename(value) => { let mut path = prefix.clone(); path.push(id(&value.ident)); out.push((path, id(&value.rename))); }
        UseTree::Glob(_) => { let mut path = prefix.clone(); path.push("*".into()); out.push((path, "*".into())); }
        UseTree::Group(value) => value.items.iter().for_each(|tree| flatten_use(tree, prefix, out)),
    }
}
struct AliasVisitor<'a>(&'a mut BTreeSet<String>);
impl<'ast> Visit<'ast> for AliasVisitor<'_> {
    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        if ty_name(&item.ty).is_some_and(|name| self.0.contains(&name)) { self.0.insert(id(&item.ident)); }
        visit::visit_item_type(self, item);
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut uses = Vec::new(); flatten_use(&item.tree, &mut Vec::new(), &mut uses);
        for (path, local) in uses { if path.last().is_some_and(|name| self.0.contains(name)) { self.0.insert(local); } }
    }
}
struct SourceVisitor<'a> { file: &'a str, aliases: BTreeSet<String>, owner: String,
    impl_owner: String, findings: Vec<Finding>, attrs: Vec<String> }
impl SourceVisitor<'_> {
    fn legacy(&self, path: &syn::Path, qself: Option<&syn::QSelf>) -> bool {
        let alias = |name: &str| self.aliases.contains(name) || (name == "Self" && self.impl_owner == "SDKError");
        path.segments.last().is_some_and(|part| id(&part.ident) == "ProviderError")
            && (path.segments.iter().rev().skip(1).any(|part| alias(&id(&part.ident)))
                || qself.and_then(|value| ty_name(&value.ty)).is_some_and(|name| alias(&name)))
    }
    fn hit(&mut self, role: &str) { self.findings.push((self.file.into(), self.owner.clone(), role.into())); }
    fn owned(&mut self, owner: String, run: impl FnOnce(&mut Self)) {
        let old = std::mem::replace(&mut self.owner, owner); run(self); self.owner = old;
    }
    fn macro_tokens(&mut self, source: &str) {
        let compact = source.split('"').step_by(2).collect::<String>().replace(' ', "");
        if ["allow(warnings)", "expect(warnings)", "allow(deprecated)", "expect(deprecated)"].iter().any(|item| compact.contains(item)) { self.attrs.push("macro:suppression".into()); }
        let words: Vec<_> = source.split_whitespace().map(|word| word.trim_start_matches("r#")).collect();
        for index in 2..words.len() {
            if words[index] == "ProviderError" && words[index - 1] == "::"
                && words[index.saturating_sub(8)..index - 1].iter().any(|word| {
                    self.aliases.contains(*word) || (*word == "Self" && self.impl_owner == "SDKError")
                }) { self.hit("macro"); }
        }
    }
}
impl<'ast> Visit<'ast> for SourceVisitor<'_> {
    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let old = std::mem::replace(&mut self.impl_owner, impl_name(item));
        self.owned(format!("impl:{}", self.impl_owner), |this| {
            item.attrs.iter().for_each(|attr| this.visit_attribute(attr));
            item.items.iter().for_each(|child| this.visit_impl_item(child));
        }); self.impl_owner = old;
    }
    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.owned(format!("{}@{}", id(&item.sig.ident), self.impl_owner), |this| {
            item.attrs.iter().for_each(|attr| this.visit_attribute(attr)); this.visit_block(&item.block);
        });
    }
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.owned(id(&item.sig.ident), |this| {
            item.attrs.iter().for_each(|attr| this.visit_attribute(attr)); this.visit_block(&item.block);
        });
    }
    fn visit_expr_closure(&mut self, item: &'ast syn::ExprClosure) { self.owned(format!("closure@{}", self.owner), |this| visit::visit_expr_closure(this, item)); }
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.owned(format!("mod:{}", id(&item.ident)), |this| {
            item.attrs.iter().for_each(|attr| this.visit_attribute(attr));
            if let Some((_, items)) = &item.content { items.iter().for_each(|child| this.visit_item(child)); }
        });
    }
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if lint_suppression(&attr.meta) {
            let kind = attr.path().get_ident().map(id).unwrap_or_default();
            let body = match &attr.meta { Meta::List(list) => list.tokens.to_string().replace(' ', ""), _ => String::new() };
            self.attrs.push(format!("{kind}:{body}"));
        }
    }
    fn visit_expr_call(&mut self, item: &'ast syn::ExprCall) {
        if let Expr::Path(path) = &*item.func && self.legacy(&path.path, path.qself.as_ref()) {
            self.hit("construct"); item.args.iter().for_each(|arg| self.visit_expr(arg));
        } else { visit::visit_expr_call(self, item); }
    }
    fn visit_expr_path(&mut self, item: &'ast syn::ExprPath) {
        if self.legacy(&item.path, item.qself.as_ref()) { self.hit("value"); }
        visit::visit_expr_path(self, item);
    }
    fn visit_pat_tuple_struct(&mut self, item: &'ast syn::PatTupleStruct) {
        if self.legacy(&item.path, None) {
            self.hit("pattern"); item.elems.iter().for_each(|pat| self.visit_pat(pat));
        } else { visit::visit_pat_tuple_struct(self, item); }
    }
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let mut uses = Vec::new(); flatten_use(&item.tree, &mut Vec::new(), &mut uses);
        if uses.iter().any(|(path, _)| path.windows(2).any(|part| {
            part == ["SDKError", "ProviderError"] || part == ["SDKError", "*"]
        })) { self.hit("import"); }
    }
    fn visit_macro(&mut self, item: &'ast syn::Macro) { self.macro_tokens(&item.tokens.to_string()); }
}
fn scan(path: &str, source: &str) -> Result<(Vec<Finding>, Vec<String>), String> {
    let file = syn::parse_file(source).map_err(|error| format!("{path}: {error}"))?;
    let mut aliases = BTreeSet::from(["SDKError".into()]);
    loop { let count = aliases.len(); AliasVisitor(&mut aliases).visit_file(&file); if aliases.len() == count { break; } }
    let mut visitor = SourceVisitor {
        file: path, aliases, owner: "<module>".into(), impl_owner: String::new(),
        findings: Vec::new(), attrs: Vec::new(),
    };
    file.attrs.iter().for_each(|attr| visitor.visit_attribute(attr));
    file.items.iter().for_each(|item| visitor.visit_item(item));
    Ok((visitor.findings, visitor.attrs))
}
fn walk(path: &Path, rust_only: bool, files: &mut BTreeSet<PathBuf>) -> Result<(), String> {
    if !path.exists() { return Ok(()); }
    for entry in fs::read_dir(path).map_err(|error| format!("{}: {error}", path.display()))? {
        let child = entry.map_err(|error| error.to_string())?.path();
        if child.is_dir() {
            if child.file_name().is_none_or(|name| name != "__pycache__") { walk(&child, rust_only, files)?; }
        } else if !rust_only || child.extension().is_some_and(|ext| ext == "rs") { files.insert(child); }
    } Ok(())
}
fn inventory() -> Result<(Sources, LintFiles), String> {
    let root = PathBuf::from(std::env::var("SP965_T023B_GUARD_ROOT")
        .unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").into())).canonicalize().map_err(|error| error.to_string())?;
    let output = Command::new("cargo").args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(&root).output().map_err(|error| format!("cargo metadata: {error}"))?;
    if !output.status.success() { return Err(String::from_utf8_lossy(&output.stderr).into_owned()); }
    let data: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let mut roots = BTreeSet::from([root.clone()]); let mut rust = BTreeSet::new(); let mut lint = BTreeSet::new();
    for package in data["packages"].as_array().ok_or("metadata packages")? {
        let manifest = PathBuf::from(package["manifest_path"].as_str().ok_or("manifest")?);
        roots.insert(manifest.parent().ok_or("manifest parent")?.into()); lint.insert(manifest);
        for target in package["targets"].as_array().ok_or("targets")? {
            let path = PathBuf::from(target["src_path"].as_str().ok_or("src_path")?); if path.exists() { rust.insert(path); }
        }
    }
    if root.join(".git").exists() && let Ok(output) = Command::new("git").args(["ls-files", "*.rs"]).current_dir(&root).output()
        && output.status.success() {
        for path in String::from_utf8_lossy(&output.stdout).lines().map(|path| root.join(path)) {
            if path.exists() { rust.insert(path); }
        }
    }
    for package in &roots {
        for dir in ["src", "tests", "examples", "benches"] { walk(&package.join(dir), true, &mut rust)?; }
        for dir in [".cargo", ".github/workflows", "scripts", "checks", "xtask"] { walk(&package.join(dir), false, &mut lint)?; }
        for entry in fs::read_dir(package).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_file() && path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy(); name.starts_with("Makefile") || name.starts_with("justfile")
                    || name.starts_with("rust-toolchain") || name == "clippy.toml" || name == "build.rs"
            }) { lint.insert(path.clone()); if path.file_name().is_some_and(|name| name == "build.rs") { rust.insert(path); } }
        }
    }
    let read = |path: PathBuf| -> Result<(String, String), String> {
        let label = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().into_owned();
        fs::read_to_string(&path).map(|source| (label, source)).map_err(|error| format!("{}: {error}", path.display()))
    };
    Ok((rust.into_iter().map(&read).collect::<Result<_, _>>()?,
        lint.into_iter().filter(|path| path.is_file()).map(read).collect::<Result<_, _>>()?))
}
fn lint_ok(source: &str, path: &str) -> Result<(), String> {
    let words: Vec<_> = source.to_lowercase().split(|ch: char| ch.is_whitespace() || "\"'=,[]".contains(ch))
        .filter(|word| !word.is_empty()).map(str::to_owned).collect();
    let pairs = [["-a", "deprecated"], ["-a", "warnings"], ["--allow", "deprecated"],
        ["--allow", "warnings"], ["--cap-lints", "allow"], ["--cap-lints", "warn"]];
    if words.iter().any(|word| matches!(word.as_str(), "-adeprecated" | "-awarnings"))
        || words.windows(2).any(|pair| pairs.iter().any(|bad| pair == bad))
    { Err(format!("lint downgrade: {path}")) } else { Ok(()) }
}
fn expected_findings() -> Vec<Finding> {
    let errors = "src/sdk/errors.rs".to_string();
    let mut expected = vec![
        (errors.clone(), "from@GatewayError".into(), "construct".into()),
        (errors.clone(), "from@ProviderError".into(), "construct".into()),
        (errors.clone(), "is_retryable@SDKError".into(), "macro".into()),
        (errors.clone(), "sdk_variant".into(), "pattern".into()),
        (errors.clone(), "test_sdk_error_provider_error".into(), "construct".into()),
        (errors.clone(), "test_is_retryable_provider_error".into(), "construct".into()),
        (errors.clone(), "test_from_gateway_error_provider_unavailable".into(), "macro".into()),
        (errors, "test_sdk_error_empty_message".into(), "construct".into()),
        ("src/sdk/client/completions.rs".into(), "execute_chat_request@LLMClient".into(), "construct".into()),
    ]; expected.sort(); expected
}
fn expected_attrs(sources: &Sources) -> BTreeMap<String, usize> {
    let mut expected = BTreeMap::from([
        ("src/sdk/errors.rs".into(), 8), ("src/sdk/client/completions.rs".into(), 1),
        ("src/core/traits/provider/llm_provider/sub_traits.rs".into(), 9),
        ("src/server/routes/mod.rs".into(), 1),
    ]);
    for path in ["src/core/router/tests/concurrency_edge_case_tests.rs", "src/core/router/tests/execution_tests.rs",
        "src/core/router/tests/router_tests.rs", "src/core/router/tests/selection_tests.rs",
        "src/core/router/tests/strategy_tests.rs", "tests/integration/router_tests.rs"] {
        if sources.contains_key(path) { expected.insert(path.into(), 1); }
    } expected.retain(|path, _| sources.contains_key(path)); expected
}
fn verify(sources: &Sources, lint: &[(String, String)]) -> Result<(), String> {
    let mut findings = Vec::new(); let mut attrs = BTreeMap::new();
    for (path, source) in sources {
        let (mut found, lint_attrs) = scan(path, source)?; findings.append(&mut found);
        if !lint_attrs.is_empty() {
            if lint_attrs.iter().any(|attr| attr != "allow:deprecated") { return Err(format!("broad lint attr: {path}: {lint_attrs:?}")); }
            attrs.insert(path.clone(), lint_attrs.len());
        }
    }
    findings.sort(); if findings != expected_findings() { return Err(format!("legacy findings: {findings:#?}")); }
    if attrs != expected_attrs(sources) { return Err(format!("deprecated attrs: {attrs:#?}")); }
    let errors = &sources["src/sdk/errors.rs"]; let completions = &sources["src/sdk/client/completions.rs"];
    if errors.matches(META).count() != 1 { return Err("legacy metadata changed".into()); }
    let adjacent = |source: &str| source.lines().collect::<Vec<_>>().windows(2)
        .filter(|lines| lines[0].trim() == format!("// {MARKER}") && lines[1].trim() == "#[allow(deprecated)]").count();
    if adjacent(errors) != 8 || adjacent(completions) != 1 { return Err("marker/allow adjacency changed".into()); }
    let anchors = [
        ("src/server/routes/mod.rs", "fn test_api_response_to_http_response_remains_compatibility_shim"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "impl<T: LLMProvider> LLMChat for T"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "impl<T: LLMProvider> LLMEmbed for T"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "impl<T: LLMProvider> LLMStream for T"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "async fn test_llm_chat_blanket_impl"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "async fn test_llm_embed_blanket_impl"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "async fn test_llm_stream_blanket_impl"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "fn _accepts_chat<T: LLMChat>"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "fn _accepts_embed<T: LLMEmbed>"),
        ("src/core/traits/provider/llm_provider/sub_traits.rs", "fn _accepts_stream<T: LLMStream>"),
    ];
    for (path, anchor) in anchors {
        if let Some(source) = sources.get(path) {
            let offset = source.find(anchor).ok_or_else(|| format!("missing anchor: {path}: {anchor}"))?;
            if !source[..offset].trim_end().ends_with("#[allow(deprecated)]") { return Err(format!("moved allow: {path}: {anchor}")); }
        }
    }
    for (path, source) in lint { lint_ok(source, path)?; } Ok(())
}
fn replace_once(source: &str, old: &str, new: &str) -> String {
    assert_eq!(source.matches(old).count(), 1, "mutation anchor: {old}"); source.replacen(old, new, 1)
}
fn rejected(label: &str, sources: &Sources, lint: &[(String, String)]) {
    assert!(verify(sources, lint).is_err(), "mutation accepted: {label}");
}
#[test]
fn legacy_provider_error_deprecation_allowlist_does_not_grow() {
    let (sources, lint) = inventory().unwrap(); verify(&sources, &lint).unwrap();
    let errors = &sources["src/sdk/errors.rs"]; let mut mutated = sources.clone();
    mutated.insert("src/sdk/errors.rs".into(), replace_once(errors, "mod tests {\n", "mod tests {\n    #![allow(warnings)]\n"));
    rejected("allow warnings", &mutated, &lint);
    mutated = sources.clone(); mutated.insert("src/sdk/errors.rs".into(), format!("macro_rules! smuggle {{ () => {{ #[cfg_attr(all(), allow(warnings))] fn hidden() {{}} }}; }}\n{errors}"));
    rejected("macro cfg_attr smuggle", &mutated, &lint);
    mutated.insert("src/sdk/errors.rs".into(), replace_once(errors, "mod tests {\n", "mod tests {\n    #![cfg_attr(all(), allow(warnings))]\n"));
    rejected("cfg_attr warnings", &mutated, &lint);
    mutated.insert("src/sdk/errors.rs".into(), replace_once(errors,
        "let error = SDKError::ProviderError(\"unavailable\".to_string());",
        "let make = SDKError::ProviderError;\n        let error = make(\"unavailable\".to_string());"));
    rejected("value alias", &mutated, &lint);
    mutated.insert("src/sdk/errors.rs".into(), replace_once(errors, "let error = SDKError::ProviderError(\"unavailable\".to_string());",
        "let make = |message| SDKError::ProviderError(message);\n        let error = make(\"unavailable\".to_string());"));
    rejected("closure alias", &mutated, &lint);
    mutated.insert("src/sdk/errors.rs".into(), replace_once(errors,
        "let error = SDKError::ProviderError(\"API unavailable\".to_string());",
        "let error = SDKError::ProviderError(\"API unavailable\".to_string());\n        let _ = <SDKError>::ProviderError(String::new());\n        let _ = SDKError::r#ProviderError(String::new());"));
    rejected("qself/raw paths", &mutated, &lint);
    let completions = &sources["src/sdk/client/completions.rs"];
    mutated = sources.clone(); mutated.insert("src/sdk/client/completions.rs".into(),
        format!("macro_rules! relocated {{ () => {{ SDKError::ProviderError(String::new()) }}; }}\n{}",
            replace_once(completions, "_ => Err(SDKError::ProviderError(format!(", "_ => Err(SDKError::Internal(format!(")));
    rejected("macro owner", &mutated, &lint);
    mutated = sources.clone(); mutated.remove("tests/integration/router_tests.rs");
    verify(&mutated, &lint).unwrap(); assert!(lint_ok("RUSTFLAGS='--cap-lints allow'", "mutation").is_err());
}
}
