---
name: a2a-protocol
description: LiteLLM-RS A2A Protocol Architecture. Covers Agent-to-Agent communication, JSON-RPC 2.0 messaging, multi-provider orchestration, agent registry, and task state management. Use when implementing or debugging A2A agent communication, registry integration, or provider adapters.
---

# A2A Protocol Architecture Guide

## Overview

The A2A (Agent-to-Agent) Protocol enables autonomous agents to communicate and collaborate through a standardized interface. LiteLLM-RS implements A2A with support for multiple agent providers (LangGraph, Vertex AI Agent Builder, Azure AI Agent Service, Amazon Bedrock Agents, Pydantic AI).

### A2A Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                       Client Application                        │
│  (User, Orchestrator, or Another Agent)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼ JSON-RPC 2.0
┌─────────────────────────────────────────────────────────────────┐
│                       A2A Gateway                               │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                  Agent Registry                          │   │
│  │  - Agent discovery and registration                      │   │
│  │  - Health monitoring and load balancing                  │   │
│  │  - Capability matching                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
         │              │              │              │
         ▼              ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│   LangGraph  │ │  Vertex AI   │ │   Azure AI   │ │   Bedrock    │
│    Agent     │ │    Agent     │ │    Agent     │ │    Agent     │
│  (Python)    │ │   Builder    │ │   Service    │ │   Agents     │
└──────────────┘ └──────────────┘ └──────────────┘ └──────────────┘
```

---

## JSON-RPC 2.0 Message Format

### Request Structure

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2ARequest {
    pub jsonrpc: String,  // Always "2.0"
    pub id: A2ARequestId,
    pub method: A2AMethod,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum A2ARequestId {
    Number(i64),
    String(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum A2AMethod {
    /// Send a task to an agent
    TaskSend,
    /// Get task status
    TaskGet,
    /// Cancel a running task
    TaskCancel,
    /// Subscribe to task updates (SSE)
    TaskSendSubscribe,
    /// Get agent capabilities
    AgentCapabilities,
    /// Ping for health check
    Ping,
}

impl A2ARequest {
    pub fn task_send(task: &Task) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: A2ARequestId::String(uuid::Uuid::new_v4().to_string()),
            method: A2AMethod::TaskSend,
            params: Some(serde_json::to_value(task).unwrap()),
        }
    }

    pub fn task_get(task_id: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: A2ARequestId::String(uuid::Uuid::new_v4().to_string()),
            method: A2AMethod::TaskGet,
            params: Some(serde_json::json!({ "task_id": task_id })),
        }
    }

    pub fn task_cancel(task_id: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: A2ARequestId::String(uuid::Uuid::new_v4().to_string()),
            method: A2AMethod::TaskCancel,
            params: Some(serde_json::json!({ "task_id": task_id })),
        }
    }
}
```

### Response Structure

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AResponse {
    pub jsonrpc: String,
    pub id: A2ARequestId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<A2AError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

// Standard A2A Error Codes
impl A2AError {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // A2A-specific error codes
    pub const TASK_NOT_FOUND: i32 = -32001;
    pub const AGENT_NOT_FOUND: i32 = -32002;
    pub const AGENT_BUSY: i32 = -32003;
    pub const TASK_CANCELLED: i32 = -32004;
    pub const CAPABILITY_MISMATCH: i32 = -32005;
    pub const RATE_LIMITED: i32 = -32006;

    pub fn task_not_found(task_id: &str) -> Self {
        Self {
            code: Self::TASK_NOT_FOUND,
            message: format!("Task not found: {}", task_id),
            data: None,
        }
    }

    pub fn agent_not_found(agent_id: &str) -> Self {
        Self {
            code: Self::AGENT_NOT_FOUND,
            message: format!("Agent not found: {}", agent_id),
            data: None,
        }
    }

    pub fn agent_busy(agent_id: &str) -> Self {
        Self {
            code: Self::AGENT_BUSY,
            message: format!("Agent is busy: {}", agent_id),
            data: Some(serde_json::json!({ "retry_after": 5 })),
        }
    }
}
```

---

## Task State Machine

### Task States

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Task received, waiting to be processed
    Pending,
    /// Task is currently being executed
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed with error
    Failed,
    /// Task was cancelled
    Cancelled,
    /// Task requires user input
    InputRequired,
}

impl TaskState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn can_transition_to(&self, next: TaskState) -> bool {
        match (self, next) {
            // From Pending
            (Self::Pending, Self::Running) => true,
            (Self::Pending, Self::Cancelled) => true,

            // From Running
            (Self::Running, Self::Completed) => true,
            (Self::Running, Self::Failed) => true,
            (Self::Running, Self::Cancelled) => true,
            (Self::Running, Self::InputRequired) => true,

            // From InputRequired
            (Self::InputRequired, Self::Running) => true,
            (Self::InputRequired, Self::Cancelled) => true,

            // Terminal states cannot transition
            _ => false,
        }
    }
}
```

### Task Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub state: TaskState,
    pub input: TaskInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<TaskOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<TaskError>,
    pub metadata: TaskMetadata,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    /// Natural language instruction for the agent
    pub instruction: String,
    /// Context data for the task
    #[serde(default)]
    pub context: serde_json::Value,
    /// Files or artifacts to process
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOutput {
    /// Agent's response
    pub response: String,
    /// Structured data output
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Generated artifacts
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,  // Base64 encoded for binary
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    pub agent_id: String,
    pub provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    pub retryable: bool,
}

impl Task {
    pub fn new(agent_id: &str, provider: &str, instruction: &str) -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            state: TaskState::Pending,
            input: TaskInput {
                instruction: instruction.to_string(),
                context: serde_json::Value::Null,
                artifacts: vec![],
            },
            output: None,
            error: None,
            metadata: TaskMetadata {
                agent_id: agent_id.to_string(),
                provider: provider.to_string(),
                parent_task_id: None,
                session_id: None,
                tags: vec![],
            },
            created_at: now,
            updated_at: now,
        }
    }

    pub fn transition(&mut self, new_state: TaskState) -> Result<(), A2AError> {
        if !self.state.can_transition_to(new_state) {
            return Err(A2AError {
                code: A2AError::INVALID_PARAMS,
                message: format!(
                    "Invalid state transition from {:?} to {:?}",
                    self.state, new_state
                ),
                data: None,
            });
        }
        self.state = new_state;
        self.updated_at = chrono::Utc::now().timestamp();
        Ok(())
    }
}
```

---

## References

- [reference/registry.md](reference/registry.md) — Agent card schema and the in-memory AgentRegistry: registration, capability lookup, background health monitoring.
- [reference/adapters.md](reference/adapters.md) — The A2AProvider trait with LangGraph and Vertex AI Agent Builder adapter implementations.
- [reference/gateway.md](reference/gateway.md) — A2AGateway task routing, cancellation, and streaming, plus the in-memory TaskStore.
- [reference/configuration.md](reference/configuration.md) — YAML configuration for enabling A2A, gateway settings, and agent endpoint/capability definitions.
- [reference/errors.md](reference/errors.md) — The A2AProtocolError enum covering lookup, provider, transition, timeout, and rate-limit failures.
- [reference/best-practices.md](reference/best-practices.md) — Do/don't patterns for agent registration, state transitions, error handling, and capability matching.
