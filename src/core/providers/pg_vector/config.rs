//! PostgreSQL pgvector Configuration
//!
//! Configuration for PostgreSQL with pgvector extension.

use std::env;

use crate::core::providers::unified_provider::ProviderError;

/// Provider name constant
pub const PROVIDER_NAME: &str = "pg_vector";

/// Index type for pgvector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexType {
    /// IVFFlat index - good balance of speed and accuracy
    #[default]
    IvfFlat,
    /// HNSW index - faster search, higher memory usage
    Hnsw,
    /// No index - exact search, slowest but most accurate
    None,
}

impl std::fmt::Display for IndexType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexType::IvfFlat => write!(f, "ivfflat"),
            IndexType::Hnsw => write!(f, "hnsw"),
            IndexType::None => write!(f, "none"),
        }
    }
}

impl std::str::FromStr for IndexType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ivfflat" | "ivf_flat" | "ivf" => Ok(IndexType::IvfFlat),
            "hnsw" => Ok(IndexType::Hnsw),
            "none" | "" => Ok(IndexType::None),
            _ => Err(format!("Unknown index type: {}", s)),
        }
    }
}

/// Distance metric for similarity search
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DistanceMetric {
    /// L2 (Euclidean) distance - use <-> operator
    L2,
    /// Cosine distance - use <=> operator
    #[default]
    Cosine,
    /// Inner product (negative) - use <#> operator
    InnerProduct,
}

impl DistanceMetric {
    /// Get the SQL operator for this distance metric
    pub fn operator(&self) -> &'static str {
        match self {
            DistanceMetric::L2 => "<->",
            DistanceMetric::Cosine => "<=>",
            DistanceMetric::InnerProduct => "<#>",
        }
    }

    /// Get the index ops class for this distance metric
    pub fn index_ops(&self, index_type: IndexType) -> &'static str {
        match (index_type, self) {
            (IndexType::IvfFlat, DistanceMetric::L2) => "vector_l2_ops",
            (IndexType::IvfFlat, DistanceMetric::Cosine) => "vector_cosine_ops",
            (IndexType::IvfFlat, DistanceMetric::InnerProduct) => "vector_ip_ops",
            (IndexType::Hnsw, DistanceMetric::L2) => "vector_l2_ops",
            (IndexType::Hnsw, DistanceMetric::Cosine) => "vector_cosine_ops",
            (IndexType::Hnsw, DistanceMetric::InnerProduct) => "vector_ip_ops",
            (IndexType::None, _) => "",
        }
    }
}

impl std::fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceMetric::L2 => write!(f, "l2"),
            DistanceMetric::Cosine => write!(f, "cosine"),
            DistanceMetric::InnerProduct => write!(f, "inner_product"),
        }
    }
}

impl std::str::FromStr for DistanceMetric {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "l2" | "euclidean" => Ok(DistanceMetric::L2),
            "cosine" => Ok(DistanceMetric::Cosine),
            "inner_product" | "ip" | "dot" => Ok(DistanceMetric::InnerProduct),
            _ => Err(format!("Unknown distance metric: {}", s)),
        }
    }
}

/// Configuration for PostgreSQL pgvector provider
#[derive(Debug, Clone)]
pub struct PgVectorConfig {
    /// PostgreSQL connection string (required)
    /// Format: postgresql://user:password@host:port/database
    pub database_url: String,

    /// Table name for storing vectors (default: "embeddings")
    pub table_name: String,

    /// Vector dimension (default: 1536 for OpenAI embeddings)
    pub dimension: usize,

    /// Index type to use
    pub index_type: IndexType,

    /// Distance metric for similarity search
    pub distance_metric: DistanceMetric,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Connection timeout in seconds
    pub connection_timeout: u64,

    /// Whether to create the table automatically
    pub auto_create_table: bool,

    /// Whether to create the index automatically
    pub auto_create_index: bool,

    /// IVFFlat lists parameter (for IVFFlat index)
    /// Recommended: rows / 1000 for tables up to 1M rows
    pub ivfflat_lists: Option<u32>,

    /// HNSW m parameter (for HNSW index) - max number of connections per layer
    pub hnsw_m: Option<u32>,

    /// HNSW ef_construction parameter (for HNSW index)
    pub hnsw_ef_construction: Option<u32>,

    /// Schema name (default: "public")
    pub schema: String,
}

impl Default for PgVectorConfig {
    fn default() -> Self {
        Self {
            database_url: String::new(),
            table_name: "embeddings".to_string(),
            dimension: 1536, // OpenAI default
            index_type: IndexType::default(),
            distance_metric: DistanceMetric::default(),
            max_connections: 10,
            connection_timeout: 30,
            auto_create_table: true,
            auto_create_index: true,
            ivfflat_lists: None,
            hnsw_m: Some(16),
            hnsw_ef_construction: Some(64),
            schema: "public".to_string(),
        }
    }
}

