use super::CostCalculator;

#[test]
fn multimodal_cost_rejects_unpriced_static_fallback() {
    let cost = CostCalculator::calculate_multimodal_cost(
        "gemini-3.6-flash-unpriced-static-fallback-test",
        1_000,
        500,
        Some(200),
        None,
        None,
        None,
    );

    assert_eq!(cost, None);
}

#[test]
fn audio_estimate_replaces_regular_input_rate_without_double_billing() {
    for (model, text_rate, audio_rate, output_rate) in [
        ("gemini-2.5-flash", 0.30, 1.0, 2.50),
        ("gemini-2.5-flash-lite", 0.10, 0.30, 0.40),
    ] {
        let cost =
            CostCalculator::calculate_multimodal_cost(model, 2020, 100, None, None, None, Some(60))
                .unwrap();
        let expected =
            (100.0 * text_rate + 1920.0 * audio_rate + 100.0 * output_rate) / 1_000_000.0;
        assert!(
            (cost - expected).abs() < 1e-12,
            "{model}: {cost} != {expected}"
        );
        assert!(
            CostCalculator::calculate_multimodal_cost(
                model,
                2020,
                100,
                Some(100),
                None,
                None,
                Some(60),
            )
            .is_none()
        );
        assert!(
            CostCalculator::calculate_multimodal_cost(model, 100, 100, None, None, None, Some(60),)
                .is_none()
        );
    }
}
