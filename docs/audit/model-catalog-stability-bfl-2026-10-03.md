# Stability and Black Forest Labs static review — 2026-10-03

Scope: the 10 Stability entries (nine generation, one editing) and 12 BFL entries
at `7919b463`. This is another bounded batch of issue #1373. Supplier availability
and schemas are audited here; no paid inference or invoice reconciliation ran.

## Stability: identify the model that actually runs

The [official API reference](https://platform.stability.ai/docs/api-reference)
and its [public OpenAPI source](https://api.stability.ai/v2alpha/openapi) say that
since April 17, 2025 the SD3 Large, Large Turbo and Medium model selectors have
been rerouted to their SD3.5 equivalents. Those are no longer independent SD3
models. The runtime catalog now exposes only the actual current identities;
it does not silently substitute a new model for a requested old one.

| Previous entry | Current decision |
| --- | --- |
| `stable-image-core` | Retain native `/v2beta/stable-image/generate/core`. |
| `stable-image-ultra` | Retain native `/v2beta/stable-image/generate/ultra`. |
| `sd3` | Remove this local alias to `sd3.5-large`. |
| `sd3-large` | Remove the old identity, replaced upstream by `sd3.5-large`. |
| `sd3-large-turbo` | Remove the old identity, replaced upstream by `sd3.5-large-turbo`. |
| `sd3-medium` | Remove the old identity, replaced upstream by `sd3.5-medium`. |
| `sd3.5-large` | Retain; send this exact model selector to `/v2beta/stable-image/generate/sd3`. |
| `sd3.5-large-turbo` | Retain; same endpoint with the exact selector. |
| `sd3.5-medium` | Retain; same endpoint with the exact selector. |
| `inpaint` | Retain editing-only `/v2beta/stable-image/edit/inpaint`. |

This is a model identity correction, not a claim that the upstream rejects the
old selectors. The gateway rejects them before networking with model-not-found.
The three SD3.5 selectors are verified by both prose and the current schema enum.
Flash appears in the documentation prose but is missing from that enum; adding
new models is outside this batch, and that inconsistency is not papered over.

The existing runtime generation request contains text, while only `inpaint`
accepts image input through the implemented interface. The model metadata now
reflects that distinction. Upstream image-to-image modes for other models are
not claimed as implemented. Token-based cost calculation already returns an
explicit not-supported error; no zero-price or new per-image accounting claim
is introduced. Historical catalog pricing is unchanged.

## BFL: all existing endpoint identities remain published

The [official endpoint list](https://docs.bfl.ai/quick_start/generating_images)
and [live public OpenAPI](https://api.bfl.ai/openapi.json) contain all 12 existing
IDs. Each was checked against its request schema:

| Runtime ID | Upstream dimensions / image inputs |
| --- | --- |
| `flux-2-max` | Width/height; input images. |
| `flux-2-pro-preview` | Width/height; input images. |
| `flux-2-pro` | Width/height; input images. |
| `flux-2-flex` | Width/height; input images. |
| `flux-2-klein-4b` | Width/height; input images. |
| `flux-2-klein-9b-preview` | Width/height; input images. |
| `flux-2-klein-9b` | Width/height; input images. |
| `flux-pro-1.1` | Width/height; optional image prompt. |
| `flux-pro-1.1-ultra` | Aspect ratio; optional image prompt. |
| `flux-dev` | Width/height; optional image prompt. |
| `flux-kontext-pro` | Aspect ratio; input images. |
| `flux-kontext-max` | Aspect ratio; input images. |

No BFL runtime ID is removed or added. The gateway currently exposes generic
image editing only for the two Kontext models; other upstream input-image
fields do not automatically acquire the `ImageEdit` capability. The native BFL
request surface and OpenAI-compatible generation are distinct interfaces.
Existing size restrictions and explicit unsupported token pricing remain in
place. Newer supplier endpoints outside this static list are not automatically
promoted from either this evidence or the price catalog.

## Evidence identity

The following SHA-256 digests identify the fetched public specification bodies:

- [stability OpenAPI](https://api.stability.ai/v2alpha/openapi): `1ea5455da5ce2efef103daa142cc44a3e70b4fb3cfcdfa0c9a6a6f46d93912ea`.
- [bfl OpenAPI](https://api.bfl.ai/openapi.json): `8b1a6df6e54b43578582f516b8ddae281807f7768f702a0529b9e44e9deb152f`.

## Verification

- 16 focused Stability HTTP/routing tests passed, including three actual SD3.5
  multipart requests and rejection of the four replaced identities before I/O.
- `cargo fmt --check`, `cargo check`, full default `cargo test` (7,134 library
  tests passed, one ignored; integration/docs passed), and default all-target
  clippy with warnings denied passed.
- Full `cargo test --features gateway,sqlite,providers-extended` (10,048 library
  tests passed, one ignored; all integration/docs passed), and matching
  all-target clippy with warnings denied passed.
- The full feature run includes existing BFL generation/edit transport tests.
  This is mock-upstream validation, not live supplier-account verification.
