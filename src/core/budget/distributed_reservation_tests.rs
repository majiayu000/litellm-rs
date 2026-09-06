use super::{
    BudgetReservationError, ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
};

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
