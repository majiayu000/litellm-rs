# Tech Spec

## Linked Issue

GH-727 / #727

## Product Spec

Link to `product.md`.

## Current Evidence

At `origin/main@edec83d71bdd`, 30 tracked Rust files remain over the U-16 800-line ceiling.
The current largest file is `src/utils/event/tests.rs` at 889 lines. It is a
test-only suite that mixes event type, event builder, subscription handle,
broker lifecycle, publish/drop, concurrency, subscriber trait, edge-case, and
config coverage in one module.

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
| P1 | Test suites | DataUtils tests, router tests, utils/event tests, provider test files, integration route tests | focused `cargo test` for the moved module plus line-count proof |
| P2 | Public type facades | SDK, analytics, monitoring, observability, audio, config, user/key/cache types | `cargo check --all-features --locked` plus import-path smoke coverage when available |
| P3 | Runtime orchestrators | OpenTelemetry, OAuth session, request validator | focused module tests or affected integration tests plus all-features check |
| P4 | Utility modules | config helpers, net client utils, sync containers | focused utility tests plus concurrency/behavior checks where relevant |
| P5 | Closure scan | all Rust files | full over-800 scan, final SpecRail update, final PR may close #727 |

## Current Tranche: Event Test-Suite Split

### Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Test root | `src/utils/event/tests.rs` | Contains all event tests inline. | It is 889 lines and mixes unrelated behavior domains. |
| Production broker | `src/utils/event/broker.rs` | Implements EventBroker and stats behavior. | Production code must remain untouched in this tranche. |
| Production types | `src/utils/event/types.rs` | Implements EventType, Event, Subscriber, and SubscriptionHandle. | Type behavior is covered by moved tests only. |
| Module mount | `src/utils/event/mod.rs` | Mounts `#[cfg(test)] mod tests;`. | The focused test path must remain `utils::event::tests`. |

### Design

1. Keep `src/utils/event/tests.rs` as the shared test root with common imports, `TestData`, and child module declarations only.
2. Add `event_type_tests.rs` for EventType predicates, display, equality, and clone coverage.
3. Add `event_tests.rs` for Event constructors, builder methods, type checks, IDs, and clone coverage.
4. Add `subscription_handle_tests.rs` for handle creation, cancellation, default, and unique IDs.
5. Add `broker_creation_tests.rs` and `broker_subscription_tests.rs` for broker construction, subscribe, unsubscribe, and clear behavior.
6. Add `broker_publish_tests.rs` for publish, non-blocking delivery, closed-channel cleanup, and stats behavior.
7. Add `broker_concurrency_tests.rs` for concurrent subscribe, publish, subscribe/unsubscribe, and clear behavior.
8. Add `subscriber_trait_tests.rs`, `broker_edge_case_tests.rs`, and `broker_config_tests.rs` for trait, edge-case, and config coverage.
9. Do not edit production event modules.

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 | `tests.rs` | Root keeps shared helpers and child module declarations only. |
| P2 | `tests/*.rs` | Original test functions remain discoverable by name under behavior-domain modules. |
| P3 | file size | `wc -l src/utils/event/tests.rs src/utils/event/tests/*.rs` shows every touched file below 800. |
| P4 | focused test suite | `cargo test utils::event::tests --lib --all-features` runs the moved tests. |
| P5 | queue count | tracked-file scan shows the remaining queue no longer includes `src/utils/event/tests.rs`. |

## Risks

- Child modules must import the shared parent helpers correctly; losing `TestData` or async imports breaks discovery.
- Concurrent tests rely on the same timing windows, barriers, and atomic counters; assertions and sleeps should remain unchanged.
- Splitting test modules must not change the public event module mount path.

## Test Plan

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo test utils::event::tests --lib --all-features`
- [ ] `cargo check --lib --all-features`
- [ ] `cargo check --all-features --locked`
- [ ] `cargo check`
- [ ] Line-count proof for `src/utils/event/tests.rs` and `src/utils/event/tests/*.rs`

## Rollback

Revert the Event test-suite split and `specs/GH727` edits. No production code,
schema changes, or runtime behavior changes are involved.
