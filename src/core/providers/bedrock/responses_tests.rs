use super::*;
use crate::core::net::ProviderEndpointAccess;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use serde_json::json;
use std::sync::Mutex;

#[test]
fn responses_matrix_keeps_runtime_profiles_and_mantle_wire_models_separate() {
    for model in [
        "openai.gpt-6-astra",
        "openai.gpt-6-sol",
        "openai.gpt-6-luna",
        "openai.gpt-6.1-sol",
    ] {
        assert_eq!(
            response_target(model, "us-east-1").unwrap(),
            (ResponsesEndpoint::Mantle, model, "/openai/v1/responses")
        );
        for profile in ["us", "global"] {
            let id = format!("{profile}.{model}");
            assert_eq!(
                response_target(&id, "us-east-1").unwrap(),
                (
                    ResponsesEndpoint::Runtime,
                    id.as_str(),
                    "/openai/v1/responses"
                )
            );
        }
    }
    for size in ["20b", "120b"] {
        let model = format!("openai.gpt-oss-{size}-1:0");
        let (endpoint, wire, path) = response_target(&model, "us-east-1").unwrap();
        assert_eq!(endpoint, ResponsesEndpoint::Mantle);
        assert_eq!(wire, format!("openai.gpt-oss-{size}"));
        assert_eq!(path, "/v1/responses");
        assert!(response_target(&format!("global.{model}"), "us-east-1").is_err());
        assert!(response_target(&model, "ca-central-1").is_err());
    }
    for model in [
        "amazon.nova-pro-v1:0",
        "openai.gpt-6-sol-fake",
        "eu.openai.gpt-6-sol",
        "openai.gpt-6.1-sol-20260929",
    ] {
        assert!(response_target(model, "us-east-1").is_err(), "{model}");
    }
    assert!(response_target("us.openai.gpt-6-sol", "eu-west-1").is_err());
    assert!(response_target("openai.gpt-6-sol", "eu-west-1").is_err());
    assert!(response_target("global.openai.gpt-6-sol", "us-gov-west-1").is_err());
}

fn client() -> BedrockClient {
    BedrockClient::new(BedrockConfig {
        aws_access_key_id: "AKIATEST123456789012".into(),
        aws_secret_access_key: "local-test-secret".into(),
        aws_session_token: Some("local-test-session".into()),
        ..Default::default()
    })
    .unwrap()
}

#[tokio::test]
async fn runtime_rejects_background_and_server_tools_without_inference() {
    let client = client();
    for extra in [
        json!({"background":true}),
        json!({"tools":[{"type":"web_search"}]}),
    ] {
        let mut body = json!({"model":"us.openai.gpt-6-sol","input":"Hi"});
        body.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(matches!(
            client.native_response(body).await,
            Err(ProviderError::NotSupported { .. })
        ));
    }
}

#[tokio::test]
async fn native_transport_signs_correct_service_and_preserves_json_and_sse() {
    type Seen = Arc<Mutex<Vec<(String, String, String, Value)>>>;
    async fn receive(
        req: HttpRequest,
        body: web::Json<Value>,
        seen: web::Data<Seen>,
    ) -> HttpResponse {
        let header = |name| {
            req.headers()
                .get(name)
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        };
        seen.lock().unwrap().push((
            req.path().into(),
            header("authorization"),
            header("x-amz-security-token"),
            body.clone(),
        ));
        if let Some(status) = body.get("test_status").and_then(Value::as_u64) {
            return HttpResponse::build(
                actix_web::http::StatusCode::from_u16(status as u16).unwrap(),
            )
            .insert_header(("retry-after", "7"))
            .json(json!({"message":"native failure"}));
        }
        if body["stream"] == true {
            HttpResponse::Ok().insert_header(("content-type", "text/event-stream")).body("event: response.completed\ndata: {\"type\":\"response.completed\",\"native\":true}\n\n")
        } else {
            HttpResponse::Ok().json(
                json!({"id":"native","output":[],"usage":{"input_tokens":4,"output_tokens":1}}),
            )
        }
    }
    let seen: Seen = Arc::default();
    let data = seen.clone();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .default_service(web::post().to(receive))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let client = client();
    let base = format!("http://{address}");
    let http = BaseHttpClient::new_for_provider_no_redirect(
        "bedrock",
        BaseConfig {
            api_base: Some(base.clone()),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            ..Default::default()
        },
    )
    .unwrap();
    for (model, stream) in [
        ("us.openai.gpt-6-sol", false),
        ("openai.gpt-oss-120b-1:0", true),
    ] {
        let (endpoint, wire, path) = response_target(model, "us-east-1").unwrap();
        let body = json!({"model":wire,"input":"Hi","stream":stream,"tools":[{"type":"function","name":"test"}],"reasoning":{"effort":"low"},"extension":true});
        let response = client
            .send_responses_request(&http, &format!("{base}{path}"), endpoint, body.clone())
            .await
            .unwrap();
        let text = response.text().await.unwrap();
        if stream {
            assert!(text.contains("event: response.completed"));
        } else {
            assert!(text.contains("input_tokens"));
        }
        let seen = seen.lock().unwrap();
        let sent = seen.last().unwrap();
        assert_eq!(sent.0, path);
        assert_eq!(sent.2, "local-test-session");
        assert_eq!(sent.3, body);
        assert!(sent.1.contains(if endpoint == ResponsesEndpoint::Runtime {
            "/us-east-1/bedrock/aws4_request"
        } else {
            "/us-east-1/bedrock-mantle/aws4_request"
        }));
    }
    for status in [401, 429, 503] {
        let error = client
            .send_responses_request(
                &http,
                &format!("{base}/openai/v1/responses"),
                ResponsesEndpoint::Runtime,
                json!({"model":"us.openai.gpt-6-sol","input":"Hi","test_status":status}),
            )
            .await
            .unwrap_err();
        match status {
            401 => assert!(matches!(error, ProviderError::Authentication { .. })),
            429 => assert!(matches!(error, ProviderError::RateLimit { .. })),
            _ => assert!(matches!(error, ProviderError::ApiError { status: 503, .. })),
        }
    }
    handle.stop(false).await;
}
