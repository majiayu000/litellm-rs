use super::model_catalog_authority::{
    CatalogAuthority, CatalogClassification, CatalogDecision, CatalogEndpoint, CatalogResolution,
};
use crate::core::types::model::ProviderCapability;
use sha2::{Digest, Sha256};

fn with_valid_classification_digest(mut document: serde_json::Value) -> String {
    let metadata = document["_metadata"].clone();
    let semantic = serde_json::json!({
        "schema_version": metadata["schema_version"],
        "revision": metadata["revision"],
        "enforced_providers": metadata["enforced_providers"],
        "provider_aliases": document["provider_aliases"],
        "entries": document["entries"],
    });
    let canonical = serde_json::to_vec(&semantic).expect("test semantic JSON must serialize");
    document["_metadata"]["classification_sha256"] =
        serde_json::json!(format!("{:x}", Sha256::digest(canonical)));
    document.to_string()
}

fn authority_json() -> String {
    with_valid_classification_digest(serde_json::json!({
        "_metadata": {
            "schema_version": 1,
            "revision": "test-ledger-1",
            "decision_source_sha256": "a".repeat(64),
            "pricing_universe_sha256": "b".repeat(64),
            "classification_sha256": "c".repeat(64),
            "total_entry_count": 5,
            "enforced_providers": ["azure", "azure_ai", "openai"],
            "provider_coverage": {
                "openai": {"callable": 1, "pricing_only": 1, "unreviewed": 0},
                "other": {"callable": 0, "pricing_only": 0, "unreviewed": 1},
                "together_ai": {"callable": 2, "pricing_only": 0, "unreviewed": 0}
            }
        },
        "provider_aliases": {
            "openai": [],
            "other": [],
            "together_ai": ["together"]
        },
        "entries": [
            {
                "provider": "openai",
                "pricing_key": "gpt-test",
                "decision": "callable",
                "evidence_sources": ["review"],
                "catalog_model_id": "gpt-test",
                "endpoints": ["chat_completions"],
                "capabilities": ["chat_completion", "tool_calling"],
                "supported_parameters": ["messages", "tools"],
                "aliases": ["gpt-test-latest"]
            },
            {
                "provider": "openai",
                "pricing_key": "openai/container",
                "decision": "pricing_only",
                "evidence_sources": ["review"],
                "reason": "tool_or_session_charge"
            },
            {
                "provider": "other",
                "pricing_key": "other/model",
                "decision": "unreviewed",
                "evidence_sources": ["review"]
            },
            {
                "provider": "together_ai",
                "pricing_key": "together_ai/BAAI/bge-base-en-v1.5",
                "decision": "callable",
                "evidence_sources": ["review"],
                "catalog_model_id": "BAAI/bge-base-en-v1.5",
                "endpoints": ["embeddings"],
                "capabilities": ["embeddings"],
                "supported_parameters": ["input"],
                "aliases": []
            },
            {
                "provider": "together_ai",
                "pricing_key": "together_ai/baai/bge-base-en-v1.5",
                "decision": "callable",
                "evidence_sources": ["review"],
                "catalog_model_id": "baai/bge-base-en-v1.5",
                "aliases": []
            }
        ]
    }))
}

