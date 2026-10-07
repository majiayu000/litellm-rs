# Reliability repair queue: integration and delivery evidence

This work preserves the full reliability scope from the 2026-10-07 portfolio
review. It reuses the existing repair heads instead of implementing them again.
The source candidate is an integration branch, not the published 0.8.2 artifact.

## Existing repair queue

All eight PRs start from main `a0aada3521b72e4ea0ebbf9044b8f4dd359a5d6f`.
The initial integration merge `aecaa8feb53973e9dd4f1558ce18fc9ce658dfb1` contains
these exact heads and was produced without text conflicts:

| PR | Exact head | Repair |
| --- | --- | --- |
| [1458](https://github.com/majiayu000/litellm-rs/pull/1458) | `d35b75cd407b61a7b1dd70be4b629edc7a662bd0` | Runtime-bound SDK embeddings |
| [1459](https://github.com/majiayu000/litellm-rs/pull/1459) | `3848322dcf6f9d3e788017b700b469ea690f0638` | Budget lease identity, async gateway bridge, bounded waits |
| [1460](https://github.com/majiayu000/litellm-rs/pull/1460) | `7fc7f25422279c0a2158bbcbba1e0605a08e2128` | SSE terminal validation |
| [1461](https://github.com/majiayu000/litellm-rs/pull/1461) | `8c516942e8a2a552371ab8a507d2e2524b9bb122` | SDK tool-result identity and streamed tool calls |
| [1462](https://github.com/majiayu000/litellm-rs/pull/1462) | `21097d5305759744bcecc45e86d5839e595b9a28` | Credential authority for key management |
| [1463](https://github.com/majiayu000/litellm-rs/pull/1463) | `77cf4c2d179cd5d3cf961f78caa391a82a27afc2` | Missing/inactive team key rejection |
| [1464](https://github.com/majiayu000/litellm-rs/pull/1464) | `719f6dd10a9226c3ede839eae7d726abd45b2de8` | Runtime reload quota continuity |
| [1465](https://github.com/majiayu000/litellm-rs/pull/1465) | `b804d4cff3c30cc3ee30cfebde6c69d176babd4f` | Await routing Redis and preserve successful cancellation accounting |

These original PRs were subsequently closed as superseded by the existing
[audit combination #1470](https://github.com/majiayu000/litellm-rs/pull/1470),
not marked merged. The accounting candidate #1471 builds on its validated head
`d88fec44a166b2c566a5b62fb34ae29f0b9867d4`, retaining its lifecycle/SDK and cache
follow-ups rather than opening a competing repair delivery. Three settlement
conflicts combine lifecycle usage observation/completion with captured billing
facts; the scoped settlement futures remain heap-pinned for the default thread
stack. Original commit ancestry is preserved. Acceptance of either previous
candidate alone is not acceptance of this new combination.

The original review correctly identified synchronous budget Redis waits in main.
The latest #1459 head now handles them through the existing SDK worker bridge.
#1465 still excludes that separate budget backend. Both are needed in the combined
candidate. See [budget ownership/deadlines](redis-budget-leases.md) and
[async routing](async-routing.md). Explicit synchronous SDK entry points still
block; they are not the HTTP execution path.

## Compatibility and upgrade

- The SDK DTO changes in #1461 are source-breaking for struct literals. Follow
  [the SDK migration notes](../architecture/router-runtime.md#sdk-tool-turns-and-streaming-migration):
  add `Message.tool_call_id` and `ChatChunk.usage`, and consume canonical
  `ToolCallDelta` fragments. HTTP text-message JSON remains valid.
- Routing lease success/failure completion is asynchronous. Embedded callers of
  those APIs must await completion; retain the lease rather than detaching its ID.
- SSE EOF without a provider terminal indication is an error, not a completed
  answer. Consumers must handle that existing stream-error boundary.
- Key management requires the credential's own management grant as well as the
  user's role. Team-bound keys stop working when their team is missing/inactive.
- Distributed budget hashes retain pending reservation identities after capacity
  expiry. All replicas must adopt the new behavior before a rolling upgrade is
  complete. No timer may discard a pending identity while accepting late costs.
- API-key budgets remain process-local. This work does not establish shared or
  restart-persistent per-key balances; the performance profile exercises shared
  provider/model budgets.
- Rust callers constructing public `RequestLedgerFacts` or `RequestLedgerRecord`
  literals must provide `billing` (use `None` when no facts are captured).
- Request-ledger billing metadata is an additive SQL migration. Supplier-verified
  cost does not automatically rewrite a previously committed Redis estimate.
  Differences remain explicitly actionable rather than silently called settled.

The source candidate is version 0.9.0 and follows the existing pre-1.0
breaking-release path (minor version rather than a 0.8 patch).
No tag, registry upload, archive, container or Homebrew update is implied by local
source validation. The independent native Responses draft #1449 is not merged
into this repair scope.

## Full requested scope

| Scope | Implementation / evidence |
| --- | --- |
| Existing #1458–#1465 repairs | Exact heads above; combination checks recorded below |
| Budget Redis HTTP waits and cancellation | Reused #1459; real Redis current-thread, deadline, atomic/idempotent and cancellation regressions |
| Explain and process unknown costs | Existing request ledger and admin query/reconciliation workflow; see [billing documentation](request-ledger-billing.md) |
| Deployment performance | [Authentication/shared-budget/ledger and long SSE/slow-consumer runner](../benchmarks/deployment-profile.md) |
| Profile provider-count wording | [Merged profile PR #5](https://github.com/majiayu000/majiayu000/pull/5), verified main commit `ca78d39d98115438522b91d5342ad78820291006`: 60+ runtime-wired providers |
| litellm-rs / Cove / Rekey boundaries | Gateway deployment/accounting only; no Cove/Rekey source or settings modified; the separate fault-regression chat has read-only access here |

## Candidate checks and publication

Checks are recorded only after they finish. Original per-PR CI is evidence for
those heads, not a substitute for combined-candidate CI. Deployment measurements
state their exact binary/source/profile and synthetic-upstream boundary.

Completed before freezing this source: all eight original heads have 15/15
successful CI checks; remaining inline review threads have been inspected and
resolved. The dashboard DOM suite passed 31/31 and the existing overhead-runner
contract suite passed 14/14. These are not combined Rust acceptance claims.
Final combined Rust/CI and deployment measurements are retained with the task
evidence and reported in the candidate pull request.

Publication remains pending until candidate CI/reviews, immutable version selection,
and the existing release artifact/installation checks complete. The actual published
version must be verified by tag commit, registry checksum and archive/container
identities before this scope can be called delivered.
