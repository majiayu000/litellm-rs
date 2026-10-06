#[cfg(feature = "gateway")]
use super::ModelLimitConfig;
use super::{BudgetReservationError, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits};

#[test]
fn unavailable_backend_fails_closed_without_overspend() {
    let limits = UnifiedBudgetLimits::with_unavailable_backend();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );

    assert!(matches!(
        limits.providers.reserve_provider_spend("openai", 1.0),
        Err(BudgetReservationError::BackendUnavailable)
    ));
    assert_eq!(
        limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
}

#[cfg(feature = "gateway")]
mod redis {
    use super::*;
    use crate::config::models::storage::RedisConfig;
    use crate::storage::redis::RedisPool;
    use std::sync::Arc;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_managers_last_token_allows_one_reservation() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-race");
        let model = unique("model-race");
        let a = Arc::new(seeded(pool.clone(), &provider, &model, 10.0));
        let b = Arc::new(seeded(pool.clone(), &provider, &model, 10.0));

        let a_task = {
            let a = Arc::clone(&a);
            let provider = provider.clone();
            let model = model.clone();
            tokio::spawn(async move { a.reserve_spend(&provider, &model, 10.0) })
        };
        let b_task = {
            let b = Arc::clone(&b);
            let provider = provider.clone();
            let model = model.clone();
            tokio::spawn(async move { b.reserve_spend(&provider, &model, 10.0) })
        };

        let a_result = a_task.await.expect("join a");
        let b_result = b_task.await.expect("join b");
        let successes = [&a_result, &b_result]
            .iter()
            .filter(|result| result.is_ok())
            .count();
        assert_eq!(
            successes,
            1,
            "exactly one of two competing last-token reservations should succeed (a_ok={}, b_ok={})",
            a_result.is_ok(),
            b_result.is_ok()
        );
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn model_deny_rolls_back_provider_outstanding() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-rollback");
        let model = unique("model-rollback");
        let limits = seeded(pool.clone(), &provider, &model, 100.0);
        limits
            .models
            .set_model_limit(&model, ModelLimitConfig::new(5.0, ResetPeriod::Monthly));

        assert!(matches!(
            limits.reserve_spend(&provider, &model, 10.0),
            Err(BudgetReservationError::ModelBudgetExceeded)
        ));

        let other = seeded(pool.clone(), &provider, &model, 100.0);
        other
            .models
            .set_model_limit(&model, ModelLimitConfig::new(5.0, ResetPeriod::Monthly));
        other
            .providers
            .reserve_provider_spend(&provider, 100.0)
            .expect("rolled-back provider outstanding should free the full budget");
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn model_reservation_capacity_is_infrastructure_failure_and_rolls_back_provider() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-capacity-rollback");
        let model = unique("model-capacity");
        let limits = seeded(pool.clone(), &provider, &model, 100.0);
        let key = RedisPool::budget_lease_key("model", &model);
        let mut conn = pool.open_live_connection().await.unwrap();
        let _: i64 = ::redis::Script::new(
            r#"
            for i = 1, tonumber(ARGV[1]) do
              redis.call('HSET', KEYS[1], 'p:orphan-' .. i, '1:100:0')
            end
            return 1
            "#,
        )
        .key(&key)
        .arg(crate::storage::redis::budget::MAX_UNSETTLED_BUDGET_LEASES)
        .invoke_async(&mut conn)
        .await
        .unwrap();

        assert!(matches!(
            limits.reserve_spend(&provider, &model, 10.0),
            Err(BudgetReservationError::BackendUnavailable)
        ));
        let state: (i64, i64) = ::redis::cmd("HMGET")
            .arg(&key)
            .arg(&["c", "o"])
            .query_async(&mut conn)
            .await
            .unwrap();
        assert_eq!(state, (0, 0), "capacity refusal must not change spend");
        limits
            .providers
            .reserve_provider_spend(&provider, 100.0)
            .expect("a model infrastructure failure must roll back its provider reservation")
            .cancel();
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn settle_and_cancel_keep_committed_and_outstanding_distinct() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-distinct");
        let model = unique("model-distinct");
        let limits = seeded(pool.clone(), &provider, &model, 10.0);
        let other = seeded(pool.clone(), &provider, &model, 10.0);

        let reservation = limits.reserve_spend(&provider, &model, 10.0).unwrap();
        assert!(
            other.reserve_spend(&provider, &model, 1.0).is_err(),
            "outstanding hold must block the remaining budget"
        );
        reservation.settle(3.0).unwrap();

        other
            .reserve_spend(&provider, &model, 7.0)
            .expect("committed 3 should leave 7 of 10");
        cleanup(&pool, &provider, &model).await;

