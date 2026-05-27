## File Structure 

### August 27th, 2024 

To make it easy to see how calls are transformed for each model/provider:

we are working on moving all supported litellm providers to a folder structure, where folder name is the supported litellm provider name. 

Each folder will contain a `*_transformation.py` file, which has all the request/response transformation logic, making it easy to see how calls are modified. 

E.g. `cohere/`, `bedrock/`. 

## Bedrock setup

AWS Bedrock has two distinct integration paths in LiteLLM-RS. Pick one based
on whether the gateway should talk to AWS directly or through an
OpenAI-compatible proxy:

- **Native AWS Bedrock runtime** (`provider_type: "bedrock"`, SigV4-signed
  Converse / Invoke) — see [`docs/providers/bedrock.md`](../../../docs/providers/bedrock.md).
- **OpenAI-compatible proxy** (`provider_type: "openai_compatible"`, e.g.
  [Bedrock Access Gateway](https://github.com/aws-samples/bedrock-access-gateway))
  — see [`docs/providers/openai-compatible-bedrock-proxy.md`](../../../docs/providers/openai-compatible-bedrock-proxy.md).

Do not configure a proxy deployment as `provider_type: "bedrock"`; the native
provider expects AWS credentials and signs requests with SigV4, while proxy
deployments expect an HTTPS base URL and a bearer token.
