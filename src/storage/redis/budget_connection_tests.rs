use super::*;
use crate::config::models::storage::RedisConfig;

async fn test_pool() -> Option<RedisPool> {
    let Ok(url) = std::env::var("REDIS_URL") else {
        assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
        return None;
    };
    Some(
        RedisPool::new(&RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap(),
    )
}

fn reserve_args(lease_id: &str, amount: i64) -> BudgetLeaseArgs<'_> {
    BudgetLeaseArgs {
        op: "reserve",
        now_ms: 1_000,
        period_epoch: 1,
        amount,
        max_or_actual_or_force: 100,
        seed_committed: 0,
        lease_id,
        ttl_ms: 60_000,
    }
}

#[tokio::test]
async fn killed_budget_connection_recovers_without_replaying_the_failed_operation() {
    let Some(pool) = test_pool().await else {
        return;
    };
    // This endpoint holder is private to this test. Killing its exact client
    // cannot disrupt other Redis tests using the same server concurrently.
    let cached = BudgetRuntimeConnection::default();
    let key = RedisPool::budget_lease_key("provider", &uuid::Uuid::new_v4().to_string());
    let first = cached
        .invoke(&pool, &key, reserve_args("first", 3))
        .await
        .unwrap();
    assert_eq!(first.outstanding, 3);
    let stale = cached.connect(&pool).await.unwrap();
    let mut old_connection = stale.as_ref().clone();
    let client_id: i64 = redis::cmd("CLIENT")
        .arg("ID")
        .query_async(&mut old_connection)
        .await
        .unwrap();
    let mut control = pool.open_live_connection().await.unwrap();
    let killed: i64 = redis::cmd("CLIENT")
        .arg("KILL")
        .arg("ID")
        .arg(client_id)
        .query_async(&mut control)
        .await
        .unwrap();
    assert_eq!(killed, 1);

    assert!(
        cached
            .invoke(&pool, &key, reserve_args("failed", 2))
            .await
            .is_err()
    );
    assert!(cached.live.lock().await.is_none());
    let second = cached
        .invoke(&pool, &key, reserve_args("second", 4))
        .await
        .unwrap();
    assert_eq!(second.committed, 0);
    assert_eq!(
        second.outstanding, 7,
        "the failed reservation must not be replayed"
    );
    let fresh = cached.connect(&pool).await.unwrap();
    assert!(!Arc::ptr_eq(&stale, &fresh));

    // Model another in-flight request reporting the old connection error late.
    cached.invalidate(&stale).await;
    assert!(Arc::ptr_eq(&fresh, &cached.connect(&pool).await.unwrap()));
    let third = cached
        .invoke(&pool, &key, reserve_args("third", 1))
        .await
        .unwrap();
    assert_eq!(third.outstanding, 8);

    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut control)
        .await
        .unwrap();
}

#[tokio::test]
async fn concurrent_budget_connects_share_one_generation() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let cached = BudgetRuntimeConnection::default();
    let (first, second, third) = tokio::join!(
        cached.connect(&pool),
        cached.connect(&pool),
        cached.connect(&pool)
    );
    let first = first.unwrap();
    assert!(Arc::ptr_eq(&first, &second.unwrap()));
    assert!(Arc::ptr_eq(&first, &third.unwrap()));
}

#[tokio::test]
async fn budget_script_error_preserves_the_healthy_connection() {
    let Some(pool) = test_pool().await else {
        return;
    };
    let cached = BudgetRuntimeConnection::default();
    let live = cached.connect(&pool).await.unwrap();
    let mut control = pool.open_live_connection().await.unwrap();
    let key = RedisPool::budget_lease_key("provider", &uuid::Uuid::new_v4().to_string());
    let _: () = redis::cmd("SET")
        .arg(&key)
        .arg("wrong-type")
        .query_async(&mut control)
        .await
        .unwrap();
    assert!(
        cached
            .invoke(&pool, &key, reserve_args("wrong-type", 2))
            .await
            .is_err()
    );
    assert!(Arc::ptr_eq(&live, &cached.connect(&pool).await.unwrap()));
    let _: i64 = redis::cmd("DEL")
        .arg(&key)
        .query_async(&mut control)
        .await
        .unwrap();
}
