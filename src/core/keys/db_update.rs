use super::db_mapping::{
    parse_payload, to_domain_permissions, to_domain_rate_limits, write_payload,
};
use super::types::UpdateKeyConfig;
use crate::core::models::ApiKey;
use crate::utils::error::gateway_error::Result;

pub(crate) fn apply_update_config(domain_key: &mut ApiKey, config: UpdateKeyConfig) -> Result<()> {
    let mut payload = parse_payload(&domain_key.metadata.extra);

    if let Some(name) = config.name {
        domain_key.name = name;
    }
    if let Some(description) = config.description {
        payload.description = description;
    }
    if let Some(permissions) = config.permissions {
        domain_key.permissions = to_domain_permissions(&permissions);
        payload.permissions = Some(permissions);
    }
    if let Some(rate_limits) = config.rate_limits {
        domain_key.rate_limits = to_domain_rate_limits(&rate_limits);
    }
    if let Some(budget_id) = config.budget_id {
        payload.budget_id = budget_id;
    }
    if let Some(expires_at) = config.expires_at {
        domain_key.expires_at = expires_at;
    }
    if let Some(metadata) = config.metadata {
        payload.metadata = metadata;
    }

    domain_key.metadata.touch();
    write_payload(&mut domain_key.metadata.extra, &payload)
}
