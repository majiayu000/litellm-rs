# Native Responses routing

F06 adapts the existing native Responses gateway transport and each provider's
existing authentication. It does not derive endpoint support from prices or model
name prefixes. Lifecycle persistence remains the responsibility of F07.

## Evidence and implementation decision (2026-10-03)

- OpenAI: retain the reviewed exact model endpoint contracts and their source
  digests in the model catalog.
- Copilot: use the authenticated account `/models` response and each exact model's
  `supported_endpoints`. Microsoft [VS Code chatEndpoint.ts](https://github.com/microsoft/vscode/blob/253b7648aa69de1a651a327370098f3814b8922f/extensions/copilot/src/platform/endpoint/node/chatEndpoint.ts)
  uses this metadata to select `/responses`; its [model metadata fetcher](https://github.com/microsoft/vscode/blob/253b7648aa69de1a651a327370098f3814b8922f/extensions/copilot/src/platform/endpoint/node/modelMetadataFetcher.ts)
  obtains account-specific models. Unknown or missing endpoint evidence fails
  closed. A model list failure never falls back to Chat Completions. Existing
  OAuth, subscription endpoint, integration headers and native payloads are reused.
- Bedrock: use exact documented Runtime profile IDs and Mantle model IDs. Reuse
  AWS credentials, region, endpoint policy and SigV4 signing. Runtime uses
  `/openai/v1/responses` and signing service `bedrock`; Mantle GPT OSS uses
  `/v1/responses` and signing service `bedrock-mantle`. No user configuration is
  added. Unsupported profile/endpoint combinations fail before inference.

Alternatives rejected: LiteLLM's [Copilot Responses adapter](https://github.com/BerriAI/litellm/blob/8efb4a21f6ebb6a2c4f71e0f422ff9dbc9318972/litellm/llms/github_copilot/responses/transformation.py)
provides useful native payload/header precedent, but its price-table endpoint
inference is not authority for account-specific Copilot support. Guessing
Responses support from an OpenAI-compatible URL or GPT prefix is also rejected.
A new provider abstraction or endpoint configuration mode is unnecessary.

AWS primary references: [Responses API](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-responses-api.html),
[endpoint contracts](https://docs.aws.amazon.com/bedrock/latest/userguide/endpoints.html),
[GPT OSS 120B](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-oss-120b.html),
[GPT OSS 20B](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-oss-20b.html),
[GPT-6 Astra](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-6-astra.html),
[GPT-6 Sol](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-6-sol.html),
[GPT-6 Luna](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-6-luna.html),
[GPT-6.1 Sol](https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-openai-gpt-6-1-sol.html).

Validation plan: local HTTP tests for discovery, exact endpoint/model selection,
JSON/SSE preservation, provider headers/signatures, unsupported models and mapped
HTTP errors; existing native gateway tests for auth, budget, callbacks and
stream errors; default complete checks and relevant feature checks. These tests
cannot establish account entitlement or successful paid provider inference.

## Bedrock model/endpoint selection

| Configured model | Native endpoint and wire ID |
| --- | --- |
| `us.openai.gpt-6-{astra,sol,luna}` / `global.openai.gpt-6-{astra,sol,luna}` | Runtime `/openai/v1/responses`, exact profile preserved; source Regions from the respective model card |
| `us.openai.gpt-6.1-sol` / `global.openai.gpt-6.1-sol` | Runtime `/openai/v1/responses`; this implementation verifies `us-east-1` only because the card does not enumerate the other enabled source Regions |
| `openai.gpt-6-{sol,luna}` / `openai.gpt-6.1-sol` | Mantle `/openai/v1/responses`, exact model preserved, `us-east-1` |
| `openai.gpt-6-astra` | Mantle `/openai/v1/responses`, exact model preserved, `us-east-1` or `us-west-2` |
| `openai.gpt-oss-{20b,120b}-1:0` | Mantle `/v1/responses`, official wire ID `openai.gpt-oss-{20b,120b}`; intersect the model card's Regions with published Mantle endpoint Regions |

Runtime does not accept `background=true` or server-side tools. The F07 stack enables persistent lifecycle only for native OpenAI; Copilot and
Bedrock still require `store=false` and `background=false`. Account
entitlement and current AWS profile permissions remain upstream checks. Arbitrary
ARNs, invented profile prefixes and unreviewed model IDs do not gain Responses
support. Other Bedrock catalog models retain their existing non-Responses APIs.

The fresh AWS 6.1 Sol page fetched directly on 2026-10-03 explicitly includes the
global profile. An older indexed copy says US-only; the fresh source governs this
endpoint correction. Below are SHA-256 digests of the fetched official HTML:

- `gpt-6-sol`: `9cf9af7695f25014ed9e97fc636a92266b1d263be6cd9d3a4301ff298cc96870`
- `gpt-6-luna`: `b3de98d44aabd0e308ca4764a1809e89352db6349f5b89d3ae0d890c93ca4591`
- `gpt-6-astra`: `65c7ae8da799ce41df34c44dc5354920e9d37262692f2d5f9be62f37eb04f0a5`
- `gpt-6-1-sol`: `bc1ef99b8f5b8865a239ac0021422b3442e946cea6f7f27254c7be40a4bff2fe`
- `gpt-oss-120b`: `b7a8cdf87f144f0dbdd039f85e18af751ca361d1e76a00069f9c68e457018f5b`
- `gpt-oss-20b`: `4d24d0edbbd6110a63e8c2fcc786b1e7ae2b9050dd23e88fafc5f7905631c2e3`

Budget and usage settlement use the same endpoint decision: Mantle looks up
`bedrock_mantle` pricing for its actual wire model; Runtime retains the exact
Bedrock profile. This prevents applying global reference prices to regional
Mantle requests. No second price table is introduced. Copilot subscriptions do
not imply a zero token price: existing `unpriced_model_policy` remains in force
(default reject). Operators need their own verified price configuration or the
existing explicit `allow_unpriced` policy; endpoint support is still checked
against the authenticated account catalog.


## F07: durable background accounting

Decision: adapt the existing SQL storage and budget backends, with one dedicated
`response_settlements` table. [LiteLLM's documented background poller](https://github.com/BerriAI/litellm-docs/blob/main/docs/response_api.md)
also retains pending responses and retrieves terminal usage independently of
client GETs. Its general managed-object/enterprise polling infrastructure is not
adopted: this gateway already owns SQL, deployment binding, pricing snapshots and
Redis leases. [OpenAI background mode](https://developers.openai.com/api/docs/guides/background)
allows polling existing response IDs and documents roughly ten-minute retention
when `store=true` is not explicitly requested.

The gateway writes a billing intent **before** POST, including the selected
account/deployment digest, frozen price metadata and budget lease IDs. It binds
the upstream response ID as soon as a valid JSON response or `response.created`
frame is stored. No prompt, output or credential is copied into the billing row.
Deleting/expiring response content never deletes the billing obligation.

On normal completion the creating worker freezes observed terminal usage before
releasing its SQL recovery lease, so a later upstream deletion cannot erase a
known charge. After
a process stops, another instance picks up the abandoned intent at the existing
nine-minute dispatch deadline. Startup recovery scans at most 32 due records per
cycle, bounds each GET to 20 seconds, and never repeats POST. It uses the original
price metadata even if the live catalog has changed. Provider/model settlement
runs idempotently in Redis; API-key usage and its receipt commit together in one
SQL transaction. SQL completion is acknowledged only after those backend steps.
Redis and SQL are separate systems; this is retry-safe reconciliation, not a
cross-database atomic transaction. Backend failures retain pending obligations.

Supported deployment combinations:

- Shared enabled SQL with no provider/model limits: native OpenAI background and
  API-key usage accounting, including replica takeover and restart recovery.
- Shared SQL plus the existing Redis backend: also supports enabled provider/model
  limits. The Redis dataset must be persisted and shared; deleting it discards
  the budget counters and idempotency receipts it owns.
- Process-local provider/model limits, or an API key referencing an in-process
  `budget_id`: rejected before upstream dispatch. This change does not introduce
  a persistent API-key budget subsystem or claim that local limits span replicas.

If a dispatch's response ID or trustworthy terminal usage cannot be recovered,
`outcome=reserved_unknown` retains the conservative reservation amount separately.
It commits that amount only to provider/model budget protection; API-key actual
cost and token usage remain zero and its unpriced-request counter is incremented.
This unresolved amount is **not verified supplier spend**. No request is replayed
to try to resolve it. Response content TTL and billing evidence retention differ:
SQL settlement rows and Redis receipts are retained, with no automatic pruning
in this change. External callback delivery after a crash is not replayed.

Validation includes SQL write-failure rollback, two instances reopening shared
SQLite and recovering a pre-existing dispatch with frozen pricing, duplicate
acknowledgement retry, deleted/expired content isolation, public API-key creation
with `api.chat` through auth middleware, and local real-Redis lease-expiry/retry
checks. Recovery fixtures simulate the durable crash boundary; they do not claim
that a paid upstream process was exercised or killed. Responses creation and its
lifecycle share the existing `api.chat` permission.

Additional recovery acceptance evidence: `http_created_background_response_recovers_after_creator_runtime_stops` creates a background response through a real TCP HTTP POST and normal API-key middleware. The creator owns a separate Actix runtime; the test stops its HTTP workers, drops its `ResponseSettlementTask` owner, drops the runtime and joins its thread before the upstream becomes terminal. Two fresh gateway startup workers then recover the actual POST-created SQL obligation. The test expires the persisted creator lease explicitly instead of waiting nine wall-clock minutes; it never fabricates the dispatch row or calls the recovery function directly. Repeated authorized lifecycle reads leave generation POST count at one and actual key request/token/cost settlement at one. This combined test uses SQLite and no configured provider/model caps; Redis lease-expiry/idempotency remains covered separately by the Redis tests, not claimed as part of this HTTP scenario. PostgreSQL multi-instance recovery has not been exercised locally.
