//! Strict, provider-scoped catalog classification authority.

use crate::core::types::model::ProviderCapability;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

const CATALOG_SCHEMA_VERSION: u32 = 1;
const ENFORCED_PROVIDERS: &[&str] = &["azure", "azure_ai", "openai"];
const EMBEDDED_CATALOG_AUTHORITY: &str =
    include_str!("../../../../config/model_catalog_authority.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogDecision {
    Callable,
    PricingOnly,
    Unreviewed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogEndpoint {
    ChatCompletions,
    Responses,
    Embeddings,
    ImageGeneration,
    ImageEdit,
    ImageVariation,
    AudioTranscription,
    AudioTranslation,
    TextToSpeech,
    Moderation,
    Rerank,
    Realtime,
    VideoGeneration,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CallableCatalogModel {
    provider: String,
    pricing_key: String,
    catalog_model_id: String,
    endpoints: Option<Vec<CatalogEndpoint>>,
    capabilities: Option<Vec<ProviderCapability>>,
    supported_parameters: Option<Vec<String>>,
    evidence_sources: Vec<String>,
}

impl CallableCatalogModel {
    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn pricing_key(&self) -> &str {
        &self.pricing_key
    }

    pub fn catalog_model_id(&self) -> &str {
        &self.catalog_model_id
    }

    pub fn explicit_endpoints(&self) -> Option<&[CatalogEndpoint]> {
        self.endpoints.as_deref()
    }

    pub fn explicit_capabilities(&self) -> Option<&[ProviderCapability]> {
        self.capabilities.as_deref()
    }

    pub fn explicit_supported_parameters(&self) -> Option<&[String]> {
        self.supported_parameters.as_deref()
    }

    pub fn evidence_sources(&self) -> &[String] {
        &self.evidence_sources
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogResolution<'a> {
    Callable(&'a CallableCatalogModel),
    PricingOnly,
    Unreviewed,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogClassification {
    Callable,
    PricingOnly,
    Unreviewed,
    Unknown,
}

#[derive(Debug, thiserror::Error)]
pub enum CatalogAuthorityError {
    #[error("invalid catalog authority JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid catalog authority: {0}")]
    Invalid(String),
}

#[derive(Debug)]
pub struct CatalogAuthority {
    provider_aliases: HashMap<String, String>,
    canonical_providers: HashSet<String>,
    callable_models: Vec<CallableCatalogModel>,
    catalog_by_provider: HashMap<String, HashMap<String, usize>>,
    pricing_by_provider: HashMap<String, HashMap<String, PricingLookup>>,
}

#[derive(Debug, Clone, Copy)]
struct PricingLookup {
    decision: CatalogDecision,
    callable_index: Option<usize>,
}

impl CatalogAuthority {
    pub fn from_embedded() -> Result<Self, CatalogAuthorityError> {
        Self::from_json(EMBEDDED_CATALOG_AUTHORITY)
    }

    pub fn from_json(content: &str) -> Result<Self, CatalogAuthorityError> {
        let document: AuthorityDocument = serde_json::from_str(content)?;
        Self::from_document(document)
    }

    fn from_document(document: AuthorityDocument) -> Result<Self, CatalogAuthorityError> {
        if document.metadata.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema version {} is not supported",
                document.metadata.schema_version
            )));
        }
        if document.metadata.revision.is_empty() {
            return Err(invalid("metadata revision cannot be empty"));
        }
        validate_digest(
            "decision_source_sha256",
            &document.metadata.decision_source_sha256,
        )?;
        validate_digest(
            "pricing_universe_sha256",
            &document.metadata.pricing_universe_sha256,
        )?;
        validate_digest(
            "classification_sha256",
            &document.metadata.classification_sha256,
        )?;
        if document.metadata.total_entry_count != document.entries.len() {
            return Err(invalid(format!(
                "metadata total_entry_count {} does not match {} entries",
                document.metadata.total_entry_count,
                document.entries.len()
            )));
        }
        if document.metadata.enforced_providers != ENFORCED_PROVIDERS {
            return Err(invalid(
                "metadata enforced_providers does not match the phase-1 provider set",
            ));
        }
        validate_callable_ledger_collisions(&document.entries)?;

        let mut canonical_providers: HashSet<String> =
            document.provider_aliases.keys().cloned().collect();
        canonical_providers.extend(
            document
                .entries
                .iter()
                .map(|entry| entry.provider().to_owned()),
        );
        let provider_aliases =
            build_provider_aliases(&document.provider_aliases, &canonical_providers)?;

        let mut authority = Self {
            provider_aliases,
            canonical_providers,
            callable_models: Vec::new(),
            catalog_by_provider: HashMap::new(),
            pricing_by_provider: HashMap::new(),
        };
        let mut observed_coverage: HashMap<String, ProviderCoverage> = HashMap::new();

        for entry in document.entries {
            let provider = entry.provider().to_owned();
            let pricing_key = entry.pricing_key().to_owned();
            let decision = entry.decision();
            if authority
                .pricing_by_provider
                .get(&provider)
                .is_some_and(|prices| prices.contains_key(&pricing_key))
            {
                return Err(invalid(format!(
                    "duplicate pricing classification for {provider:?}/{pricing_key:?}"
                )));
            }
            observed_coverage
                .entry(provider.clone())
                .or_default()
                .increment(decision);

            let callable_index = match entry {
                AuthorityEntry::Callable {
                    evidence_sources,
                    catalog_model_id,
                    endpoints,
                    capabilities,
                    supported_parameters,
                    aliases,
                    ..
                } => {
                    if evidence_sources.is_empty()
                        || endpoints.as_ref().is_some_and(Vec::is_empty)
                        || capabilities.as_ref().is_some_and(Vec::is_empty)
                        || supported_parameters.as_ref().is_some_and(Vec::is_empty)
                    {
                        return Err(invalid(format!(
                            "callable {provider:?}/{pricing_key:?} has an empty contract"
                        )));
                    }
                    let index = authority.callable_models.len();
                    authority.callable_models.push(CallableCatalogModel {
                        provider: provider.clone(),
                        pricing_key: pricing_key.clone(),
                        catalog_model_id: catalog_model_id.clone(),
                        endpoints,
                        capabilities,
                        supported_parameters,
                        evidence_sources,
                    });
                    authority.insert_catalog_identity(&provider, &catalog_model_id, index)?;
                    for alias in aliases {
                        authority.insert_catalog_identity(&provider, &alias, index)?;
                    }
                    Some(index)
                }
                AuthorityEntry::PricingOnly {
                    evidence_sources,
                    reason,
                    ..
                } => {
                    if evidence_sources.is_empty() || reason.is_empty() {
                        return Err(invalid(format!(
                            "pricing_only {provider:?}/{pricing_key:?} has an empty contract"
                        )));
                    }
                    None
                }
                AuthorityEntry::Unreviewed {
                    evidence_sources, ..
                } => {
                    if evidence_sources.is_empty() {
                        return Err(invalid(format!(
                            "unreviewed {provider:?}/{pricing_key:?} lacks evidence"
                        )));
                    }
                    None
                }
            };
            authority
                .pricing_by_provider
                .entry(provider)
                .or_default()
                .insert(
                    pricing_key,
                    PricingLookup {
                        decision,
                        callable_index,
                    },
                );
        }

        if observed_coverage != document.metadata.provider_coverage {
            return Err(invalid("provider coverage metadata does not match entries"));
        }
        Ok(authority)
    }

    fn insert_catalog_identity(
        &mut self,
        provider: &str,
        model: &str,
        index: usize,
    ) -> Result<(), CatalogAuthorityError> {
        if model.is_empty() {
            return Err(invalid("callable catalog identity cannot be empty"));
        }
        let models = self
            .catalog_by_provider
            .entry(provider.to_owned())
            .or_default();
        if let Some(previous) = models.insert(model.to_owned(), index) {
            let previous_model = &self.callable_models[previous];
            let current_model = &self.callable_models[index];
            return Err(invalid(format!(
                "catalog identity collision for {provider:?}/{model:?}: {:?} and {:?}",
                previous_model.pricing_key, current_model.pricing_key
            )));
        }
        Ok(())
    }

    pub fn resolve_model<'a>(&'a self, provider: &str, model: &str) -> CatalogResolution<'a> {
        let Some(provider) = self.canonical_provider(provider) else {
            return CatalogResolution::Unknown;
        };
        if let Some(resolution) = self.resolve_exact(provider, model) {
            return resolution;
        }
        let Some((qualifier, remainder)) = model.split_once('/') else {
            return CatalogResolution::Unknown;
        };
        if self.canonical_provider(qualifier) != Some(provider) {
            return CatalogResolution::Unknown;
        }
        if remainder
            .split_once('/')
            .is_some_and(|(nested, _)| self.canonical_provider(nested).is_some())
        {
            return CatalogResolution::Unknown;
        }
        self.resolve_exact(provider, remainder)
            .unwrap_or(CatalogResolution::Unknown)
    }

    pub fn decision_for_pricing_key(
        &self,
        provider: &str,
        pricing_key: &str,
    ) -> Option<CatalogDecision> {
        let provider = self.canonical_provider(provider)?;
        self.pricing_by_provider
            .get(provider)?
            .get(pricing_key)
            .map(|lookup| lookup.decision)
    }

    pub fn classification(&self, provider: &str, model: &str) -> CatalogClassification {
        match self.resolve_model(provider, model) {
            CatalogResolution::Callable(_) => CatalogClassification::Callable,
            CatalogResolution::PricingOnly => CatalogClassification::PricingOnly,
            CatalogResolution::Unreviewed => CatalogClassification::Unreviewed,
            CatalogResolution::Unknown => CatalogClassification::Unknown,
        }
    }

    pub fn explicit_capabilities(
        &self,
        provider: &str,
        model: &str,
    ) -> Option<&[ProviderCapability]> {
        match self.resolve_model(provider, model) {
            CatalogResolution::Callable(model) => model.explicit_capabilities(),
            CatalogResolution::PricingOnly
            | CatalogResolution::Unreviewed
            | CatalogResolution::Unknown => None,
        }
    }

    fn canonical_provider<'a>(&'a self, provider: &'a str) -> Option<&'a str> {
        if self.canonical_providers.contains(provider) {
            Some(provider)
        } else {
            self.provider_aliases.get(provider).map(String::as_str)
        }
    }

    fn resolve_exact<'a>(&'a self, provider: &str, model: &str) -> Option<CatalogResolution<'a>> {
        if let Some(index) = self
            .catalog_by_provider
            .get(provider)
            .and_then(|models| models.get(model))
        {
            return Some(CatalogResolution::Callable(&self.callable_models[*index]));
        }
        let lookup = self
            .pricing_by_provider
            .get(provider)
            .and_then(|models| models.get(model))?;
        Some(match (lookup.decision, lookup.callable_index) {
            (CatalogDecision::Callable, Some(index)) => {
                CatalogResolution::Callable(&self.callable_models[index])
            }
            (CatalogDecision::PricingOnly, _) => CatalogResolution::PricingOnly,
            (CatalogDecision::Unreviewed, _) => CatalogResolution::Unreviewed,
            (CatalogDecision::Callable, None) => CatalogResolution::Unknown,
        })
    }
}

