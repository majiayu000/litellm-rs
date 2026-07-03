use super::*;
use crate::core::providers::registry::types::{ProviderDispatchKind, provider_type_registry};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const PROVIDER_IMPL_MARKERS: &[&str] = &[
    "LLMProvider for",
    "define_http_provider_with_hooks!(",
    "define_pooled_http_provider_with_hooks!(",
    "define_openai_compatible_provider!(",
    "standard_provider!(",
];

fn lifecycle_module_names() -> BTreeSet<&'static str> {
    PROVIDER_MODULE_LIFECYCLE
        .iter()
        .map(|entry| entry.module_name)
        .collect()
}

fn orphan_baseline_module_names() -> BTreeSet<&'static str> {
    PROVIDER_ORPHAN_BASELINE
        .iter()
        .map(|entry| entry.module_name)
        .collect()
}

fn registry_runtime_module_names() -> BTreeSet<&'static str> {
    let mut modules = provider_type_registry()
        .iter()
        .filter(|entry| entry.dispatch_kind == ProviderDispatchKind::Native)
        .map(|entry| entry.canonical_name)
        .collect::<BTreeSet<_>>();

    assert!(
        provider_type_registry()
            .iter()
            .any(
                |entry| entry.dispatch_kind == ProviderDispatchKind::ExplicitOpenAiLike
                    || entry.dispatch_kind == ProviderDispatchKind::CatalogOpenAiLike
            ),
        "OpenAI-like runtime dispatch entries should be present"
    );
    modules.insert("openai_like");
    modules
}

fn lifecycle_for(module_name: &str) -> ProviderModuleLifecycle {
    PROVIDER_MODULE_LIFECYCLE
        .iter()
        .find(|entry| entry.module_name == module_name)
        .unwrap_or_else(|| panic!("missing lifecycle entry for {module_name}"))
        .lifecycle
}

fn providers_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/providers")
}

fn provider_directories() -> BTreeSet<String> {
    fs::read_dir(providers_dir())
        .expect("providers directory should be readable")
        .filter_map(|entry| {
            let entry = entry.expect("provider directory entry should be readable");
            if !entry
                .file_type()
                .expect("file type should be readable")
                .is_dir()
            {
                return None;
            }
            entry.file_name().into_string().ok()
        })
        .collect()
}

fn is_registry_runtime_module(module_name: &str) -> bool {
    registry_runtime_module_names().contains(module_name)
}

fn directory_contains_provider_impl_marker(module_name: &str) -> bool {
    if matches!(module_name, "macros" | "registry") {
        return false;
    }

    let mut pending_dirs = vec![providers_dir().join(module_name)];
    while let Some(dir) = pending_dirs.pop() {
        for entry in fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("provider directory {dir:?} should be readable: {err}"))
        {
            let entry = entry.expect("provider directory entry should be readable");
            let path = entry.path();
            let file_type = entry.file_type().expect("file type should be readable");

            if file_type.is_dir() {
                pending_dirs.push(path);
                continue;
            }

            if path.extension().is_some_and(|extension| extension == "rs") {
                let source = fs::read_to_string(&path).unwrap_or_else(|err| {
                    panic!("provider source {path:?} should be readable: {err}")
                });
                if PROVIDER_IMPL_MARKERS
                    .iter()
                    .any(|marker| source.contains(marker))
                {
                    return true;
                }
            }
        }
    }

    false
}

