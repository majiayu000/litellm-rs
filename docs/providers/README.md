# Provider Documentation

LiteLLM-RS supports 100+ AI providers through a unified interface. This section provides detailed documentation for each supported provider.

## 🎯 Supported Providers

### **Tier 1 Providers** (Full Feature Support)
- [**OpenAI**](./openai.md) - GPT-5.5, GPT-5.5 Pro, GPT-5.4 mini, Embeddings, GPT Image 1.5
- [**Anthropic**](./anthropic.md) - Claude Opus 4.7, Sonnet 4.6, Haiku 4.5
- [**AWS Bedrock**](./bedrock.md) - Native SigV4 Bedrock Runtime provider
- [**DeepSeek**](./deepseek.md) - DeepSeek V4 Flash & Pro
- [**Xiaomi MiMo**](./xiaomi-mimo.md) - MiMo V2.5 OpenAI-compatible endpoint
- [**Google**](./google.md) - Gemini Pro, PaLM, Vertex AI
- [**Azure OpenAI**](./azure-openai.md) - Enterprise OpenAI deployment
- **xAI** - OpenAI-compatible Grok routing with pass-through model IDs

### **Tier 2 Providers** (Core Features)
- **Cohere** - Command models and embeddings
- **Mistral** - Mistral Large 3, Medium, Small 4, Magistral, Devstral, Pixtral
- **Together AI** - Open source models
- **Groq** - High-speed inference
- **Meta Llama** - Llama 4 Scout and Maverick through the Llama-compatible endpoint
- **Replicate** - Custom model hosting

### **Tier 3 Providers** (Basic Support)
- **Hugging Face** - Transformers and hosted models
- [**OpenAI-compatible Bedrock proxy**](./openai-compatible-bedrock-proxy.md) - Bedrock Access Gateway and similar proxy deployments
- **Ollama** - Local model serving
- **OpenRouter** - Model routing service
- **Fireworks AI** - Fast inference platform

## 📋 Provider Capabilities Matrix

| Provider | Chat | Streaming | Tools | Vision | Embeddings | Audio |
|----------|------|-----------|-------|---------|------------|-------|
| OpenAI | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Anthropic | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| AWS Bedrock | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| DeepSeek | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Xiaomi MiMo | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| Google | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Azure OpenAI | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cohere | ✅ | ✅ | ❌ | ❌ | ✅ | ❌ |
| Mistral | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| xAI | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| AWS Bedrock / Nova | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| Meta Llama | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |

## 🚀 Quick Usage Examples

### OpenAI Compatible
```rust
// Works with OpenAI, Azure OpenAI, OpenRouter, etc.
let response = completion("gpt-5.5", messages, None).await?;
```

### Provider-Specific Models
```rust
// DeepSeek V4
let response = completion("deepseek-v4-flash", messages, None).await?;
let reasoning = completion("deepseek-v4-pro", messages, None).await?;

// Xiaomi MiMo V2.5
let response = completion("xiaomi_mimo/mimo-v2.5", messages, None).await?;

// Anthropic Claude
let response = completion("claude-opus-4-7", messages, None).await?;

// Google Gemini
let response = completion("gemini-3.1-pro-preview", messages, None).await?;

// xAI Grok, routed through the OpenAI-compatible provider catalog
let response = completion("xai/grok-4.3", messages, None).await?;
```

### Provider Prefixes
```rust
// Explicit provider specification
let openai_response = completion("openai/gpt-5.5", messages, None).await?;
let anthropic_response = completion("anthropic/claude-opus-4-7", messages, None).await?;
let deepseek_response = completion("deepseek/deepseek-v4-flash", messages, None).await?;
let mimo_response = completion("xiaomi_mimo/mimo-v2.5-pro", messages, None).await?;
let xai_response = completion("xai/grok-4.3", messages, None).await?;
```

### xAI Pass-Through Models
The xAI entry in the Tier 1 provider catalog is OpenAI-compatible and does not
maintain a static model registry. Pass current xAI model IDs directly, such as
`xai/grok-4.3`; the provider definition supplies `https://api.x.ai/v1` and
`XAI_API_KEY`.

## ⚙️ Configuration

