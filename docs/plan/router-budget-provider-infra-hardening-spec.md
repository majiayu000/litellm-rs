# Router, Budget, and Provider Infrastructure Hardening Spec

Date: 2026-06-25
Review base: `origin/main` at `328636373d783bc686da85584ec5e95e0b661643`
Implementation base: `origin/main` at `f9c5e4ab01a6165dce31e47b374d269db6b6bc33`

Tracking issues:

- `#709`: Router hard parallel reservations and lease cleanup
- `#710`: Router routing metadata snapshot boundary
- `#711`: Budget reserve and settle semantics
- `#713`: Provider enum / trait / handle contract alignment
- `#714`: Provider registry declaration and conformance tests
- `#715`: Provider failure / retry policy / HTTP mapping split
- `#716`: SDK / router / gateway / provider / storage workspace RFC
- `#519`: Existing provider/type-tree architecture meta issue

## Goal

Turn the June 2026 static review into a bounded implementation roadmap for
hard routing limits, consistent routing metadata, budget pre-authorization, and
provider abstraction cleanup.

The review is substantially correct. The main risk is not Rust syntax or local
compile health; it is that several user-visible hard constraints are currently
implemented as soft counters or split state without a single consistency
boundary.

## Non-Goals

- Do not rewrite the whole router, provider system, budget system, and crate
  layout in one PR.
- Do not weaken existing tests to make a refactor pass.
- Do not break public APIs without an explicit migration PR and release note.
- Do not treat successful local `cargo check` as proof of runtime quota
  correctness.

## Findings

### F1. Router parallel limits are not atomic reservations

`select_deployment` checks `active_requests` before selection and increments
the selected deployment afterwards. Concurrent callers can observe the same
pre-increment value and all pass the limit. This makes
`max_parallel_requests` a best-effort statistic rather than a hard cap.

Cancellation risk also exists because selection returns a plain deployment ID
and relies on callers to pair it with `release_deployment`. Some server-side
streaming code already works around this with a local RAII lease, but the core
router API has not absorbed that contract.

RPM and TPM checks are also pre-flight observations, not reservations. RPM is
incremented only by `record_success`; failed requests are counted as failures
but do not consume the current-minute RPM counter before the upstream call.
TPM cannot be reserved accurately with the current selection signature because
no request token estimate is available at selection time.

### F2. Routing metadata has no snapshot boundary

The router maintains separate `deployments`, `model_index`, and `model_aliases`
maps. Individual `DashMap` operations are thread-safe, but the business
invariant across maps is not atomic.

Concrete bugs:

- Re-adding the same deployment ID appends duplicate IDs to `model_index`.
- Re-adding an existing ID under a different model leaves the old model index
  stale.
- Selection reads IDs from `model_index` and does not validate that the loaded
  deployment still belongs to the resolved model.
- `set_model_list` builds new maps locally but installs them entry by entry, so
  readers can observe mixed generations during hot update.
- Alias cycle detection walks alias chains, but selection resolves only one
  alias hop.
- `DeploymentState::clone` copies atomic values into new atomics, so cloned
  deployments diverge instead of sharing runtime counters.

### F3. Budget checks are not atomic pre-authorizations

`BudgetTracker::check_spend` reads whether an amount would fit, while
`record_spend` later mutates the balance. There is no atomic
`reserve + settle` token that prevents concurrent callers from all seeing the
last remaining budget.

Budget amounts are represented as `f64` and spend mutation does not reject
negative, NaN, or infinite values at the value boundary. Floating point is
acceptable for display and estimates, but not as the only type used for hard
authorization or persisted accounting.

### F4. Provider abstraction has split contracts

`LLMProvider` is documented as the core provider abstraction, while router
deployments store the closed `Provider` enum. Third-party implementors of
`LLMProvider` cannot be routed without modifying the enum and dispatch code.

`ProviderHandle` is public and advertised as a type-erased routing wrapper, but
its core methods are stubs: chat completion returns unimplemented, model/tool
support return true, health is always healthy, cost is zero, latency is fixed,
and success rate is 100%.

This should be resolved in favor of either a closed built-in provider set or an
object-safe custom provider path. Until then, `ProviderHandle` must not be
presented as a real routing abstraction.

### F5. Provider registration has multiple sources of truth

Provider availability is split across module declarations, feature gates,
`ProviderType`, factory paths, dispatch macro arms, data-driven Tier 1 catalog
entries, capability declarations, and documentation. The existing meta issue
`#519` already tracks the broader provider/type-tree drift, but this review
adds a concrete acceptance criterion: every advertised provider must be
constructible from config and callable through the same router dispatch path for
its declared endpoint.

### F6. Provider errors mix facts, retry policy, and HTTP mapping

`ProviderError` variants store upstream facts but also decide retryability,
retry delay, and HTTP status. Correct retry behavior depends on request context:
operation idempotency, stream stage, emitted tokens, retry budget, deadline, and
provider `Retry-After`. A variant alone cannot know these.

The dynamic `LLMProvider::name()` plus static `ProviderError` provider string
is another signal that provider identity and error facts need a clearer model.

### F7. SDK, gateway, and infrastructure live in one crate

The crate exposes SDK and gateway surfaces together. Default features pull in
storage, Redis, HTTP server, auth, and related dependencies. The current
feature setup is workable for a single package, but long-term kernel stability
would improve with a workspace split.

## Architecture Direction

### Router

- Introduce a core `DeploymentLease`/reservation API.
- Enforce `max_parallel_requests` with an atomic compare-and-reserve operation,
  not a read-then-increment sequence.
