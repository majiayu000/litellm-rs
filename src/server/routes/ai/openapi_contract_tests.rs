use super::stable_routes::stable_inference_routes;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

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

    for route in stable_inference_routes() {
        let path = route.path;
        let method = route.method.openapi_key();
        let operation = paths
            .get(path)
            .and_then(|item| item.get(method))
            .unwrap_or_else(|| panic!("missing {method} {path} from stable OpenAPI contract"));
        let operation_id = operation["operationId"]
            .as_str()
            .unwrap_or_else(|| panic!("missing operationId for {method} {path}"));
        assert!(
            operation_ids.insert(operation_id),
            "duplicate operationId: {operation_id}"
        );
        assert!(
            operation.get("responses").is_some(),
            "missing responses for {method} {path}"
        );
    }

    let contract_operations = paths
        .iter()
        .flat_map(|(path, item)| {
            item.as_object().into_iter().flat_map(move |item| {
                item.keys()
                    .filter(|method| matches!(method.as_str(), "delete" | "get" | "post"))
                    .map(move |method| (path.clone(), method.clone()))
            })
        })
        .collect::<HashSet<_>>();
    let registered_operations = stable_inference_routes()
        .iter()
        .map(|route| {
            (
                route.path.to_string(),
                route.method.openapi_key().to_string(),
            )
        })
        .collect::<HashSet<_>>();
    assert_eq!(
        contract_operations, registered_operations,
        "the stable contract operations must exactly match the executable inventory"
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
            .get("audio/opus")
            .is_some(),
        "speech must document its Opus binary media type"
    );
}

#[test]
fn stable_inference_openapi_contract_matches_runtime_request_and_response_shapes() {
    let contract = contract();
    let schemas = &contract["components"]["schemas"];

    assert_eq!(
        schemas["EmbeddingRequest"]["properties"]["encoding_format"]["enum"],
        serde_json::json!(["float"])
    );

    for schema in [
        "ImageGenerationRequest",
        "ImageEditRequest",
        "ImageVariationRequest",
    ] {
        assert!(
            schemas[schema]["required"]
                .as_array()
                .expect("required must be an array")
                .iter()
                .any(|field| field == "model"),
            "{schema} must require the runtime-required model"
        );
    }

    let usage = &schemas["ResponseUsage"]["properties"];
    for field in [
        "input_tokens",
        "output_tokens",
        "total_tokens",
        "input_tokens_details",
        "output_tokens_details",
    ] {
        assert!(
            usage.get(field).is_some(),
            "missing Responses usage field {field}"
        );
    }
    assert_eq!(
        schemas["ResponseObject"]["properties"]["usage"]["$ref"],
        "#/components/schemas/ResponseUsage"
    );

    let input_item_parameters = contract["paths"]["/v1/responses/{response_id}/input_items"]["get"]
        ["parameters"]
        .as_array()
        .expect("input_items query parameters must be declared");
    let names = input_item_parameters
        .iter()
        .filter_map(|parameter| parameter["name"].as_str())
        .collect::<HashSet<_>>();
    assert_eq!(names, HashSet::from(["after", "include", "limit", "order"]));
}

#[test]
fn stable_inference_openapi_contract_has_typed_fallback_errors() {
    let contract = contract();
    for route in stable_inference_routes() {
        let operation = &contract["paths"][route.path][route.method.openapi_key()];
        assert_eq!(
            operation["responses"]["default"]["$ref"],
            "#/components/responses/OpenAIError",
            "missing typed fallback error for {} {}",
            route.method.openapi_key(),
            route.path
        );
    }
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
