use super::*;

pub(super) fn ensure_image_proxy_candidate_configured(
    providers: &[ProviderConfig],
    requested_model: &str,
) -> Result<(), GatewayError> {
    let candidates = image_proxy_candidate_configs(providers);
    if candidates.is_empty() {
        return Err(missing_image_proxy_provider_error());
    }
    if candidates
        .iter()
        .any(|provider| image_provider_supports_requested_model(provider, Some(requested_model)))
    {
        Ok(())
    } else {
        Err(GatewayError::Provider(ProviderError::model_not_found(
            "image_proxy",
            requested_model,
        )))
    }
}

pub(super) fn ensure_image_edit_candidate_configured(
    state: &AppState,
    requested_model: &str,
) -> Result<bool, GatewayError> {
    let native_candidate = state
        .unified_router
        .get_deployments_for_model(requested_model)
        .into_iter()
        .filter_map(|deployment_id| state.unified_router.get_deployment(&deployment_id))
        .any(|deployment| {
            native_edit::is_native_image_provider(&deployment.provider)
                && deployment.provider.supports_capability_for_model(
                    &deployment.model,
                    &ProviderCapability::ImageEdit,
                )
        });
    if native_candidate {
        return Ok(true);
    }
    ensure_image_proxy_candidate_configured(
        state.config().gateway.providers.as_slice(),
        requested_model,
    )?;
    Ok(false)
}

pub(super) fn image_proxy_router_models(
    providers: &[ProviderConfig],
    requested_model: &str,
    include_requested_model: bool,
) -> Vec<String> {
    let candidates = image_proxy_candidate_configs(providers);
    let mut router_models = Vec::new();
    if include_requested_model
        || candidates.iter().any(|provider| {
            !provider.models.is_empty()
                && provider.models.iter().any(|model| model == requested_model)
        })
    {
        router_models.push(requested_model.to_string());
    }
    let wildcard_models = candidates
        .into_iter()
        .filter(|provider| provider.models.is_empty())
        .map(|provider| provider.name.clone())
        .collect::<Vec<_>>();
    if wildcard_models.is_empty() && router_models.is_empty() {
        router_models.push(requested_model.to_string());
    }
    router_models.extend(wildcard_models);
    router_models
}

pub(super) fn selected_image_proxy_provider(
    providers: &[ProviderConfig],
    selected_provider: &Provider,
    selected_model: &str,
    requested_model: &str,
) -> Result<ImageProxyProvider, ProviderError> {
    let provider_name = selected_provider.name();
    let candidates = image_proxy_candidate_configs(providers);
    let matching = candidates
        .iter()
        .copied()
        .filter(|provider| image_provider_supports_requested_model(provider, Some(requested_model)))
        .find(|provider| {
            provider.name == provider_name
                || provider
                    .settings
                    .get("provider_name")
                    .and_then(|value| value.as_str())
                    == Some(provider_name)
                || (provider.models.is_empty() && provider.name == selected_model)
        })
        .ok_or_else(|| {
            ProviderError::configuration(
                "image_proxy",
                format!(
                    "selected image provider '{provider_name}' for model '{selected_model}' has no matching gateway provider config"
                ),
            )
        })?;
    image_proxy_provider_from_config(matching).map_err(image_proxy_gateway_error_to_provider_error)
}

fn image_proxy_candidate_configs(providers: &[ProviderConfig]) -> Vec<&ProviderConfig> {
    providers
        .iter()
        .filter(|provider| provider.enabled)
        .filter(|provider| is_openai_image_provider(provider))
        .collect()
}

fn image_provider_supports_requested_model(
    provider: &ProviderConfig,
    requested_model: Option<&str>,
) -> bool {
    let Some(requested_model) = requested_model else {
        return true;
    };
    provider.models.is_empty() || provider.models.iter().any(|model| model == requested_model)
}

pub(super) fn missing_image_proxy_provider_error() -> GatewayError {
    GatewayError::BadRequest(
        "Image edits and variations API requires an enabled openai or openai_compatible provider"
            .to_string(),
    )
}

fn is_openai_image_provider(provider: &ProviderConfig) -> bool {
    let provider_type = provider_config::normalize_provider_selector(&provider.provider_type);
    let provider_name = provider_config::normalize_provider_selector(&provider.name);
    provider_type == "openai"
        || provider_type == "openaicompatible"
        || provider_name == "openai"
        || provider_name == "openaicompatible"
}
