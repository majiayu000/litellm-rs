use super::SSETransformer;
use crate::core::providers::unified_provider::ProviderError;

pub(super) fn finalize_transform_error<T: SSETransformer>(
    transformer: &T,
    error: ProviderError,
) -> ProviderError {
    match transformer.finish_stream() {
        Ok(_) => error,
        Err(finalization) => {
            combine_stream_errors(transformer.provider_name(), error, finalization)
        }
    }
}

pub(super) fn combine_stream_errors(
    provider: &'static str,
    first: ProviderError,
    finalization: ProviderError,
) -> ProviderError {
    // Lifecycle finalization must not erase the accounting invalidation signal.
    if super::invalidates_stream_usage(&first) {
        return first;
    }
    if super::invalidates_stream_usage(&finalization) {
        return finalization;
    }
    let first = format!("{:?}", first.redacted());
    match finalization.redacted() {
        ProviderError::Streaming {
            provider,
            stream_type,
            position,
            message,
            ..
        } => ProviderError::Streaming {
            provider,
            stream_type,
            position,
            last_chunk: None,
            message: format!("{message}; preceding stream error: {first}"),
        },
        finalization => ProviderError::streaming_error(
            provider,
            "sse.termination",
            None,
            None,
            format!("multiple stream errors: {first}; finalization: {finalization:?}"),
        ),
    }
}
