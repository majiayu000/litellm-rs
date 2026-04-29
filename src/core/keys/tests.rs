//! Tests for the API Key Management System
//!
//! This module contains integration tests for the keys module.

use super::manager::KeyManager;
use super::repository::InMemoryKeyRepository;
use super::types::{CreateKeyConfig, KeyPermissions, KeyRateLimits, KeyStatus, UpdateKeyConfig};
use chrono::{Duration, Utc};
use uuid::Uuid;

fn create_test_manager() -> KeyManager {
    KeyManager::new(InMemoryKeyRepository::new())
}

// ==================== Key Generation Tests ====================

#[tokio::test]
async fn test_generate_key_basic() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Basic Key".to_string(),
        ..Default::default()
    };

    let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

    assert!(raw_key.starts_with("gw-"));
    assert_eq!(raw_key.len(), 35); // "gw-" + 32 chars

    let key = manager.get_key(key_id).await.unwrap().unwrap();
    assert_eq!(key.name, "Basic Key");
    assert_eq!(key.status, KeyStatus::Active);
}

#[tokio::test]
async fn test_generate_key_with_full_config() {
    let manager = create_test_manager();

    let user_id = Uuid::new_v4();
    let team_id = Uuid::new_v4();
    let expires_at = Utc::now() + Duration::days(30);

    let config = CreateKeyConfig {
        name: "Full Config Key".to_string(),
        description: Some("A fully configured key".to_string()),
        user_id: Some(user_id),
        team_id: Some(team_id),
        permissions: KeyPermissions::full_access(),
        rate_limits: KeyRateLimits::premium(),
        expires_at: Some(expires_at),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    let key = manager.get_key(key_id).await.unwrap().unwrap();
    assert_eq!(key.user_id, Some(user_id));
    assert_eq!(key.team_id, Some(team_id));
    assert!(key.description.is_some());
}

#[tokio::test]
async fn test_generate_multiple_keys() {
    let manager = create_test_manager();

    let mut key_ids = Vec::new();
    for i in 0..10 {
        let config = CreateKeyConfig {
            name: format!("Key {}", i),
            ..Default::default()
        };
        let (key_id, _) = manager.generate_key(config).await.unwrap();
        key_ids.push(key_id);
    }

    // All keys should be unique
    let unique_ids: std::collections::HashSet<_> = key_ids.iter().collect();
    assert_eq!(unique_ids.len(), 10);

    // All keys should be retrievable
    for id in key_ids {
        assert!(manager.get_key(id).await.unwrap().is_some());
    }
}

#[tokio::test]
async fn test_key_manager_clone_shares_repository_state() {
    let manager = create_test_manager();
    let cloned_manager = manager.clone();

    let config = CreateKeyConfig {
        name: "Shared State Key".to_string(),
        ..Default::default()
    };

    let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

    let key_from_clone = cloned_manager.get_key(key_id).await.unwrap();
    assert!(key_from_clone.is_some());

    let verify_from_clone = cloned_manager.validate_key(&raw_key).await.unwrap();
    assert!(verify_from_clone.valid);
}

// ==================== Key Validation Tests ====================

#[tokio::test]
async fn test_validate_valid_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Valid Key".to_string(),
        ..Default::default()
    };

    let (_, raw_key) = manager.generate_key(config).await.unwrap();

    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(result.valid);
    assert!(result.key.is_some());
    assert!(result.invalid_reason.is_none());
}

#[tokio::test]
async fn test_validate_nonexistent_key() {
    let manager = create_test_manager();

    let result = manager
        .validate_key("gw-nonexistent12345678901234567890")
        .await
        .unwrap();
    assert!(!result.valid);
    assert!(result.key.is_none());
    assert!(
        result
            .invalid_reason
            .as_ref()
            .unwrap()
            .contains("not found")
    );
}

#[tokio::test]
async fn test_validate_revoked_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Revoked Key".to_string(),
        ..Default::default()
    };

    let (key_id, raw_key) = manager.generate_key(config).await.unwrap();
    manager.revoke_key(key_id).await.unwrap();

    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(!result.valid);
    assert!(result.invalid_reason.as_ref().unwrap().contains("revoked"));
}

#[tokio::test]
async fn test_validate_expired_key() {
    let manager = create_test_manager();

    // Create a key that expires in 1 second
    let config = CreateKeyConfig {
        name: "Expiring Key".to_string(),
        expires_at: Some(Utc::now() + Duration::milliseconds(100)),
        ..Default::default()
    };

    let (_, raw_key) = manager.generate_key(config).await.unwrap();

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(!result.valid);
    assert!(result.invalid_reason.as_ref().unwrap().contains("expired"));
}