        let provider = unique("provider-cancel");
        let model = unique("model-cancel");
        let limits = seeded(pool.clone(), &provider, &model, 10.0);
        let other = seeded(pool.clone(), &provider, &model, 10.0);
        let reservation = limits.reserve_spend(&provider, &model, 10.0).unwrap();
        reservation.cancel();
        other
            .reserve_spend(&provider, &model, 10.0)
            .expect("cancel must release outstanding without committing spend");
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn current_thread_runtime_can_reserve_without_panic() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-current-thread");
        let model = unique("model-current-thread");
        let limits = seeded(pool.clone(), &provider, &model, 10.0);
        let reservation = limits
            .reserve_spend(&provider, &model, 4.0)
            .expect("Actix workers use current_thread; reserve must not panic");
        reservation.settle(4.0).unwrap();
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn abandoned_lease_is_recovered_after_expiry() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-expire");
        let model = unique("model-expire");
        let limits = UnifiedBudgetLimits::new().with_redis_lease_ttl(pool.clone(), 1_000);
        limits.providers.set_provider_limit(
            &provider,
            ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        limits
            .models
            .set_model_limit(&model, ModelLimitConfig::new(10.0, ResetPeriod::Monthly));

        let reservation = limits.reserve_spend(&provider, &model, 10.0).unwrap();
        std::mem::forget(reservation);
        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;

        let other = UnifiedBudgetLimits::new().with_redis_lease_ttl(pool.clone(), 1_000);
        other.providers.set_provider_limit(
            &provider,
            ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        other
            .models
            .set_model_limit(&model, ModelLimitConfig::new(10.0, ResetPeriod::Monthly));
        other
            .reserve_spend(&provider, &model, 10.0)
            .expect("expired lease must be reclaimed for a later reserve");
        cleanup(&pool, &provider, &model).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn ordinary_settlement_after_expiry_preserves_new_reservations() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("provider-late-settlement");
        let model = unique("model-late-settlement");
        let creator = UnifiedBudgetLimits::new().with_redis_lease_ttl(pool.clone(), 1);
        creator.providers.set_provider_limit(
            &provider,
            ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        creator
            .models
            .set_model_limit(&model, ModelLimitConfig::new(10.0, ResetPeriod::Monthly));

        let old = creator.reserve_spend(&provider, &model, 8.0).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let other = seeded(pool.clone(), &provider, &model, 10.0);
        let current = other.reserve_spend(&provider, &model, 6.0).unwrap();

        old.settle(3.0).unwrap();
        assert!(
            other.reserve_spend(&provider, &model, 1.1).is_err(),
            "late spend 3 plus the new outstanding 6 must leave only 1"
        );
        current.cancel();
        other
            .reserve_spend(&provider, &model, 7.0)
            .expect("only actual spend must remain after cancelling the new hold")
            .cancel();
        assert!(other.reserve_spend(&provider, &model, 7.1).is_err());
        cleanup(&pool, &provider, &model).await;
    }

    fn seeded(redis: Arc<RedisPool>, provider: &str, model: &str, max: f64) -> UnifiedBudgetLimits {
        let limits = UnifiedBudgetLimits::new().with_redis(redis);
        limits.providers.set_provider_limit(
            provider,
            ProviderLimitConfig::new(max, ResetPeriod::Monthly),
        );
        limits
            .models
            .set_model_limit(model, ModelLimitConfig::new(max, ResetPeriod::Monthly));
        limits
    }

    fn unique(prefix: &str) -> String {
        format!("{prefix}-{}", uuid::Uuid::new_v4())
    }

    async fn cleanup(pool: &RedisPool, provider: &str, model: &str) {
        let _ = pool
            .delete(&RedisPool::budget_lease_key("provider", provider))
            .await;
        let _ = pool
            .delete(&RedisPool::budget_lease_key("model", model))
            .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn durable_response_settlement_survives_expiry_restart_and_duplicate_retry() {
        let Some(pool) = live_redis_pool().await else {
            return;
        };
        let provider = unique("response-recovery");
        let model = unique("response-recovery");
        let creator = UnifiedBudgetLimits::new().with_redis_lease_ttl(pool.clone(), 1);
        creator.providers.set_provider_limit(
            &provider,
            ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
        );
        creator
            .models
            .set_model_limit(&model, ModelLimitConfig::new(10.0, ResetPeriod::Monthly));
        let reservation = creator.reserve_spend(&provider, &model, 8.0).unwrap();
        let descriptor = reservation.response_leases().unwrap();
        let encoded = serde_json::to_string(&descriptor).unwrap();
        reservation.detach_response();
        drop(creator);
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        let restarted = seeded(pool.clone(), &provider, &model, 10.0);
        let descriptor = serde_json::from_str(&encoded).unwrap();
        restarted
            .settle_response_leases(&provider, &model, &descriptor, 3.0)
            .unwrap();
        // Simulate Redis committed but SQL acknowledgement was lost.
        let other = seeded(pool.clone(), &provider, &model, 10.0);
        other
            .settle_response_leases(&provider, &model, &descriptor, 3.0)
            .unwrap();
        assert!((other.models.get_model_usage(&model).unwrap().current_spend - 3.0).abs() < 1e-9);
        assert!(
            (other
                .providers
                .get_provider_usage(&provider)
                .unwrap()
                .current_spend
                - 3.0)
                .abs()
                < 1e-9
        );
        other
            .reserve_spend(&provider, &model, 7.0)
            .unwrap()
            .cancel();
        assert!(other.reserve_spend(&provider, &model, 7.1).is_err());
        cleanup(&pool, &provider, &model).await;
    }

    async fn live_redis_pool() -> Option<Arc<RedisPool>> {
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
        let config = RedisConfig {
            url: redis_url.clone(),
            enabled: true,
            max_connections: 10,
            connection_timeout: 1,
            cluster: false,
            allow_degraded: false,
        };

        match RedisPool::new(&config).await {
            Ok(pool) => match pool.health_check().await {
                Ok(()) => Some(Arc::new(pool)),
                Err(err) => {
                    if std::env::var("CI").is_ok() {
                        panic!("Redis should pass health check in CI at {redis_url}: {err}");
                    }
                    eprintln!("Skipping distributed budget Redis integration test: {err}");
                    None
                }
            },
            Err(err) => {
                if std::env::var("CI").is_ok() {
                    panic!("Redis should be reachable in CI at {redis_url}: {err}");
                }
                eprintln!("Skipping distributed budget Redis integration test: {err}");
                None
            }
        }
    }
}
