use super::types::{KeyPermissions, KeyRateLimits, KeyStatus, KeyUsageStats, ManagedApiKey};
use crate::core::models::{ApiKey, Metadata, RateLimits, UsageStats};
use crate::utils::error::gateway_error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

const CORE_KEYS_EXTRA_NAMESPACE: &str = "__core_keys";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CoreKeysExtraPayload {
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(default)]
    pub(crate) budget_id: Option<Uuid>,
    #[serde(default)]
    pub(crate) permissions: Option<KeyPermissions>,
    #[serde(default)]
    pub(crate) metadata: serde_json::Value,
}

impl Default for CoreKeysExtraPayload {
    fn default() -> Self {
        Self {
            description: None,
            budget_id: None,
            permissions: None,
            metadata: serde_json::Value::Null,
        }
    }
}

pub(crate) fn to_domain_rate_limits(rate_limits: &KeyRateLimits) -> Option<RateLimits> {
    let has_limits = rate_limits.requests_per_minute.is_some()
        || rate_limits.tokens_per_minute.is_some()
        || rate_limits.requests_per_day.is_some()
        || rate_limits.tokens_per_day.is_some()
        || rate_limits.max_concurrent_requests.is_some();

    if !has_limits {
        return None;
    }

    Some(RateLimits {
        rpm: rate_limits.requests_per_minute,
        tpm: rate_limits.tokens_per_minute,
        rpd: rate_limits.requests_per_day,
        tpd: rate_limits.tokens_per_day,
        concurrent: rate_limits.max_concurrent_requests,
    })
}

fn from_domain_rate_limits(rate_limits: Option<&RateLimits>) -> KeyRateLimits {
    match rate_limits {
        Some(limits) => KeyRateLimits {
            requests_per_minute: limits.rpm,
            tokens_per_minute: limits.tpm,
            requests_per_day: limits.rpd,
            tokens_per_day: limits.tpd,
            max_concurrent_requests: limits.concurrent,
        },
        None => KeyRateLimits::default(),
    }
}

pub(crate) fn to_domain_permissions(permissions: &KeyPermissions) -> Vec<String> {
    let mut result = permissions.custom_permissions.clone();
    if permissions.is_admin && !result.iter().any(|p| p == "system.admin") {
        result.push("system.admin".to_string());
    }
    result
}

fn derive_permissions_from_domain(raw_permissions: &[String]) -> KeyPermissions {
    let is_admin = raw_permissions.iter().any(|p| p == "system.admin");
    KeyPermissions {
        allowed_models: Vec::new(),
        allowed_endpoints: Vec::new(),
        max_tokens_per_request: None,
        is_admin,
        custom_permissions: raw_permissions.to_vec(),
    }
}

pub(crate) fn parse_payload(extra: &HashMap<String, serde_json::Value>) -> CoreKeysExtraPayload {
    extra
        .get(CORE_KEYS_EXTRA_NAMESPACE)
        .and_then(|v| serde_json::from_value::<CoreKeysExtraPayload>(v.clone()).ok())
        .unwrap_or_default()
}

pub(crate) fn write_payload(
    extra: &mut HashMap<String, serde_json::Value>,
    payload: &CoreKeysExtraPayload,
) -> Result<()> {
    let value = serde_json::to_value(payload)?;
    extra.insert(CORE_KEYS_EXTRA_NAMESPACE.to_string(), value);
    Ok(())
}

pub(crate) fn to_domain_api_key(managed: &ManagedApiKey) -> Result<ApiKey> {
    let mut extra = HashMap::new();
    let payload = CoreKeysExtraPayload {
        description: managed.description.clone(),
        budget_id: managed.budget_id,
        permissions: Some(managed.permissions.clone()),
        metadata: managed.metadata.clone(),
    };
    write_payload(&mut extra, &payload)?;

    Ok(ApiKey {
        metadata: Metadata {
            id: managed.id,
            created_at: managed.created_at,
            updated_at: managed.updated_at,
            version: 1,
            extra,
        },
        name: managed.name.clone(),
        key_hash: managed.key_hash.clone(),
        key_prefix: managed.key_prefix.clone(),
        user_id: managed.user_id,
        team_id: managed.team_id,
        permissions: to_domain_permissions(&managed.permissions),
        rate_limits: to_domain_rate_limits(&managed.rate_limits),
        expires_at: managed.expires_at,
        is_active: managed.status != KeyStatus::Revoked,
        last_used_at: managed.last_used_at,
        usage_stats: UsageStats {
            total_requests: managed.usage_stats.total_requests,
            total_tokens: managed.usage_stats.total_tokens,
            total_cost: managed.usage_stats.total_cost,
            requests_today: managed.usage_stats.requests_today,
            tokens_today: managed.usage_stats.tokens_today,
            cost_today: managed.usage_stats.cost_today,
            last_reset: managed.usage_stats.last_reset,
        },
    })
}

pub(crate) fn from_domain_api_key(api_key: &ApiKey) -> ManagedApiKey {
    let payload = parse_payload(&api_key.metadata.extra);
    let permissions = payload
        .permissions
        .unwrap_or_else(|| derive_permissions_from_domain(&api_key.permissions));

    ManagedApiKey {
        id: api_key.metadata.id,
        key_hash: api_key.key_hash.clone(),
        key_prefix: api_key.key_prefix.clone(),
        name: api_key.name.clone(),
        description: payload.description,
        user_id: api_key.user_id,
        team_id: api_key.team_id,
        budget_id: payload.budget_id,
        permissions,
        rate_limits: from_domain_rate_limits(api_key.rate_limits.as_ref()),
        status: if !api_key.is_active {
            KeyStatus::Revoked
        } else if let Some(expires_at) = api_key.expires_at {
            if Utc::now() > expires_at {
                KeyStatus::Expired
            } else {
                KeyStatus::Active
            }
        } else {
            KeyStatus::Active
        },
        expires_at: api_key.expires_at,
        created_at: api_key.metadata.created_at,
        updated_at: api_key.metadata.updated_at,
        last_used_at: api_key.last_used_at,
        usage_stats: KeyUsageStats {
            total_requests: api_key.usage_stats.total_requests,
            total_tokens: api_key.usage_stats.total_tokens,
            total_cost: api_key.usage_stats.total_cost,
            requests_today: api_key.usage_stats.requests_today,
            tokens_today: api_key.usage_stats.tokens_today,
            cost_today: api_key.usage_stats.cost_today,
            last_reset: api_key.usage_stats.last_reset,
        },
        metadata: payload.metadata,
    }
}
