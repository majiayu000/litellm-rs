use super::operation_for_path;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

const STABLE_OPERATIONS: &[(&str, &str)] = &[
    ("/v1/chat/completions", "post"),
    ("/v1/responses", "post"),
    ("/v1/responses/{response_id}", "get"),
    ("/v1/responses/{response_id}", "delete"),
    ("/v1/responses/{response_id}/cancel", "post"),
    ("/v1/responses/{response_id}/input_items", "get"),
    ("/v1/embeddings", "post"),
    ("/v1/images/generations", "post"),
    ("/v1/images/edits", "post"),
    ("/v1/images/variations", "post"),
    ("/v1/audio/speech", "post"),
    ("/v1/audio/transcriptions", "post"),
    ("/v1/audio/translations", "post"),
    ("/v1/moderations", "post"),
    ("/v1/rerank", "post"),
    ("/v1/models", "get"),
    ("/v1/models/{model_id}", "get"),
];

fn contract() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs")
        .join("openapi")
        .join("inference.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn concrete_path(path: &str) -> String {
    path.replace("{response_id}", "resp_test")
        .replace("{model_id}", "model_test")
}

#[test]
fn stable_inference_openapi_contract_matches_registered_route_surface() {
    let contract = contract();
    assert_eq!(contract["openapi"], "3.2.0");
    assert_eq!(
        contract["components"]["securitySchemes"]["bearerAuth"]["scheme"],
        "bearer"
    );

    let paths = contract["paths"]
        .as_object()
        .expect("OpenAPI paths must be an object");
    let mut operation_ids = HashSet::new();

    for (path, method) in STABLE_OPERATIONS {
        let operation = paths
            .get(*path)
            .and_then(|item| item.get(*method))
            .unwrap_or_else(|| panic!("missing {method} {path} from stable OpenAPI contract"));
        let operation_id = operation["operationId"]
            .as_str()
            .unwrap_or_else(|| panic!("missing operationId for {method} {path}"));
        assert!(
            operation_ids.insert(operation_id),
            "duplicate operationId: {operation_id}"
        );
        assert!(
            operation_for_path(&concrete_path(path)).is_some(),
            "contract operation is not recognized by gateway route policy: {method} {path}"
        );
        assert!(
            operation.get("responses").is_some(),
            "missing responses for {method} {path}"
        );
    }

    assert_eq!(
        paths.len(),
        STABLE_OPERATIONS
            .iter()
            .map(|(path, _)| *path)
            .collect::<HashSet<_>>()
            .len(),
        "the stable contract must not silently add experimental or admin paths"
    );
}

#[test]
fn stable_inference_openapi_contract_declares_transport_boundaries() {
    let contract = contract();

    for path in [
        "/v1/chat/completions",
        "/v1/responses",
        "/v1/embeddings",
        "/v1/images/generations",
        "/v1/audio/speech",
        "/v1/moderations",
        "/v1/rerank",
    ] {
        assert!(
            contract["paths"][path]["post"]["requestBody"]["content"]
                .get("application/json")
                .is_some(),
            "{path} must declare its JSON request body"
        );
    }

    for path in [
        "/v1/images/edits",
        "/v1/images/variations",
        "/v1/audio/transcriptions",
        "/v1/audio/translations",
    ] {
        assert!(
            contract["paths"][path]["post"]["requestBody"]["content"]
                .get("multipart/form-data")
                .is_some(),
            "{path} must declare its multipart request body"
        );
    }

    assert!(
        contract["paths"]["/v1/chat/completions"]["post"]["responses"]["200"]["content"]
            .get("text/event-stream")
            .is_some(),
        "chat must document streaming SSE"
    );
    assert!(
        contract["paths"]["/v1/responses"]["post"]["responses"]["200"]["content"]
            .get("text/event-stream")
            .is_some(),
        "Responses must document streaming SSE"
    );
    assert!(
        contract["paths"]["/v1/audio/speech"]["post"]["responses"]["200"]["content"]
            .get("application/octet-stream")
            .is_some(),
        "speech must document binary audio output"
    );
}

#[test]
fn stable_inference_openapi_contract_declares_shared_error_and_auth_contracts() {
    let contract = contract();
    let schemas = contract["components"]["schemas"]
        .as_object()
        .expect("components.schemas must be an object");

    for schema in [
        "OpenAIErrorResponse",
        "ChatCompletionRequest",
        "ChatCompletionResponse",
        "ResponseObject",
        "EmbeddingRequest",
        "EmbeddingResponse",
        "ImageResponse",
        "ModerationRequest",
        "RerankRequest",
        "Model",
    ] {
        assert!(
            schemas.contains_key(schema),
            "missing shared schema {schema}"
        );
    }

    assert_eq!(contract["security"][0]["bearerAuth"], serde_json::json!([]));
    assert_eq!(
        contract["components"]["responses"]["Unauthorized"]["$ref"],
        "#/components/responses/OpenAIError"
    );
}