fn cross_class_authority_json(
    target_provider: &str,
    target_key: &str,
    target_decision: &str,
    catalog_model_id: &str,
    aliases: &[&str],
) -> String {
    let target_entry = match target_decision {
        "callable" => serde_json::json!({
            "provider": target_provider,
            "pricing_key": target_key,
            "decision": "callable",
            "evidence_sources": ["review"],
            "catalog_model_id": "target-model",
            "aliases": []
        }),
        "pricing_only" => serde_json::json!({
            "provider": target_provider,
            "pricing_key": target_key,
            "decision": "pricing_only",
            "evidence_sources": ["review"],
            "reason": "non_callable_charge"
        }),
        "unreviewed" => serde_json::json!({
            "provider": target_provider,
            "pricing_key": target_key,
            "decision": "unreviewed",
            "evidence_sources": ["review"]
        }),
        other => panic!("unsupported test decision {other}"),
    };
    let target_counts = match target_decision {
        "callable" => serde_json::json!({"callable": 1, "pricing_only": 0, "unreviewed": 0}),
        "pricing_only" => {
            serde_json::json!({"callable": 0, "pricing_only": 1, "unreviewed": 0})
        }
        "unreviewed" => {
            serde_json::json!({"callable": 0, "pricing_only": 0, "unreviewed": 1})
        }
        _ => unreachable!(),
    };
    let provider_coverage = if target_provider == "openai" {
        let callable = 1 + usize::from(target_decision == "callable");
        serde_json::json!({
            "openai": {
                "callable": callable,
                "pricing_only": usize::from(target_decision == "pricing_only"),
                "unreviewed": usize::from(target_decision == "unreviewed")
            }
        })
    } else {
        serde_json::json!({
            "openai": {"callable": 1, "pricing_only": 0, "unreviewed": 0},
            target_provider: target_counts
        })
    };
    with_valid_classification_digest(serde_json::json!({
        "_metadata": {
            "schema_version": 1,
            "revision": "test-ledger-1",
            "decision_source_sha256": "a".repeat(64),
            "pricing_universe_sha256": "b".repeat(64),
            "classification_sha256": "c".repeat(64),
            "total_entry_count": 2,
            "enforced_providers": ["azure", "azure_ai", "openai"],
            "provider_coverage": provider_coverage
        },
        "provider_aliases": {"openai": [], "other": []},
        "entries": [
            {
                "provider": "openai",
                "pricing_key": "source-price",
                "decision": "callable",
                "evidence_sources": ["review"],
                "catalog_model_id": catalog_model_id,
                "aliases": aliases
            },
            target_entry
        ]
    }))
}

#[test]
fn classification_digest_must_match_the_canonical_entries() {
    let mut document: serde_json::Value =
        serde_json::from_str(&authority_json()).expect("valid test authority");
    document["entries"][0]["catalog_model_id"] = serde_json::json!("tampered-model");

    let error = CatalogAuthority::from_json(&document.to_string())
        .expect_err("entry mutation with the old classification digest must fail");
    assert!(error.to_string().contains("classification_sha256 mismatch"));
}

#[test]
fn empty_entry_provider_and_pricing_identity_fail_closed() {
    let mut empty_provider: serde_json::Value =
        serde_json::from_str(&authority_json()).expect("valid test authority");
    empty_provider["entries"][2]["provider"] = serde_json::json!("");
    let other_coverage = empty_provider["_metadata"]["provider_coverage"]["other"].take();
    empty_provider["_metadata"]["provider_coverage"]
        .as_object_mut()
        .expect("coverage object")
        .remove("other");
    empty_provider["_metadata"]["provider_coverage"][""] = other_coverage;
    empty_provider["provider_aliases"]
        .as_object_mut()
        .expect("provider aliases object")
        .remove("other");
    let error = CatalogAuthority::from_json(&with_valid_classification_digest(empty_provider))
        .expect_err("empty entry provider must fail");
    assert!(error.to_string().contains("entry provider cannot be empty"));

    let mut empty_pricing_key: serde_json::Value =
        serde_json::from_str(&authority_json()).expect("valid test authority");
    empty_pricing_key["entries"][2]["pricing_key"] = serde_json::json!("");
    let error = CatalogAuthority::from_json(&with_valid_classification_digest(empty_pricing_key))
        .expect_err("empty pricing identity must fail");
    assert!(error.to_string().contains("pricing key cannot be empty"));
}

