# DeepSeek Provider

Checked against the [official API quickstart](https://api-docs.deepseek.com/)
and [model/pricing contract](https://api-docs.deepseek.com/quick_start/pricing/)
on 2026-10-03. LiteLLM-RS uses the OpenAI-compatible chat transport at
`https://api.deepseek.com`; supplier support for Responses and Anthropic APIs
does not mean this provider has those native gateway transports.

## Current native models

| Request model | Served model | Context / maximum output | Vision | Tools / thinking |
| --- | --- | --- | --- | --- |
| `deepseek-flash` | V4.1 Flash | 1M / 384K | Yes | Yes; thinking enabled by default, can be disabled |
| `deepseek-v4-flash` | V4.1 Flash redirect | 1M / 384K | Yes | Same as Flash |
| `deepseek-v4-flash-vision-exp` | V4.1 Flash redirect | 1M / 384K | Yes | Same as Flash |
| `deepseek-v4-pro` | V4 Pro 0813 | 1M / 384K | No | Yes; thinking enabled by default, can be disabled |

The old V4 Flash model versions are retired, but their two request names remain
accepted and billed as current Flash. In contrast, `deepseek-chat` and
`deepseek-reasoner` were discontinued on **2026-07-24**, according to the
[official change log](https://api-docs.deepseek.com/updates/). Their retained
historical pricing records do not establish current availability. Use
`deepseek-flash` for new code. R1 helpers also support historical or third-party
response formats; they are not a list of callable native DeepSeek models.

## Chat configuration

Set `DEEPSEEK_API_KEY` for the catalog provider, or configure the gateway's
existing provider list:

```yaml
providers:
  - name: deepseek
    provider_type: deepseek
    api_key: "${DEEPSEEK_API_KEY}"
    api_base: "https://api.deepseek.com"
    models: [deepseek-flash, deepseek-v4-pro]
```

Send `model: "deepseek-flash"` to `/v1/chat/completions` with the normal gateway
API-key authentication. Streaming and function tools use the existing
OpenAI-compatible chat path. This review did not perform account-backed calls.

## Cost notes

Prices below are USD per million tokens. The catalog stores off-peak base rates
and the runtime applies the configured UTC peak windows.

| Model | Off-peak input / cached input / output | Peak input / cached input / output |
| --- | --- | --- |
| Flash and its two accepted redirects | 0.15 / 0.003 / 0.60 | 0.30 / 0.006 / 1.20 |
| V4 Pro | 0.66 / 0.022 / 1.98 | 1.32 / 0.044 / 3.96 |

Official peak windows are 01:00–04:00 and 06:00–10:00 UTC on weekdays, excluding
Chinese public holidays. The current runtime implements weekly windows but
**does not yet encode the holiday exception**, so those holidays may be
estimated at peak rates. Historical aliases retain historical prices. Neither
historical nor unreviewed pricing entries automatically become callable models.

See the [three-provider audit](../audit/model-catalog-deepseek-xai-cohere-2026-10-03.md)
for per-entry evidence and remaining limitations.
