//! PostgreSQL pgvector Provider
//!
//! Vector storage provider using PostgreSQL with pgvector extension.
//! Provides vector storage, similarity search, and index management.

use std::sync::Arc;

use tracing::{debug, info};

use super::config::{DistanceMetric, IndexType, PROVIDER_NAME, PgVectorConfig};
use super::models::{SearchOptions, SearchResult, VectorPoint};
use crate::core::providers::unified_provider::ProviderError;

/// PostgreSQL pgvector provider for vector storage and similarity search
#[derive(Debug, Clone)]
pub struct PgVectorProvider {
    /// Configuration
    config: PgVectorConfig,
    /// HTTP client for PostgreSQL REST API (when using PostgREST/Supabase)
    /// or internal connection state
    _client: Arc<reqwest::Client>,
    /// Connection URL for display/logging (without password)
    _safe_url: String,
}

impl PgVectorProvider {
    /// Create a new PgVector provider with the given configuration
    pub async fn new(config: PgVectorConfig) -> Result<Self, ProviderError> {
        // Validate configuration
        config.validate()?;

        // Create HTTP client for potential REST API access
        let client = Arc::new(reqwest::Client::new());

        // Create safe URL for logging (hide password)
        let safe_url = Self::make_safe_url(&config.database_url);

        let provider = Self {
            config,
            _client: client,
            _safe_url: safe_url,
        };

        info!(
            "PgVector provider initialized for table: {}",
            provider.config.full_table_name()
        );

        Ok(provider)
    }

    /// Create provider from environment variables
    pub async fn from_env() -> Result<Self, ProviderError> {
        let config = PgVectorConfig::from_env()?;
        Self::new(config).await
    }

    /// Create a safe URL for logging (password hidden)
    fn make_safe_url(url: &str) -> String {
        // Simple password masking
        if let Some(at_pos) = url.find('@')
            && let Some(colon_pos) = url[..at_pos].rfind(':')
        {
            let prefix = &url[..colon_pos + 1];
            let suffix = &url[at_pos..];
            return format!("{}****{}", prefix, suffix);
        }
        url.to_string()
    }

    /// Get the provider name
    pub fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    /// Get the configuration
    pub fn config(&self) -> &PgVectorConfig {
        &self.config
    }

    /// Generate SQL for creating the vector extension
    pub fn create_extension_sql(&self) -> String {
        "CREATE EXTENSION IF NOT EXISTS vector".to_string()
    }

    /// Generate SQL for creating the embeddings table
    pub fn create_table_sql(&self) -> String {
        format!(
            r#"
CREATE TABLE IF NOT EXISTS {} (
    id TEXT PRIMARY KEY,
    embedding vector({}),
    metadata JSONB,
    content TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
)"#,
            self.config.full_table_name(),
            self.config.dimension
        )
    }

    /// Generate SQL for creating an index on the vector column
    pub fn create_index_sql(&self) -> Option<String> {
        let index_name = format!(
            "{}_{}_embedding_idx",
            self.config.schema, self.config.table_name
        );
        let quoted_index_name = format!("\"{}\"", index_name);
        let full_table = self.config.full_table_name();
        let ops_class = self
            .config
            .distance_metric
            .index_ops(self.config.index_type);

        match self.config.index_type {
            IndexType::IvfFlat => {
                let lists = self.config.ivfflat_lists.unwrap_or(100);
                Some(format!(
                    "CREATE INDEX IF NOT EXISTS {} ON {} USING ivfflat (embedding {}) WITH (lists = {})",
                    quoted_index_name, full_table, ops_class, lists
                ))
            }
            IndexType::Hnsw => {
                let m = self.config.hnsw_m.unwrap_or(16);
                let ef_construction = self.config.hnsw_ef_construction.unwrap_or(64);
                Some(format!(
                    "CREATE INDEX IF NOT EXISTS {} ON {} USING hnsw (embedding {}) WITH (m = {}, ef_construction = {})",
                    quoted_index_name, full_table, ops_class, m, ef_construction
                ))
            }
            IndexType::None => None,
        }
    }

