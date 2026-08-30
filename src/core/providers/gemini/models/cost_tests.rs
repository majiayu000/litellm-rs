use super::CostCalculator;

#[test]
fn multimodal_cost_rejects_unpriced_static_fallback() {
    let cost = CostCalculator::calculate_multimodal_cost(
        "unpriced-static-fallback-test",
        1_000,
        500,
        Some(200),
        None,
        None,
        None,
    );

    assert_eq!(cost, None);
}
