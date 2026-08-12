# Cost compatibility lifecycle

`PricingService` is the sole runtime authority for user-visible pricing, spend,
budget reservation, and the pricing HTTP API. `core::cost` remains public only
as a compatibility and provider-catalog adapter during the 0.6 migration
window; it is not a second live pricing source.

## Compatibility inventory

| Surface | Current consumers | 0.6 disposition | Earliest removal |
| --- | --- | --- | --- |
| `core::cost::calculator::{generic_cost_per_token, estimate_cost, get_model_pricing}` | Provider adapters and compatibility callers | Keep as authority-first adapters; root re-exports deprecated | 0.7.0 |
| `core::cost::calculator::CostCalculator` | Legacy provider calculators | Keep direct module path; root re-export deprecated | 0.7.0 |
| `core::cost::types::{ModelPricing, UsageTokens, CostBreakdown, CostError}` | Provider catalogs and authority conversion | Keep as internal adapter DTOs; do not claim runtime authority | After consumers migrate |
| `core::cost::CostResult` | Compatibility callers only | Root re-export deprecated in favor of `core::pricing_service::CostResult` | 0.7.0 |
| `core::cost::types::CostResult` | Provider-adapter compatibility | Retain as an adapter DTO while the root re-export migrates | After consumers migrate |
| `core::cost::providers::{openai,anthropic,azure,generic}` | Legacy constructors/external callers | Public compatibility retained as authority-first adapters | 0.7.0 |
| `src/core/cost/providers/mod.rs` factory/re-exports | None; shadowed by the inline module and never compiled | Remove unreachable file | Removed in 0.6 tranche |
| Provider-local pricing tables | `PricingService` fallback adapters for catalog-only models | Keep only where the embedded authority lacks an equivalent row | Evidence-based, not mechanical |

Production route and settlement code must call `PricingService`. Compatibility
adapters first query the embedded `PricingService` authority and enter a
provider-local fallback only for a model-not-found result. Other authority
errors remain errors; unknown pricing must not become zero-cost success.

## Removal gate

Removal requires all of the following:

1. a published 0.6 release containing these deprecations;
2. migration of remaining provider/catalog DTO consumers;
3. public API approval for the 0.7 breaking change;
4. parity tests for aliases, fallback-only models, unknown pricing, and spend;
5. a rollback path that restores only the compatibility adapter, never a
   second user-visible pricing authority.

Until that gate is met, callers can migrate without a behavior change:

```rust
// Runtime pricing authority
use litellm_rs::core::pricing_service::PricingService;

// Temporary provider-adapter path
use litellm_rs::core::cost::calculator::generic_cost_per_token;
```
