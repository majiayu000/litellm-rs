# AWS Bedrock Provider (Native)

LiteLLM-RS routes `provider_type: "bedrock"` through the native AWS Bedrock
runtime provider. Requests are signed with AWS SigV4 and sent directly to
`bedrock-runtime.<region>.amazonaws.com`. Model IDs, including geo and global
inference profiles, are preserved verbatim and never normalized into an
OpenAI-style alias.

Use this page when you want LiteLLM-RS to call AWS Bedrock Runtime directly
(`Converse`, `ConverseStream`, `InvokeModel`, `InvokeModelWithResponseStream`).
If you instead front Bedrock with an OpenAI-compatible proxy such as
[Bedrock Access Gateway](https://github.com/aws-samples/bedrock-access-gateway),
read [`openai-compatible-bedrock-proxy.md`](./openai-compatible-bedrock-proxy.md)
instead.

For the longer-term routing design (`bedrock-runtime`, `bedrock-mantle`,
proxy split, inference-profile policy) see
[`../plan/bedrock-native-routing-and-model-catalog-plan.md`](../plan/bedrock-native-routing-and-model-catalog-plan.md).

## When to use the native provider

Pick native Bedrock when **any** of the following apply:

- You can give the process AWS access key credentials through config or
  environment variables.
- You need to invoke geo (`us.`, `eu.`, `apac.`) or global (`global.`)
  inference profile IDs and ARNs with the original execution `modelId`.
- You want the lowest latency path (no extra proxy hop, no extra hostname).
- You want Bedrock-specific behavior such as Converse streaming tool calls,
  guardrails, and the full Bedrock feature surface.

Pick the OpenAI-compatible proxy instead when you already operate a Bedrock
Access Gateway, want to share an OpenAI-shaped REST surface, or do not want
AWS access keys available where LiteLLM-RS runs.

## Authentication

Native Bedrock signs requests with AWS SigV4. `aws_access_key_id` and
`aws_secret_access_key` are required through config or environment variables.
`aws_session_token` is optional for temporary credentials. `aws_region` is
optional and defaults to `us-east-1` when neither `AWS_REGION` nor
`AWS_DEFAULT_REGION` is set.

### Environment variables

```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_SESSION_TOKEN="..."        # optional, for short-lived credentials
export AWS_REGION="us-east-1"         # AWS_DEFAULT_REGION also accepted
```

## Gateway configuration

The generic `ProviderConfig.api_key` field is unused by native Bedrock. Put
AWS-specific values under `settings`, or provide the same values through
environment variables.

### Explicit settings

```yaml
providers:
  - name: "bedrock-native"
    provider_type: "bedrock"
    api_key: ""
    timeout: 60
    max_retries: 3
    settings:
      aws_region: "us-east-1"
      aws_access_key_id: "${AWS_ACCESS_KEY_ID}"
      aws_secret_access_key: "${AWS_SECRET_ACCESS_KEY}"
      aws_session_token: "${AWS_SESSION_TOKEN}"
    models:
      - "us.anthropic.claude-3-5-sonnet-20241022-v2:0"
      - "anthropic.claude-3-5-sonnet-20241022-v2:0"
    enabled: true
```

### Setting aliases

The factory accepts these spellings inside `settings`:

| Field          | Aliases                                                       |
|----------------|---------------------------------------------------------------|
| Region         | `aws_region`, `aws_region_name`, `region`                     |
| Access key     | `aws_access_key_id`, `aws_access_key`, `access_key`           |
| Secret key     | `aws_secret_access_key`, `aws_secret_key`, `secret_key`       |
| Session token  | `aws_session_token`, `session_token`                          |

## Model IDs

The native provider must use the `bedrock/` prefix or a canonical Bedrock
model ID. The prefix is only a LiteLLM-RS provider selector and is stripped
before SigV4 signing; the remainder is sent to AWS exactly as written.

```rust
use litellm_rs::{completion, user_message};

let messages = vec![user_message("Summarize the launch process.")];

// Foundation model ID
completion(
    "bedrock/anthropic.claude-3-5-sonnet-20241022-v2:0",
    messages.clone(),
    None,
).await?;

// US geo inference profile (preserved verbatim)
completion(
    "bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    messages.clone(),
    None,
).await?;

// Global inference profile (preserved verbatim)
completion(
    "bedrock/global.anthropic.claude-opus-4-20250514-v1:0",
    messages.clone(),
    None,
).await?;

// Inference profile ARN (preserved verbatim)
completion(
    "bedrock/arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    messages,
    None,
).await?;
```

### Critical: inference-profile prefixes are preserved

Geo (`us.`, `eu.`, `apac.`), global (`global.`), region-prefixed
(`us-east-1.`), and ARN model IDs are sent to AWS **without normalization**.
Stripping the prefix would route the request to the wrong inference profile
and silently change billing, region, or routing tier.

Metadata lookup (capabilities, pricing, context window) may fall back to the
canonical foundation model ID, but the fallback ID is **not** sent to AWS.

## Example chat request

```bash
curl -sS http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer $LITELLM_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "bedrock/us.anthropic.claude-3-5-sonnet-20241022-v2:0",
    "messages": [
      {"role": "system", "content": "You are a release engineer."},
      {"role": "user", "content": "Generate a 3-step rollback plan."}
    ],
    "max_tokens": 512,
    "temperature": 0.2,
    "stream": false
  }'
```

Streaming uses the same endpoint with `"stream": true` and emits SSE chunks
translated from Bedrock `ConverseStream` events, including tool-call deltas.

## Current limits

Native runtime wiring and safe inference-profile handling are implemented.
Catalog convergence, stricter model-specific parameter policies, the
`bedrock-mantle` Anthropic Messages endpoint mode, and optional live AWS
smoke tests are tracked separately and not part of this provider's stable
surface yet — see the
[long-term plan](../plan/bedrock-native-routing-and-model-catalog-plan.md).
