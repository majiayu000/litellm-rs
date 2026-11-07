use super::*;

#[actix_web::test]
async fn key_budget_and_interruption_fallback_use_existing_reservations() {
    let (state, _, _, _, handles) = fixture().await;
    state.budget_limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(10.0, ResetPeriod::Monthly),
    );
    let (key_id, _) = state
        .key_manager
        .generate_key(crate::core::keys::CreateKeyConfig {
            name: "realtime".into(),
            max_budget: Some(0.5),
            ..Default::default()
        })
        .await
        .unwrap();
    let id = state
        .key_manager
        .get_key(key_id)
        .await
        .unwrap()
        .unwrap()
        .budget_id;
    let bound = rates().bound(rates().max_output);
    let pending = Pending::reserve(
        &state,
        "openai",
        "gpt-realtime-mini",
        id,
        bound,
        rates().max_output,
    )
    .await
    .unwrap();
    assert!(
        Pending::reserve(
            &state,
            "openai",
            "gpt-realtime-mini",
            id,
            bound,
            rates().max_output
        )
        .await
        .is_err()
    );
    pending
        .settle(
            &state,
            None,
            Some(rates().cost(&usage(), rates().max_output).unwrap().unwrap()),
        )
        .await
        .unwrap();
    let expected = rates()
        .cost(&usage(), rates().max_output)
        .unwrap()
        .unwrap()
        .0;
    assert!(
        ((state
            .storage
            .database
            .api_key_budget(id.unwrap())
            .await
            .unwrap()
            .unwrap()
            .used_amount as f64
            / 1e9)
            - expected)
            .abs()
            < 1e-9
    );
    let pending = Pending::reserve(
        &state,
        "openai",
        "gpt-realtime-mini",
        id,
        bound,
        rates().max_output,
    )
    .await
    .unwrap();
    drop(pending);
    assert!(
        ((state
            .storage
            .database
            .api_key_budget(id.unwrap())
            .await
            .unwrap()
            .unwrap()
            .used_amount as f64
            / 1e9)
            - expected
            - bound)
            .abs()
            < 1e-9
    );
    assert!(
        (state
            .budget_limits
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend
            - expected
            - bound)
            .abs()
            < 1e-9
    );
    for handle in handles {
        handle.stop(false).await;
    }
}
