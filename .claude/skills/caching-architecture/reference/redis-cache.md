## Contents

- Redis Cache (L2)

## Redis Cache (L2)

### Redis Cache Manager

```rust
use redis::{AsyncCommands, Client, aio::ConnectionManager};

pub struct RedisCache {
    client: ConnectionManager,
    prefix: String,
    default_ttl: Duration,
}

impl RedisCache {
    pub async fn new(redis_url: &str, prefix: &str) -> Result<Self, CacheError> {
        let client = Client::open(redis_url)
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        let manager = ConnectionManager::new(client)
            .await
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        Ok(Self {
            client: manager,
            prefix: prefix.to_string(),
            default_ttl: Duration::from_secs(3600),
        })
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, CacheError> {
        let mut conn = self.client.clone();
        let prefixed = self.prefixed_key(key);

        let result: Option<Vec<u8>> = conn
            .get(&prefixed)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(result)
    }

    pub async fn set(&self, key: &str, value: &[u8], ttl: Option<Duration>) -> Result<(), CacheError> {
        let mut conn = self.client.clone();
        let prefixed = self.prefixed_key(key);
        let ttl = ttl.unwrap_or(self.default_ttl);

        conn.set_ex(&prefixed, value, ttl.as_secs() as usize)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<(), CacheError> {
        let mut conn = self.client.clone();
        let prefixed = self.prefixed_key(key);

        conn.del(&prefixed)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    pub async fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<Option<T>, CacheError> {
        let bytes = self.get(key).await?;

        match bytes {
            Some(b) => {
                let value = serde_json::from_slice(&b)
                    .map_err(|e| CacheError::Serialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    pub async fn set_json<T: serde::Serialize>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<(), CacheError> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| CacheError::Serialization(e.to_string()))?;

        self.set(key, &bytes, ttl).await
    }

    /// Batch get multiple keys
    pub async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<Vec<u8>>>, CacheError> {
        let mut conn = self.client.clone();
        let prefixed: Vec<String> = keys.iter().map(|k| self.prefixed_key(k)).collect();

        let results: Vec<Option<Vec<u8>>> = conn
            .get(&prefixed[..])
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(results)
    }

    /// Pattern-based key deletion
    pub async fn delete_pattern(&self, pattern: &str) -> Result<u64, CacheError> {
        let mut conn = self.client.clone();
        let prefixed_pattern = self.prefixed_key(pattern);

        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(&prefixed_pattern)
            .query_async(&mut conn)
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        if keys.is_empty() {
            return Ok(0);
        }

        let deleted: u64 = conn
            .del(&keys[..])
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(deleted)
    }
}
```