#[test]
fn exact_provider_scoped_resolution_distinguishes_all_three_decisions() {
    let authority = CatalogAuthority::from_json(&authority_json()).expect("valid authority");

    let CatalogResolution::Callable(model) = authority.resolve_model("openai", "gpt-test") else {
        panic!("gpt-test should be callable");
    };
    assert_eq!(model.catalog_model_id(), "gpt-test");
    assert_eq!(
        model.explicit_endpoints(),
        Some(&[CatalogEndpoint::ChatCompletions][..])
    );
    assert!(
        model
            .explicit_capabilities()
            .expect("explicit capabilities")
            .contains(&ProviderCapability::ToolCalling)
    );
    assert_eq!(
        model.explicit_supported_parameters(),
        Some(&["messages".to_string(), "tools".to_string()][..])
    );
    assert_eq!(
        authority.classification("openai", "gpt-test"),
        CatalogClassification::Callable
    );

    assert_eq!(
        authority.resolve_model("openai", "openai/container"),
        CatalogResolution::PricingOnly
    );
    assert_eq!(
        authority.resolve_model("other", "other/model"),
        CatalogResolution::Unreviewed
    );
    assert_eq!(
        authority.resolve_model("openai", "fake-gpt-test-2026-08-28"),
        CatalogResolution::Unknown
    );
}

#[test]
fn provider_qualification_is_bounded_and_native_slash_is_lossless() {
    let authority = CatalogAuthority::from_json(&authority_json()).expect("valid authority");

    for model in [
        "BAAI/bge-base-en-v1.5",
        "together_ai/BAAI/bge-base-en-v1.5",
        "together/BAAI/bge-base-en-v1.5",
    ] {
        let CatalogResolution::Callable(resolved) = authority.resolve_model("together_ai", model)
        else {
            panic!("{model} should resolve exactly");
        };
        assert_eq!(resolved.catalog_model_id(), "BAAI/bge-base-en-v1.5");
    }

    assert_eq!(
        authority.resolve_model("together_ai", "openai/BAAI/bge-base-en-v1.5"),
        CatalogResolution::Unknown
    );
    assert_eq!(
        authority.resolve_model("together_ai", "together/together_ai/BAAI/bge-base-en-v1.5"),
        CatalogResolution::Unknown
    );
}

#[test]
fn model_ids_are_case_sensitive_without_lowercase_collision_loss() {
    let authority = CatalogAuthority::from_json(&authority_json()).expect("valid authority");

    let CatalogResolution::Callable(upper) =
        authority.resolve_model("together_ai", "BAAI/bge-base-en-v1.5")
    else {
        panic!("upper-case native ID should resolve");
    };
    let CatalogResolution::Callable(lower) =
        authority.resolve_model("together_ai", "baai/bge-base-en-v1.5")
    else {
        panic!("lower-case native ID should resolve");
    };
    assert_ne!(upper.catalog_model_id(), lower.catalog_model_id());
    assert_eq!(
        authority.explicit_capabilities("together_ai", "baai/bge-base-en-v1.5"),
        None
    );
    assert_eq!(
        authority.resolve_model("together_ai", "BaAi/bge-base-en-v1.5"),
        CatalogResolution::Unknown
    );
}

#[test]
fn unreviewed_rows_remain_exactly_price_addressable_but_have_no_capabilities() {
    let authority = CatalogAuthority::from_json(&authority_json()).expect("valid authority");

    assert_eq!(
        authority.decision_for_pricing_key("other", "other/model"),
        Some(CatalogDecision::Unreviewed)
    );
    assert_eq!(authority.decision_for_pricing_key("other", "model"), None);
    assert_eq!(
        authority.resolve_model("other", "other/model"),
        CatalogResolution::Unreviewed
    );
}

#[test]
fn strict_schema_and_exact_collisions_fail_closed() {
    let with_unknown = authority_json().replace(
        "\"revision\":\"test-ledger-1\"",
        "\"revision\":\"test-ledger-1\",\"future\":true",
    );
    assert!(CatalogAuthority::from_json(&with_unknown).is_err());

    let mut duplicate: serde_json::Value =
        serde_json::from_str(&authority_json()).expect("test fixture must parse");
    duplicate["entries"][1]["pricing_key"] = serde_json::json!("gpt-test");
    let error = CatalogAuthority::from_json(&with_valid_classification_digest(duplicate))
        .expect_err("duplicate must fail");
    assert!(
        error
            .to_string()
            .contains("duplicate pricing classification")
    );

    let mut missing_aliases: serde_json::Value =
        serde_json::from_str(&authority_json()).expect("test fixture must parse");
    missing_aliases["entries"][0]
        .as_object_mut()
        .expect("callable entry must be an object")
        .remove("aliases");
    assert!(CatalogAuthority::from_json(&missing_aliases.to_string()).is_err());

    let self_alias = cross_class_authority_json(
        "other",
        "restricted",
        "unreviewed",
        "catalog-only",
        &["source-price"],
    );
    let error = CatalogAuthority::from_json(&self_alias).expect_err("self alias must fail");
    assert!(error.to_string().contains("own exact pricing row"));
}

