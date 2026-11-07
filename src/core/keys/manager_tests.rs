use super::*;
use crate::core::keys::repository::InMemoryKeyRepository;
use crate::core::keys::types::{KeyPermissions, KeyRateLimits};

fn create_manager() -> KeyManager {
    KeyManager::new(InMemoryKeyRepository::new())
}

#[test]
fn test_prune_last_used_cache_enforces_hard_cap() {
    let manager = create_manager();
    let now = Instant::now();

    for _ in 0..=LAST_USED_CACHE_MAX_ENTRIES {
        manager.last_used_cache.insert(Uuid::new_v4(), now);
    }

    manager.prune_last_used_cache(now);

    assert!(manager.last_used_cache.len() < LAST_USED_CACHE_MAX_ENTRIES);
}

#[test]
fn test_clone_reuses_hmac_secret_allocation() {
    let manager = create_manager().with_hmac_secret(Some("shared-secret".to_string()));
    let cloned = manager.clone();

    let (Some(original_secret), Some(cloned_secret)) =
        (manager.hmac_secret.as_ref(), cloned.hmac_secret.as_ref())
    else {
        panic!("secret should be set on both managers");
    };

    assert!(Arc::ptr_eq(original_secret, cloned_secret));
    assert_eq!(cloned.hmac_secret(), Some("shared-secret"));
}

#[tokio::test]
async fn test_generate_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Test Key".to_string(),
        ..Default::default()
    };

    let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

    assert!(!raw_key.is_empty());
    assert!(raw_key.starts_with("gw-"));

    // Verify key was stored
    let key_info = manager.get_key(key_id).await.unwrap();
    assert!(key_info.is_some());
    assert_eq!(key_info.unwrap().name, "Test Key");
}

#[tokio::test]
async fn test_validate_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Validation Test".to_string(),
        ..Default::default()
    };

    let (_, raw_key) = manager.generate_key(config).await.unwrap();

    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(result.valid);
    assert!(result.key.is_some());
}

#[tokio::test]
async fn test_validate_invalid_key() {
    let manager = create_manager();

    let result = manager.validate_key("invalid-key").await.unwrap();
    assert!(!result.valid);
    assert!(result.invalid_reason.is_some());
}

#[tokio::test]
async fn test_revoke_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Revoke Test".to_string(),
        ..Default::default()
    };

    let (key_id, raw_key) = manager.generate_key(config).await.unwrap();

    manager.revoke_key(key_id).await.unwrap();

    // Key should no longer be valid
    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(!result.valid);
    assert!(result.invalid_reason.as_ref().unwrap().contains("revoked"));
}

#[tokio::test]
async fn test_rotate_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Rotate Test".to_string(),
        permissions: KeyPermissions::full_access(),
        rate_limits: KeyRateLimits::standard(),
        ..Default::default()
    };

    let (old_key_id, old_raw_key) = manager.generate_key(config).await.unwrap();

    let (new_key_id, new_raw_key) = manager.rotate_key(old_key_id).await.unwrap();

    // Old key should be revoked
    let old_result = manager.validate_key(&old_raw_key).await.unwrap();
    assert!(!old_result.valid);

    // New key should be valid
    let new_result = manager.validate_key(&new_raw_key).await.unwrap();
    assert!(new_result.valid);

    // New key should have same permissions
    let new_key = manager.get_key(new_key_id).await.unwrap().unwrap();
    assert!(new_key.name.contains("rotated"));
}