    /// Generate SQL for inserting a vector (upsert)
    pub fn upsert_sql(&self) -> String {
        format!(
            r#"
INSERT INTO {} (id, embedding, metadata, content, updated_at)
VALUES ($1, $2::vector, $3, $4, CURRENT_TIMESTAMP)
ON CONFLICT (id) DO UPDATE SET
    embedding = EXCLUDED.embedding,
    metadata = EXCLUDED.metadata,
    content = EXCLUDED.content,
    updated_at = CURRENT_TIMESTAMP"#,
            self.config.full_table_name()
        )
    }

    /// Generate SQL for batch upsert
    pub fn batch_upsert_sql(&self, count: usize) -> String {
        let mut values = Vec::with_capacity(count);
        for i in 0..count {
            let base = i * 4;
            values.push(format!(
                "(${}, ${}::vector, ${}, ${})",
                base + 1,
                base + 2,
                base + 3,
                base + 4
            ));
        }

        format!(
            r#"
INSERT INTO {} (id, embedding, metadata, content)
VALUES {}
ON CONFLICT (id) DO UPDATE SET
    embedding = EXCLUDED.embedding,
    metadata = EXCLUDED.metadata,
    content = EXCLUDED.content,
    updated_at = CURRENT_TIMESTAMP"#,
            self.config.full_table_name(),
            values.join(", ")
        )
    }

    /// Generate SQL for similarity search
    pub fn search_sql(&self, options: &SearchOptions) -> String {
        let operator = self.config.distance_metric.operator();
        let full_table = self.config.full_table_name();

        let mut select_columns = vec!["id".to_string()];

        // For cosine distance, convert to similarity (1 - distance)
        let score_expr = match self.config.distance_metric {
            DistanceMetric::Cosine => format!("1 - (embedding {} $1::vector) as score", operator),
            DistanceMetric::L2 => format!("embedding {} $1::vector as score", operator),
            DistanceMetric::InnerProduct => {
                format!("-(embedding {} $1::vector) as score", operator)
            }
        };
        select_columns.push(score_expr);

        if options.include_metadata {
            select_columns.push("metadata".to_string());
        }

        if options.include_content {
            select_columns.push("content".to_string());
        }

        if options.include_vector {
            select_columns.push("embedding::text as vector".to_string());
        }

        let mut sql = format!("SELECT {} FROM {}", select_columns.join(", "), full_table);

        // Add WHERE clause for threshold and metadata filter
        let mut conditions = Vec::new();
        // Parameter index starts at 2 (1 is the vector)
        let mut _param_index = 2;
        let mut _filter_params = Vec::new();

        if let Some(threshold) = options.threshold {
            let threshold_condition = match self.config.distance_metric {
                DistanceMetric::Cosine => {
                    format!("(embedding {} $1::vector) <= {}", operator, 1.0 - threshold)
                }
                DistanceMetric::L2 => {
                    format!("(embedding {} $1::vector) <= {}", operator, threshold)
                }
                DistanceMetric::InnerProduct => {
                    format!("(embedding {} $1::vector) >= {}", operator, -threshold)
                }
            };
            conditions.push(threshold_condition);
        }

        // Use safe parameterized filters instead of raw SQL string
        if let Some(ref filters) = options.metadata_filters
            && !filters.is_empty()
        {
            let (filter_sql, params) = filters.to_sql_with_params(_param_index);
            if !filter_sql.is_empty() {
                conditions.push(filter_sql);
                _filter_params = params;
                _param_index += _filter_params.len();
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        // Order by distance
        sql.push_str(&format!(" ORDER BY embedding {} $1::vector", operator));

        // Limit
        sql.push_str(&format!(" LIMIT {}", options.limit));

        sql
    }

    /// Generate SQL for getting a vector by ID
    pub fn get_by_id_sql(&self) -> String {
        format!(
            "SELECT id, embedding::text as vector, metadata, content FROM {} WHERE id = $1",
            self.config.full_table_name()
        )
    }

    /// Generate SQL for deleting a vector by ID
    pub fn delete_sql(&self) -> String {
        format!(
            "DELETE FROM {} WHERE id = $1",
            self.config.full_table_name()
        )
    }

    /// Generate SQL for counting vectors
    pub fn count_sql(&self) -> String {
        format!("SELECT COUNT(*) FROM {}", self.config.full_table_name())
    }

    /// Generate SQL for table statistics
    pub fn stats_sql(&self) -> String {
        let full_table = self.config.full_table_name();
        format!(
            r#"
SELECT
    (SELECT COUNT(*) FROM {table}) as total_vectors,
    pg_total_relation_size({table}::regclass) as table_size,
    (SELECT pg_relation_size(indexrelid)
     FROM pg_index
     WHERE indrelid = {table}::regclass
     LIMIT 1) as index_size
"#,
            table = full_table
        )
    }

    /// Parse a vector from PostgreSQL text format "[0.1,0.2,0.3]"
    pub fn parse_vector(text: &str) -> Result<Vec<f32>, ProviderError> {
        let trimmed = text.trim_matches(|c| c == '[' || c == ']');
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }

        trimmed
            .split(',')
            .map(|s| {
                s.trim().parse::<f32>().map_err(|e| {
                    ProviderError::response_parsing(
                        PROVIDER_NAME,
                        format!("Failed to parse vector component '{}': {}", s, e),
                    )
                })
            })
            .collect()
    }

    /// Format a vector for PostgreSQL "[0.1,0.2,0.3]"
    pub fn format_vector(vector: &[f32]) -> String {
        let components: Vec<String> = vector.iter().map(|v| v.to_string()).collect();
        format!("[{}]", components.join(","))
    }

    /// Store a single vector point
    /// Note: This generates the SQL; actual execution requires a database connection
    pub fn prepare_store(&self, point: &VectorPoint) -> Result<PreparedStatement, ProviderError> {
        // Validate dimension
        if point.dimension() != self.config.dimension {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!(
                    "Vector dimension mismatch: expected {}, got {}",
                    self.config.dimension,
                    point.dimension()
                ),
            ));
        }

