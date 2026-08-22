## Contents

- Configuration Validation
- Hot Reloading

---

## Configuration Validation

### Validation Trait

```rust
pub trait Validate {
    fn validate(&self) -> Result<(), ConfigError>;
}

impl Validate for GatewayConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        self.server.validate()?;
        self.auth.validate()?;
        self.providers.validate()?;
        self.routing.validate()?;
        self.cache.validate()?;
        Ok(())
    }
}

impl Validate for ServerConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.port == 0 {
            return Err(ConfigError::Validation("Port cannot be 0".to_string()));
        }

        if self.request_timeout == 0 {
            return Err(ConfigError::Validation("Request timeout cannot be 0".to_string()));
        }

        // Parse max request size
        parse_size(&self.max_request_size)
            .map_err(|e| ConfigError::Validation(format!("Invalid max_request_size: {}", e)))?;

        Ok(())
    }
}

impl Validate for AuthConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        if self.enabled {
            if self.jwt.secret.is_none() && !self.api_key.enabled {
                return Err(ConfigError::Validation(
                    "At least one auth method must be configured when auth is enabled".to_string()
                ));
            }

            if let Some(secret) = &self.jwt.secret {
                if secret.len() < 32 {
                    return Err(ConfigError::Validation(
                        "JWT secret must be at least 32 characters".to_string()
                    ));
                }
            }
        }

        Ok(())
    }
}

impl Validate for ProvidersConfig {
    fn validate(&self) -> Result<(), ConfigError> {
        let mut enabled_count = 0;

        if let Some(ref openai) = self.openai {
            if openai.enabled {
                if openai.api_key.is_none() {
                    return Err(ConfigError::Validation(
                        "OpenAI provider enabled but api_key not set".to_string()
                    ));
                }
                enabled_count += 1;
            }
        }

        // Similar validation for other providers...

        if enabled_count == 0 {
            return Err(ConfigError::Validation(
                "At least one provider must be enabled".to_string()
            ));
        }

        Ok(())
    }
}
```

---

## Hot Reloading

### Config Watcher

```rust
use notify::{Watcher, RecursiveMode, watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

pub struct ConfigWatcher {
    config: Arc<RwLock<GatewayConfig>>,
    loader: ConfigLoader,
    path: String,
}

impl ConfigWatcher {
    pub fn new(path: &str, initial_config: GatewayConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(initial_config)),
            loader: ConfigLoader::new(),
            path: path.to_string(),
        }
    }

    pub fn get_config(&self) -> GatewayConfig {
        self.config.read().unwrap().clone()
    }

    pub fn start_watching(&self) -> Result<(), ConfigError> {
        let (tx, rx) = channel();
        let config = self.config.clone();
        let loader = ConfigLoader::new();
        let path = self.path.clone();

        // Create watcher
        let mut watcher = watcher(tx, Duration::from_secs(2))
            .map_err(|e| ConfigError::Watch(e.to_string()))?;

        watcher.watch(&self.path, RecursiveMode::NonRecursive)
            .map_err(|e| ConfigError::Watch(e.to_string()))?;

        // Spawn watch thread
        std::thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(notify::DebouncedEvent::Write(_)) |
                    Ok(notify::DebouncedEvent::Create(_)) => {
                        match loader.load_from_file(&path) {
                            Ok(new_config) => {
                                let mut config = config.write().unwrap();
                                *config = new_config;
                                tracing::info!("Configuration reloaded successfully");
                            }
                            Err(e) => {
                                tracing::error!("Failed to reload config: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Watch error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(())
    }
}
```
