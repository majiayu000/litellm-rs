# Provider Macro Status

This directory still contains crate-root `#[macro_export]` helpers. F4 keeps
the legacy exports for compatibility, but the recommended implementation path is
provider registry/catalog wiring plus normal Rust modules.

## Active In-Tree Macros

- `define_http_provider_with_hooks!`
  - Used by `custom_api` and `deepl`.
  - Keep until those providers are converted to explicit modules.
- `define_pooled_http_provider_with_hooks!`
  - Used by `ai21`, `amazon_nova`, and `datarobot`.
  - Keep until those providers are converted to explicit modules.

## Compatibility-Only Macros

These exports have no non-macro in-tree call sites as of F4. They remain
compiled so downstream users are not broken by this cleanup sweep:

- `impl_provider_basics!`
- `impl_error_conversion!`
- `provider_config!`
- `impl_health_check!`
- `build_request!`
- `not_implemented!`
- `model_list!`
- `impl_streaming!`
- `validate_response!`
- `with_retry!`
- `extract_usage!`
- `require_config!`
- `standard_provider!`
- `define_openai_compatible_provider!`

New provider code should not introduce additional call sites for compatibility
macros; use a concrete provider module or the active hook macros only when the
existing hook shape already fits.
