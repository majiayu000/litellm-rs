//! Module
//!
//! Contains base components shared by all providers

pub mod config;
pub mod connection_pool;
pub mod http;
pub mod model_entry;
pub mod pricing;
pub mod sse;

pub use crate::core::pricing::{PricingDatabase, get_pricing_db};
pub use crate::utils::net::http::ProviderRequestBuilder;
pub use config::BaseConfig;
pub use connection_pool::{
    ConnectionPool, GlobalPoolManager, HeaderPair, HttpMethod, PoolConfig,
    STREAMING_ERROR_BODY_MAX_BYTES, STREAMING_ERROR_BODY_TIMEOUT_SECS,
    STREAMING_HEADER_TIMEOUT_SECS, StreamingRequestError, apply_headers, global_client, header,
    header_owned, header_static, read_streaming_error_body, read_streaming_error_body_with_limits,
    send_streaming_request, send_streaming_request_with_timeout, streaming_client,
    streaming_unbounded_client,
};
pub use http::{
    BaseHttpClient, HttpErrorMapper, OpenAIRequestTransformer, UrlBuilder, apply_provider_headers,
    validate_chat_request_common,
};
pub use model_entry::ProviderModelEntry;
pub use sse::{
    AnthropicTransformer, CohereTransformer, DatabricksTransformer, GeminiTransformer,
    OpenAICompatibleTransformer, SSEEvent, SSEEventType, SSETransformer, UnifiedSSEParser,
    UnifiedSSEStream, create_provider_sse_stream,
};
