# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@27ac684370e1`, 40 Rust files remain over the U-16 800-line ceiling.
The highest-risk files are not all equivalent: some are pure test suites, some are public
type facades, and some are runtime orchestrators. They need different split patterns.

## Architecture Principles

1. Facade compatibility: when splitting public type files, the original module path keeps
   re-exporting the same public names with `pub use`.
2. Runtime ownership: runtime files split by one responsibility at a time, such as request
   mapping, response mapping, operation handlers, storage helpers, or error conversion.
3. Test-suite ownership: test-only files split by behavior domain while keeping original
   assertions and focused test command coverage.
4. No silent degradation: moved code must preserve current error propagation and must not
   add warning-only fallbacks for previously failing states.
5. Bounded PRs: each tranche owns one file family and includes line-count proof plus focused
   tests.

## Queue Design

| Phase | Lane | Target examples | Verification pattern |
| --- | --- | --- | --- |
| P1 | Test suites | cost calculator, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, security, cost, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | Vertex AI client, unified provider, teams route, SeaORM team repository, OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: Bedrock Model Config Projection

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Bedrock model config facade | `src/core/providers/bedrock/model_config.rs` | Defines Bedrock model family/API/config types, a large legacy `MODEL_CONFIGS` map, lookup helpers, and focused tests. | The file is 1118 lines and duplicates data already represented in `bedrock/catalog`. |
| Bedrock catalog | `src/core/providers/bedrock/catalog/` | Stores typed catalog entries split by vendor/family and already projects each entry to `ModelConfig`. | This is the existing architectural boundary for Bedrock model metadata. |
| Bedrock callers | `chat`, `provider`, `transformation`, `model_id`, route spend/token policy | Call `get_model_config` / `get_model_config_for_model_id` or use exported config types. | Public lookup paths and error behavior must remain unchanged. |

### Design

1. Keep `src/core/providers/bedrock/model_config.rs` as the public facade for `BedrockModelFamily`, `BedrockApiType`, `ModelConfig`, and lookup helpers.
2. Replace the hand-written 900+ line `MODEL_CONFIGS` initializer with a projection from `super::catalog::all_entries()`:
   - key: `entry.model_id`
   - value: `entry.to_model_config()`
3. Preserve `get_model_config`, `model_supports_capability`, and `get_all_model_ids` signatures and error behavior.
4. Update catalog module docs to reflect the new data ownership: the catalog drives the legacy `model_config` facade instead of being only a validation mirror.
5. Keep existing model_config tests in place; they now verify the public facade behavior.
6. Keep existing catalog cross-reference tests in place; they continue to verify catalog integrity and projection shape.
7. Do not edit Bedrock model IDs, pricing values, capabilities, limits, request transformation, provider routing, or model ID parsing.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `model_config.rs` projection initializer | Focused Bedrock model_config tests pass through the unchanged public helpers. |
| P2 | `catalog/mod.rs` and existing entries | Catalog tests still pass, including projection and pricing-state invariants. |
| P3 | Bedrock callers | `cargo check --all-features --locked` compiles existing imports and public re-exports. |
| P4 | file size | `wc -l src/core/providers/bedrock/model_config.rs src/core/providers/bedrock/catalog/*.rs src/core/providers/bedrock/catalog/entries/*.rs` shows all touched files below 800. |
| P5 | queue count | `git ls-files '*.rs' | xargs wc -l | awk '$1 > 800 && $2 != "total" { print $1 " " $2 }' | sort -nr` shows the remaining queue no longer includes `model_config.rs`. |

## Risks

- `model_config.rs` and `catalog` reference each other by design: the catalog imports the public config types, while the facade imports `all_entries()` for data projection. Avoid adding runtime calls from catalog entry construction back into `get_model_config`.
- Catalog tests that previously compared catalog entries to the legacy map become facade-behavior checks after this change; keep model_config unit tests as the direct public surface smoke test.
- `get_all_model_ids()` order is not specified today because it reads `HashMap` keys; this tranche must not add any order guarantee.
- This tranche reduces one of the remaining 40 files; the issue remains a tracker after merge.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test core::providers::bedrock::model_config --lib --all-features`
- [ ] `cargo test core::providers::bedrock::catalog --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] Line-count proof for `src/core/providers/bedrock/model_config.rs` and touched catalog files

## Rollback

Revert the Bedrock model-config projection and `specs/GH727` edits. No migrations
or runtime config changes are involved.