#[tokio::test]
async fn test_update_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Update Test".to_string(),
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
async fn test_list_user_keys() {
    let manager = create_manager();
    let user_id = Uuid::new_v4();

    for i in 0..3 {
        let config = CreateKeyConfig {
            name: format!("User Key {}", i),
            user_id: Some(user_id),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let keys = manager.list_user_keys(user_id).await.unwrap();
    assert_eq!(keys.len(), 3);
}

#[tokio::test]
async fn test_expired_key_validation() {
    let manager = create_manager();

    // We need to create a key and then manually expire it
    // Since we can't create an already-expired key, we'll update it

    let config = CreateKeyConfig {
        name: "Expiring Key".to_string(),
        expires_at: Some(Utc::now() + chrono::Duration::seconds(1)),
        ..Default::default()
    };

    let (_, raw_key) = manager.generate_key(config).await.unwrap();

    // Wait for expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let result = manager.validate_key(&raw_key).await.unwrap();
    assert!(!result.valid);
    assert!(result.invalid_reason.as_ref().unwrap().contains("expired"));
}

#[tokio::test]
async fn test_validation_empty_name() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "".to_string(),
        ..Default::default()
    };

    let result = manager.generate_key(config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_delete_key() {
    let manager = create_manager();

    let config = CreateKeyConfig {
        name: "Delete Test".to_string(),
        ..Default::default()
    };

    let (key_id, _) = manager.generate_key(config).await.unwrap();

    manager.delete_key(key_id).await.unwrap();

    let key = manager.get_key(key_id).await.unwrap();
    assert!(key.is_none());
}

#[tokio::test]
async fn test_count_keys() {
    let manager = create_manager();

    for i in 0..5 {
        let config = CreateKeyConfig {
            name: format!("Count Key {}", i),
            ..Default::default()
        };
        manager.generate_key(config).await.unwrap();
    }

    let count = manager.count_keys(None).await.unwrap();
    assert_eq!(count, 5);
}

#[tokio::test]
async fn cancelled_waiter_retains_write_capacity_until_repository_finishes() {
    let mut manager = KeyManager::new(InMemoryKeyRepository::new());
    manager.usage_write_slots = Arc::new(Semaphore::new(1));
    let clone = manager.clone();
    let (key_id, _) = manager
        .generate_key(CreateKeyConfig {
            name: "Cancelled usage waiter".to_string(),
            ..Default::default()
        })
        .await
        .unwrap();

    let mut write = Box::pin(manager.record_usage(key_id, 100, 0.25));
    assert!(futures::poll!(&mut write).is_pending());
    drop(write);
    // On this current-thread runtime the owned task has not run yet.
    assert!(matches!(
        clone.record_usage(key_id, 200, 0.50).await,
        Err(GatewayError::Unavailable(message))
            if message == "Key usage write capacity exhausted"
    ));
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if manager
                .get_usage_stats(key_id)
                .await
                .unwrap()
                .total_requests
                == 1
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    assert_eq!(manager.usage_write_slots.available_permits(), 1);
    clone.record_usage(key_id, 200, 0.50).await.unwrap();
    let stats = manager.get_usage_stats(key_id).await.unwrap();
    assert_eq!(stats.total_requests, 2);
    assert_eq!(stats.total_tokens, 300);
    assert_eq!(stats.total_cost, 0.75);
}

#[test]
fn usage_recording_preserves_non_tokio_executor_compatibility() {
    assert!(tokio::runtime::Handle::try_current().is_err());
    for local_pool in [false, true] {
        let exercise = async {
            let mut manager = create_manager();
            manager.usage_write_slots = Arc::new(Semaphore::new(1));
            let (key_id, _) = manager
                .generate_key(CreateKeyConfig {
                    name: "Executor-independent usage".to_string(),
                    ..Default::default()
                })
                .await
                .unwrap();

            drop(manager.record_usage(key_id, 999, 9.0));
            assert_eq!(
                manager
                    .get_usage_stats(key_id)
                    .await
                    .unwrap()
                    .total_requests,
                0
            );
            manager.record_usage(key_id, 100, 0.25).await.unwrap();
            manager
                .record_usage_record(key_id, UsageRecord::unpriced(50, 0.125, "allow_unpriced"))
                .await
                .unwrap();
            let stats = manager.get_usage_stats(key_id).await.unwrap();
            assert_eq!(stats.total_requests, 2);
            assert_eq!(stats.total_tokens, 150);
            assert!((stats.total_cost - 0.375).abs() < f64::EPSILON);
            assert_eq!(stats.unpriced_requests, 1);
            assert_eq!(stats.unpriced_tokens, 50);
            assert!((stats.unpriced_cost - 0.125).abs() < f64::EPSILON);
            assert_eq!(manager.usage_write_slots.available_permits(), 1);

            let permit = manager
                .usage_write_slots
                .clone()
                .try_acquire_owned()
                .unwrap();
            assert!(matches!(
                manager.record_usage(key_id, 200, 0.5).await,
                Err(GatewayError::Unavailable(message))
                    if message == "Key usage write capacity exhausted"
            ));
            drop(permit);
            assert_eq!(
                manager
                    .get_usage_stats(key_id)
                    .await
                    .unwrap()
                    .total_requests,
                2
            );
            // Preserve the in-memory repository's missing-key no-op contract.
            let missing_id = Uuid::new_v4();
            manager.record_usage(missing_id, 1, 0.01).await.unwrap();
            assert!(manager.get_key(missing_id).await.unwrap().is_none());
            assert_eq!(manager.usage_write_slots.available_permits(), 1);
        };
        if local_pool {
            futures::executor::LocalPool::new().run_until(exercise);
        } else {
            futures::executor::block_on(exercise);
        }
    }
}
