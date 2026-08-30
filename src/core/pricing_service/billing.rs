/// Pricing schedule selected for one usage record.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum PricingBillingMode {
    #[default]
    Standard,
    Batch,
}
