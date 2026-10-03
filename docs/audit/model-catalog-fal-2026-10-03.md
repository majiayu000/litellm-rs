# fal static model review — 2026-10-03

Scope: all 13 entries in the fal static registry at `f62c5adb`. This is a
bounded part of issue #1373, not completion of the provider-wide audit. Model
availability and input schema evidence comes from fal itself. Pricing rows from
LiteLLM are retained; prices alone do not establish current model availability.

## Sources and decisions

For each model below, the linked model card and its `/api` page were inspected.
The machine-readable schema is available at
`https://fal.ai/api/openapi/queue/openapi.json?endpoint_id=<model-id>`.
Twelve returned an OpenAPI document. The Imagen preview schema returned HTTP 404
and its two model pages contained only the site navigation, with no endpoint
schema. This establishes missing current evidence, not a documented shutdown.

| Original ID / official card | Images per request | Pricing result / decision |
| --- | --- | --- |
| [fal-ai/flux/schnell](https://fal.ai/models/fal-ai/flux/schnell) | 1–4 | $0.003 per megapixel; remove fixed per-image price. |
| [fal-ai/flux-pro/v1.1](https://fal.ai/models/fal-ai/flux-pro/v1.1) | 1–4 | $0.04 per megapixel; remove old $0.05 per-image value. |
| [fal-ai/flux-pro/v1.1-ultra](https://fal.ai/models/fal-ai/flux-pro/v1.1-ultra) | 1–4 | Card says $0.06/image and also describes megapixel billing. Conservatively leave fixed price unset until this contradiction is resolved. Schema accepts aspect ratio, not pixel dimensions. |
| [fal-ai/stable-diffusion-v3-medium](https://fal.ai/models/fal-ai/stable-diffusion-v3-medium) | 1–4 | Retain $0.035/image. |
| [fal-ai/recraft/v3/text-to-image](https://fal.ai/models/fal-ai/recraft/v3/text-to-image) | 1; no batch parameter | Raster $0.04/image, vector $0.08/image. Remove unconditional fixed price. |
| [fal-ai/imagen4/preview](https://fal.ai/models/fal-ai/imagen4/preview) | Unverified | Remove static callable declaration; no automatic alias substitution. Historical unreviewed prices remain. |
| [fal-ai/ideogram/v3](https://fal.ai/models/fal-ai/ideogram/v3) | 1–8 | TURBO/BALANCED/QUALITY cost $0.03/$0.06/$0.09. Remove unconditional old $0.08 value. |
| [fal-ai/bria/text-to-image/hd](https://fal.ai/models/fal-ai/bria/text-to-image/hd) | 1–4; upstream defaults to 4 | Card says $0.04/generation, not $0.02/image. Leave fixed per-image price unset. Schema accepts aspect ratio. |
| [fal-ai/recraft/v4/text-to-image](https://fal.ai/models/fal-ai/recraft/v4/text-to-image) | 1; no batch parameter | Retain $0.04/image. |
| [fal-ai/recraft/v4/pro/text-to-image](https://fal.ai/models/fal-ai/recraft/v4/pro/text-to-image) | 1; no batch parameter | Retain $0.25/image. |
| [fal-ai/flux-2-pro](https://fal.ai/models/fal-ai/flux-2-pro) | 1; no batch parameter | First output megapixel $0.03, extra input/output megapixels $0.015; fixed price stays unset. |
| [fal-ai/flux-2-flex](https://fal.ai/models/fal-ai/flux-2-flex) | 1; no batch parameter | Input/output megapixel pricing; fixed price stays unset. Card contains inconsistent worked-example amounts, so no new formula is inferred. |
| [ideogram/v4](https://fal.ai/models/ideogram/v4) | 1–4 | Depends on megapixels, rendering mode and prompt expansion; fixed price stays unset. |

The 12 retained entries are text-to-image endpoints. `supports_multimodal` no
longer claims image input merely because the endpoint produces images. Upstream
Ideogram v4 streaming is documented, but the current provider does not implement
that protocol and continues not to advertise streaming.

## Runtime changes and limits

- Routing now consults fal's concrete registry, so removed or unknown model IDs
  cannot inherit family-level image capability. The direct image request and
  pricing methods return model-not-found for unknown IDs instead of invoking an
  unverified endpoint or returning zero cost.
- The existing request builder applies the audited batch bounds and omits
  `num_images` for single-image schemas. For BRIA, omission of OpenAI `n` now
  sends one explicitly instead of accepting the upstream default of four.
- Exact pixel dimensions are preserved as `image_size` objects. Ultra and BRIA
  reject a requested pixel size because their schemas only offer aspect ratios;
  this change does not silently approximate a requested resolution.
- Recraft schemas do not expose `output_format` or `sync_mode`; those global
  provider defaults are omitted. SD3, BRIA and Ideogram v3 also do not expose an
  output-format selector. The API retains the upstream format in these cases.
  `response_format=url` is supported; other response formats are explicitly
  rejected rather than claiming that the response contains base64.
- `calculate_cost(model, input_tokens, output_tokens)` can return a unit image
  price only for the three unconditionally fixed tariffs. It cannot calculate
  a request total, batch cost, megapixels, styles, rendering modes or actual
  provider charges from token counts. Those cases return not-supported; this
  is not a new gateway image-budget implementation.
- Additional quality/style controls, image editing, native aspect-ratio
  parameters, queue operations and streaming are outside this batch. The
  legacy generic parameter-mapping helper is not the runtime image builder.
- No paid model calls were made. Availability here means current official
  endpoint documentation, not verification with an entitled account.

## Captured schema evidence

SHA-256 digests identify the public OpenAPI response bodies fetched during this
review (the dynamic public pages may change later):

- `fal-ai_bria_text-to-image_hd`: `e029f36258d971b1580308ac50c029648dacce6494e6a41626434c3bac0a143c`
- `fal-ai_flux-2-flex`: `7d4480de9fbfc5a01ac781be3ab5be8081d9fcb720b4183ac4cf70cd4e3892d9`
- `fal-ai_flux-2-pro`: `cfa982b34d59574137ca72eb1535db93a55aeb81e4a3920ada04066d6c662813`
- `fal-ai_flux-pro_v1.1-ultra`: `8072f10f98faa4661feff7f0a027a025b0af5fe1e9522a53ed4a1454ddbf6f02`
- `fal-ai_flux-pro_v1.1`: `47dc484c10ba7215160f07041429f2aa58e20541aed5359ccc0a90e3d1993209`
- `fal-ai_flux_schnell`: `d3429f0e54220b66d3664e6df99e8e6e2ffda3a4335cf8ff0e78888674e25909`
- `fal-ai_ideogram_v3`: `b7dd4de1fbe3071b527f1e5d481bc3045aa8caf7beb47f1cab414e89347482f1`
- `fal-ai_recraft_v3_text-to-image`: `2926209110be5028ce3c3721efb2f24b69275e50f0c4fee4a54b3f29f77610a1`
- `fal-ai_recraft_v4_pro_text-to-image`: `f86078773badd1094f4baa9cbe35e1a52768df9562430e420086a77da87e090b`
- `fal-ai_recraft_v4_text-to-image`: `dad269360970da7d997cc3ce839245e55838901c981c1bee627804f558213ae1`
- `fal-ai_stable-diffusion-v3-medium`: `3051f1043cdbe2b4c5e8918c8124377d07f9b709babbafb0783604d84e4a371d`
- `ideogram_v4`: `50997cd08a6de57db7083e4842665efec1091c71371d2b6dd524962583f98c9c`

## Verification

- `cargo fmt --check`, `cargo check`, default full `cargo test` (7,142 library
  tests passed, one ignored; integration and doc suites also passed), and
  `cargo clippy --all-targets -- -D warnings` passed.
- `cargo test --features gateway,sqlite,providers-extended` passed (10,058
  library tests, one ignored; all integration and doc suites passed), as did
  the same feature profile's all-target clippy with warnings denied.
- 53 focused fal tests passed, including mock HTTP schema transmission,
  exact pixel preservation, one-image BRIA default, single-image rejection,
  model-specific routing, nonzero/unknown pricing, and unchanged upstream 403.
- No live generation, billing invoice reconciliation, or entitlement test ran.
