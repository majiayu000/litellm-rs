## Agent Registry

### Agent Definition

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub id: String,
    pub name: String,
    pub description: String,
    pub provider: AgentProvider,
    pub capabilities: Vec<Capability>,
    pub endpoint: AgentEndpoint,
    pub status: AgentStatus,
    pub metadata: AgentMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    LangGraph,
    VertexAI,
    AzureAI,
    Bedrock,
    PydanticAI,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: Option<serde_json::Value>,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoint {
    pub url: String,
    pub auth: Option<EndpointAuth>,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EndpointAuth {
    Bearer { token: String },
    ApiKey { header: String, key: String },
    OAuth2 { client_id: String, client_secret: String, token_url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Online,
    Offline,
    Degraded,
    Maintenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    pub version: String,
    pub created_at: i64,
    pub last_seen_at: i64,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub extra: HashMap<String, serde_json::Value>,
}
```

### Registry Implementation

```rust
use dashmap::DashMap;
use std::sync::Arc;

pub struct AgentRegistry {
    agents: DashMap<String, Arc<AgentCard>>,
    health_checker: Arc<HealthChecker>,
}

impl AgentRegistry {
    pub fn new(health_check_interval: Duration) -> Self {
        let registry = Self {
            agents: DashMap::new(),
            health_checker: Arc::new(HealthChecker::new()),
        };

        // Start background health checker
        registry.start_health_monitoring(health_check_interval);

        registry
    }

    pub fn register(&self, agent: AgentCard) -> Result<(), A2AError> {
        // Validate agent configuration
        self.validate_agent(&agent)?;

        let id = agent.id.clone();
        self.agents.insert(id.clone(), Arc::new(agent));

        tracing::info!(agent_id = %id, "Agent registered");
        Ok(())
    }

    pub fn unregister(&self, agent_id: &str) -> Option<Arc<AgentCard>> {
        let removed = self.agents.remove(agent_id).map(|(_, v)| v);
        if removed.is_some() {
            tracing::info!(agent_id = %agent_id, "Agent unregistered");
        }
        removed
    }

    pub fn get(&self, agent_id: &str) -> Option<Arc<AgentCard>> {
        self.agents.get(agent_id).map(|entry| entry.clone())
    }

    pub fn list(&self) -> Vec<Arc<AgentCard>> {
        self.agents.iter().map(|entry| entry.value().clone()).collect()
    }

    pub fn list_online(&self) -> Vec<Arc<AgentCard>> {
        self.agents
            .iter()
            .filter(|entry| entry.value().status == AgentStatus::Online)
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn find_by_capability(&self, capability_name: &str) -> Vec<Arc<AgentCard>> {
        self.agents
            .iter()
            .filter(|entry| {
                entry.value().status == AgentStatus::Online &&
                entry.value().capabilities.iter().any(|c| c.name == capability_name)
            })
            .map(|entry| entry.value().clone())
            .collect()
    }

    pub fn update_status(&self, agent_id: &str, status: AgentStatus) {
        if let Some(mut entry) = self.agents.get_mut(agent_id) {
            let agent = Arc::make_mut(&mut entry);
            agent.status = status;
            agent.metadata.last_seen_at = chrono::Utc::now().timestamp();
        }
    }

    fn validate_agent(&self, agent: &AgentCard) -> Result<(), A2AError> {
        if agent.id.is_empty() {
            return Err(A2AError {
                code: A2AError::INVALID_PARAMS,
                message: "Agent ID cannot be empty".to_string(),
                data: None,
            });
        }

        if agent.capabilities.is_empty() {
            return Err(A2AError {
                code: A2AError::INVALID_PARAMS,
                message: "Agent must have at least one capability".to_string(),
                data: None,
            });
        }

        Ok(())
    }

    fn start_health_monitoring(&self, interval: Duration) {
        let agents = self.agents.clone();
        let health_checker = self.health_checker.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;

                for entry in agents.iter() {
                    let agent = entry.value().clone();
                    let agents_ref = agents.clone();
                    let checker = health_checker.clone();

                    tokio::spawn(async move {
                        let status = checker.check(&agent).await;
                        if let Some(mut entry) = agents_ref.get_mut(&agent.id) {
                            let agent = Arc::make_mut(&mut entry);
                            agent.status = status;
                            agent.metadata.last_seen_at = chrono::Utc::now().timestamp();
                        }
                    });
                }
            }
        });
    }
}
```

---
