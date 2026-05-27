# OpenAI-Compatible Bedrock Proxy

Use `provider_type: "openai_compatible"` when LiteLLM-RS talks to Amazon
Bedrock through an OpenAI-compatible proxy such as
[`aws-samples/bedrock-access-gateway`](https://github.com/aws-samples/bedrock-access-gateway).
The proxy fronts Bedrock with an OpenAI-shaped REST surface
(`/v1/chat/completions`), accepts a bearer token, and handles AWS SigV4 on the
gateway side.

**Do not** use `provider_type: "bedrock"` for these deployments. The native
provider expects an AWS region and SigV4 credentials; a proxy expects an
HTTPS base URL and an API key. Mixing the two will either drop credentials or
double-sign requests.

For the native AWS Bedrock runtime path (direct SigV4), read
[`bedrock.md`](./bedrock.md) instead.

## When to use this path

Pick the proxy when **any** of the following are true:

- You already operate a Bedrock Access Gateway (or similar converter) in your
  AWS account and want LiteLLM-RS to share that auth surface.
- You want a single OpenAI-shaped endpoint for every downstream caller and
  do not want to expose AWS credentials to LiteLLM-RS.
- The clients calling LiteLLM-RS only know how to speak the OpenAI
  `chat/completions` schema, and your proxy handles tool / image / streaming
  translation.
- AWS credentials are not available where LiteLLM-RS runs (different VPC,
  different account, on-prem reverse proxy in front of Bedrock).

Pick [native Bedrock](./bedrock.md) when LiteLLM-RS itself has AWS access key
credentials and a region, or when you need inference profile IDs and ARNs
preserved exactly as AWS execution `modelId` values.

## About Bedrock Access Gateway

[Bedrock Access Gateway](https://github.com/aws-samples/bedrock-access-gateway)
is an AWS-published sample that exposes Bedrock via an OpenAI-compatible
HTTP API. It accepts standard `chat/completions`, `embeddings`, and related
requests, signs the outbound call to Bedrock with SigV4 using its own IAM
role, and returns OpenAI-shaped responses. From LiteLLM-RS's perspective it
is just another `openai_compatible` provider — the gateway, not LiteLLM-RS,
owns AWS authentication and model-name translation.

Other proxies that follow the same shape (custom in-house gateways,
LiteLLM Python deployments fronting Bedrock, etc.) can be configured the
same way.

## Gateway configuration

```yaml
providers:
  - name: "bedrock-access-gateway"
    provider_type: "openai_compatible"
    api_key: "${BEDROCK_ACCESS_GATEWAY_API_KEY}"
    base_url: "https://bedrock-access-gateway.example.com/api/v1"
    timeout: 60
    max_retries: 3
    settings:
      provider_name: "bedrock-access-gateway"
    models:
      - "us.anthropic.claude-3-5-sonnet-20241022-v2:0"
      - "anthropic.claude-3-5-sonnet-20241022-v2:0"
      - "amazon.nova-pro-v1:0"
    enabled: true
```

Notes:

- `api_key` is the proxy's bearer token, **not** an AWS access key.
- `base_url` must include the `/v1` (or whatever path) prefix the proxy uses.
  LiteLLM-RS will append `/chat/completions`, `/embeddings`, etc.
- The proxy owns the actual AWS credentials; LiteLLM-RS only needs HTTPS
  reachability to `base_url`.

## Model IDs

Model ID semantics are defined by the proxy, not by LiteLLM-RS or the native
Bedrock provider. Two common conventions:

1. **Pass-through Bedrock model IDs** — Bedrock Access Gateway forwards the
   `model` field directly to AWS. Use canonical foundation IDs
   (`anthropic.claude-3-5-sonnet-20241022-v2:0`) or geo inference profile IDs
   (`us.anthropic.claude-3-5-sonnet-20241022-v2:0`) as the proxy documents.
2. **OpenAI-shaped aliases** — some proxies expose `gpt-4`-style aliases that
   they translate to Bedrock model IDs internally. Use whatever alias the
   proxy expects.

Whichever convention the proxy uses, do **not** add the `bedrock/` prefix
here. That prefix is reserved for the native provider and triggers SigV4
signing.

```rust
use litellm_rs::{completion, user_message};

let messages = vec![user_message("Summarize the launch process.")];

// Pass-through ID — proxy forwards directly to Bedrock
completion(
    "us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    messages,
    None,
).await?;
```

## Example chat request

```bash
curl -sS http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer $LITELLM_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    "messages": [
      {"role": "system", "content": "You are a release engineer."},
      {"role": "user", "content": "Generate a 3-step rollback plan."}
    ],
    "max_tokens": 512,
    "temperature": 0.2,
    "stream": false
  }'
```

The request shape is exactly the standard OpenAI `chat/completions` schema;
LiteLLM-RS forwards it to `${base_url}/chat/completions` with the proxy's
bearer token. Streaming (`"stream": true`) works the same way: the proxy
emits SSE chunks and LiteLLM-RS relays them unchanged.

## Trade-offs vs native

| Concern                   | Native (`bedrock`)                | Proxy (`openai_compatible`)             |
|---------------------------|-----------------------------------|------------------------------------------|
| AWS credentials in LiteLLM| Access key/secret required        | Not required                             |
| Extra network hop         | No                                | Yes (through the gateway)                |
| Inference-profile IDs     | Preserved verbatim                | Whatever the proxy preserves             |
| Bedrock-specific features | Full surface                      | Whatever the proxy exposes               |
| Auth model                | AWS SigV4                         | Bearer token (proxy-defined)             |
| Best for                  | Direct AWS execution, low latency | Shared OpenAI surface, no AWS creds here |

See [`bedrock.md`](./bedrock.md) for the native counterpart and
[`../plan/bedrock-native-routing-and-model-catalog-plan.md`](../plan/bedrock-native-routing-and-model-catalog-plan.md)
for the broader split rationale.