#[test]
fn lifecycle_classifies_phase0_key_provider_modules() {
    assert_eq!(lifecycle_for("bedrock"), ProviderModuleLifecycle::Wire);
    assert_eq!(
        lifecycle_for("vertex_ai"),
        if cfg!(feature = "providers-extra") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("azure"),
        if cfg!(feature = "providers-extra") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("azure_ai"),
        if cfg!(feature = "providers-extra") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("github_copilot"),
        if cfg!(feature = "providers-extended") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("cohere"),
        if cfg!(feature = "providers-extended") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("fal_ai"),
        if cfg!(feature = "providers-extended") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("replicate"),
        if cfg!(feature = "providers-extended") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
    assert_eq!(
        lifecycle_for("gemini"),
        if cfg!(feature = "providers-extended") {
            ProviderModuleLifecycle::Wire
        } else {
            ProviderModuleLifecycle::Stub
        }
    );
}

#[test]
fn lifecycle_wire_entries_match_registry_runtime_modules() {
    let actual = PROVIDER_MODULE_LIFECYCLE
        .iter()
        .filter(|entry| entry.lifecycle == ProviderModuleLifecycle::Wire)
        .map(|entry| entry.module_name)
        .collect::<BTreeSet<_>>();
    let expected = registry_runtime_module_names();

    assert_eq!(actual, expected);
}

#[test]
fn lifecycle_covers_every_provider_directory() {
    let actual = provider_directories();
    let declared = lifecycle_module_names()
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();

    assert_eq!(actual, declared);
}

#[test]
fn lifecycle_has_no_delete_decisions_without_owner_confirmation() {
    assert!(
        PROVIDER_MODULE_LIFECYCLE
            .iter()
            .all(|entry| entry.lifecycle != ProviderModuleLifecycle::Delete),
        "Delete lifecycle requires explicit owner confirmation"
    );
}

#[test]
fn lifecycle_entries_have_reasons() {
    for entry in PROVIDER_MODULE_LIFECYCLE {
        assert!(
            !entry.reason.trim().is_empty(),
            "{} lifecycle entry must include a reason",
            entry.module_name
        );
    }
}

#[test]
fn lifecycle_blocks_unapproved_stub_provider_modules() {
    let baseline = orphan_baseline_module_names();
    let mut unapproved = Vec::new();

    for entry in PROVIDER_MODULE_LIFECYCLE {
        if entry.lifecycle != ProviderModuleLifecycle::Stub {
            continue;
        }
        let module_name = entry.module_name;
        if is_registry_runtime_module(module_name) {
            continue;
        }
        if baseline.contains(module_name) {
            continue;
        }
        unapproved.push(module_name.to_string());
    }

    assert!(
        unapproved.is_empty(),
        "unapproved stub provider modules must be wired, deleted, demoted, explicitly gated, or added to the GH837 baseline: {unapproved:?}"
    );
}

#[test]
fn orphan_baseline_entries_are_live_and_bounded() {
    let provider_dirs = provider_directories();
    let mut seen = BTreeSet::new();

    for entry in PROVIDER_ORPHAN_BASELINE {
        assert!(
            seen.insert(entry.module_name),
            "{} appears more than once in the orphan baseline",
            entry.module_name
        );
        assert!(
            provider_dirs.contains(entry.module_name),
            "{} baseline entry must reference an existing provider directory",
            entry.module_name
        );
        assert!(
            matches!(
                lifecycle_for(entry.module_name),
                ProviderModuleLifecycle::Stub | ProviderModuleLifecycle::Internal
            ),
            "{} baseline entry must reference a non-runtime provider module",
            entry.module_name
        );
        assert!(
            !is_registry_runtime_module(entry.module_name),
            "{} is natively reachable and should not be in the orphan baseline",
            entry.module_name
        );
        if !directory_contains_provider_impl_marker(entry.module_name) {
            assert_eq!(
                entry.lane, "non-llm-lane",
                "{} markerless provider module must be tracked in the non-LLM lane",
                entry.module_name
            );
        }
        assert!(
            matches!(
                entry.lane,
                "delete-native" | "demote-to-catalog" | "non-llm-lane" | "exempt"
            ),
            "{} baseline entry has unsupported lane {}",
            entry.module_name,
            entry.lane
        );
        assert_eq!(entry.issue, "GH837");
        assert!(
            !entry.owner.trim().is_empty(),
            "{} baseline entry must include an owner",
            entry.module_name
        );
        assert!(
            !entry.expires.trim().is_empty(),
            "{} baseline entry must include an expiry condition",
            entry.module_name
        );
        assert!(
            !entry.reason.trim().is_empty(),
            "{} baseline entry must include a reason",
            entry.module_name
        );
    }
}
