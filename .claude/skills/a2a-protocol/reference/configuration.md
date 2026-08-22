## Configuration

```yaml
a2a:
  enabled: true

  gateway:
    health_check_interval_seconds: 30
    task_timeout_seconds: 300
    max_concurrent_tasks: 100

  agents:
    - id: "research-agent"
      name: "Research Agent"
      description: "Performs web research and summarization"
      provider: "langgraph"
      endpoint:
        url: "http://localhost:8100"
        auth:
          type: "bearer"
          token: ${LANGGRAPH_API_KEY}
        timeout_seconds: 120
      capabilities:
        - name: "web_search"
          description: "Search the web for information"
        - name: "summarize"
          description: "Summarize documents and content"

    - id: "code-agent"
      name: "Code Agent"
      description: "Writes and reviews code"
      provider: "vertex_ai"
      endpoint:
        url: "https://us-central1-aiplatform.googleapis.com"
        auth:
          type: "oauth2"
          client_id: ${GOOGLE_CLIENT_ID}
          client_secret: ${GOOGLE_CLIENT_SECRET}
          token_url: "https://oauth2.googleapis.com/token"
        timeout_seconds: 180
      capabilities:
        - name: "code_generation"
          description: "Generate code from requirements"
        - name: "code_review"
          description: "Review and improve code"

    - id: "data-agent"
      name: "Data Analysis Agent"
      description: "Analyzes data and creates visualizations"
      provider: "bedrock"
      endpoint:
        url: "https://bedrock-runtime.us-east-1.amazonaws.com"
        timeout_seconds: 240
      capabilities:
        - name: "data_analysis"
          description: "Analyze datasets"
        - name: "visualization"
          description: "Create charts and graphs"
```

---
