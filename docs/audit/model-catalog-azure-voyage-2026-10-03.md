# Azure AI static catalog and Voyage review — 2026-10-03

Related to #1373 (F10). Base: `decce90fef5dfe076baa1bc01b7978b8726d3034`.
This increment reviews the 18 entries in `azure_ai/models.rs` and 24 entries in
`voyage.rs`. It does not certify every Azure deployment, price-table row or
protocol implemented by either supplier. No account credentials or paid calls
were used.

## Evidence and decisions

- [Microsoft retired models](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/retired-models): retirement dates for the old GPT-4, Command R/R+, Jamba, Mistral and rerank families.
- [Microsoft current models](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-sold-directly-by-azure): exact FLUX/rerank IDs, GPT-4o version differences, image input limits and embeddings.
- [Microsoft partner models](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-from-partners): Phi-4 and the text-only Foundry multilingual Embed V3 contract.
- [Microsoft retirement schedule](https://learn.microsoft.com/en-us/azure/foundry/openai/concepts/model-retirement-schedule): distinguishes future retirement/deprecation from models already retired.
- [Microsoft image generation guide](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/dall-e): DALL-E 3 retired March 4, 2026; existing deployments are nonfunctional.
- [Microsoft GPT-3.5 model specification](https://learn.microsoft.com/en-us/azure/ai-foundry/foundry-models/concepts/models?view=azure-node-latest#gpt-35): 1106/0125 input 16,385, output 4,096. Current regional table still distinguishes them from retired 0301/0613. Regional/deployment availability is not tested here.
- [Voyage embeddings](https://docs.voyageai.com/docs/embeddings), [rerankers](https://docs.voyageai.com/docs/reranker), and [pricing](https://docs.voyageai.com/docs/pricing): current and older models, context, dimensions and token pricing. Both model guides explicitly retain older API models.
- [MongoDB Voyage lifecycle scope](https://www.mongodb.com/docs/voyageai/models/lifecycle/): applies to `ai.mongodb.com`, expressly excludes `api.voyageai.com`. It cannot establish retirement for this provider's default endpoint.

Decision: adapt the existing exact registry and catalog decision ledger; keep
existing native HTTP clients and runtime pricing authority. No lifecycle engine,
new transport, price source, configuration or model aliases are introduced.
Price rows alone are not model availability evidence. Versionless Azure IDs can
identify customer deployments; this patch removes incorrect global catalog
claims, not arbitrary deployment strings from low-level transport.

## Azure AI: all 18 prior static entries

| Prior ID | Result |
| --- | --- |
| `Phi-4` | Retain 16,384 input/output, no tools; existing conservative nonstreaming declaration unchanged. |
| `gpt-4o` | Retain 128,000/4,096 minimum across published versions; preserve vision in ModelInfo projection. Later versions support 16,384 output, requiring deployment-specific identity. |
| `gpt-4` | Remove old 8K record: its 0314/0613 family retired June 6, 2025. This is not a claim that every Azure GPT-4 Turbo deployment retired. |
| `gpt-35-turbo` | Retain 1106/0125 contract; correct input 4,096 → 16,385. Do not infer whole-family retirement from 0301/0613. |
| `command-r-plus` | Remove retired family and non-Foundry alias. |
| `command-r` | Remove retired family and non-Foundry alias. |
| `mistral-large-latest` | Remove unverified Foundry alias; upstream Mistral names do not establish Azure identity. Old Foundry Large retired April 15, 2025. |
| `ai21-jamba-instruct` | Remove; Jamba Instruct retired March 1, 2025. |
| `text-embedding-3-large` | Retain text embedding, 8,192 input. |
| `text-embedding-3-small` | Retain text embedding, 8,192 input. |
| `cohere-embed-v3-multilingual` | Correct to `Cohere-embed-v3-multilingual`; retain 512 input; remove unsupported image modality. |
| `dall-e-3` | Remove; Azure retirement March 4, 2026. |
| `flux-1.1-pro` | Correct to `FLUX-1.1-pro`, 5,000 input. |
| `flux.1-kontext-pro` | Correct to `FLUX.1-Kontext-pro`, 5,000 input; this implementation remains generation-only. |
| `Cohere-rerank-v4.0-pro` | Keep exact model/type; gateway rerank dispatch is not connected (see limits). |
| `Cohere-rerank-v4.0-fast` | Keep exact model/type; same gateway limit. |
| `cohere-rerank-v3` | Remove ambiguous old alias; English/multilingual V3 retired June 30, 2025. |
| `cohere-rerank-v3.5` | Remove; retired May 14, 2026. |

The remaining static records do not duplicate deployment prices. In particular,
FLUX per-image amounts must not appear as input-token prices. The existing
`calculate_cost` path still uses the runtime pricing authority and preserves
missing-price errors. Six historical Azure AI decisions become `pricing_only`:
the two V3 rerank rows, Jamba Instruct, Mistral Large, Large 2407, and the unverified
Large latest alias. Historical price rows remain unchanged; the latter alias can
no longer regain routing capabilities via an explicit catalog mapping.

`EmbeddingRequest` contains text. Its HTTP handler now always selects
`/embeddings`, including customer deployment names containing `multimodal`.
Previously a substring heuristic incorrectly selected `/images/embeddings`.
The request's exact deployment/model string is preserved.

## Voyage: all 24 existing records retained

`F` means flexible dimensions 256/512/1024/2048; numbers in the dimension column
are fixed output dimensions. These are endpoint/model facts, not account access
results. Context and dimension records match the official guide; no runtime
Voyage model change is warranted.

| Exact ID | Surface | Context | Dimensions |
| --- | --- | ---: | --- |
| `voyage-4-large` | embeddings | 32,000 | F |
| `voyage-4` | embeddings | 32,000 | F |
| `voyage-4-lite` | embeddings | 32,000 | F |
| `voyage-code-4` | embeddings | 32,000 | F |
| `voyage-3-large` | embeddings | 32,000 | F |
| `voyage-3.5` | embeddings | 32,000 | F |
| `voyage-3.5-lite` | embeddings | 32,000 | F |
| `voyage-3` | embeddings | 32,000 | 1024 |
| `voyage-3-lite` | embeddings | 32,000 | 512 |
| `voyage-code-3` | embeddings | 32,000 | F |
| `voyage-finance-2` | embeddings | 32,000 | 1024 |
| `voyage-law-2` | embeddings | 16,000 | 1024 |
| `voyage-multilingual-2` | embeddings | 32,000 | 1024 |
| `voyage-large-2-instruct` | embeddings | 16,000 | 1024 |
| `voyage-large-2` | embeddings | 16,000 | 1536 |
| `voyage-2` | embeddings | 4,000 | 1024 |
| `rerank-3` | rerank | 32,000 | — |
| `rerank-3-lite` | rerank | 32,000 | — |
| `rerank-2.5` | rerank | 32,000 | — |
| `rerank-2.5-lite` | rerank | 32,000 | — |
| `rerank-2` | rerank | 16,000 | — |
| `rerank-2-lite` | rerank | 8,000 | — |
| `rerank-1` | rerank | 8,000 | — |
| `rerank-lite-1` | rerank | 4,000 | — |

Runtime Voyage prices match the public token rates: per million, 4-large/code-4
$0.12, 4 $0.06, 4-lite $0.02; older embedding models $0.02–$0.18; rerank full
$0.05 and lite $0.02. Rerank usage counts the query for each document. Free-account
credits and negotiated prices are not inferred. These checks do not promote
unrelated pricing-only/contextual/multimodal rows to callable text models.

## Scope limits and remaining coverage

- Azure AI's 35 other callable-authority identities and region/deployment-specific
  metadata require a separate audit. This batch covers the static 18, not all
  catalog-backed mapped models. In particular native Anthropic Messages and
  Responses-only Azure models must be checked against their actual endpoints.
- The public Azure rerank helper is separate from gateway rerank dispatch. Both
  V4 models are refused by the gateway's Rerank and Chat selections. No new
  rerank implementation is claimed. Historical helper defaults are not billing
  authority or evidence that retired models remain callable.
- FLUX model facts and route selection are verified here; Azure deployment URL,
  credentials and image generation protocol were not live-tested. This patch
  does not claim a new image gateway protocol.
- Voyage only implements float text embeddings and existing rerank. Contextual,
  multimodal, batch and files interfaces remain outside this increment.
- Earlier F10 batches: #1376 OpenAI/Azure retirement decisions, Cloudflare,
  Copilot and Bedrock; #1371 Bedrock model facts; #1385 Anthropic/Gemini surface;
  #1396 Mistral. #1408 covers DeepSeek/xAI/Cohere separately. Perplexity and
  GitHub Models are being handled by the root lane, not this PR.
- Remaining dedicated static review: Replicate, fal, BFL, Stability, audio
  catalogs, Nova, Meta Llama, v0 and non-Gemini Vertex paths. Some current model
  fields were checked in #1360; that is not a complete legacy catalog review.
  Generic passthrough providers and customer deployments are not counted by
  pricing-row totals.

## Validation

- Failing-before regression reproduced the obsolete Azure static record.
- Focused Azure provider, model, mapping and HTTP tests pass, including actual
  configured Router selection and rejection of historical Mistral mapping.
- HTTP mock confirms exact text embedding model/deployment names use the text
  endpoint and preserve returned usage; it is not a paid supplier call.
- Default complete `cargo test`: 7,142 library tests passed, one ignored; integration/doctests passed.
- Complete `cargo test --features gateway,sqlite,providers-extra`: 10,329 library tests passed, one ignored; integration/doctests passed, including existing Voyage HTTP retrieval tests.
- `cargo fmt --check`, `cargo check`, default and feature all-target clippy (`-D warnings`) passed.
- 51 pricing-sync Python tests and immutable-source `--check` passed. Only classification metadata changed in the price file; amounts are unchanged.

## Retrieved source digests

Microsoft Markdown responses were fetched on 2026-10-03 using the documented
`?accept=text/markdown` representation. Digests identify the retrieved bytes,
not a promise that these mutable pages will retain the same contents. Voyage
pages were read through the web tool; direct Markdown retrieval returned 403,
so no raw-body hash is claimed for those pages.

| Source | SHA-256 |
| --- | --- |
| `azure-retired` | `d13715ef814d1f166712ebfcd61ba17ca6f08cccd28e8afac5135723797528a0` |
| `azure-current` | `92c503a2389e7aec9cab002f49532302c4a92f21f31021c95b58cb5c7eb49c6e` |
| `azure-partners` | `301e7b422688689413f80513edb87ef2bb67e6335cf63445895bea2d9a32e6c4` |
| `azure-lifecycle` | `143c202b1c8c8c647c8b14e5ddef3acd78c428459ddbf511fba1120ed09d60ef` |
