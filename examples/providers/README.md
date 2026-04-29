# LiteLLM Provider Examples

This directory contains completion examples for each AI provider supported by LiteLLM.

## Quick Start

All examples use the Python-style `completion()` function that works with any provider:

```rust
use litellm_rs::completion;

let response = completion("gpt-3.5-turbo", messages).await?;
```

## Provider Examples

| Provider | Example File | Models | API Key Required |
|----------|-------------|---------|------------------|
| **OpenAI** | `openai_completion.rs` | GPT-3.5, GPT-4, GPT-4o | `OPENAI_API_KEY` |
| **Anthropic** | `anthropic_completion.rs` | Claude 3 (Haiku, Sonnet, Opus) | `ANTHROPIC_API_KEY` |
| **Azure OpenAI** | `azure_completion.rs` | GPT models via Azure | `AZURE_API_KEY`, `AZURE_API_BASE` |
| **Mistral** | `mistral_completion.rs` | Mistral Small/Medium/Large, Codestral | `MISTRAL_API_KEY` |
| **DeepSeek** | `deepseek_completion.rs` | DeepSeek Chat, DeepSeek Coder | `DEEPSEEK_API_KEY` |
| **Moonshot** | `moonshot_completion.rs` | Kimi K2.5, Kimi K2 Thinking, Moonshot v1 | `MOONSHOT_API_KEY` |
| **MiniMax** | `moonshot_completion.rs` (shared OpenAI-compatible example) | MiniMax M2.5 series (OpenAI-compatible) | `MINIMAX_API_KEY` |
| **Zhipu** | `moonshot_completion.rs` (shared OpenAI-compatible example) | GLM-5 / GLM-4.7 / GLM-4.6 (OpenAI-compatible) | `ZHIPU_API_KEY` |
| **Meta Llama** | `meta_llama_completion.rs` | Llama 3 (8B, 70B), Code Llama | Various providers |
| **OpenRouter** | `openrouter_completion.rs` | 100+ models from all providers | `OPENROUTER_API_KEY` |
| **Vertex AI** | `vertex_ai_completion.rs` | Gemini Pro, PaLM 2 | `GCP_PROJECT_ID` |
| **V0 (Vercel)** | `v0_completion.rs` | UI/UX component generation | `V0_API_KEY` |

## Running Examples

### Basic Usage

```bash
# Set your API key
export OPENAI_API_KEY="your-key-here"

# Run the example
cargo run --example openai_completion
```

### Multiple Providers

```bash
# Set multiple API keys
export OPENAI_API_KEY="xxx"
export ANTHROPIC_API_KEY="xxx"
export MISTRAL_API_KEY="xxx"

# Run any example
cargo run --example anthropic_completion
cargo run --example mistral_completion
```

### Using OpenRouter (Access All Models)

OpenRouter provides access to models from multiple providers through one API:

```bash
export OPENROUTER_API_KEY="xxx"
cargo run --example openrouter_completion
```

## Model Naming Conventions

### Direct Provider Access
```rust
completion("gpt-3.5-turbo", messages)           // OpenAI
completion("claude-3-sonnet-20240229", messages) // Anthropic
completion("mistral-large-latest", messages)     // Mistral
completion("moonshot/kimi-k2.5", messages)       // Moonshot/Kimi
completion("minimax/MiniMax-M2.5", messages)     // MiniMax
completion("glm/glm-5", messages)                // Zhipu GLM
```

### Via OpenRouter
```rust
completion("openrouter/openai/gpt-4", messages)
completion("openrouter/anthropic/claude-3-opus", messages)
completion("openrouter/meta-llama/llama-3-70b", messages)
```

### Special Formats
```rust
completion("azure/deployment-name", messages)    // Azure OpenAI
completion("vertex_ai/gemini-pro", messages)     // Google Vertex AI
```

## Provider-Specific Features

### OpenAI
- Supports embeddings and image generation (DALL-E)
- Function calling / Tools API
- Streaming responses
- GPT-4 Vision for image understanding

### Anthropic (Claude)
- Long context windows (up to 200K tokens)
- Strong reasoning capabilities
- XML tag support in prompts

### Mistral
- Codestral model specialized for code
- European data residency options
- Competitive pricing

### DeepSeek
- Specialized code understanding and generation
- Optimized for programming tasks
- Code review capabilities

### Moonshot (Kimi)
- Kimi K2.5 / K2 Thinking support
- Ultra-long context (up to 262K tokens on K2.5)
- Excellent at document analysis
- Chinese language optimization

### OpenRouter
- Single API for 100+ models
- Automatic fallback and routing
- Usage-based pricing across providers

### V0 (Vercel)
- Specialized for UI/UX component generation
- React, Next.js, Tailwind CSS focus
- TypeScript support

## Common Options

All examples support additional options:

```rust
use litellm_rs::{ChatOptions, completion_with_options};

let mut options = ChatOptions::default();
options.temperature = Some(0.7);
options.max_tokens = Some(1000);
options.top_p = Some(0.9);

let response = completion_with_options(
    "gpt-3.5-turbo",
    messages,
    options
).await?;
```

## Environment Variables

Create a `.env` file in the project root:

```env
# OpenAI
OPENAI_API_KEY=sk-xxx
OPENAI_ORG_ID=org-xxx

# Anthropic
ANTHROPIC_API_KEY=sk-ant-xxx

# Azure OpenAI
AZURE_API_KEY=xxx
AZURE_API_BASE=https://your-resource.openai.azure.com
AZURE_DEPLOYMENT_ID=your-deployment
AZURE_API_VERSION=2024-02-15-preview

# Mistral
MISTRAL_API_KEY=xxx

# DeepSeek
DEEPSEEK_API_KEY=xxx

# Moonshot
MOONSHOT_API_KEY=xxx

# MiniMax
MINIMAX_API_KEY=xxx

# Zhipu
ZHIPU_API_KEY=xxx

# OpenRouter
OPENROUTER_API_KEY=sk-or-xxx

# Google Vertex AI
GCP_PROJECT_ID=your-project

# V0 (Vercel)
V0_API_KEY=xxx
```

## Tips

1. **Start with OpenRouter** if you want to test multiple models without setting up individual accounts
2. **Use streaming** for real-time responses in chat applications
3. **Set temperature=0** for deterministic outputs
4. **Check model pricing** before using expensive models like GPT-4 or Claude Opus
5. **Use appropriate context lengths** - don't send 100K tokens to a model that supports 4K

## Troubleshooting

### API Key Issues
- Ensure environment variables are set correctly
- Check API key permissions and quotas
- Verify billing is active on your account

### Model Not Found
- Check model name spelling
- Ensure model is available in your region
- Verify you have access to the model (some require approval)

### Rate Limits
- Implement exponential backoff
- Use different models for development vs production
- Consider caching responses when appropriate

## More Examples

For more complex examples, see:
- `../python_style_api.rs` - Python LiteLLM-style API usage
- `../all_providers_comprehensive.rs` - Comprehensive provider features
- `../provider_macros_demo.rs` - How macros optimize provider operations
