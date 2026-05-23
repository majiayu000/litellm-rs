# Provider Documentation

LiteLLM-RS supports 100+ AI providers through a unified interface. This section provides detailed documentation for each supported provider.

## 🎯 Supported Providers

### **Tier 1 Providers** (Full Feature Support)
- [**OpenAI**](./openai.md) - GPT-5.4, GPT-5.4 mini, GPT-4.1, Embeddings, GPT Image 1.5
- [**Anthropic**](./anthropic.md) - Claude Opus 4.7, Sonnet 4.6, Haiku 4.5
- [**DeepSeek**](./deepseek.md) - DeepSeek V4 Flash & Pro
- [**Google**](./google.md) - Gemini Pro, PaLM, Vertex AI
- [**Azure OpenAI**](./azure-openai.md) - Enterprise OpenAI deployment

### **Tier 2 Providers** (Core Features)
- **Cohere** - Command models and embeddings
- **Mistral** - Mistral 7B, 8x7B, Large
- **Together AI** - Open source models
- **Groq** - High-speed inference
- **Replicate** - Custom model hosting

### **Tier 3 Providers** (Basic Support)
- **Hugging Face** - Transformers and hosted models
- **AWS Bedrock** - Amazon's model marketplace
- **Ollama** - Local model serving
- **OpenRouter** - Model routing service
- **Fireworks AI** - Fast inference platform

## 📋 Provider Capabilities Matrix

| Provider | Chat | Streaming | Tools | Vision | Embeddings | Audio |
|----------|------|-----------|-------|---------|------------|-------|
| OpenAI | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Anthropic | ✅ | ✅ | ✅ | ✅ | ❌ | ❌ |
| DeepSeek | ✅ | ✅ | ✅ | ❌ | ❌ | ❌ |
| Google | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ |
| Azure OpenAI | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cohere | ✅ | ✅ | ❌ | ❌ | ✅ | ❌ |
| Mistral | ✅ | ✅ | ✅ | ❌ | ✅ | ❌ |

## 🚀 Quick Usage Examples

### OpenAI Compatible
```rust
// Works with OpenAI, Azure OpenAI, OpenRouter, etc.
let response = completion("gpt-5.4", messages, None).await?;
```

### Provider-Specific Models
```rust
// DeepSeek V4
let response = completion("deepseek-v4-flash", messages, None).await?;
let reasoning = completion("deepseek-v4-pro", messages, None).await?;

// Anthropic Claude
let response = completion("claude-opus-4-7", messages, None).await?;

// Google Gemini
let response = completion("gemini-3.1-pro-preview", messages, None).await?;
```

### Provider Prefixes
```rust
// Explicit provider specification
let openai_response = completion("openai/gpt-5.4", messages, None).await?;
let anthropic_response = completion("anthropic/claude-opus-4-7", messages, None).await?;
let deepseek_response = completion("deepseek/deepseek-v4-flash", messages, None).await?;
```

## ⚙️ Configuration

### Environment Variables
```bash
# OpenAI
export OPENAI_API_KEY=your_key_here

# Anthropic
export ANTHROPIC_API_KEY=your_key_here

# DeepSeek
export DEEPSEEK_API_KEY=your_key_here

# Google
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/credentials.json

# Azure OpenAI
export AZURE_OPENAI_API_KEY=your_key_here
export AZURE_OPENAI_ENDPOINT=https://your-resource.openai.azure.com
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
```

## 🔧 Advanced Features

### Model Routing
```rust
// Router automatically selects best provider
let router = Router::new()
    .add_provider("openai", openai_provider)
    .add_provider("deepseek", deepseek_provider)
    .with_strategy(RoutingStrategy::LeastLatency);

let response = router.completion("gpt-5.4", messages).await?;
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
