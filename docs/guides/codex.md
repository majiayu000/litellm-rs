# Codex through LiteLLM-RS

LiteLLM-RS exposes the Responses API endpoint Codex expects at
`POST /v1/responses`. The gateway translates portable function calls and Codex
custom/freeform tools to each provider's normal tool-calling protocol while
preserving response item and call IDs across turns.

## Start the gateway

Configure at least one tool-capable model in `config/gateway.yaml`, then start
the server:

```bash
cargo run --bin gateway
```

The development example listens on `http://127.0.0.1:8080`. Production
deployments should enable authentication and provide the API key through an
environment variable.

## Configure Codex

Add a provider to the user-level `~/.codex/config.toml`. This is a manual step;
LiteLLM-RS never edits Codex configuration.

```toml
model = "your-gateway-model"
model_provider = "litellm_rs"

[model_providers.litellm_rs]
name = "LiteLLM-RS"
base_url = "http://127.0.0.1:8080/v1"
env_key = "LITELLM_RS_API_KEY"
wire_api = "responses"
```

Set `LITELLM_RS_API_KEY` in the environment that launches Codex. Do not put the
key directly in `config.toml`.

## Smoke test

First verify the gateway independently of Codex:

```bash
curl --fail-with-body http://127.0.0.1:8080/v1/responses \
  -H "Authorization: Bearer ${LITELLM_RS_API_KEY}" \
  -H "Content-Type: application/json" \
  -d '{"model":"your-gateway-model","input":"Reply with OK","store":false}'
```

Then run Codex with the configured provider:

```bash
codex exec "Reply with OK"
```

## Compatibility contract

| Codex feature | Gateway behavior |
| --- | --- |
| Messages and text | Forwarded through the selected provider |
| Function calls and outputs | Converted to provider tool calls with `call_id` preserved |
| Custom/freeform tools | Reversibly wrapped as a string-valued `input` function argument |
| OpenAI-compatible, Anthropic, Gemini | Supported when the configured model advertises tool calling |
| Streaming | Emits output-item added/delta/done events and one terminal response event |
| Stored multi-turn responses | Replays prior function/custom calls before correlating tool outputs |

Tier 2 Codex extensions are deliberately fail-closed: namespaces, tool search,
additional tools, local shell calls, compaction items, hosted web search, MCP,
and computer use return `unsupported_codex_feature` before an upstream request.
The gateway does not execute tools; Codex remains responsible for running them
and sending their outputs on the next turn.

`strict=true` and deferred function loading are also rejected because the
current chat-tool representation cannot preserve those semantics. Use a model
and provider with ordinary function/tool-calling support for this compatibility
path.
