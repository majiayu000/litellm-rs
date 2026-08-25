## Contents

- Semantic Cache (L3)

## Semantic Cache (L3)

### Vector Database Interface

```rust
#[async_trait]
pub trait VectorCache: Send + Sync {
    async fn search(&self, embedding: &[f32], limit: usize, threshold: f32) -> Result<Vec<CacheHit>, CacheError>;
    async fn insert(&self, key: &str, embedding: &[f32], value: &[u8], metadata: &CacheMetadata) -> Result<(), CacheError>;
    async fn delete(&self, key: &str) -> Result<(), CacheError>;
}

#[derive(Clone)]
pub struct CacheHit {
    pub key: String,
    pub value: Vec<u8>,
    pub score: f32,
    pub metadata: CacheMetadata,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    pub model: String,
    pub created_at: i64,
    pub token_count: u32,
}
```

### Qdrant Implementation

```rust
use qdrant_client::prelude::*;
use qdrant_client::qdrant::{SearchPoints, PointStruct, vectors_config::Config, VectorParams, Distance};

pub struct QdrantCache {
    client: QdrantClient,
    collection_name: String,
    vector_size: u64,
}

impl QdrantCache {
    pub async fn new(url: &str, collection_name: &str, vector_size: u64) -> Result<Self, CacheError> {
        let client = QdrantClient::from_url(url)
            .build()
            .map_err(|e| CacheError::Connection(e.to_string()))?;

        // Create collection if not exists
        let collections = client.list_collections().await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        let exists = collections.collections.iter().any(|c| c.name == collection_name);

        if !exists {
            client.create_collection(&CreateCollection {
                collection_name: collection_name.to_string(),
                vectors_config: Some(VectorsConfig {
                    config: Some(Config::Params(VectorParams {
                        size: vector_size,
                        distance: Distance::Cosine.into(),
                        ..Default::default()
                    })),
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;
        }

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
            vector_size,
        })
    }
}

#[async_trait]
impl VectorCache for QdrantCache {
    async fn search(&self, embedding: &[f32], limit: usize, threshold: f32) -> Result<Vec<CacheHit>, CacheError> {
        let search_result = self.client
            .search_points(&SearchPoints {
                collection_name: self.collection_name.clone(),
                vector: embedding.to_vec(),
                limit: limit as u64,
                score_threshold: Some(threshold),
                with_payload: Some(true.into()),
                ..Default::default()
            })
            .await
            .map_err(|e| CacheError::Operation(e.to_string()))?;

        let hits = search_result.result
            .into_iter()
            .filter_map(|point| {
                let payload = point.payload;
                let key = payload.get("key")?.as_str()?.to_string();
                let value = payload.get("value")?.as_str()?.as_bytes().to_vec();
                let model = payload.get("model")?.as_str()?.to_string();
                let created_at = payload.get("created_at")?.as_integer()?;
                let token_count = payload.get("token_count")?.as_integer()? as u32;

                Some(CacheHit {
                    key,
                    value,
                    score: point.score,
                    metadata: CacheMetadata {
                        model,
                        created_at,
                        token_count,
                    },
                })
            })
            .collect();

        Ok(hits)
    }

    async fn insert(&self, key: &str, embedding: &[f32], value: &[u8], metadata: &CacheMetadata) -> Result<(), CacheError> {
        let point_id = uuid::Uuid::new_v4().to_string();

        let mut payload = serde_json::Map::new();
        payload.insert("key".to_string(), serde_json::json!(key));
        payload.insert("value".to_string(), serde_json::json!(String::from_utf8_lossy(value)));
        payload.insert("model".to_string(), serde_json::json!(metadata.model));
        payload.insert("created_at".to_string(), serde_json::json!(metadata.created_at));
        payload.insert("token_count".to_string(), serde_json::json!(metadata.token_count));

        self.client.upsert_points_blocking(
            &self.collection_name,
            None,
            vec![PointStruct::new(
                point_id,
                embedding.to_vec(),
                payload.into(),
            )],
            None,
        )
        .await
        .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<(), CacheError> {
        self.client.delete_points(
            &self.collection_name,
            None,
            &qdrant_client::qdrant::PointsSelector {
                points_selector_one_of: Some(
                    qdrant_client::qdrant::points_selector::PointsSelectorOneOf::Filter(
                        qdrant_client::qdrant::Filter {
                            must: vec![qdrant_client::qdrant::Condition {
                                condition_one_of: Some(
                                    qdrant_client::qdrant::condition::ConditionOneOf::Field(
                                        qdrant_client::qdrant::FieldCondition {
                                            key: "key".to_string(),
                                            r#match: Some(qdrant_client::qdrant::Match {
                                                match_value: Some(
                                                    qdrant_client::qdrant::r#match::MatchValue::Keyword(key.to_string())
                                                ),
                                            }),
                                            ..Default::default()
                                        }
                                    )
                                ),
                            }],
                            ..Default::default()
                        }
                    )
                ),
            },
            None,
        )
        .await
        .map_err(|e| CacheError::Operation(e.to_string()))?;

        Ok(())
    }
}
```