impl PgVectorConfig {
    /// Create a new config with the given database URL
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            ..Default::default()
        }
    }

    /// Create config from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        let database_url = env::var("PG_VECTOR_DATABASE_URL")
            .or_else(|_| env::var("PGVECTOR_URL"))
            .or_else(|_| env::var("DATABASE_URL"))
            .map_err(|_| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    "PG_VECTOR_DATABASE_URL, PGVECTOR_URL, or DATABASE_URL environment variable is required",
                )
            })?;

        let mut config = Self::new(database_url);

        // Optional environment variables
        if let Ok(table_name) = env::var("PG_VECTOR_TABLE_NAME") {
            config.table_name = table_name;
        }

        if let Ok(dimension) = env::var("PG_VECTOR_DIMENSION") {
            config.dimension = dimension.parse().map_err(|_| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    format!("Invalid PG_VECTOR_DIMENSION value: '{dimension}' (expected integer)"),
                )
            })?;
        }

        if let Ok(index_type) = env::var("PG_VECTOR_INDEX_TYPE") {
            config.index_type = index_type.parse().map_err(|e| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    format!("Invalid PG_VECTOR_INDEX_TYPE: {e}"),
                )
            })?;
        }

        if let Ok(metric) = env::var("PG_VECTOR_DISTANCE_METRIC") {
            config.distance_metric = metric.parse().map_err(|e| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    format!("Invalid PG_VECTOR_DISTANCE_METRIC: {e}"),
                )
            })?;
        }

        if let Ok(max_conn) = env::var("PG_VECTOR_MAX_CONNECTIONS") {
            config.max_connections = max_conn.parse().map_err(|_| {
                ProviderError::configuration(
                    PROVIDER_NAME,
                    format!(
                        "Invalid PG_VECTOR_MAX_CONNECTIONS value: '{max_conn}' (expected integer)"
                    ),
                )
            })?;
        }

        if let Ok(schema) = env::var("PG_VECTOR_SCHEMA") {
            config.schema = schema;
        }

        Ok(config)
    }

    /// Set the table name
    pub fn with_table_name(mut self, table_name: impl Into<String>) -> Self {
        self.table_name = table_name.into();
        self
    }

    /// Set the vector dimension
    pub fn with_dimension(mut self, dimension: usize) -> Self {
        self.dimension = dimension;
        self
    }

    /// Set the index type
    pub fn with_index_type(mut self, index_type: IndexType) -> Self {
        self.index_type = index_type;
        self
    }

    /// Set the distance metric
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set maximum connections
    pub fn with_max_connections(mut self, max_connections: u32) -> Self {
        self.max_connections = max_connections;
        self
    }

    /// Set connection timeout
    pub fn with_connection_timeout(mut self, timeout: u64) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Set auto create table flag
    pub fn with_auto_create_table(mut self, auto_create: bool) -> Self {
        self.auto_create_table = auto_create;
        self
    }

    /// Set auto create index flag
    pub fn with_auto_create_index(mut self, auto_create: bool) -> Self {
        self.auto_create_index = auto_create;
        self
    }

    /// Set schema name
    pub fn with_schema(mut self, schema: impl Into<String>) -> Self {
        self.schema = schema.into();
        self
    }

    /// Set IVFFlat lists parameter
    pub fn with_ivfflat_lists(mut self, lists: u32) -> Self {
        self.ivfflat_lists = Some(lists);
        self
    }

    /// Set HNSW m parameter
    pub fn with_hnsw_m(mut self, m: u32) -> Self {
        self.hnsw_m = Some(m);
        self
    }

    /// Set HNSW ef_construction parameter
    pub fn with_hnsw_ef_construction(mut self, ef_construction: u32) -> Self {
        self.hnsw_ef_construction = Some(ef_construction);
        self
    }

    /// Get the fully qualified table name with PostgreSQL identifier quoting
    pub fn full_table_name(&self) -> String {
        format!("\"{}\".\"{}\"", self.schema, self.table_name)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), ProviderError> {
        if self.database_url.is_empty() {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Database URL cannot be empty",
            ));
        }

        if !self.database_url.starts_with("postgresql://")
            && !self.database_url.starts_with("postgres://")
        {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Database URL must start with postgresql:// or postgres://",
            ));
        }

        if self.table_name.is_empty() {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Table name cannot be empty",
            ));
        }

        if !self
            .table_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Table name must contain only alphanumeric characters and underscores",
            ));
        }

        if self.schema.is_empty() {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Schema name cannot be empty",
            ));
        }

        if !self
            .schema
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Schema name must contain only alphanumeric characters and underscores",
            ));
        }

        if self.dimension == 0 {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Dimension must be greater than 0",
            ));
        }

        // pgvector has a max dimension of 16000
        if self.dimension > 16000 {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Dimension cannot exceed 16000 (pgvector limit)",
            ));
        }

        if self.max_connections == 0 {
            return Err(ProviderError::configuration(
                PROVIDER_NAME,
                "Max connections must be greater than 0",
            ));
        }

        Ok(())
    }
}

