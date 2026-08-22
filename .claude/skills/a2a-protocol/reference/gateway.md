## A2A Gateway

### Gateway Implementation

```rust
pub struct A2AGateway {
    registry: Arc<AgentRegistry>,
    providers: HashMap<AgentProvider, Arc<dyn A2AProvider>>,
    task_store: Arc<TaskStore>,
    config: A2AConfig,
}

impl A2AGateway {
    pub fn new(config: A2AConfig) -> Self {
        let registry = Arc::new(AgentRegistry::new(
            Duration::from_secs(config.health_check_interval_seconds),
        ));

        let mut providers: HashMap<AgentProvider, Arc<dyn A2AProvider>> = HashMap::new();
        providers.insert(AgentProvider::LangGraph, Arc::new(LangGraphProvider::new()));
        // Add other providers...

        Self {
            registry,
            providers,
            task_store: Arc::new(TaskStore::new()),
            config,
        }
    }

    pub fn register_agent(&self, agent: AgentCard) -> Result<(), A2AError> {
        self.registry.register(agent)
    }

    pub fn unregister_agent(&self, agent_id: &str) -> Option<Arc<AgentCard>> {
        self.registry.unregister(agent_id)
    }

    pub fn list_agents(&self) -> Vec<Arc<AgentCard>> {
        self.registry.list_online()
    }

    pub fn find_agents_by_capability(&self, capability: &str) -> Vec<Arc<AgentCard>> {
        self.registry.find_by_capability(capability)
    }

    pub async fn send_task(&self, agent_id: &str, task: Task) -> Result<Task, A2AError> {
        let agent = self.registry.get(agent_id)
            .ok_or_else(|| A2AError::agent_not_found(agent_id))?;

        if agent.status != AgentStatus::Online {
            return Err(A2AError {
                code: A2AError::AGENT_BUSY,
                message: format!("Agent {} is not online", agent_id),
                data: None,
            });
        }

        let provider = self.providers.get(&agent.provider)
            .ok_or_else(|| A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("No provider for {:?}", agent.provider),
                data: None,
            })?;

        // Store task
        self.task_store.store(task.clone()).await?;

        // Send to provider
        let result = provider.send_task(&agent, &task).await;

        // Update task state
        if let Ok(ref updated_task) = result {
            self.task_store.update(updated_task.clone()).await?;
        }

        result
    }

    pub async fn get_task(&self, task_id: &str) -> Result<Task, A2AError> {
        self.task_store.get(task_id).await
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<(), A2AError> {
        let task = self.task_store.get(task_id).await?;

        if task.state.is_terminal() {
            return Err(A2AError {
                code: A2AError::INVALID_PARAMS,
                message: "Cannot cancel a completed task".to_string(),
                data: None,
            });
        }

        let agent = self.registry.get(&task.metadata.agent_id)
            .ok_or_else(|| A2AError::agent_not_found(&task.metadata.agent_id))?;

        let provider = self.providers.get(&agent.provider)
            .ok_or_else(|| A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("No provider for {:?}", agent.provider),
                data: None,
            })?;

        provider.cancel_task(&agent, task_id).await?;

        // Update task state
        let mut updated_task = task;
        updated_task.transition(TaskState::Cancelled)?;
        self.task_store.update(updated_task).await?;

        Ok(())
    }

    pub async fn stream_task(
        &self,
        agent_id: &str,
        task: Task,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<TaskUpdate, A2AError>> + Send>>, A2AError> {
        let agent = self.registry.get(agent_id)
            .ok_or_else(|| A2AError::agent_not_found(agent_id))?;

        let provider = self.providers.get(&agent.provider)
            .ok_or_else(|| A2AError {
                code: A2AError::INTERNAL_ERROR,
                message: format!("No provider for {:?}", agent.provider),
                data: None,
            })?;

        provider.stream_task(&agent, &task).await
    }
}
```

---

## Task Storage

```rust
use dashmap::DashMap;

pub struct TaskStore {
    tasks: DashMap<String, Task>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    pub async fn store(&self, task: Task) -> Result<(), A2AError> {
        self.tasks.insert(task.id.clone(), task);
        Ok(())
    }

    pub async fn get(&self, task_id: &str) -> Result<Task, A2AError> {
        self.tasks
            .get(task_id)
            .map(|entry| entry.clone())
            .ok_or_else(|| A2AError::task_not_found(task_id))
    }

    pub async fn update(&self, task: Task) -> Result<(), A2AError> {
        if self.tasks.contains_key(&task.id) {
            self.tasks.insert(task.id.clone(), task);
            Ok(())
        } else {
            Err(A2AError::task_not_found(&task.id))
        }
    }

    pub async fn delete(&self, task_id: &str) -> Result<(), A2AError> {
        self.tasks.remove(task_id);
        Ok(())
    }

    pub async fn list_by_agent(&self, agent_id: &str) -> Vec<Task> {
        self.tasks
            .iter()
            .filter(|entry| entry.value().metadata.agent_id == agent_id)
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub async fn list_by_state(&self, state: TaskState) -> Vec<Task> {
        self.tasks
            .iter()
            .filter(|entry| entry.value().state == state)
            .map(|entry| entry.value().clone())
            .collect()
    }
}
```

---
