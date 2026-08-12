#![cfg(feature = "providers-extended")]

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::json;

const LIVE_OPT_IN: &str = "LITELLM_RS_LIVE_GEMINI_CATALOG";
const MODELS_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1000";

#[derive(Deserialize)]
struct ModelList {
    models: Vec<RemoteModel>,
}

#[derive(Deserialize)]
struct RemoteModel {
    name: String,
    #[serde(rename = "supportedGenerationMethods", default)]
    supported_generation_methods: Vec<String>,
}

fn failure_class(status: reqwest::StatusCode) -> &'static str {
    match status.as_u16() {
        401 | 403 => "auth",
        404 => "not_found",
        429 => "quota",
        _ => "protocol",
    }
}

#[tokio::test]
async fn live_developer_catalog_matches_list_models_and_accepts_minimal_calls() {
    if std::env::var(LIVE_OPT_IN).as_deref() != Ok("1") {
        return;
    }
    let api_key = std::env::var("GEMINI_API_KEY")
        .or_else(|_| std::env::var("GOOGLE_API_KEY"))
        .unwrap_or_else(|_| panic!("{LIVE_OPT_IN}=1 requires GEMINI_API_KEY or GOOGLE_API_KEY"));
    let client = reqwest::Client::new();
    let list_response = client
        .get(MODELS_URL)
        .header("x-goog-api-key", &api_key)
        .send()
        .await
        .unwrap_or_else(|_| panic!("Gemini list-models failed: network"));
    assert!(
        list_response.status().is_success(),
        "Gemini list-models failed: {}",
        failure_class(list_response.status())
    );
    let remote: ModelList = list_response
        .json()
        .await
        .unwrap_or_else(|_| panic!("Gemini list-models failed: protocol"));
    let callable = remote
        .models
        .into_iter()
        .filter(|model| {
            model
                .supported_generation_methods
                .iter()
                .any(|method| method == "generateContent")
        })
        .filter_map(|model| model.name.strip_prefix("models/").map(str::to_string))
        .collect::<HashSet<_>>();

    let static_models = litellm_rs::core::providers::gemini::supported_models();
    for model in &static_models {
        assert!(
            callable.contains(model),
            "Gemini catalog mismatch: {model} not_found"
        );
    }

    for model in static_models {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
        );
        let response = client
            .post(url)
            .header("x-goog-api-key", &api_key)
            .json(&json!({
                "contents": [{"role": "user", "parts": [{"text": "Reply OK"}]}],
                "generationConfig": {"maxOutputTokens": 1}
            }))
            .send()
            .await
            .unwrap_or_else(|_| panic!("Gemini minimal call failed for {model}: network"));
        assert!(
            response.status().is_success(),
            "Gemini minimal call failed for {model}: {}",
            failure_class(response.status())
        );
    }
}
