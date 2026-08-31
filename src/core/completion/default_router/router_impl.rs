//! `DefaultRouter` compatibility methods delegated to the canonical runtime.

use super::*;
use crate::core::completion::{
    convert_from_chat_completion_response, convert_messages_to_chat_messages,
    convert_to_chat_completion_request,
};
use crate::core::router::RuntimeHandle;
use crate::core::types::context::RequestContext;
use crate::core::types::model::ProviderCapability;
use futures::stream::StreamExt;

fn reject_provider_overrides(options: &CompletionOptions) -> Result<()> {
    if options.api_key.is_some()
        || options.api_base.is_some()
        || options.api_version.is_some()
        || options.organization.is_some()
        || options.headers.is_some()
        || options.timeout.is_some()
    {
        return Err(GatewayError::from(
            crate::core::providers::ProviderError::invalid_request(
                "completion",
                "request-level provider overrides require a matching canonical runtime deployment",
            ),
        ));
    }

    Ok(())
}

pub(super) async fn complete_with_runtime_handle(
    handle: &RuntimeHandle,
    model: &str,
    messages: Vec<Message>,
    options: CompletionOptions,
) -> Result<CompletionResponse> {
    reject_provider_overrides(&options)?;
    let chat_messages = convert_messages_to_chat_messages(messages);
    let chat_request = convert_to_chat_completion_request(model, chat_messages, options)?;
    let context = RequestContext::new();
    let execution = handle
        .execute_with_selected_deployment_capability_typed(
            model,
            &ProviderCapability::ChatCompletion,
            move |deployment| {
                let mut request = chat_request.clone();
                let context = context.clone();
                async move {
                    request.model = deployment.model.clone();
                    let response = deployment
                        .provider
                        .chat_completion(request, context)
                        .await?;
                    let tokens = response
                        .usage
                        .as_ref()
                        .map(|usage| u64::from(usage.total_tokens))
                        .unwrap_or_default();
                    Ok((response, tokens))
                }
            },
        )
        .await
        .map_err(GatewayError::from)?;

    convert_from_chat_completion_response(execution.result)
}

pub(super) async fn complete_stream_with_runtime_handle(
    handle: &RuntimeHandle,
    model: &str,
    messages: Vec<Message>,
    options: CompletionOptions,
) -> Result<CompletionStream> {
    reject_provider_overrides(&options)?;
    let chat_messages = convert_messages_to_chat_messages(messages);
    let mut chat_request = convert_to_chat_completion_request(model, chat_messages, options)?;
    chat_request.stream = true;
    let context = RequestContext::new();
    let lease = handle
        .select_deployment_lease_for_capability_typed(
            model,
            &ProviderCapability::ChatCompletionStream,
        )
        .map_err(GatewayError::from)?;
    let deployment = lease.clone_deployment();
    chat_request.model = deployment.model.clone();
    let stream = deployment
        .provider
        .chat_completion_stream(chat_request, context)
        .await
        .map_err(GatewayError::from)?;

    Ok(Box::pin(stream.map(move |chunk| {
        let _lease = &lease;
        chunk
            .map(convert_chat_chunk_to_completion_chunk)
            .map_err(GatewayError::from)
    })))
}

#[async_trait]
impl Router for DefaultRouter {
    async fn complete(
        &self,
        model: &str,
        messages: Vec<Message>,
        options: CompletionOptions,
    ) -> Result<CompletionResponse> {
        let handle = match &self.runtime_binding {
            Some(binding) => binding.bind(),
            None => default_runtime().map_err(GatewayError::from)?,
        };
        complete_with_runtime_handle(&handle, model, messages, options).await
    }

    async fn complete_stream(
        &self,
        model: &str,
        messages: Vec<Message>,
        options: CompletionOptions,
    ) -> Result<CompletionStream> {
        let handle = match &self.runtime_binding {
            Some(binding) => binding.bind(),
            None => default_runtime().map_err(GatewayError::from)?,
        };
        complete_stream_with_runtime_handle(&handle, model, messages, options).await
    }
}
