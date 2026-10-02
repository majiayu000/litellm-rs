# Cloudflare Workers AI

Configure `CloudflareConfig` with your account ID and API token (or set
`CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_API_TOKEN`). `api_base` /
`CLOUDFLARE_API_BASE` is the Cloudflare API root, normally
`https://api.cloudflare.com/client/v4`.

Chat uses `/accounts/{account_id}/ai/v1/chat/completions`. Both unary and SSE
streaming paths preserve standard messages, image URL parts, tool calls,
reasoning content, finish reasons and token usage. Model IDs may have the
`cloudflare/` prefix; the provider removes it before sending the request.

Use `reasoning_effort` or provider-native fields in `extra_params` for reasoning
configuration. Generic `ThinkingConfig` is rejected explicitly. For example,
`options.rejectIfBusy` is forwarded without replacing the canonical model or
messages. HTTP errors retain their status and rate-limit retry delay. Invalid
responses and stream errors return provider errors.

The eight current model entries (DeepSeek V4 Flash/Pro, Gemma 4, GLM 5.3/Flash,
GPT-OSS 120B, Kimi K2.7 Code and Qwen3 30B) advertise streaming and tools.
Image inputs are advertised for Gemma 4, GLM 5.3 Flash and Kimi K2.7 Code.
Availability, paid-plan requirements and supported parameters depend on the
selected model. Embeddings and native Responses are separate work items and
are not implemented by this adapter yet.

Sources checked 2026-10-03:

- [OpenAI-compatible endpoints](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/)
- [DeepSeek V4 Flash](https://developers.cloudflare.com/workers-ai/models/deepseek-v4-flash-0731/) and [Pro](https://developers.cloudflare.com/workers-ai/models/deepseek-v4-pro-0813/)
- [Gemma 4](https://developers.cloudflare.com/workers-ai/models/gemma-4-26b-a4b-it/)
- [GLM 5.3](https://developers.cloudflare.com/workers-ai/models/glm-5.3/) and [Flash](https://developers.cloudflare.com/workers-ai/models/glm-5.3-flash/)
- [GPT-OSS 120B](https://developers.cloudflare.com/workers-ai/models/gpt-oss-120b/)
- [Kimi K2.7 Code](https://developers.cloudflare.com/workers-ai/models/kimi-k2.7-code/)
- [Qwen3 30B](https://developers.cloudflare.com/workers-ai/models/qwen3-30b-a3b-fp8/)

Verification uses a local HTTP server for wire requests, responses, streaming,
usage and error behavior. It does not claim live access to a Cloudflare account.
