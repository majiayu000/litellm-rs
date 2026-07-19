# Task Plan

## Linked Issue

GH-1066 / #1066

## Spec Packet

- Product: `specs/GH1066/product.md`
- Tech: `specs/GH1066/tech.md`

## Implementation Tasks

- [x] `SP1066-T1` Covers: P1, P9. Owner: coordinator. Dependencies: none. Done when: default-empty typed callback configuration embeds existing OTel/Datadog/Langfuse config types, validates capacity/timeout/backend uniqueness and backend requirements, and the example config documents opt-in use without literal secrets. Verify: focused config model and validator tests; `rg` confirms no duplicate backend config structs.

- [x] `SP1066-T2` Covers: P2-P4 and shutdown. Owner: coordinator. Dependencies: T1. Done when: the bounded callback runtime accepts a public `IntegrationManager`, preserves queue order, reports full/closed queue errors, isolates backend failures, and drains/flushes/shuts down; Langfuse is adapted to the canonical `Integration` trait. Verify: callback runtime, manager, and Langfuse adapter tests.

- [x] `SP1066-T3` Covers: P2. Owner: coordinator. Dependencies: T1-T2. Done when: `HttpServer::new` independently initializes configured backends, logs and skips a failed backend, injects the dispatcher into `AppState`, and `HttpServer::start` drains it after Actix shutdown. Verify: startup registration and partial-failure tests.

- [x] `SP1066-T4` Covers: P3-P8. Owner: coordinator. Dependencies: T2-T3. Done when: provider-backed chat, completion, response, and embedding unary/streaming paths emit metadata-only start plus exactly one terminal event with selected target, latency, usage/cost when available, and explicit error/disconnect outcomes; cache hits do not fabricate provider events. Verify: focused lifecycle unit tests and real mock-provider route tests for success, provider error, stream completion, stream error, timeout, and disconnect.

- [x] `SP1066-T5` Covers: all. Owner: verification owner. Dependencies: T1-T4. Done when: focused tests, formatting, build, strict clippy, full test suite, SpecRail workflow/spec checks, scope guard, and overlap guard pass from this worktree with evidence saved under `artifacts/logs/gh1066/`. Verify: commands listed below.

- [x] `SP1066-T7a` Covers: P2-P4, P9. Owner: Datadog remediation owner. Dependencies: T5. Done when: PR #1081 keeps Datadog site selection on the exact approved hostname allowlist and makes exporter batching cancellation-safe with a bounded two-batch pending/in-flight state machine; manager timeout, non-2xx mixed metric/log partial failure, successful-category removal, retry, and saturation fixtures pass without detached tasks or silent loss. Verify: focused Datadog tests, formatting, build, strict clippy, SpecRail checks, scope, overlap, exact-head review, CI, and PR gate.

- [ ] `SP1066-T7b` Covers: P3-P8. Owner: final callback remediation owner. Dependencies: T7a. Done when: a later final PR hardens callback start/terminal pairing, joins OpenTelemetry and Langfuse delivery semantics, and completes embedding hook coverage with deterministic success/error/cancellation fixtures. Verify: focused callback pair, OpenTelemetry, Langfuse, embedding lifecycle, full repository, review, CI, and PR-gate evidence.

- [ ] `SP1066-T6` Covers: all acceptance criteria. Owner: coordinator and independent reviewer. Dependencies: T5, T7a, T7b. Done when: the T7b final-slice `mixed_impl` PR closes #1066; independent exact-head review has no blocking findings; blocking CI wait, GraphQL review-thread query, merge-state check, and `pr_gate.py` are current and green; merge and issue closure are remotely confirmed. Verify: PR/check/gate/merge evidence recorded in the runtime checkpoint.

## Parallelization

Implementation is serial in the current worktree because config, runtime,
AppState, and route lifecycle changes are dependent and cargo must not run
concurrently. The only native parallel lanes are read-only architecture review
before implementation and an independent exact-head reviewer after the PR is
pushed.

## Verification

Run focused tests during implementation, then once before PR readiness:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
python3 checks/check_workflow.py --repo .
python3 checks/check_workflow.py --repo . --spec-dir specs/GH1066
bash scripts/guards/check_pr_scope.sh
bash scripts/guards/check_pr_overlap.sh
```

After push:

```bash
gh pr checks <pr> --repo majiayu000/litellm-rs --watch --fail-fast
```

Then collect current PR evidence, run the repository PR gate serially, merge
only on an allowed decision, and confirm the merged PR plus closed issue
remotely.

## Handoff Notes

- `Integration` is the canonical callback boundary; do not add a third trait or
  an `Any`-typed public hook.
- Request/exporter failures are independent: callback failure is observable but
  never replaces the gateway response.
- No callback event may contain authorization headers, provider/backend
  credentials, prompt content, or generated content.
- All cargo commands belong to the current GH1066 worktree and one verification
  owner.
