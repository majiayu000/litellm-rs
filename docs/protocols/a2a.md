# A2A Protocol

> Deprecated 0.6 library-only surface scheduled for 0.7 removal. Compile with Cargo feature `a2a`. The
> gateway binary does not construct this module or mount an A2A HTTP route;
> enabling the feature alone does not make A2A a runtime capability.

Agent-to-Agent (A2A) Protocol enables LiteLLM-RS to communicate with AI agents from multiple platforms, providing a unified interface for agent orchestration.

## Overview

The A2A Gateway implements the [A2A Protocol](https://github.com/google/a2a-protocol) specification, supporting:

- **Multi-Provider Support**: LangGraph, Vertex AI, Azure AI Foundry, Bedrock AgentCore, Pydantic AI
- **JSON-RPC 2.0**: Standard protocol for agent communication
- **Task Management**: Async task execution with status tracking
- **Agent Registry**: Discovery and health monitoring
- **Cost Tracking**: Per-agent invocation cost monitoring

## Quick Start

```rust
use litellm_rs::core::a2a::{A2AGateway, AgentConfig, AgentProvider};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create gateway
    let gateway = A2AGateway::new();

    // Register an agent
    let config = AgentConfig::new("my-agent", "https://agent.example.com/a2a")
        .with_provider(AgentProvider::LangGraph)
        .with_api_key("your-api-key")
        .with_description("My helpful AI agent");

    gateway.register_agent(config).await?;

    // Send a message
    let response = gateway.send_message("my-agent", "Hello, agent!").await?;
    println!("Response: {:?}", response);

    Ok(())
}
```

## Supported Providers

| Provider | Description | Streaming | Async Tasks |
|----------|-------------|-----------|-------------|
| `A2A` | Generic A2A-compatible agent | Yes | Yes |
| `LangGraph` | LangChain LangGraph agents | Yes | Yes |
| `VertexAI` | Google Vertex AI Agent Engine | Yes | No |
| `AzureAIFoundry` | Azure AI Foundry agents | Yes | No |
| `BedrockAgentCore` | AWS Bedrock AgentCore | No | Yes |
| `PydanticAI` | Pydantic AI agents | No | No |
| `Custom` | Custom provider implementation | Configurable | Configurable |

## Agent Configuration

### Basic Configuration

```rust
use litellm_rs::core::a2a::{AgentConfig, AgentProvider, AgentCapabilities};

let config = AgentConfig {
    name: "my-agent".to_string(),
    provider: AgentProvider::LangGraph,
    url: "https://agent.example.com/a2a".to_string(),
    api_key: Some("your-api-key".to_string()),
    headers: HashMap::new(),
    timeout_ms: 60000,
    enabled: true,
    description: Some("My AI agent".to_string()),
    capabilities: AgentCapabilities::default(),
    rate_limit_rpm: Some(60),
    cost_per_request: Some(0.01),
    tags: vec!["production".to_string()],
    provider_config: HashMap::new(),
};
```

### Builder Pattern

```rust
let config = AgentConfig::new("my-agent", "https://agent.example.com/a2a")
    .with_provider(AgentProvider::LangGraph)
    .with_api_key("your-api-key")
    .with_timeout(30000)
    .with_description("Production agent for customer support");
```

### Agent Capabilities

```rust
use litellm_rs::core::a2a::config::AgentCapabilities;

// Full capabilities
let caps = AgentCapabilities::full();
// streaming: true, push_notifications: true, task_cancellation: true,
// multi_turn: true, file_attachments: true

// Minimal capabilities
let caps = AgentCapabilities::minimal();
// streaming: false, push_notifications: false, task_cancellation: false,
// multi_turn: false, file_attachments: false
```

## Sending Messages

### Simple Message

```rust
let response = gateway.send_message("agent-name", "Your message here").await?;
```

### Full A2A Message

```rust
use litellm_rs::core::a2a::message::{A2AMessage, Message, MessagePart};

let message = A2AMessage::send("Hello, agent!")
    .with_config(serde_json::json!({
        "temperature": 0.7,
        "max_tokens": 1000
    }));

let response = gateway.send("agent-name", message).await?;
```

### Multi-Part Messages

```rust
use litellm_rs::core::a2a::message::{Message, MessagePart};

let message = Message::new("user")
    .with_part(MessagePart::text("Analyze this image:"))
    .with_part(MessagePart::image("https://example.com/image.png", Some("image/png")));
```

## Task Management

### Async Tasks

```rust
// Send message and get task ID
let response = gateway.send_message("agent-name", "Long running task...").await?;

if let Some(task_id) = response.task_id() {
    // Poll for task status
    let result = gateway.get_task("agent-name", &task_id).await?;
    println!("Task state: {:?}", result.status.state);

    // Wait for completion
    let final_result = gateway.wait_for_task("agent-name", &task_id, 60000).await?;
    println!("Task completed: {:?}", final_result);
}
```

### Task States

```rust
use litellm_rs::core::a2a::message::TaskState;

match task_result.status.state {
    TaskState::Submitted => println!("Task submitted"),
    TaskState::Working => println!("Task in progress"),
    TaskState::InputRequired => println!("Agent needs more input"),
    TaskState::Completed => println!("Task completed successfully"),
    TaskState::Failed => println!("Task failed"),
    TaskState::Canceled => println!("Task was canceled"),
}
```

### Cancel Task

```rust
gateway.cancel_task("agent-name", &task_id).await?;
```

## Agent Registry

### Managing Agents

```rust
// List all agents
let agents = gateway.list_agents().await;

// List available (healthy) agents
let available = gateway.list_available_agents().await;

// Get specific agent
let config = gateway.get_agent("agent-name").await?;

// Unregister agent
gateway.unregister_agent("agent-name").await;
```

### Health Monitoring

```rust
use litellm_rs::core::a2a::registry::AgentState;

// Update agent state
gateway.set_agent_state("agent-name", AgentState::Healthy).await;

// Agent states
AgentState::Unknown    // Not yet checked
AgentState::Healthy    // Responding normally
AgentState::Degraded   // Slow responses
AgentState::Unhealthy  // Errors occurring
AgentState::Disabled   // Manually disabled
```

### Gateway Statistics

```rust
let stats = gateway.stats().await;
println!("Total agents: {}", stats.registry.total_agents);
println!("Healthy agents: {}", stats.registry.healthy_agents);
println!("Total invocations: {}", stats.registry.total_invocations);
println!("Total cost: ${:.2}", stats.registry.total_cost);
```

## Configuration from File

```rust
use litellm_rs::core::a2a::{A2AGateway, A2AGatewayConfig, AgentConfig};

let mut config = A2AGatewayConfig::default();
config.enable_logging = true;
config.enable_cost_tracking = true;

config.add_agent(
    AgentConfig::new("agent1", "https://agent1.example.com/a2a")
        .with_provider(AgentProvider::LangGraph)
);
config.add_agent(
    AgentConfig::new("agent2", "https://agent2.example.com/a2a")
        .with_provider(AgentProvider::VertexAI)
);

let gateway = A2AGateway::from_config(config).await?;
```

## Error Handling

```rust
use litellm_rs::core::a2a::error::A2AError;

match gateway.send_message("agent", "message").await {
    Ok(response) => println!("Success: {:?}", response),
    Err(A2AError::AgentNotFound { agent_name }) => {
        println!("Agent {} not found", agent_name);
    }
    Err(A2AError::Timeout { agent_name, timeout_ms }) => {
        println!("Agent {} timed out after {}ms", agent_name, timeout_ms);
    }
    Err(A2AError::RateLimitExceeded { agent_name, retry_after_ms }) => {
        if let Some(retry) = retry_after_ms {
            println!("Rate limited, retry after {}ms", retry);
        }
    }
    Err(A2AError::AuthenticationError { agent_name, message }) => {
        println!("Auth failed for {}: {}", agent_name, message);
    }
    Err(e) => println!("Other error: {}", e),
}
```

## Architecture

```
src/core/a2a/
├── mod.rs          # Module entry point and re-exports
├── config.rs       # Agent configuration and provider types
├── error.rs        # Error types (A2AError, A2AResult)
├── message.rs      # A2A message types (JSON-RPC 2.0)
├── provider.rs     # Provider adapters (LangGraph, Vertex, etc.)
├── registry.rs     # Agent registry and health tracking
└── gateway.rs      # Main gateway for agent management
```

## Message Format

A2A uses JSON-RPC 2.0 for communication:

```json
{
  "jsonrpc": "2.0",
  "id": "unique-id",
  "method": "message/send",
  "params": {
    "message": {
      "role": "user",
      "parts": [
        {"type": "text", "text": "Hello, agent!"}
      ]
    },
    "config": {
      "temperature": 0.7
    }
  }
}
```

Response format:

```json
{
  "jsonrpc": "2.0",
  "id": "unique-id",
  "result": {
    "id": "task-id",
    "status": {
      "state": "completed"
    },
    "artifacts": [
      {
        "type": "text",
        "text": "Hello! How can I help you?"
      }
    ]
  }
}
```

## Best Practices

1. **Use Connection Pooling**: The gateway uses shared HTTP clients automatically
2. **Set Appropriate Timeouts**: Agents can be slow; use 60+ second timeouts
3. **Enable Cost Tracking**: Monitor per-agent costs in production
4. **Implement Retry Logic**: Use exponential backoff for transient failures
5. **Monitor Agent Health**: Regularly check agent states and update accordingly
6. **Use Tags**: Categorize agents with tags for easier management

## References

- [A2A Protocol Specification](https://github.com/google/a2a-protocol)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [LiteLLM Python A2A Documentation](https://docs.litellm.ai/docs/a2a)
- [LangGraph Documentation](https://langchain-ai.github.io/langgraph/)