/// Builder for PgVectorConfig
#[derive(Debug, Default)]
pub struct PgVectorConfigBuilder {
    config: PgVectorConfig,
}

impl PgVectorConfigBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the database URL
    pub fn database_url(mut self, url: impl Into<String>) -> Self {
        self.config.database_url = url.into();
        self
    }

    /// Set the table name
    pub fn table_name(mut self, name: impl Into<String>) -> Self {
        self.config.table_name = name.into();
        self
    }

    /// Set the dimension
    pub fn dimension(mut self, dimension: usize) -> Self {
        self.config.dimension = dimension;
        self
    }

    /// Set the index type
    pub fn index_type(mut self, index_type: IndexType) -> Self {
        self.config.index_type = index_type;
        self
    }

    /// Set the distance metric
    pub fn distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.config.distance_metric = metric;
        self
    }

    /// Build and validate the config
    pub fn build(self) -> Result<PgVectorConfig, ProviderError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PgVectorConfig::default();
        assert_eq!(config.table_name, "embeddings");
        assert_eq!(config.dimension, 1536);
        assert_eq!(config.index_type, IndexType::IvfFlat);
        assert_eq!(config.distance_metric, DistanceMetric::Cosine);
        assert_eq!(config.schema, "public");
    }

    #[test]
    fn test_config_new() {
        let config = PgVectorConfig::new("postgresql://localhost:5432/test");
        assert_eq!(config.database_url, "postgresql://localhost:5432/test");
    }

    #[test]
    fn test_config_builder() {
        let config = PgVectorConfigBuilder::new()
            .database_url("postgresql://localhost:5432/test")
            .table_name("custom_table")
            .dimension(768)
            .index_type(IndexType::Hnsw)
            .distance_metric(DistanceMetric::L2)
            .build();

        assert!(config.is_ok());
        let config = config.unwrap();
        assert_eq!(config.table_name, "custom_table");
        assert_eq!(config.dimension, 768);
        assert_eq!(config.index_type, IndexType::Hnsw);
        assert_eq!(config.distance_metric, DistanceMetric::L2);
    }

    #[test]
    fn test_config_validation_empty_url() {
        let config = PgVectorConfig::default();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_url() {
        let config = PgVectorConfig::new("mysql://localhost:3306/test");
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_zero_dimension() {
        let mut config = PgVectorConfig::new("postgresql://localhost:5432/test");
        config.dimension = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_dimension_too_large() {
        let mut config = PgVectorConfig::new("postgresql://localhost:5432/test");
        config.dimension = 20000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_index_type_parse() {
        assert_eq!("ivfflat".parse::<IndexType>().unwrap(), IndexType::IvfFlat);
        assert_eq!("hnsw".parse::<IndexType>().unwrap(), IndexType::Hnsw);
        assert_eq!("none".parse::<IndexType>().unwrap(), IndexType::None);
    }

    #[test]
    fn test_distance_metric_operator() {
        assert_eq!(DistanceMetric::L2.operator(), "<->");
        assert_eq!(DistanceMetric::Cosine.operator(), "<=>");
        assert_eq!(DistanceMetric::InnerProduct.operator(), "<#>");
    }

    #[test]
    fn test_full_table_name() {
        let config = PgVectorConfig::new("postgresql://localhost:5432/test")
            .with_schema("custom_schema")
            .with_table_name("custom_table");
        assert_eq!(
            config.full_table_name(),
            "\"custom_schema\".\"custom_table\""
        );
    }

    #[test]
    fn test_validate_rejects_special_chars_in_table_name() {
        let mut config = PgVectorConfig::new("postgresql://localhost:5432/test");
        config.table_name = "bad; DROP TABLE users--".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_special_chars_in_schema() {
        let mut config = PgVectorConfig::new("postgresql://localhost:5432/test");
        config.schema = "public\"; DROP TABLE users--".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_accepts_valid_identifiers() {
        let config = PgVectorConfig::new("postgresql://localhost:5432/test")
            .with_schema("my_schema")
            .with_table_name("my_table_123");
        assert!(config.validate().is_ok());
    }
}
