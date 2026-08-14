use crate::core::providers::base::{HeaderPair, header, header_owned};

use super::config::OpenAILikeConfig;

pub(super) fn build_request_headers(config: &OpenAILikeConfig) -> Vec<HeaderPair> {
    let mut headers =
        Vec::with_capacity(4 + config.base.headers.len() + config.custom_headers.len());

    if let Some(api_key) = &config.base.api_key {
        headers.push(header("Authorization", format!("Bearer {api_key}")));
    }

    let organization_header = (config.provider_name == "meta_llama"
        || config.base.organization.is_some())
    .then(|| {
        crate::core::providers::registry::catalog_policy::organization_header(&config.provider_name)
    });
    if let Some(org) = &config.base.organization {
        let name = crate::core::providers::registry::catalog_policy::organization_header(
            &config.provider_name,
        );
        headers.push(header(name, org.clone()));
    }

    for (key, value) in &config.base.headers {
        push_configured_header(
            &mut headers,
            header_owned(key.clone(), value.clone()),
            organization_header,
        );
    }
    for (key, value) in &config.custom_headers {
        push_configured_header(
            &mut headers,
            header_owned(key.clone(), value.clone()),
            organization_header,
        );
    }

    if config.provider_name == "openrouter" {
        if let Ok(site_url) = std::env::var("OR_SITE_URL") {
            headers.push(header_owned("HTTP-Referer".to_string(), site_url));
        }
        if let Ok(app_name) = std::env::var("OR_APP_NAME") {
            headers.push(header_owned("X-Title".to_string(), app_name));
        }
    }

    headers
}

fn push_configured_header(
    headers: &mut Vec<HeaderPair>,
    header: HeaderPair,
    organization_header: Option<&str>,
) {
    if organization_header.is_some_and(|name| name.eq_ignore_ascii_case(header.0.as_ref()))
        && let Some(existing) = headers
            .iter_mut()
            .find(|(name, _)| name.eq_ignore_ascii_case(header.0.as_ref()))
    {
        *existing = header;
        return;
    }
    headers.push(header);
}
