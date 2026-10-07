# Request ledger billing and supplier reconciliation

Enable the existing `storage.request_ledger` configuration and use the admin
Request Logs view or `GET /admin/request-ledger?request_id=...`. These are
metadata-only terminal rows; neither request/response bodies nor credentials are
recorded. Existing retention and write-failure policy still apply.

`cost` remains the known cost calculated from trusted usage and gateway pricing.
Successful known-cost requests have no unknown reason or supplier-waiting state.
When usage is missing or pricing is unavailable, `cost` stays null: reserved or
fallback budget charges are estimates, never substituted for actual cost.

`billing` explains the liability:

- `provider_reserved_amount`, `model_reserved_amount`, and `key_reserved_amount`
  record only configured, tracked budgets. They independently enforce the same
  request. Do **not** add them together. Unconfigured scopes remain null.
- `charge_basis` identifies `gateway_pricing`, `reserved_estimate`, or
  `fallback_pricing`. Charge amounts record acknowledged budget accounting,
  independently from `cost`.
- `unknown_reason` distinguishes missing usage, a cancelled stream before usage,
  and unavailable pricing. `reserved_at` records the hold timestamp;
  `awaiting_since` starts the waiting period. The query computes
  `awaiting_duration_ms` at read time until supplier verification is recorded.
- `provider_settlement`, `model_settlement`, and `key_settlement` distinguish
  reserved, pending, settled, and unconfirmed backend operations. `settled` requires
  acknowledgement. A failed combined provider/model settlement is conservatively
  `settlement_unconfirmed` for both scopes, even if one side applied the charge.
- Distributed cancellation is `cancellation_unconfirmed`: the synchronous SDK
  cancellation API logs backend failures but does not return an acknowledgement.
  Local acknowledged cancellation is `released`.

If a future is cancelled or times out while Redis is still processing a write,
the budget worker retains accounting responsibility. The terminal ledger snapshot
can remain pending; later worker completion does not rewrite an already persisted
row. Inspect shared budget state and supplier evidence before treating such an
operation as complete. No retry of an ambiguous accounting write is introduced.
Native durable Responses recovery continues to use its existing SQL settlement
receipt and response lifecycle queries; this ledger does not duplicate recovery.
A process crash before a terminal row is persisted is outside terminal ledger
coverage. Null `billing` means facts were not recorded, not an invented unknown
charge. This migration does not backfill facts into older records.

## Resolve unknown costs

1. Find the request ID and compare the separate holds, acknowledged charges,
   unknown reason, and elapsed waiting time.
2. For an unacknowledged Redis write, use `provider_lease_id` and `model_lease_id`
   to inspect the existing shared-budget hash for the row's provider/model.
   Inspect that identity's `l:` / `p:` fields and the committed/outstanding amounts;
   these UUIDs are accounting identities, not credentials. Do not replay a write
   merely because an HTTP result was lost. Then verify the supplier charge from an invoice/receipt or provider usage record.
   Save only a short evidence reference, never credentials or response content.
3. Use the Request Logs detail form or send this administrator-only request:

   ```http
   POST /admin/request-ledger/{request_id}/reconciliation
   Content-Type: application/json

   {"verified_actual_cost": 0.002, "evidence_reference": "invoice-2026-10-line-42"}
   ```

   The amount uses the same currency as gateway pricing. This records
   `reconciliation.verified_actual_cost`, the evidence reference, and verification
   timestamp. It preserves the original unknown `cost` and budget charges.
4. `budget_review_required` compares each tracked scope independently with the
   verified amount and requires review when acknowledgement is missing or a
   charge differs. It is recomputed against the latest billing facts on query.
   **Recording supplier verification does not adjust Redis or key budget
   counters.** Resolve that discrepancy operationally; this feature does not claim
   budget reconciliation has occurred.

Identical verification submissions are idempotent. A different amount or evidence
reference returns HTTP 409 instead of overwriting the first verification. Missing
request IDs return HTTP 404; invalid amounts/references return HTTP 400. Terminal
row upserts preserve supplier verification in its separate JSON column. Parsing
invalid stored billing or verification metadata fails visibly instead of silently
omitting evidence.

The implementation adapts the existing request-ledger storage/admin path and
budget acknowledgement boundary. No new financial subsystem or dependency was
introduced. Regression checks cover unknown usage and disconnect estimates,
known cost with unavailable backend, real Redis acknowledgement delay/cancel,
untracked budgets, terminal future cancellation, SQL persistence, verification
conflicts, admin authorization, and elapsed waiting time.


## Realtime session rows

Realtime keeps **one terminal ledger row per WebSocket session**, persisted when
its upgraded HTTP body ends or is dropped, rather than at the 101 handshake.
The relay explicitly retains the middleware's facts handle across its task spawn.
Each active response uses the existing `RequestLedgerFacts` type temporarily;
its accounting metadata merges into the session once, without a new table or
per-response persistence model.

`known_cost_subtotal` sums only responses with trusted usage and known pricing.
`unknown_response_count` counts completed/interrupted responses whose cost is
unknown; this is sticky when later responses have known cost.
`pending_response_count` counts admitted responses not yet finalized. Session
`cost` is a complete known total only when both counts are zero. A body ending
while a generation remains pending never presents the known subtotal as the
complete actual charge. An unknown session remains `cost: null` and retains its
first unknown reason until supplier verification is recorded separately.

For Realtime, each scope's reserved and acknowledged charged amounts accumulate
across responses **within that scope**. Provider/model/key amounts still must
not be added to each other. `reserved_at` and the scalar lease UUID fields refer
to the **latest generation**, not a unique identity for all session accounting.
An unacknowledged historical operation remains `settlement_unconfirmed` even
when the latest generation's accounting succeeds. Consult provider billing and
shared-budget evidence before resolving that discrepancy.

Realtime close/error ordering is unchanged. A close frame may end the HTTP body
before pending fallback settlement finishes. Detached accounting explicitly
passes the captured generation handle to its worker, which records an ACK only
when the actual operation succeeds. The session snapshot may conservatively
retain an unconfirmed result when that ACK arrives after its generation metadata
was merged; terminal rows do not claim automatic correction from a late ACK.
