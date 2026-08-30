use super::{
    PROVIDER_CATALOG, PROVIDER_MODULE_LIFECYCLE, PROVIDER_TYPE_REGISTRY, ProviderDispatchKind,
    ProviderModuleLifecycle, ProviderRegistryEntry, entry_for_name,
};
use crate::core::providers::provider_type::ProviderType;
use std::collections::BTreeSet;

#[derive(Debug)]
struct ReadmeTier2Row {
    selector: String,
    feature_cell: String,
    capability_cells: [String; 5],
    row: String,
}

struct ExpectedReadmeTier2Row {
    feature_cell: &'static str,
    capability_cells: [&'static str; 5],
}

fn readme_provider_support_section() -> &'static str {
    section_between(
        include_str!("../../../../README.md"),
        "## Provider Support",
        "## Environment Variables",
    )
}

fn section_between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = text
        .find(start)
        .unwrap_or_else(|| panic!("missing README section start: {start}"));
    let after_start = &text[start_index + start.len()..];
    let end_index = after_start
        .find(end)
        .unwrap_or_else(|| panic!("missing README section end: {end}"));
    &after_start[..end_index]
}

fn code_spans(line: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = line;

    while let Some(start) = rest.find('`') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        spans.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }

    spans
}

fn markdown_table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn readme_tier2_rows() -> Vec<ReadmeTier2Row> {
    let tier2 = section_between(
        readme_provider_support_section(),
        "### Tier 2",
        "### Tier 1",
    );

    tier2
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with('|')
                && !trimmed.starts_with("| Provider")
                && !trimmed.starts_with("|---")
                && !trimmed.starts_with("|----------")
        })
        .map(|line| {
            let cells = markdown_table_cells(line);
            let provider_cell = cells.first().map(String::as_str).unwrap_or("");
            let feature_cell = cells
                .get(1)
                .unwrap_or_else(|| panic!("README Tier 2 row is missing feature cell: {line}"))
                .clone();
            let capability_cells = [
                readme_table_cell(&cells, 2, line, "Chat"),
                readme_table_cell(&cells, 3, line, "Stream"),
                readme_table_cell(&cells, 4, line, "Embed"),
                readme_table_cell(&cells, 5, line, "Image"),
                readme_table_cell(&cells, 6, line, "Audio"),
            ];
            let selector = code_spans(provider_cell)
                .into_iter()
                .next()
                .unwrap_or_else(|| {
                    panic!("README Tier 2 row is missing selector in first column: {line}")
                });
            ReadmeTier2Row {
                selector,
                feature_cell,
                capability_cells,
                row: line.to_string(),
            }
        })
        .collect()
}

fn readme_table_cell(cells: &[String], index: usize, line: &str, name: &str) -> String {
    cells
        .get(index)
        .unwrap_or_else(|| panic!("README Tier 2 row is missing {name} cell: {line}"))
        .clone()
}

fn readme_code_list_selectors(section_start: &str, section_end: &str) -> BTreeSet<String> {
    let section = section_between(
        readme_provider_support_section(),
        section_start,
        section_end,
    );

    section
        .lines()
        .filter(|line| line.trim_start().starts_with('`'))
        .flat_map(code_spans)
        .collect()
}

fn expected_readme_tier2_row(entry: &ProviderRegistryEntry) -> Option<ExpectedReadmeTier2Row> {
    match entry.provider_type {
        ProviderType::OpenAI => Some(expected("always", ["✅", "✅", "✅", "✅", "✅"])),
        ProviderType::Anthropic => Some(expected("always", ["✅", "✅", "–", "–", "–"])),
        ProviderType::Mistral => Some(expected("always", ["✅", "✅", "passthrough", "–", "–"])),
        ProviderType::Cloudflare => Some(expected("always", ["✅", "–", "–", "–", "–"])),
        ProviderType::Bedrock => Some(expected("always", ["✅", "✅", "✅", "helper API", "–"])),
        ProviderType::Databricks | ProviderType::Snowflake => {
            Some(expected("always", ["✅", "✅", "–", "–", "–"]))
        }
        ProviderType::Oci => Some(expected("always", ["✅", "✅", "✅", "–", "–"])),
        ProviderType::Watsonx => Some(expected("always", ["✅", "–", "✅", "–", "–"])),
        ProviderType::SageMaker => Some(expected("always", ["✅", "–", "–", "–", "–"])),
        ProviderType::OpenAICompatible => Some(expected("always", ["✅", "✅", "–", "–", "–"])),
        ProviderType::Azure | ProviderType::AzureAI => Some(expected(
            "native factory (`providers-extra`); OpenAILike fallback",
            ["✅", "✅", "✅", "✅", "–"],
        )),
        ProviderType::VertexAI => Some(expected(
            "native factory (`providers-extra`)",
            ["✅", "✅", "✅", "✅", "–"],
        )),
        ProviderType::Cohere => Some(expected(
            "native factory (`providers-extended`)",
            ["✅", "✅", "✅", "–", "–"],
        )),
        ProviderType::Gemini | ProviderType::GitHubCopilot => Some(expected(
            "native factory (`providers-extended`)",
            ["✅", "✅", "–", "–", "–"],
        )),
        ProviderType::FalAI => Some(expected(
            "native factory (`providers-extended`)",
            ["–", "–", "–", "✅", "–"],
        )),
        ProviderType::Replicate => Some(expected(
            "native factory (`providers-extended`)",
            ["✅", "✅", "–", "✅", "–"],
        )),
        ProviderType::Ollama => Some(expected(
            "native factory (`providers-extended`)",
            ["✅", "✅", "✅", "–", "–"],
        )),
        ProviderType::MetaLlama
        | ProviderType::V0
        | ProviderType::AmazonNova
        | ProviderType::GitHub => Some(expected(
            "catalog-only (`OpenAILike`)",
            ["✅", "✅", "–", "–", "–"],
        )),
        _ => None,
    }
}