        let sql = self.upsert_sql();
        let vector_str = Self::format_vector(&point.vector);
        let metadata_str = point
            .metadata
            .as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_default());

        debug!("Prepared store for vector: {}", point.id);

        Ok(PreparedStatement {
            sql,
            params: vec![
                StatementParam::Text(point.id.clone()),
                StatementParam::Text(vector_str),
                StatementParam::Json(metadata_str),
                StatementParam::Text(point.content.clone().unwrap_or_default()),
            ],
        })
    }

    /// Prepare a similarity search query
    pub fn prepare_search(
        &self,
        query_vector: &[f32],
        options: SearchOptions,
    ) -> Result<PreparedStatement, ProviderError> {
        // Validate dimension
        if query_vector.len() != self.config.dimension {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                format!(
                    "Query vector dimension mismatch: expected {}, got {}",
                    self.config.dimension,
                    query_vector.len()
                ),
            ));
        }

        let sql = self.search_sql(&options);
        let vector_str = Self::format_vector(query_vector);

        debug!(
            "Prepared search with limit {} and threshold {:?}",
            options.limit, options.threshold
        );

        Ok(PreparedStatement {
            sql,
            params: vec![StatementParam::Text(vector_str)],
        })
    }

    /// Prepare a get by ID query
    pub fn prepare_get(&self, id: &str) -> PreparedStatement {
        PreparedStatement {
            sql: self.get_by_id_sql(),
            params: vec![StatementParam::Text(id.to_string())],
        }
    }

    /// Prepare a delete query
    pub fn prepare_delete(&self, id: &str) -> PreparedStatement {
        PreparedStatement {
            sql: self.delete_sql(),
            params: vec![StatementParam::Text(id.to_string())],
        }
    }

    /// Health check - validates configuration
    pub async fn health_check(&self) -> Result<(), ProviderError> {
        // Basic validation check
        self.config.validate()?;

        debug!("PgVector provider health check passed");
        Ok(())
    }

    /// Get table statistics (SQL only, needs connection to execute)
    pub fn get_stats_sql(&self) -> String {
        self.stats_sql()
    }
}

/// Prepared SQL statement with parameters
#[derive(Debug, Clone)]
pub struct PreparedStatement {
    /// The SQL query
    pub sql: String,
    /// Query parameters
    pub params: Vec<StatementParam>,
}