#[test]
fn callable_identities_cannot_upgrade_non_callable_exact_ledger_keys() {
    for target_decision in ["callable", "pricing_only", "unreviewed"] {
        for target_key in ["restricted", "BAAI/restricted-model"] {
            for alias_collision in [false, true] {
                let catalog_model_id = if alias_collision {
                    "source-model"
                } else {
                    target_key
                };
                let aliases = if alias_collision {
                    vec![target_key]
                } else {
                    Vec::new()
                };
                let content = cross_class_authority_json(
                    "openai",
                    target_key,
                    target_decision,
                    catalog_model_id,
                    &aliases,
                );
                let error = CatalogAuthority::from_json(&content)
                    .expect_err("a different exact pricing key must not be shadowed");
                assert!(
                    error.to_string().contains("different pricing row"),
                    "{target_decision}/{target_key}/{alias_collision}: {error}"
                );
            }
        }
    }
}

#[test]
fn callable_identity_ledger_checks_are_provider_and_case_sensitive() {
    for content in [
        cross_class_authority_json("other", "restricted", "unreviewed", "restricted", &[]),
        cross_class_authority_json("openai", "Restricted", "unreviewed", "restricted", &[]),
    ] {
        CatalogAuthority::from_json(&content).expect("exact non-conflicting identity must load");
    }

    let content = cross_class_authority_json(
        "other",
        "restricted",
        "unreviewed",
        "catalog-only",
        &["friendly-alias"],
    );
    let authority = CatalogAuthority::from_json(&content).expect("valid non-key identities");
    for identity in ["source-price", "catalog-only", "friendly-alias"] {
        let CatalogResolution::Callable(model) = authority.resolve_model("openai", identity) else {
            panic!("{identity} must resolve to its sole owner");
        };
        assert_eq!(model.pricing_key(), "source-price");
    }
}

#[test]
fn raw_pricing_identity_precedes_catalog_alias_index_defense_in_depth() {
    let content =
        cross_class_authority_json("openai", "callable-target", "callable", "source-model", &[]);
    let mut authority = CatalogAuthority::from_json(&content).expect("valid callable rows");
    assert!(authority.inject_catalog_shadow_for_test("openai", "callable-target", "source-price"));

    let CatalogResolution::Callable(model) = authority.resolve_model("openai", "callable-target")
    else {
        panic!("raw callable pricing identity must resolve");
    };
    assert_eq!(model.pricing_key(), "callable-target");
}

#[test]
fn embedded_authority_enforces_reviewed_and_unreviewed_rows_without_inference() {
    let authority = CatalogAuthority::from_embedded().expect("embedded authority must validate");

    assert_eq!(
        authority.classification("openai", "gpt-4o-2024-05-13"),
        CatalogClassification::Callable
    );
    assert_eq!(
        authority.classification("openai", "chatgpt-4o-latest"),
        CatalogClassification::PricingOnly
    );
    assert_eq!(
        authority.classification("azure_ai", "azure_ai/FW-DeepSeek-V4-Pro"),
        CatalogClassification::Unreviewed
    );
    assert_eq!(
        authority.classification("azure_ai", "FW-DeepSeek-V4-Pro"),
        CatalogClassification::Unknown,
        "an unreviewed pricing key must not invent a catalog model identity"
    );
    assert_eq!(
        authority.explicit_capabilities("openai", "gpt-4o-2024-05-13"),
        None,
        "Callable does not imply endpoint or capability metadata"
    );
    assert_eq!(
        authority.resolve_model("azure_ai", "azure_ai/Cohere-embed-v3-english"),
        authority.resolve_model("azure-ai", "Cohere-embed-v3-english")
    );
}