// ==================== Key Revocation Tests ====================

#[tokio::test]
async fn test_revoke_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "To Revoke".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();
    manager.revoke_key(key_id).await.unwrap();

    let key = manager.get_key(key_id).await.unwrap().unwrap();
    assert_eq!(key.status, KeyStatus::Revoked);
}

#[tokio::test]
async fn test_revoke_already_revoked_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Already Revoked".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();
    manager.revoke_key(key_id).await.unwrap();

    let result = manager.revoke_key(key_id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_revoke_nonexistent_key() {
    let manager = create_test_manager();

    let result = manager.revoke_key(Uuid::new_v4()).await;
    assert!(result.is_err());
}

// ==================== Key Rotation Tests ====================

#[tokio::test]
async fn test_rotate_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Original Key".to_string(),
        permissions: KeyPermissions::full_access(),
        rate_limits: KeyRateLimits::premium(),
        ..Default::default()
    };

    let (old_id, old_key) = manager.generate_key(config).await.unwrap();
    let (new_id, new_key) = manager.rotate_key(old_id).await.unwrap();

    // Old key should be revoked
    let old_result = manager.validate_key(&old_key).await.unwrap();
    assert!(!old_result.valid);

    // New key should be valid
    let new_result = manager.validate_key(&new_key).await.unwrap();
    assert!(new_result.valid);

    // New key should have similar config
    let new_key_info = manager.get_key(new_id).await.unwrap().unwrap();
    assert!(new_key_info.name.contains("rotated"));
}

#[tokio::test]
async fn test_rotate_revoked_key_fails() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Revoked for Rotation".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();
    manager.revoke_key(key_id).await.unwrap();

    let result = manager.rotate_key(key_id).await;
    assert!(result.is_err());
}

// ==================== Key Update Tests ====================

#[tokio::test]
async fn test_update_key_name() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Original Name".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    let update = UpdateKeyConfig {
        name: Some("Updated Name".to_string()),
        ..Default::default()
    };

    let updated = manager.update_key(key_id, update).await.unwrap();
    assert_eq!(updated.name, "Updated Name");
}

#[tokio::test]
async fn test_update_key_permissions() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Permissions Test".to_string(),
        permissions: KeyPermissions::default(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    let update = UpdateKeyConfig {
        permissions: Some(KeyPermissions::admin()),
        ..Default::default()
    };

    let updated = manager.update_key(key_id, update).await.unwrap();
    assert!(updated.permissions.is_admin);
}

#[tokio::test]
async fn test_update_revoked_key_fails() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Revoked for Update".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();
    manager.revoke_key(key_id).await.unwrap();

    let update = UpdateKeyConfig {
        name: Some("New Name".to_string()),
        ..Default::default()
    };

    let result = manager.update_key(key_id, update).await;
    assert!(result.is_err());
}

// ==================== Key Listing Tests ====================