- Use RAII for router-owned execution paths so cancellation drops the lease.
- Add a follow-up `RateReservation` API that reserves RPM before upstream calls
  and supports TPM estimate/settle semantics.
- Move routing metadata toward an immutable `RoutingSnapshot` installed with one
  swap:

```text
RoutingSnapshot {
  by_id: HashMap<DeploymentId, Arc<Deployment>>,
  by_model: HashMap<ModelName, Arc<[DeploymentId]>>,
  aliases: HashMap<ModelName, ModelName>,
}
```

Runtime counters should be stored in shared runtime state rather than copied by
`DeploymentState::clone`.

### Budget

- Introduce a fixed-point money type for authorization paths.
- Add `reserve_spend(scope, max_amount) -> BudgetReservation`.
- Add `settle(actual_amount)` and refund unused reservation amount.
- Reject negative, NaN, and infinite amounts at API boundaries while the
  transition still accepts legacy `f64` display inputs.
- For distributed deployment, implement provider/model/global reservation in a
  database transaction or Redis Lua script.

### Provider

- Either document the provider set as closed and remove public routing stubs, or
  add an object-safe `DynProvider`/`CustomProvider` adapter.
- Generate or validate provider registration from one declaration source.
- Add conformance tests proving every advertised provider selector can be
  instantiated and dispatched for declared capabilities.

### Errors

- Split `ProviderFailure` facts from `RetryPolicy` decisions.
- Move HTTP status mapping to HTTP adapters.
- Make retry decisions accept request context including stream stage and
  idempotency.

## PR Plan

### PR 1: Router hard parallel cap and low-risk consistency guards

Scope:

- Add atomic active-request reservation for `max_parallel_requests`.
- Add a router-owned lease for non-streaming execution paths.
- Keep existing public `select_deployment` compatibility, but route internal
  execution through lease-safe APIs.
- Make `add_deployment` de-duplicate index entries and remove stale model-index
  references when an ID changes model.
- Revalidate `deployment.model_name` against the resolved model during
  selection.
- Resolve aliases consistently or reject alias-to-alias explicitly.

Tests:

- `max_parallel_requests = 1` with many concurrent selections never admits more
  than one active request.
- Aborting an in-flight `execute_once` returns the active request slot.
- Re-adding the same deployment ID does not duplicate `model_index`.
- Re-adding a deployment under a new model does not leave the old model
  routable.
- Alias chains either resolve fully or are explicitly rejected.

Tracking:

- `#709`
- `#710`

### PR 2: Atomic routing snapshot

Scope:

- Replace split mutable routing maps with an immutable `RoutingSnapshot`.
- Build, validate, de-duplicate, and swap the full table in one step.
- Preserve runtime state through shared runtime handles.

Tests:

- Hot update during sustained routing never returns a deployment for the wrong
  model and never emits transient `ModelNotFound` for models present in either
  complete generation unless explicitly removed.
- `DeploymentState` clone/runtime sharing behavior is made impossible or tested
  as shared state.

Tracking:

- `#710`

### PR 3: Budget reservation API

Scope:

- Add fixed-point authorization type.
- Add atomic reserve/settle/refund guard.
- Keep legacy `record_spend` as a compatibility wrapper where possible.
- Reject invalid numeric inputs.

Tests:

- Multiple concurrent requests competing for the final budget allow at most one
  reservation.
- Dropping/cancelling a reservation refunds the amount unless settled.
- Negative, NaN, and infinite spend values are rejected.

Tracking:

- `#711`

### PR 4: Provider abstraction decision

Scope:

- Mark `ProviderHandle` deprecated or replace it with a real object-safe
  adapter.
- Align public docs with either closed built-in providers or custom-provider
  support.
- Link this work to `#519`.

Tests:

- `ProviderHandle` no longer returns optimistic stub routing data.
- If custom providers are supported, a mock provider can be routed through the
  same router path as built-ins.

Tracking:

- `#713`
- `#519`

### PR 5: Provider registration conformance

Scope:

- Add a declaration table or validation harness for provider selectors,
  factory creation, enum/dispatch support, capabilities, feature gates, and
  docs matrix.
- Generate docs or fail tests when docs drift.

Tests:

- Every advertised provider selector is constructible under the feature set that
  documents it.
- Every constructed provider can dispatch each declared endpoint through the
  router path.

Tracking:

- `#714`
- `#519`

### PR 6: Retry/error/HTTP split

Scope:

- Introduce fact-only provider failure data.
- Add retry policy inputs for idempotency, stream stage, deadline, retry budget,
  and provider retry hints.
- Move HTTP mapping into gateway adapters.

Tests:

- Streaming errors after emitted chunks are not automatically retried.
- Provider `Retry-After` participates in delay decisions.
- HTTP mapping tests do not depend on core provider error policy methods.

Tracking:

- `#715`

### PR 7: Workspace split RFC

Scope:

- Write an RFC for splitting core, provider API, providers, router, gateway,
  and storage into workspace crates.
- Include feature compatibility and migration plan.

Tests:

- RFC only; no code behavior change.

Tracking:

- `#716`

## Merge Gate

Every implementation PR must provide fresh evidence from the current head:

- `cargo check`
- targeted `cargo test` for touched modules
- full `cargo test` before merge, unless CI is the explicitly recorded
  full-suite truth source
- current PR head SHA, check rollup, merge state, and review-thread state
  before merge

No PR may merge if it has unresolved actionable review threads or if
`origin/main` advanced over overlapping files after verification.
