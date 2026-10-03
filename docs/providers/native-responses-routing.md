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

Runtime does not accept `background=true` or server-side tools. Gateway F05
currently enforces stateless creation (`store=false`) for all providers. Account
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
