use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn exported_core_module_cfgs() -> BTreeMap<String, Option<String>> {
    let source =
        std::fs::read_to_string(manifest_path("src/core/mod.rs")).expect("read src/core/mod.rs");
    let mut modules = BTreeMap::new();
    let mut pending_cfg_feature = None;

    for line in source.lines() {
        let line = line.trim();
        if let Some(feature) = cfg_feature_from_attribute(line) {
            pending_cfg_feature = Some(feature.to_string());
            continue;
        }

        if let Some(name) = pub_mod_name(line) {
            modules.insert(name.to_string(), pending_cfg_feature.take());
            continue;
        }

        if !line.is_empty() && !line.starts_with("//") {
            pending_cfg_feature = None;
        }
    }

    modules
}

fn exported_core_modules() -> BTreeSet<String> {
    exported_core_module_cfgs().into_keys().collect()
}

fn pub_mod_name(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("pub mod ")?;
    let name = rest
        .split(|ch: char| ch == ';' || ch.is_ascii_whitespace())
        .next()?;
    (!name.is_empty()).then_some(name)
}

fn cfg_feature_from_attribute(line: &str) -> Option<&str> {
    let condition = line.strip_prefix("#[cfg(")?.strip_suffix(")]")?.trim();
    let value = condition.strip_prefix("feature")?.trim_start();
    let value = value.strip_prefix('=')?.trim_start();
    value.strip_prefix('"')?.split('"').next()
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
    let source = rust_code_without_comments_and_literals(source);
    let pattern = format!("core::{module}");
    source.match_indices(&pattern).any(|(index, _)| {
        let before_match = source[..index].chars().next_back();
        let after_match = index + pattern.len();
        let after_match = source[after_match..].chars().next();
        before_match.is_none_or(|ch| !is_rust_identifier_char(ch))
            && after_match.is_none_or(|ch| !is_rust_identifier_char(ch))
    })
}

fn is_rust_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn rust_code_without_comments_and_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut sanitized = bytes.to_vec();
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index..].starts_with(b"//") {
            let end = bytes[index..]
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(bytes.len(), |offset| index + offset);
            strip_source_range(&mut sanitized, index, end);
            index = end;
            continue;
        }

        if bytes[index..].starts_with(b"/*") {
            let end = block_comment_end(bytes, index);
            strip_source_range(&mut sanitized, index, end);
            index = end;
            continue;
        }

        if let Some(end) = raw_string_literal_end(bytes, index) {
            strip_source_range(&mut sanitized, index, end);
            index = end;
            continue;
        }

        if bytes[index..].starts_with(b"b\"") {
            let end = quoted_literal_end(bytes, index + 1, b'"');
            strip_source_range(&mut sanitized, index, end);
            index = end;
            continue;
        }

        if bytes[index] == b'"' {
            let end = quoted_literal_end(bytes, index, b'"');
            if end > index + 1 {
                strip_source_range(&mut sanitized, index, end);
                index = end;
                continue;
            }
        }

        index += 1;
    }

    String::from_utf8(sanitized).expect("sanitized Rust source remains UTF-8")
}

fn strip_source_range(source: &mut [u8], start: usize, end: usize) {
    for byte in &mut source[start..end] {
        if !matches!(*byte, b'\n' | b'\r') {
            *byte = b' ';
        }
    }
}

fn block_comment_end(bytes: &[u8], start: usize) -> usize {
    let mut depth = 1;
    let mut index = start + 2;
    while index + 1 < bytes.len() {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
            continue;
        }
        if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
            if depth == 0 {
                return index;
            }
            continue;
        }
        index += 1;
    }
    bytes.len()
}

fn raw_string_literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start;
    if bytes.get(index) == Some(&b'b') {
        index += 1;
    }
    if bytes.get(index) != Some(&b'r') {
        return None;
    }
    index += 1;

    let mut hashes = 0;
    while bytes.get(index) == Some(&b'#') {
        hashes += 1;
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return None;
    }
    index += 1;

    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        let suffix_end = index + 1 + hashes;
        if suffix_end <= bytes.len()
            && bytes[index + 1..suffix_end]
                .iter()
                .all(|byte| *byte == b'#')
        {
            return Some(suffix_end);
        }
        index += 1;
    }
    Some(bytes.len())
}

fn quoted_literal_end(bytes: &[u8], start: usize, quote: u8) -> usize {
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return index + 1,
            b'\n' | b'\r' => return start + 1,
            _ => index += 1,
        }
    }
    bytes.len()
}

fn feature_from_runtime_path(runtime_path: &str) -> Option<&str> {
    runtime_path.strip_prefix("Cargo feature: ")
}

fn cargo_features() -> BTreeMap<String, Vec<String>> {
    let manifest = std::fs::read_to_string(manifest_path("Cargo.toml")).expect("read Cargo.toml");
    parse_cargo_features(&manifest)
}

