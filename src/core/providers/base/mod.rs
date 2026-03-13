//! Module
//!
//! Contains base components shared by all providers

pub mod config;
pub mod connection_pool;
pub mod http;
pub mod pricing;
pub mod sse;

pub use config::BaseConfig;
pub use connection_pool::{
    ConnectionPool, GlobalPoolManager, HeaderPair, HttpMethod, PoolConfig, apply_headers,
    global_client, header, header_owned, header_static, streaming_client,
};
pub use http::{
    BaseHttpClient, HttpErrorMapper, OpenAIRequestTransformer, UrlBuilder, create_http_client,
    validate_chat_request_common,
};
pub use pricing::{PricingDatabase, get_pricing_db};
pub use sse::{
    AnthropicTransformer, CohereTransformer, DatabricksTransformer, GeminiTransformer,
    OpenAICompatibleTransformer, SSEEvent, SSEEventType, SSETransformer, UnifiedSSEParser,
    UnifiedSSEStream, create_provider_sse_stream,
};