#[tokio::test]
async fn test_list_user_keys() {
    let manager = create_test_manager();
    let user_id = Uuid::new_v4();

    for i in 0..5 {
        let config = CreateKeyConfig {
            name: format!("User Key {}", i),
            user_id: Some(user_id),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    // Create some keys for another user
    let other_user = Uuid::new_v4();
    for i in 0..3 {
        let config = CreateKeyConfig {
            name: format!("Other User Key {}", i),
            user_id: Some(other_user),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let keys = manager.list_user_keys(user_id).await.unwrap();
    assert_eq!(keys.len(), 5);
}

#[tokio::test]
async fn test_list_team_keys() {
    let manager = create_test_manager();
    let team_id = Uuid::new_v4();

    for i in 0..4 {
        let config = CreateKeyConfig {
            name: format!("Team Key {}", i),
            team_id: Some(team_id),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let keys = manager.list_team_keys(team_id).await.unwrap();
    assert_eq!(keys.len(), 4);
}

#[tokio::test]
async fn test_list_keys_with_status_filter() {
    let manager = create_test_manager();

    // Create some active keys
    for i in 0..3 {
        let config = CreateKeyConfig {
            name: format!("Active Key {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    // Create and revoke some keys
    for i in 0..2 {
        let config = CreateKeyConfig {
            name: format!("Revoked Key {}", i),
            ..Default::default()
        };
        let (key_id, _) = manager.generate_key(config).await.unwrap();
        manager.revoke_key(key_id).await.unwrap();
    }

    let active_keys = manager
        .list_keys(Some(KeyStatus::Active), None, None)
        .await
        .unwrap();
    assert_eq!(active_keys.len(), 3);

    let revoked_keys = manager
        .list_keys(Some(KeyStatus::Revoked), None, None)
        .await
        .unwrap();
    assert_eq!(revoked_keys.len(), 2);
}

#[tokio::test]
async fn test_list_keys_pagination() {
    let manager = create_test_manager();

    for i in 0..10 {
        let config = CreateKeyConfig {
            name: format!("Page Key {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let page1 = manager.list_keys(None, Some(5), Some(0)).await.unwrap();
    assert_eq!(page1.len(), 5);

    let page2 = manager.list_keys(None, Some(5), Some(5)).await.unwrap();
    assert_eq!(page2.len(), 5);

    // Pages should be different
    let page1_ids: std::collections::HashSet<_> = page1.iter().map(|k| k.id).collect();
    let page2_ids: std::collections::HashSet<_> = page2.iter().map(|k| k.id).collect();
    assert!(page1_ids.is_disjoint(&page2_ids));
}

// ==================== Usage Stats Tests ====================

#[tokio::test]
async fn test_get_usage_stats() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Usage Stats Key".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    let stats = manager.get_usage_stats(key_id).await.unwrap();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.total_tokens, 0);
}

#[tokio::test]
async fn test_record_usage() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Usage Recording Key".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    manager.record_usage(key_id, 100, 0.01).await.unwrap();
    manager.record_usage(key_id, 200, 0.02).await.unwrap();

    let stats = manager.get_usage_stats(key_id).await.unwrap();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.total_tokens, 300);
    assert!((stats.total_cost - 0.03).abs() < f64::EPSILON);
}

// ==================== Key Deletion Tests ====================

#[tokio::test]
async fn test_delete_key() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Delete Me".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();
    manager.delete_key(key_id).await.unwrap();

    let key = manager.get_key(key_id).await.unwrap();
    assert!(key.is_none());
}

#[tokio::test]
async fn test_delete_nonexistent_key() {
    let manager = create_test_manager();

    let result = manager.delete_key(Uuid::new_v4()).await;
    assert!(result.is_err());
}

// ==================== Cleanup Tests ====================

#[tokio::test]
async fn test_cleanup_expired_keys() {
    let manager = create_test_manager();

    // Create some expired keys
    for i in 0..3 {
        let config = CreateKeyConfig {
            name: format!("Expired Key {}", i),
            expires_at: Some(Utc::now() + Duration::milliseconds(50)),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    // Create some valid keys
    for i in 0..2 {
        let config = CreateKeyConfig {
            name: format!("Valid Key {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let deleted = manager.cleanup_expired_keys().await.unwrap();
    assert_eq!(deleted, 3);

    let remaining = manager.count_keys(None).await.unwrap();
    assert_eq!(remaining, 2);
}

// ==================== Validation Tests ====================

#[tokio::test]
async fn test_validation_empty_name() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "".to_string(),
        ..Default::default()
    };

    let result = manager.generate_key(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validation_long_name() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "a".repeat(300),
        ..Default::default()
    };

    let result = manager.generate_key(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_validation_past_expiration() {
    let manager = create_test_manager();

    let config = CreateKeyConfig {
        name: "Past Expiration".to_string(),
        expires_at: Some(Utc::now() - Duration::hours(1)),
        ..Default::default()
    };

    let result = manager.generate_key(config).await;
    assert!(result.is_err());
}

// ==================== Counting Tests ====================

#[tokio::test]
async fn test_count_keys() {
    let manager = create_test_manager();

    for i in 0..7 {
        let config = CreateKeyConfig {
            name: format!("Count Key {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let count = manager.count_keys(None).await.unwrap();
    assert_eq!(count, 7);
}

#[tokio::test]
async fn test_count_keys_by_status() {
    let manager = create_test_manager();

    // Create 5 active keys
    for i in 0..5 {
        let config = CreateKeyConfig {
            name: format!("Active {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    // Create and revoke 3 keys
    for i in 0..3 {
        let config = CreateKeyConfig {
            name: format!("Revoked {}", i),
            ..Default::default()
        };
        let (key_id, _) = manager.generate_key(config).await.unwrap();
        manager.revoke_key(key_id).await.unwrap();
    }

    let active_count = manager.count_keys(Some(KeyStatus::Active)).await.unwrap();
    assert_eq!(active_count, 5);

    let revoked_count = manager.count_keys(Some(KeyStatus::Revoked)).await.unwrap();
    assert_eq!(revoked_count, 3);
}