/// Parameter types for prepared statements
#[derive(Debug, Clone)]
pub enum StatementParam {
    /// Text parameter
    Text(String),
    /// JSON parameter (as string)
    Json(Option<String>),
    /// Integer parameter
    Int(i64),
    /// Float parameter
    Float(f64),
}

impl StatementParam {
    /// Convert to a string representation for SQL
    pub fn to_sql_string(&self) -> String {
        match self {
            StatementParam::Text(s) => format!("'{}'", s.replace('\'', "''")),
            StatementParam::Json(Some(s)) => format!("'{}'::jsonb", s.replace('\'', "''")),
            StatementParam::Json(None) => "NULL".to_string(),
            StatementParam::Int(i) => i.to_string(),
            StatementParam::Float(f) => f.to_string(),
        }
    }
}

/// Trait for executing pgvector operations
/// This trait can be implemented by different database backends
#[async_trait::async_trait]
pub trait PgVectorExecutor: Send + Sync {
    /// Execute a statement that doesn't return rows
    async fn execute(&self, stmt: &PreparedStatement) -> Result<u64, ProviderError>;

    /// Execute a query and return results
    async fn query(&self, stmt: &PreparedStatement) -> Result<Vec<QueryRow>, ProviderError>;

    /// Execute raw SQL
    async fn execute_raw(&self, sql: &str) -> Result<(), ProviderError>;
}

/// A row returned from a query
#[derive(Debug, Clone)]
pub struct QueryRow {
    /// Column values as JSON
    pub columns: serde_json::Value,
}

impl QueryRow {
    /// Get a string column
    pub fn get_string(&self, column: &str) -> Option<String> {
        self.columns.get(column)?.as_str().map(|s| s.to_string())
    }

    /// Get a float column
    pub fn get_f32(&self, column: &str) -> Option<f32> {
        self.columns.get(column)?.as_f64().map(|f| f as f32)
    }

    /// Get a JSON column
    pub fn get_json(&self, column: &str) -> Option<serde_json::Value> {
        self.columns.get(column).cloned()
    }
}

/// Helper to convert query rows to search results
impl From<QueryRow> for SearchResult {
    fn from(row: QueryRow) -> Self {
        let id = row.get_string("id").unwrap_or_default();
        let score = row.get_f32("score").unwrap_or(0.0);
        let metadata = row.get_json("metadata");
        let content = row.get_string("content");
        let vector = row
            .get_string("vector")
            .and_then(|v| PgVectorProvider::parse_vector(&v).ok());

        SearchResult {
            id,
            score,
            metadata,
            content,
            vector,
        }
    }
}

