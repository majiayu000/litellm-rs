//! Seed entries for the Bedrock unified catalog, split by vendor / family.
//!
//! Every Bedrock model ID currently represented in either
//! [`super::super::model_config`] or [`super::super::utils::cost`] has a single
//! entry across these submodules. Cross-reference invariants in
//! `super::tests` enforce the union.
//!
//! Runtime model configuration and cost lookup are projected from these seeds.

use super::BedrockCatalogEntry;

mod amazon;
mod anthropic;
mod builder;
mod cohere_ai21;
mod generic_converse;
mod media;
mod meta_mistral;

use std::sync::OnceLock;

/// Lazy-initialized list of every catalog entry the seed knows about.
pub fn all_entries() -> &'static [BedrockCatalogEntry] {
    static ENTRIES: OnceLock<Vec<BedrockCatalogEntry>> = OnceLock::new();
    ENTRIES.get_or_init(build_entries)
}

fn build_entries() -> Vec<BedrockCatalogEntry> {
    let mut out: Vec<BedrockCatalogEntry> = Vec::new();
    anthropic::seed(&mut out);
    amazon::seed(&mut out);
    cohere_ai21::seed(&mut out);
    meta_mistral::seed(&mut out);
    generic_converse::seed(&mut out);
    media::seed(&mut out);
    out
}
