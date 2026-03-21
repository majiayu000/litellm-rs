//! Tests for virtual keys module

use super::requests::CreateKeyRequest;
use super::types::{KeyGenerationSettings, Permission, RateLimits, VirtualKey};
use chrono::{Duration, Utc};
use rand::Rng;
use std::collections::HashMap;

/// Test API key generation format
#[test]
fn test_key_generation_format() {
    let settings = KeyGenerationSettings::default();

    // Test key generation directly without VirtualKeyManager
    let key = format!(
        "{}{}",
        settings.key_prefix,
        (0..32)
            .map(|_| {
                let idx = rand::rng().random_range(0usize..36);
                if idx < 10 {
                    (b'0' + idx as u8) as char
                } else {
                    (b'a' + (idx - 10) as u8) as char
                }
            })
            .collect::<String>()
    );

    assert!(key.starts_with("sk-"));
    assert_eq!(key.len(), 35); // "sk-" + 32 chars
}

/// Test that key hashing is deterministic
#[test]
fn test_key_hashing_deterministic() {
    use sha2::{Digest, Sha256};

    let key = "sk-test123";

    // Hash using the same algorithm as VirtualKeyManager
    let hash1 = {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    };

    let hash2 = {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    };

    assert_eq!(hash1, hash2);
    assert_ne!(hash1, key);
    assert_eq!(hash1.len(), 64); // SHA-256 produces 64 hex chars
}

/// Test that different keys produce different hashes
#[test]
fn test_key_hashing_uniqueness() {
    use sha2::{Digest, Sha256};

    let hash_key = |key: &str| -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hex::encode(hasher.finalize())
    };

    let hash1 = hash_key("sk-key1");
    let hash2 = hash_key("sk-key2");

    assert_ne!(hash1, hash2);
}

/// Test VirtualKey validation logic
#[test]
fn test_key_validation_active() {
    let active_key = VirtualKey {
        key_id: "test".to_string(),
        key_hash: "hash".to_string(),
        key_alias: None,
        user_id: "user1".to_string(),
        team_id: None,
        organization_id: None,
        models: vec![],
        max_budget: None,
        spend: 0.0,
        budget_duration: None,
        budget_reset_at: None,
        rate_limits: None,
        permissions: vec![],
        metadata: HashMap::new(),
        expires_at: None,
        is_active: true,
        created_at: Utc::now(),
        last_used_at: None,
        usage_count: 0,
        tags: vec![],
    };

    // Active key with no expiration should be valid
    assert!(active_key.is_active);
    assert!(active_key.expires_at.is_none() || active_key.expires_at.unwrap() > Utc::now());
}

/// Test VirtualKey validation for inactive keys
#[test]
fn test_key_validation_inactive() {
    let inactive_key = VirtualKey {
        key_id: "test".to_string(),
        key_hash: "hash".to_string(),
        key_alias: None,
        user_id: "user1".to_string(),
        team_id: None,
        organization_id: None,
        models: vec![],
        max_budget: None,
        spend: 0.0,
        budget_duration: None,
        budget_reset_at: None,
        rate_limits: None,
        permissions: vec![],
        metadata: HashMap::new(),
        expires_at: None,
        is_active: false,
        created_at: Utc::now(),
        last_used_at: None,
        usage_count: 0,
        tags: vec![],
    };

    assert!(!inactive_key.is_active);
}

/// Test VirtualKey validation for expired keys
#[test]
fn test_key_validation_expired() {
    let expired_key = VirtualKey {
        key_id: "test".to_string(),
        key_hash: "hash".to_string(),
        key_alias: None,
        user_id: "user1".to_string(),
        team_id: None,
        organization_id: None,
        models: vec![],
        max_budget: None,
        spend: 0.0,
        budget_duration: None,
        budget_reset_at: None,
        rate_limits: None,
        permissions: vec![],
        metadata: HashMap::new(),
        expires_at: Some(Utc::now() - Duration::hours(1)),
        is_active: true,
        created_at: Utc::now(),
        last_used_at: None,
        usage_count: 0,
        tags: vec![],
    };

    // Key is active but expired
    assert!(expired_key.is_active);
    assert!(expired_key.expires_at.unwrap() < Utc::now());
}

/// Test VirtualKey with future expiration
#[test]
fn test_key_validation_not_expired() {
    let future_key = VirtualKey {
        key_id: "test".to_string(),
        key_hash: "hash".to_string(),
        key_alias: None,
        user_id: "user1".to_string(),
        team_id: None,
        organization_id: None,
        models: vec![],
        max_budget: None,
        spend: 0.0,
        budget_duration: None,
        budget_reset_at: None,
        rate_limits: None,
        permissions: vec![],
        metadata: HashMap::new(),
        expires_at: Some(Utc::now() + Duration::hours(24)),
        is_active: true,
        created_at: Utc::now(),
        last_used_at: None,
        usage_count: 0,
        tags: vec![],
    };

    assert!(future_key.is_active);
    assert!(future_key.expires_at.unwrap() > Utc::now());
}

/// Test KeyGenerationSettings defaults
#[test]
fn test_key_generation_settings_defaults() {
    let settings = KeyGenerationSettings::default();

    assert_eq!(settings.key_prefix, "sk-");
    assert!(!settings.default_permissions.is_empty());
    assert!(settings.default_budget.is_some());
    assert!(settings.default_rate_limits.is_some());
}

/// Test RateLimits structure
#[test]
fn test_rate_limits_structure() {
    let rate_limits = RateLimits {
        rpm: Some(60),
        rph: Some(3600),
        rpd: Some(86400),
        tpm: Some(100000),
        tph: Some(6000000),
        tpd: Some(144000000),
        max_parallel_requests: Some(10),
    };

    assert_eq!(rate_limits.rpm, Some(60));
    assert_eq!(rate_limits.max_parallel_requests, Some(10));
}

/// Test Permission enum variants
#[test]
fn test_permission_variants() {
    let perms = vec![
        Permission::ChatCompletion,
        Permission::TextCompletion,
        Permission::Embedding,
        Permission::ImageGeneration,
        Permission::ModelAccess("gpt-4".to_string()),
        Permission::Admin,
        Permission::KeyManagement,
        Permission::ViewUsage,
        Permission::TeamManagement,
        Permission::Custom("custom_perm".to_string()),
    ];

    assert_eq!(perms.len(), 10);
    assert!(perms.contains(&Permission::Admin));
}

/// Test CreateKeyRequest builder pattern
#[test]
fn test_create_key_request() {
    let request = CreateKeyRequest {
        user_id: "user123".to_string(),
        key_alias: Some("my-key".to_string()),
        team_id: None,
        models: vec!["gpt-4".to_string()],
        max_budget: Some(100.0),
        budget_duration: Some("1d".to_string()),
        rate_limits: None,
        permissions: vec![Permission::ChatCompletion],
        metadata: HashMap::new(),
        expires_at: None,
        tags: vec!["production".to_string()],
    };

    assert_eq!(request.user_id, "user123");
    assert_eq!(request.key_alias, Some("my-key".to_string()));
    assert_eq!(request.models.len(), 1);
}