fn expected(
    feature_cell: &'static str,
    capability_cells: [&'static str; 5],
) -> ExpectedReadmeTier2Row {
    ExpectedReadmeTier2Row {
        feature_cell,
        capability_cells,
    }
}

fn expected_readme_tier2_selectors() -> BTreeSet<String> {
    PROVIDER_TYPE_REGISTRY
        .iter()
        .filter(|entry| expected_readme_tier2_row(entry).is_some())
        .map(|entry| entry.canonical_name.to_string())
        .collect()
}

fn assert_readme_row_matches_dispatch_kind(row: &ReadmeTier2Row, entry: &ProviderRegistryEntry) {
    let expected_row = expected_readme_tier2_row(entry).unwrap_or_else(|| {
        panic!(
            "README Tier 2 matrix should not document unsupported provider {}",
            entry.canonical_name
        )
    });
    assert_eq!(
        row.feature_cell, expected_row.feature_cell,
        "README Tier 2 row for {} should document the exact registry feature gate: {}",
        row.selector, row.row
    );
    assert_eq!(
        row.capability_cells, expected_row.capability_cells,
        "README Tier 2 row for {} should document exact Chat/Stream/Embed/Image/Audio capability cells: {}",
        row.selector, row.row
    );

    match entry.dispatch_kind {
        ProviderDispatchKind::Native => assert!(
            row.feature_cell.contains("native factory") || row.feature_cell == "always",
            "README Tier 2 row for {} should document native dispatch: {}",
            row.selector,
            row.row
        ),
        ProviderDispatchKind::ExplicitOpenAiLike => assert!(
            row.feature_cell == "always" || row.feature_cell.contains("OpenAILike fallback"),
            "README Tier 2 row for {} should document OpenAILike dispatch: {}",
            row.selector,
            row.row
        ),
        ProviderDispatchKind::CatalogOpenAiLike => assert!(
            row.feature_cell.contains("catalog-only"),
            "README Tier 2 row for {} should document catalog-only dispatch: {}",
            row.selector,
            row.row
        ),
        ProviderDispatchKind::UnsupportedEnum => assert!(
            row.feature_cell.contains("providers-extra")
                || row.feature_cell.contains("providers-extended"),
            "README Tier 2 row for {} should document the feature gate that makes it constructible: {}",
            row.selector,
            row.row
        ),
    }
}

#[test]
fn provider_registry_readme_provider_support_matrix_matches_registry_and_catalog() {
    let tier2_rows = readme_tier2_rows();
    let tier1_selectors =
        readme_code_list_selectors("### Tier 1", "### Experimental / module-only");
    let experimental_selectors = readme_code_list_selectors(
        "### Experimental / module-only",
        "For self-hosted or unlisted OpenAI-compatible endpoints",
    );
    let expected_tier2_selectors = expected_readme_tier2_selectors();
    let mut documented_selectors = tier1_selectors.clone();

    assert!(
        !tier2_rows.is_empty(),
        "README Tier 2 matrix must not be empty"
    );
    assert!(
        !tier1_selectors.is_empty(),
        "README Tier 1 catalog list must not be empty"
    );
    assert!(
        !experimental_selectors.is_empty(),
        "README experimental provider list must not be empty"
    );

    let tier2_selectors = tier2_rows
        .iter()
        .map(|row| row.selector.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        tier2_selectors, expected_tier2_selectors,
        "README Tier 2 rows must document every expected provider selector independent of active cargo features"
    );

    for row in &tier2_rows {
        let entry = entry_for_name(&row.selector).unwrap_or_else(|| {
            panic!(
                "README Tier 2 selector {} must exist in the provider registry",
                row.selector
            )
        });
        assert_readme_row_matches_dispatch_kind(row, entry);
        documented_selectors.insert(row.selector.clone());
    }

    for selector in &tier1_selectors {
        assert!(
            PROVIDER_CATALOG.contains_key(selector.as_str()),
            "README Tier 1 selector {selector} must exist in the Tier 1 catalog"
        );
    }

    for selector in PROVIDER_CATALOG.keys() {
        assert!(
            documented_selectors.contains(*selector),
            "catalog selector {selector} must be documented in README provider support"
        );
    }

    for entry in PROVIDER_TYPE_REGISTRY {
        if entry.is_dispatchable() {
            assert!(
                documented_selectors.contains(entry.canonical_name),
                "dispatchable registry selector {} must be documented in README provider support",
                entry.canonical_name
            );
        }
    }

    for selector in experimental_selectors {
        let lifecycle_entry = PROVIDER_MODULE_LIFECYCLE
            .iter()
            .find(|entry| entry.module_name == selector)
            .unwrap_or_else(|| {
                panic!("experimental selector {selector} must exist in provider module lifecycle")
            });
        assert!(
            !PROVIDER_CATALOG.contains_key(selector.as_str()),
            "experimental selector {selector} must not be a Tier 1 catalog entry"
        );
        assert!(
            !expected_tier2_selectors.contains(&selector),
            "experimental selector {selector} must not be a Tier 2 provider support row"
        );
        assert!(
            entry_for_name(&selector).is_none_or(|entry| !entry.is_dispatchable()),
            "experimental selector {selector} must not be dispatchable under active features"
        );
        assert!(
            matches!(
                lifecycle_entry.lifecycle,
                ProviderModuleLifecycle::Stub | ProviderModuleLifecycle::Internal
            ),
            "experimental selector {selector} must be a retained stub/internal module, got {:?}",
            lifecycle_entry.lifecycle
        );
    }
}