### Environment Variables
```bash
# OpenAI
export OPENAI_API_KEY=your_key_here

# Anthropic
export ANTHROPIC_API_KEY=your_key_here

# DeepSeek
export DEEPSEEK_API_KEY=your_key_here

# Xiaomi MiMo
export MIMO_API_KEY=your_key_here

# Google
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/credentials.json

# Azure OpenAI
export AZURE_OPENAI_API_KEY=your_key_here
export AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com

# xAI
export XAI_API_KEY=your_key_here

# AWS Bedrock native runtime
export AWS_ACCESS_KEY_ID=your_key_here
export AWS_SECRET_ACCESS_KEY=your_secret_here
export AWS_REGION=us-east-1
```

### YAML Configuration
```yaml
providers:
  openai:
    api_key: "${OPENAI_API_KEY}"
    timeout_seconds: 30
    max_retries: 3
    
  deepseek:
    api_key: "${DEEPSEEK_API_KEY}"
    api_base: "https://api.deepseek.com"
    extra_params:
      reasoning_effort: "medium"
      
  anthropic:
    api_key: "${ANTHROPIC_API_KEY}"
    api_version: "2023-06-01"

  bedrock-native:
    provider_type: "bedrock"
    api_key: ""
    settings:
      aws_region: "${AWS_REGION}"

  bedrock-access-gateway:
    provider_type: "openai_compatible"
    api_key: "${BEDROCK_ACCESS_GATEWAY_API_KEY}"
    base_url: "https://bedrock-access-gateway.example.com/api/v1"
```

## 🔧 Advanced Features

### Model Routing
```rust
// Router automatically selects best provider
let router = Router::new()
    .add_provider("openai", openai_provider)
    .add_provider("deepseek", deepseek_provider)
    .with_strategy(RoutingStrategy::LeastLatency);

let response = router.completion("gpt-5.5", messages).await?;
```

### Fallback Chains
```rust
// Automatic fallback on provider failure
let router = Router::new()
    .add_fallback_chain(vec!["openai", "anthropic", "deepseek"]);
```

### Cost Optimization
```rust
// Route to cheapest provider for model class
let router = Router::new()
    .with_strategy(RoutingStrategy::CostOptimized);
```

## 📊 Provider Performance

### Latency Comparison (p95)
- **Groq**: ~200ms (Specialized hardware)
- **OpenAI**: ~800ms (Standard models)
- **DeepSeek**: ~900ms (Competitive pricing)
- **Anthropic**: ~1200ms (High quality)
- **Google**: ~1500ms (Complex models)

### Cost Comparison (per 1M tokens)
- **DeepSeek V4 Flash**: $0.14 cache-miss input, $0.0028 cache-hit input, $0.28 output
- **GPT-3.5-Turbo**: $0.50 input, $1.50 output  
- **GPT-4**: $30.00 input, $60.00 output
- **Claude Sonnet**: $3.00 input, $15.00 output
- **Gemini Pro**: $0.50 input, $1.50 output

## 🛠️ Adding New Providers

See the [Provider Implementation Guide](../architecture/provider-implementation.md) for detailed instructions on adding new providers to LiteLLM-RS.

### Implementation Checklist
- [ ] Configuration and validation
- [ ] Error handling and mapping
- [ ] Request/response transformation
- [ ] Model registry integration
- [ ] Streaming support
- [ ] Cost calculation
- [ ] Health monitoring
- [ ] Test coverage
- [ ] Documentation

## 🐛 Troubleshooting

### Common Issues

#### Authentication Errors
```bash
# Check API key is set
echo $OPENAI_API_KEY

# Verify key format
export OPENAI_API_KEY=sk-...  # OpenAI format
export ANTHROPIC_API_KEY=sk-ant-...  # Anthropic format
```

#### Rate Limiting
```rust
// Configure retry logic
let config = OpenAIConfig {
    max_retries: 5,
    timeout_seconds: 60,
    ..Default::default()
};
```

#### Model Not Found
```rust
// Check available models
let models = provider.models();
for model in models {
    println!("Available: {}", model.id);
}
```

For provider-specific issues, see individual provider documentation pages.