fn build_provider_aliases(
    aliases_by_provider: &HashMap<String, Vec<String>>,
    canonical_providers: &HashSet<String>,
) -> Result<HashMap<String, String>, CatalogAuthorityError> {
    let mut aliases = HashMap::new();
    for (provider, provider_aliases) in aliases_by_provider {
        if provider.is_empty() {
            return Err(invalid("canonical provider cannot be empty"));
        }
        for alias in provider_aliases {
            if alias.is_empty() || canonical_providers.contains(alias) {
                return Err(invalid(format!(
                    "provider alias {alias:?} collides with a canonical provider"
                )));
            }
            if let Some(previous) = aliases.insert(alias.clone(), provider.clone())
                && previous != *provider
            {
                return Err(invalid(format!(
                    "provider alias {alias:?} belongs to {previous:?} and {provider:?}"
                )));
            }
        }
    }
    Ok(aliases)
}

fn validate_callable_ledger_collisions(
    entries: &[AuthorityEntry],
) -> Result<(), CatalogAuthorityError> {
    let mut ledger = HashMap::new();
    for entry in entries {
        let identity = (entry.provider(), entry.pricing_key());
        if ledger.insert(identity, entry.decision()).is_some() {
            return Err(invalid(format!(
                "duplicate pricing classification for {:?}/{:?}",
                identity.0, identity.1
            )));
        }
    }
    for entry in entries {
        let AuthorityEntry::Callable {
            provider,
            catalog_model_id,
            aliases,
            ..
        } = entry
        else {
            continue;
        };
        for candidate in std::iter::once(catalog_model_id).chain(aliases) {
            if ledger
                .get(&(provider.as_str(), candidate.as_str()))
                .is_some_and(|decision| *decision != CatalogDecision::Callable)
            {
                return Err(invalid(format!(
                    "callable identity {provider:?}/{candidate:?} collides with a non-callable pricing key"
                )));
            }
        }
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &str) -> Result<(), CatalogAuthorityError> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> CatalogAuthorityError {
    CatalogAuthorityError::Invalid(message.into())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityDocument {
    #[serde(rename = "_metadata")]
    metadata: AuthorityMetadata,
    provider_aliases: HashMap<String, Vec<String>>,
    entries: Vec<AuthorityEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorityMetadata {
    schema_version: u32,
    revision: String,
    decision_source_sha256: String,
    pricing_universe_sha256: String,
    classification_sha256: String,
    total_entry_count: usize,
    enforced_providers: Vec<String>,
    provider_coverage: HashMap<String, ProviderCoverage>,
}

#[derive(Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ProviderCoverage {
    callable: usize,
    pricing_only: usize,
    unreviewed: usize,
}

impl ProviderCoverage {
    fn increment(&mut self, decision: CatalogDecision) {
        match decision {
            CatalogDecision::Callable => self.callable += 1,
            CatalogDecision::PricingOnly => self.pricing_only += 1,
            CatalogDecision::Unreviewed => self.unreviewed += 1,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
enum AuthorityEntry {
    Callable {
        provider: String,
        pricing_key: String,
        evidence_sources: Vec<String>,
        catalog_model_id: String,
        #[serde(default)]
        endpoints: Option<Vec<CatalogEndpoint>>,
        #[serde(default)]
        capabilities: Option<Vec<ProviderCapability>>,
        #[serde(default)]
        supported_parameters: Option<Vec<String>>,
        aliases: Vec<String>,
    },
    PricingOnly {
        provider: String,
        pricing_key: String,
        evidence_sources: Vec<String>,
        reason: String,
    },
    Unreviewed {
        provider: String,
        pricing_key: String,
        evidence_sources: Vec<String>,
    },
}

impl AuthorityEntry {
    fn provider(&self) -> &str {
        match self {
            Self::Callable { provider, .. }
            | Self::PricingOnly { provider, .. }
            | Self::Unreviewed { provider, .. } => provider,
        }
    }

    fn pricing_key(&self) -> &str {
        match self {
            Self::Callable { pricing_key, .. }
            | Self::PricingOnly { pricing_key, .. }
            | Self::Unreviewed { pricing_key, .. } => pricing_key,
        }
    }

    fn decision(&self) -> CatalogDecision {
        match self {
            Self::Callable { .. } => CatalogDecision::Callable,
            Self::PricingOnly { .. } => CatalogDecision::PricingOnly,
            Self::Unreviewed { .. } => CatalogDecision::Unreviewed,
        }
    }
}
