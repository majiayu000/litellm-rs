## Provider Adapters

### Provider Trait

```rust
#[async_trait]
pub trait A2AProvider: Send + Sync {
    /// Provider identifier
    fn provider_type(&self) -> AgentProvider;

    /// Send a task to the agent
    async fn send_task(&self, agent: &AgentCard, task: &Task) -> Result<Task, A2AError>;

    /// Get task status
    async fn get_task(&self, agent: &AgentCard, task_id: &str) -> Result<Task, A2AError>;

    /// Cancel a running task
    async fn cancel_task(&self, agent: &AgentCard, task_id: &str) -> Result<(), A2AError>;

    /// Stream task updates
    async fn stream_task(
        &self,
        agent: &AgentCard,
        task: &Task,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskUpdate, A2AError>> + Send>>, A2AError>;

    /// Check agent health
    async fn health_check(&self, agent: &AgentCard) -> AgentStatus;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub task_id: String,
    pub state: TaskState,
    pub progress: Option<f32>,  // 0.0 to 1.0
    pub message: Option<String>,
    pub partial_output: Option<TaskOutput>,
}
```

### LangGraph Adapter

```rust
pub struct LangGraphProvider {
    client: reqwest::Client,
}

impl LangGraphProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap(),
        }
    }
}

#[async_trait]
impl A2AProvider for LangGraphProvider {
    fn provider_type(&self) -> AgentProvider {
        AgentProvider::LangGraph
    }

    async fn send_task(&self, agent: &AgentCard, task: &Task) -> Result<Task, A2AError> {
        let url = format!("{}/runs", agent.endpoint.url);

        let body = serde_json::json!({
            "input": {
                "messages": [
                    { "role": "user", "content": task.input.instruction }
                ],
                "context": task.input.context
            },
            "config": {
                "tags": task.metadata.tags
            }
        });

        let mut request = self.client.post(&url).json(&body);

        // Add authentication
        if let Some(auth) = &agent.endpoint.auth {
            request = match auth {
                EndpointAuth::Bearer { token } => {
                    request.header("Authorization", format!("Bearer {}", token))
                }
                EndpointAuth::ApiKey { header, key } => {
                    request.header(header, key)
                }
                _ => request,
            };
        }

        let response = request
            .send()
            .await
            .map_err(|e| A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("LangGraph request failed: {}", e),
                data: None,
            })?;

        if !response.status().is_success() {
            let error_body = response.text().await.unwrap_or_default();
            return Err(A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("LangGraph error: {}", error_body),
                data: None,
            });
        }

        let result: serde_json::Value = response.json().await.map_err(|e| A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Failed to parse LangGraph response: {}", e),
            data: None,
        })?;

        // Transform LangGraph response to Task
        let mut updated_task = task.clone();
        updated_task.state = TaskState::Completed;
        updated_task.output = Some(TaskOutput {
            response: result["output"]["messages"]
                .as_array()
                .and_then(|msgs| msgs.last())
                .and_then(|msg| msg["content"].as_str())
                .unwrap_or("")
                .to_string(),
            data: Some(result["output"].clone()),
            artifacts: vec![],
        });

        Ok(updated_task)
    }

    async fn get_task(&self, agent: &AgentCard, task_id: &str) -> Result<Task, A2AError> {
        let url = format!("{}/runs/{}", agent.endpoint.url, task_id);

        let mut request = self.client.get(&url);

        if let Some(EndpointAuth::Bearer { token }) = &agent.endpoint.auth {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Failed to get task: {}", e),
            data: None,
        })?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(A2AError::task_not_found(task_id));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Failed to parse response: {}", e),
            data: None,
        })?;

        // Transform to Task...
        Ok(Task::new(&agent.id, "langgraph", ""))
    }

    async fn cancel_task(&self, agent: &AgentCard, task_id: &str) -> Result<(), A2AError> {
        let url = format!("{}/runs/{}/cancel", agent.endpoint.url, task_id);

        let mut request = self.client.post(&url);

        if let Some(EndpointAuth::Bearer { token }) = &agent.endpoint.auth {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Failed to cancel task: {}", e),
            data: None,
        })?;

        if !response.status().is_success() {
            return Err(A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: "Failed to cancel task".to_string(),
                data: None,
            });
        }

        Ok(())
    }

    async fn stream_task(
        &self,
        agent: &AgentCard,
        task: &Task,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskUpdate, A2AError>> + Send>>, A2AError> {
        let url = format!("{}/runs/stream", agent.endpoint.url);

        let body = serde_json::json!({
            "input": {
                "messages": [
                    { "role": "user", "content": task.input.instruction }
                ]
            },
            "stream_mode": "updates"
        });

        let mut request = self.client.post(&url).json(&body);

        if let Some(EndpointAuth::Bearer { token }) = &agent.endpoint.auth {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| A2AError {
            code: A2AError::INTERNAL_ERROR,
            message: format!("Failed to start stream: {}", e),
            data: None,
        })?;

        let task_id = task.id.clone();
        let stream = response.bytes_stream().map(move |result| {
            match result {
                Ok(bytes) => {
                    // Parse SSE event
                    let text = String::from_utf8_lossy(&bytes);
                    if let Some(data) = text.strip_prefix("data: ") {
                        if let Ok(update) = serde_json::from_str::<serde_json::Value>(data) {
                            return Ok(TaskUpdate {
                                task_id: task_id.clone(),
                                state: TaskState::Running,
                                progress: None,
                                message: update["message"].as_str().map(|s| s.to_string()),
                                partial_output: None,
                            });
                        }
                    }
                    Ok(TaskUpdate {
                        task_id: task_id.clone(),
                        state: TaskState::Running,
                        progress: None,
                        message: None,
                        partial_output: None,
                    })
                }
                Err(e) => Err(A2AError {
                    code: A2AError::INTERNAL_ERROR,
                    message: format!("Stream error: {}", e),
                    data: None,
                }),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn health_check(&self, agent: &AgentCard) -> AgentStatus {
        let url = format!("{}/health", agent.endpoint.url);

        match self.client.get(&url).timeout(Duration::from_secs(5)).send().await {
            Ok(response) if response.status().is_success() => AgentStatus::Online,
            Ok(_) => AgentStatus::Degraded,
            Err(_) => AgentStatus::Offline,
        }
    }
}
```

### Vertex AI Agent Builder Adapter

```rust
pub struct VertexAIProvider {
    client: reqwest::Client,
    project_id: String,
    location: String,
}

#[async_trait]
impl A2AProvider for VertexAIProvider {
    fn provider_type(&self) -> AgentProvider {
        AgentProvider::VertexAI
    }

    async fn send_task(&self, agent: &AgentCard, task: &Task) -> Result<Task, A2AError> {
        let url = format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/agents/{}:run",
            self.location, self.project_id, self.location, agent.id
        );

        let body = serde_json::json!({
            "userInput": {
                "text": task.input.instruction
            },
            "context": task.input.context
        });

        let response = self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("Vertex AI request failed: {}", e),
                data: None,
            })?;

        // Process Vertex AI response...
        let mut updated_task = task.clone();
        updated_task.state = TaskState::Completed;

        Ok(updated_task)
    }

    // Other method implementations...
}
```

---
