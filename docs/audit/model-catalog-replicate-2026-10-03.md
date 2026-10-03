# Replicate static model review — 2026-10-03

This bounded review covers all 16 static models at `f62c5adb`, under issue #1373.
It does not infer callable models from the 40 unreviewed Replicate pricing rows.
The official public model pages embed the input OpenAPI schema, model metadata
and (for official models) their billing configuration; these were inspected
alongside the human-readable documentation. No paid generation ran.

## Per-model findings

| Previous static ID / official source | Decision |
| --- | --- |
| [meta/llama-2-70b-chat](https://replicate.com/meta/llama-2-70b-chat) | Retain text/chat and streaming; 4,096 context; $0.65 input / $2.75 output per million tokens. |
| [meta/llama-2-13b-chat](https://replicate.com/meta/llama-2-13b-chat) | Retain text/chat and streaming; 4,096 context; $0.10 / $0.50 per million tokens. |
| [meta/llama-2-7b-chat](https://replicate.com/meta/llama-2-7b-chat) | Retain text/chat and streaming; 4,096 context; $0.05 / $0.25 per million tokens. |
| [meta/meta-llama-3-70b-instruct](https://replicate.com/meta/meta-llama-3-70b-instruct) | Retain text/chat and streaming; 8,192 context; $0.65 / $2.75 per million tokens. |
| [meta/meta-llama-3-8b-instruct](https://replicate.com/meta/meta-llama-3-8b-instruct) | Retain text/chat and streaming; 8,192 context; $0.05 / $0.25 per million tokens. |
| [meta/meta-llama-3.1-405b-instruct](https://replicate.com/meta/meta-llama-3.1-405b-instruct) | Current model and `/api` pages return 404. Remove unverified static callable declaration; this is not a claimed supplier shutdown date. |
| [mistralai/mistral-7b-instruct-v0.2](https://replicate.com/mistralai/mistral-7b-instruct-v0.2) | Same missing current model/API evidence; remove static callable declaration. |
| [mistralai/mixtral-8x7b-instruct-v0.1](https://replicate.com/mistralai/mixtral-8x7b-instruct-v0.1) | Same missing current model/API evidence; remove static callable declaration. |
| [black-forest-labs/flux-2-pro](https://replicate.com/black-forest-labs/flux-2-pro) | Retain; pricing includes a run fee plus input/output megapixels. Explicit width/height require `aspect_ratio=custom`. No batch parameter. |
| [stability-ai/sdxl](https://replicate.com/stability-ai/sdxl) | Retain community model at its reviewed immutable version; hardware-time pricing, not fixed $0.003/image. |
| [stability-ai/stable-diffusion](https://replicate.com/stability-ai/stable-diffusion) | Retain community model at its reviewed immutable version; hardware-time pricing, not fixed $0.002/image. |
| [black-forest-labs/flux-schnell](https://replicate.com/black-forest-labs/flux-schnell) | Retain $0.003/output image metadata. Accepts aspect ratios, not explicit pixel dimensions. |
| [black-forest-labs/flux-dev](https://replicate.com/black-forest-labs/flux-dev) | Retain $0.025/output image metadata. Accepts aspect ratios, not explicit pixel dimensions. |
| [black-forest-labs/flux-pro](https://replicate.com/black-forest-labs/flux-pro) | Deprecated, but current API schema and billing remain published; retain, correct unit price to $0.055/output image. Explicit dimensions require `aspect_ratio=custom`; no batch parameter. Deprecation is not treated as retirement. |
| [bytedance/sdxl-lightning-4step](https://replicate.com/bytedance/sdxl-lightning-4step) | Retain community model at reviewed immutable version; hardware-time pricing, not fixed $0.002/image. |
| [lucataco/playground-v2.5-1024px-aesthetic](https://replicate.com/lucataco/playground-v2.5-1024px-aesthetic) | Official redirect identifies `playgroundai/playground-v2.5-1024px-aesthetic`; use that canonical owner. Hardware-time pricing, not fixed $0.004/image. |

The five retained Llama schemas publish a minimum and default for `max_tokens`,
but no independent 2,048/4,096 output ceiling. Those invented output ceilings
are removed (`None`); the model context limits remain. Streaming was already
implemented and documented upstream, so the per-model capability list now
agrees with its existing streaming flag. No tool or vision capability is added.

## Actual request paths

Replicate's [HTTP reference](https://replicate.com/docs/reference/http) separates
`POST /models/{owner}/{name}/predictions` for official models from
`POST /predictions` with a concrete version for community models. Existing
configuration/request structures now use the audited community version metadata
and correct endpoint. Explicit version syntax also selects the version endpoint;
this does not grant an unknown version or deployment model routing capabilities.

The four pinned community versions are recorded below. They are evidence for
these specific catalog entries, not a new dynamic discovery or version-management
system. Updating a model version requires reviewing its protocol again.

Image/chat routing now consults the concrete model capability list, and direct
calls reject unknown models rather than assuming every unknown string is a chat
model. The image path also rejects text models. Private deployments and arbitrary
user model/version identities are not verified by this static audit.

Image requests preserve the documented single-image versus 1–4 batch boundary.
FLUX Pro and FLUX.2 Pro set `aspect_ratio=custom` when pixel sizes are supplied;
Schnell/Dev reject explicit pixel sizes instead of sending ignored fields. Default
Schnell/Dev requests no longer contain invented width/height. Other dimension
constraints remain upstream-validated, with upstream errors preserved. OpenAI
quality/style mapping is only implemented for SDXL; other models reject these
options. The existing URL response path rejects unsupported base64 response
requests. The public `transform_image_request` helper now returns `Result` so it
can report those errors.

## Pricing and remaining limits

`calculate_cost` retains exact reviewed token rates for the five Llama entries.
For images, token counts cannot establish output count, megapixels or runtime;
the method now returns not-supported instead of zero. Unknown models return
model-not-found instead of an invented token estimate. Fixed image unit prices
remain metadata only, and variable hardware-time prices have no fake fixed value.
Historical price records are unchanged.

This batch does not implement a new image budget engine, supplier invoice
reconciliation, private deployment discovery, complete Llama conversation-template
translation, or provider-reported token usage. Those existing protocol/accounting
limits must not be described as validated by this catalog review. Mock HTTP
proves endpoint and payload dispatch, not supplier entitlement or live billing.

## Captured public evidence

Model-page body SHA-256 and selected schema version (public pages are mutable):

- `black-forest-labs_flux-2-pro`: version `72b135090ab01dda1d8c6092b3ff92167bb44bf8c0d85b02e237b8f0bda70332`; HTML SHA-256 `48b903a1a1bec498ff2549ce615f88fa92effb2d61c8e1995163940a1e674972`.
- `black-forest-labs_flux-dev`: version `2b733d2ea9adc75354e259635372c030fb9ab7e54bc7af8f740d84845d448a01`; HTML SHA-256 `a25a33a664fd40df278ddf8a3e36275a4ecd6fd0caa95ae7017bbf359ff84d0c`.
- `black-forest-labs_flux-pro`: version `e25ddfd549bcf36deb4ca206110a8226eb8133fd0a61db1f58f61da238058b44`; HTML SHA-256 `504d20bf9111147a5c8d38fd5502d6b7184ad1cd1c5480b41c85be5850842c5b`.
- `black-forest-labs_flux-schnell`: version `d9fbf23e0a34bb6134f86d3e24f48c0dd96332378da307b5e9b60a9d47e4a9c2`; HTML SHA-256 `b372e6222698caa4906e4551ccc4fce1ddc96c5bbad645ada72f1b167532dade`.
- `bytedance_sdxl-lightning-4step`: version `6f7a773af6fc3e8de9d5a3c00be77c17308914bf67772726aff83496ba1e3bbe`; HTML SHA-256 `c86e67a3d27d4f345103dc349bdf7d7b481426494223627f7ce145bbdbcb91a8`.
- `lucataco_playground-v2.5-1024px-aesthetic`: version `a45f82a1382bed5c7aeb861dac7c7d191b0fdf74d8d57c4a0e6ed7d4d0bf7d24`; HTML SHA-256 `2db3cf5e33ba6f33e635b267eb800babf28482da22dd01f7d30c10cdf5b4a30e`.
- `meta_llama-2-13b-chat`: version `e7c2a3a1dcbd54a50673044f22cdc45e3b3acb1088716f5d8c48d0ee258ba19c`; HTML SHA-256 `87b581100e20b859810217e643d84ebe72fee108cae2b0ded9780fab3f65f358`.
- `meta_llama-2-70b-chat`: version `a2814fa5c8f04cf965ffad6f03532c151cd1fab6311141b72fc6f17e1ced72bf`; HTML SHA-256 `ecb5f551801c234f97f482f435266b3f6ab10fc21ee3171002bbb8e05883b890`.
- `meta_llama-2-7b-chat`: version `e2e43263a779f39655fec4f19f56a00cb7e8d5f43c1c0bde011728d7272a244e`; HTML SHA-256 `e785d82feee9b6c3c23d3de79edf6401d5bd4ba6e152cd968ee59ab48201f833`.
- `meta_meta-llama-3-70b-instruct`: version `2ce90869e475b267cdbb7f2b74b00851c0f3307edda4bde88eb88085832d891e`; HTML SHA-256 `bf928f4081ed7efdd4170bddc825f57afd25a496840188e693877d3d9649e522`.
- `meta_meta-llama-3-8b-instruct`: version `a69af66887855969a57f8373241b2e0f73552cd94dd6352b8e882e6fac393ec9`; HTML SHA-256 `9cb276a883bb182818e1e6ae90a6439f2e6cf8f0e52d6c4754787b2260916541`.
- `stability-ai_sdxl`: version `7762fd07cf82c948538e41f63f77d685e02b063e37e496e96eefd46c929f9bdc`; HTML SHA-256 `b7e64337117de57edffffdfd220336342a1da7606c94986ba7959f48258ba2fe`.
- `stability-ai_stable-diffusion`: version `ac732df83cea7fff18b8472768c88ad041fa750ff7682a21affe81863cbe77e4`; HTML SHA-256 `05bff4b2a00fc47c142ae0e51d3b0ef2e0863b32f39299dcd3ce3978107934b7`.

## Verification

- 113 focused Replicate tests passed, including a real mock HTTP prediction
  POST/poll sequence for both a pinned community model and an official model.
- `cargo fmt --check`, `cargo check`, default full `cargo test` (7,142 library
  tests passed, one ignored; integration/docs passed), and all-target clippy
  with warnings denied passed.
- Full `cargo test --features gateway,sqlite,providers-extended` passed (10,058
  library tests, one ignored; all integration/docs passed), and the same
  feature profile's all-target clippy with warnings denied passed.
- Negative coverage includes model/modality routing, unknown model and image
  pricing errors, single-image limits and unsupported pixel-size arguments.
