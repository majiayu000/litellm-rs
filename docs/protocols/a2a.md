# A2A gateway and shared types

The executable entry is the authenticated [A2A 1.0 HTTP gateway](../gateway/a2a.md),
built with `gateway,a2a`. It forwards JSON-RPC and supported task streams to a
configured A2A agent, with caller-bound task/context ownership. Its methods,
configuration, authentication and process-local ownership limits are documented
in that guide.

The former `A2AGateway`, provider adapters and `AgentRegistry` have been removed
in 0.8.0. The old agent execution, aggregation, cost tracking and multi-platform
client examples no longer describe executable APIs.

Configuration and domain/error types remain at qualified paths:

```rust
use litellm_rs::core::a2a::config::{AgentConfig, AgentProvider};

let config = AgentConfig::new("research", "https://agent.example.com/rpc")
    .with_provider(AgentProvider::A2A)
    .with_timeout(60_000);
```

This constructs a configuration value, not an agent client. Supply it through
`GatewayConfig::a2a_agents` or the documented JSON/YAML gateway configuration.
Retained domain records and provider enum variants do not implement native
LangGraph, Vertex, Azure or Bedrock agent adapters. The HTTP gateway rejects
unsupported configuration; see the [subsystem reconciliation](../audit/subsystem-reconciliation-2026-10-04.md).
