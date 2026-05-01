//! Compatibility exports for the shared core pricing database.
//!
//! Pricing data ownership now lives in `core::pricing`; this module keeps the
//! older provider-base import path stable while callers migrate.

pub use crate::core::pricing::{
    ModelPricing, PricingDatabase, Usage, calculate_cost, get_pricing_db,
};
