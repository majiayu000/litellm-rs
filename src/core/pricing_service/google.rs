//! Exact catalog resolution rules for Google pricing surfaces.

pub(super) const VERTEX_PROVIDER_ALIASES: &[&str] = &[
    "vertex_ai",
    "google",
    "vertex_ai-ai21_models",
    "vertex_ai-anthropic_models",
    "vertex_ai-deepseek_models",
    "vertex_ai-embedding-models",
    "vertex_ai-image-models",
    "vertex_ai-language-models",
    "vertex_ai-llama_models",
    "vertex_ai-minimax_models",
    "vertex_ai-mistral_models",
    "vertex_ai-moonshot_models",
    "vertex_ai-openai_models",
    "vertex_ai-qwen_models",
    "vertex_ai-text-models",
    "vertex_ai-video-models",
    "vertex_ai-zai_models",
];

pub(super) fn is_vertex_publisher_prefix(provider: &str, prefix: &str) -> bool {
    provider == "vertex_ai"
        && matches!(
            prefix.to_ascii_lowercase().as_str(),
            "ai21" | "meta" | "mistral"
        )
}

pub(super) fn uses_google_completion_calculator(
    requested_provider: &str,
    catalog_provider: &str,
) -> bool {
    matches!(requested_provider, "gemini" | "vertex_ai")
        || catalog_provider == "vertex_ai"
        || catalog_provider.starts_with("vertex_ai_")
}

pub(super) fn exact_pricing_candidates(
    provider: &str,
    model: &str,
    normalized_model: &str,
) -> Vec<String> {
    let model = model.to_ascii_lowercase();
    let normalized_model = normalized_model.to_ascii_lowercase();
    let mut candidates = Vec::with_capacity(5);
    for candidate in [
        model.clone(),
        normalized_model.clone(),
        format!("{provider}/{model}"),
        format!("{provider}/{normalized_model}"),
    ] {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }

    if provider == "vertex_ai"
        && let Some(alias) = vertex_pricing_alias(&model)
        && !candidates.iter().any(|candidate| candidate == alias)
    {
        candidates.push(alias.to_string());
    }
    candidates
}

fn vertex_pricing_alias(model: &str) -> Option<&'static str> {
    match model {
        "gemini-1.5-pro-001" | "gemini-1.5-pro-002" => Some("gemini-1.5-pro"),
        "gemini-1.5-flash-001" | "gemini-1.5-flash-002" => Some("gemini-1.5-flash"),
        "claude-opus-4-6@20260114" => Some("vertex_ai/claude-opus-4-6"),
        "claude-opus-4-5@20251110" => Some("vertex_ai/claude-opus-4-5"),
        "claude-3-5-sonnet@20241022" => Some("vertex_ai/claude-3-5-sonnet"),
        "meta/llama-4-scout-17b-16e-instruct" => {
            Some("vertex_ai/meta/llama-4-scout-17b-16e-instruct-maas")
        }
        "meta/llama-4-maverick-17b-128e-instruct" => {
            Some("vertex_ai/meta/llama-4-maverick-17b-128e-instruct-maas")
        }
        "mistral/mistral-nemo" => Some("vertex_ai/mistral-nemo@latest"),
        _ => None,
    }
}
