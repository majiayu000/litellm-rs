use std::collections::{HashMap, HashSet};

use super::error::RouterError;

pub(super) fn normalize_model_aliases(
    model_aliases: &HashMap<String, String>,
    canonical_models: &HashSet<String>,
) -> Result<HashMap<String, String>, RouterError> {
    let mut alias_names = model_aliases.keys().map(String::as_str).collect::<Vec<_>>();
    alias_names.sort_unstable();

    for alias in &alias_names {
        if canonical_models.contains(*alias) {
            return Err(RouterError::InvalidConfiguration(format!(
                "model alias '{alias}' collides with an enabled canonical model"
            )));
        }
    }

    let mut normalized = HashMap::with_capacity(model_aliases.len());
    for alias in alias_names {
        let mut target = &model_aliases[alias];
        while let Some(next) = model_aliases.get(target) {
            target = next;
        }
        if !canonical_models.contains(target) {
            return Err(RouterError::InvalidConfiguration(format!(
                "model alias '{alias}' resolves to unavailable model '{target}'"
            )));
        }
        normalized.insert(alias.to_string(), target.clone());
    }
    Ok(normalized)
}
