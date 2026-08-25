## Configuration

### Auth Configuration

```yaml
auth:
  enabled: true

  jwt:
    secret: ${JWT_SECRET}  # Must be at least 64 characters
    issuer: "litellm-gateway"
    audience: "litellm-api"
    token_expiry_seconds: 3600

  api_key:
    enabled: true
    key_length: 64
    prefix: "sk-"

  rate_limiting:
    enabled: true
    requests_per_minute: 60
    tokens_per_minute: 100000
    window_size_seconds: 60

  rbac:
    enabled: true
    default_role: "user"
    roles:
      - name: "admin"
        permissions:
          - "*"  # All permissions
      - name: "user"
        permissions:
          - "chat_completion"
          - "chat_completion_stream"
          - "embeddings"
          - "list_models"
      - name: "readonly"
        permissions:
          - "list_models"
          - "get_model_info"
```