fn parse_cargo_features(manifest: &str) -> BTreeMap<String, Vec<String>> {
    let mut features = BTreeMap::new();
    let mut in_features = false;
    let mut pending_feature: Option<(String, String)> = None;

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if let Some((name, values)) = &mut pending_feature {
            values.push(' ');
            values.push_str(line);
            if line.contains(']') {
                features.insert(name.clone(), quoted_toml_values(values));
                pending_feature = None;
            }
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_features = line == "[features]";
            continue;
        }
        if !in_features || line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((name, values)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().to_string();
        let values = values.trim();
        if values.starts_with('[') && !values.contains(']') {
            pending_feature = Some((name, values.to_string()));
        } else {
            features.insert(name, quoted_toml_values(values));
        }
    }

    features
}

fn quoted_toml_values(value: &str) -> Vec<String> {
    let bytes = value.as_bytes();
    let mut values = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'"' {
            index += 1;
            continue;
        }

        index += 1;
        let mut value = String::new();
        while index < bytes.len() {
            match bytes[index] {
                b'\\' if index + 1 < bytes.len() => {
                    value.push(bytes[index + 1] as char);
                    index += 2;
                }
                b'"' => break,
                byte => {
                    value.push(byte as char);
                    index += 1;
                }
            }
        }
        values.push(value);
        index += 1;
    }
    values
}

fn default_enabled_features(features: &BTreeMap<String, Vec<String>>) -> BTreeSet<String> {
    let mut enabled = BTreeSet::new();
    let mut pending = features.get("default").cloned().unwrap_or_default();

    while let Some(feature) = pending.pop() {
        let feature = feature.split('/').next().unwrap_or(&feature);
        if feature.starts_with("dep:") || !features.contains_key(feature) {
            continue;
        }
        if enabled.insert(feature.to_string()) {
            pending.extend(features[feature].iter().cloned());
        }
    }

    enabled
}

fn validate_feature_gate(
    subsystem: &CoreSubsystem,
    module_cfg_features: &BTreeMap<String, Option<String>>,
    manifest_features: &BTreeMap<String, Vec<String>>,
) -> Result<(), String> {
    let feature = subsystem
        .runtime_path
        .and_then(feature_from_runtime_path)
        .ok_or_else(|| format!("{} must name its cargo feature", subsystem.name))?;

    if !manifest_features.contains_key(feature) {
        return Err(format!(
            "{} uses unknown Cargo feature '{}'",
            subsystem.name, feature
        ));
    }

    let default_features = default_enabled_features(manifest_features);
    if default_features.contains(feature) {
        return Err(format!(
            "{} uses default-enabled/support feature '{}' as a gate",
            subsystem.name, feature
        ));
    }

    match module_cfg_features.get(subsystem.name) {
        Some(Some(cfg_feature)) if cfg_feature == feature => Ok(()),
        Some(Some(cfg_feature)) => Err(format!(
            "core::{} is gated by feature '{}' but registry names '{}'",
            subsystem.name, cfg_feature, feature
        )),
        Some(None) => Err(format!(
            "core::{} must be guarded by #[cfg(feature = \"{}\")]",
            subsystem.name, feature
        )),
        None => Err(format!(
            "core::{} is not exported from src/core/mod.rs",
            subsystem.name
        )),
    }
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
fn runtime_reference_check_ignores_config_admin_comments_and_strings() {
    let source = r##"
        let semantic_cache_enabled = cfg.gateway.cache.semantic_cache;
        let body = json!({"semantic_cache_enabled": semantic_cache_enabled});
        let status = "core::semantic_cache is disabled until wired";
        let raw = r#"admin text mentions crate::core::semantic_cache::SemanticCache"#;
        // crate::core::semantic_cache::SemanticCache is mentioned in a comment only.
        /*
         * litellm_rs::core::semantic_cache::SemanticCache is not live code here.
         */
        let warning = "core::semantic_cache_extra is not a module path match";
    "##;

    assert!(!runtime_source_has_core_module_reference(
        source,
        "semantic_cache"
    ));
}

#[test]
fn runtime_reference_check_matches_real_core_module_paths() {
    let source = r#"
        let _manager = crate::core::semantic_cache::SemanticCache::new(config);
        let _factory = litellm_rs::core::semantic_cache::SemanticCacheFactory::default();
    "#;

    assert!(runtime_source_has_core_module_reference(
        source,
        "semantic_cache"
    ));
}

#[test]
fn runtime_reference_check_keeps_paths_between_lifetimes() {
    let source = r#"
        fn build<'a>(manager: crate::core::semantic_cache::SemanticCache, label: &'a str) {
            let _ = (manager, label);
        }
    "#;

    assert!(runtime_source_has_core_module_reference(
        source,
        "semantic_cache"
    ));
}

#[test]
fn feature_gated_decisions_use_real_default_off_module_gates() {
    let module_cfg_features = exported_core_module_cfgs();
    let manifest_features = cargo_features();

    for subsystem in CORE_SUBSYSTEMS {
        if subsystem.decision != SubsystemDecision::FeatureGated {
            continue;
        }

        validate_feature_gate(subsystem, &module_cfg_features, &manifest_features)
            .unwrap_or_else(|message| panic!("{message}"));
    }
}