/// Helper to convert query rows to vector points
impl From<QueryRow> for VectorPoint {
    fn from(row: QueryRow) -> Self {
        let id = row.get_string("id").unwrap_or_default();
        let vector = row
            .get_string("vector")
            .and_then(|v| PgVectorProvider::parse_vector(&v).ok())
            .unwrap_or_default();
        let metadata = row.get_json("metadata");
        let content = row.get_string("content");

        VectorPoint {
            id,
            vector,
            metadata,
            content,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PgVectorConfig {
        PgVectorConfig::new("postgresql://localhost:5432/test")
            .with_table_name("test_embeddings")
            .with_dimension(1536)
    }

    #[tokio::test]
    async fn test_provider_creation() {
        let config = test_config();
        let provider = PgVectorProvider::new(config).await;
        assert!(provider.is_ok());
    }

    #[test]
    fn test_create_extension_sql() {
        let config = test_config();
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.create_extension_sql();
        assert!(sql.contains("CREATE EXTENSION"));
        assert!(sql.contains("vector"));
    }

    #[test]
    fn test_create_table_sql() {
        let config = test_config();
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.create_table_sql();
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("embedding vector(1536)"));
        assert!(sql.contains("metadata JSONB"));
    }

    #[test]
    fn test_create_index_sql_ivfflat() {
        let config = test_config().with_index_type(IndexType::IvfFlat);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.create_index_sql();
        assert!(sql.is_some());
        let sql = sql.unwrap();
        assert!(sql.contains("ivfflat"));
        assert!(sql.contains("vector_cosine_ops"));
    }

    #[test]
    fn test_create_index_sql_hnsw() {
        let config = test_config().with_index_type(IndexType::Hnsw);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.create_index_sql();
        assert!(sql.is_some());
        let sql = sql.unwrap();
        assert!(sql.contains("hnsw"));
        assert!(sql.contains("ef_construction"));
    }

    #[test]
    fn test_create_index_sql_none() {
        let config = test_config().with_index_type(IndexType::None);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.create_index_sql();
        assert!(sql.is_none());
    }

    #[test]
    fn test_upsert_sql() {
        let config = test_config();
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let sql = provider.upsert_sql();
        assert!(sql.contains("INSERT INTO"));
        assert!(sql.contains("ON CONFLICT"));
        assert!(sql.contains("DO UPDATE"));
    }

    #[test]
    fn test_search_sql_cosine() {
        let config = test_config().with_distance_metric(DistanceMetric::Cosine);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let options = SearchOptions::new(10).with_threshold(0.8);
        let sql = provider.search_sql(&options);
        assert!(sql.contains("<=>"));
        assert!(sql.contains("LIMIT 10"));
        assert!(sql.contains("1 -")); // Cosine similarity conversion
    }

    #[test]
    fn test_search_sql_l2() {
        let config = test_config().with_distance_metric(DistanceMetric::L2);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let options = SearchOptions::new(5);
        let sql = provider.search_sql(&options);
        assert!(sql.contains("<->"));
        assert!(sql.contains("LIMIT 5"));
    }

    #[test]
    fn test_search_sql_inner_product() {
        let config = test_config().with_distance_metric(DistanceMetric::InnerProduct);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let options = SearchOptions::new(20);
        let sql = provider.search_sql(&options);
        assert!(sql.contains("<#>"));
        assert!(sql.contains("LIMIT 20"));
    }

    #[test]
    fn test_parse_vector() {
        let vector = PgVectorProvider::parse_vector("[0.1,0.2,0.3]").unwrap();
        assert_eq!(vector.len(), 3);
        assert!((vector[0] - 0.1).abs() < f32::EPSILON);
        assert!((vector[1] - 0.2).abs() < f32::EPSILON);
        assert!((vector[2] - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_format_vector() {
        let vector = vec![0.1, 0.2, 0.3];
        let formatted = PgVectorProvider::format_vector(&vector);
        assert_eq!(formatted, "[0.1,0.2,0.3]");
    }

    #[test]
    fn test_prepare_store_dimension_mismatch() {
        let config = test_config().with_dimension(1536);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let point = VectorPoint::new("test", vec![0.1, 0.2, 0.3]); // Only 3 dimensions
        let result = provider.prepare_store(&point);
        assert!(result.is_err());
    }

    #[test]
    fn test_prepare_store_valid() {
        let config = test_config().with_dimension(3);
        let provider = tokio_test::block_on(PgVectorProvider::new(config)).unwrap();
        let point = VectorPoint::new("test", vec![0.1, 0.2, 0.3]);
        let result = provider.prepare_store(&point);
        assert!(result.is_ok());
    }

    #[test]
    fn test_make_safe_url() {
        let url = "postgresql://user:secretpassword@localhost:5432/db";
        let safe = PgVectorProvider::make_safe_url(url);
        assert!(safe.contains("****"));
        assert!(!safe.contains("secretpassword"));
    }

    #[test]
    fn test_statement_param_to_sql() {
        assert_eq!(
            StatementParam::Text("test".to_string()).to_sql_string(),
            "'test'"
        );
        assert_eq!(
            StatementParam::Json(Some(r#"{"key":"value"}"#.to_string())).to_sql_string(),
            r#"'{"key":"value"}'::jsonb"#
        );
        assert_eq!(StatementParam::Json(None).to_sql_string(), "NULL");
        assert_eq!(StatementParam::Int(42).to_sql_string(), "42");
        assert_eq!(StatementParam::Float(3.15).to_sql_string(), "3.15");
    }

    #[test]
    fn test_statement_param_escaping() {
        let text_with_quote = StatementParam::Text("it's a test".to_string());
        assert_eq!(text_with_quote.to_sql_string(), "'it''s a test'");
    }
}