#[test]
fn feature_gate_validation_rejects_default_on_support_feature() {
    let subsystem = CoreSubsystem {
        name: "fixture",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: storage"),
        note: "fixture",
    };
    let module_cfg_features =
        BTreeMap::from([("fixture".to_string(), Some("storage".to_string()))]);
    let manifest_features = BTreeMap::from([
        ("default".to_string(), vec!["storage".to_string()]),
        ("storage".to_string(), vec![]),
    ]);

    let error = validate_feature_gate(&subsystem, &module_cfg_features, &manifest_features)
        .expect_err("default-on support feature must not be a gate");
    assert!(error.contains("default-enabled/support feature"));
}

#[test]
fn feature_gate_validation_rejects_registry_only_gate() {
    let subsystem = CoreSubsystem {
        name: "fixture",
        decision: SubsystemDecision::FeatureGated,
        runtime_path: Some("Cargo feature: experimental_fixture"),
        note: "fixture",
    };
    let module_cfg_features = BTreeMap::from([("fixture".to_string(), None)]);
    let manifest_features = BTreeMap::from([
        ("default".to_string(), vec![]),
        ("experimental_fixture".to_string(), vec![]),
    ]);

    let error = validate_feature_gate(&subsystem, &module_cfg_features, &manifest_features)
        .expect_err("registry-only gate must not be accepted");
    assert!(error.contains("#[cfg(feature = \"experimental_fixture\")]"));
}

#[test]
fn cargo_feature_parser_expands_default_feature_closure() {
    let features = parse_cargo_features(
        r#"
        [features]
        default = [
            "sqlite",
        ]
        sqlite = [
            "storage",
            "sea-orm/sqlx-sqlite",
        ]
        storage = [
            "gateway",
            "dep:sea-orm",
        ]
        gateway = []
        analytics = ["storage"]
        "#,
    );

    let default_features = default_enabled_features(&features);
    assert!(default_features.contains("sqlite"));
    assert!(default_features.contains("storage"));
    assert!(default_features.contains("gateway"));
    assert!(!default_features.contains("analytics"));
}

#[test]
fn known_non_gateway_modules_are_positive_exemptions() {
    for (name, decision) in [
        ("completion", SubsystemDecision::LibraryOnly),
        ("function_calling", SubsystemDecision::LibraryOnly),
        ("traits", SubsystemDecision::InternalDependency),
        ("secret_managers", SubsystemDecision::LibraryOnly),
    ] {
        let subsystem = subsystem_for(name).expect("non-gateway subsystem exists");
        assert_eq!(subsystem.decision, decision);
        assert!(subsystem.has_gateway_reference_exemption());
    }
}

#[test]
fn issue_838_subsystems_have_explicit_non_silent_decisions() {
    let expected = [
        ("a2a", SubsystemDecision::FeatureGated),
        ("analytics", SubsystemDecision::FeatureGated),
        ("audit", SubsystemDecision::Wired),
        ("batch", SubsystemDecision::LibraryOnly),
        ("guardrails", SubsystemDecision::Wired),
        ("integrations", SubsystemDecision::Wired),
        ("ip_access", SubsystemDecision::Wired),
        ("mcp", SubsystemDecision::FeatureGated),
        ("observability", SubsystemDecision::Wired),
        ("realtime", SubsystemDecision::FeatureGated),
        ("semantic_cache", SubsystemDecision::ConfigRejected),
        ("user_management", SubsystemDecision::InternalDependency),
        ("virtual_keys", SubsystemDecision::Wired),
        ("webhooks", SubsystemDecision::FeatureGated),
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

    assert!(
        CORE_SUBSYSTEMS
            .iter()
            .all(|subsystem| subsystem.decision != SubsystemDecision::TemporaryExemption),
        "GH838 compatibility variant must not classify an active subsystem"
    );

    let source = std::fs::read_to_string(manifest_path("src/core/subsystem_registry.rs"))
        .expect("read subsystem registry source");
    for symbol in [
        "GH838_TEMPORARY_EXEMPTION_ISSUE",
        "GH838_TEMPORARY_EXEMPTIONS",
    ] {
        assert!(source.contains(symbol), "0.6 compatibility symbol {symbol}");
    }
}

#[test]
fn breaking_release_workflow_requires_explicit_confirmation() {
    let workflow = std::fs::read_to_string(manifest_path(".github/workflows/version-bump.yml"))
        .expect("read version bump workflow");

    assert!(workflow.contains("confirm_breaking_changes"));
    assert!(workflow.contains("compatibility and migration review"));
    assert!(workflow.contains("too small for detected breaking changes"));
    assert!(workflow.contains("| grep -E '(^|[[:space:]])"));
    assert!(!workflow.contains("grep -qE"));
}
