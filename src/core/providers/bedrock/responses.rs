//! Exact native Responses endpoint contracts; see docs/providers/native-responses-routing.md.
use super::*;
use crate::core::providers::base::read_streaming_error_body;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResponsesEndpoint {
    Runtime,
    Mantle,
}

fn response_target<'a>(
    model: &'a str,
    region: &str,
) -> Result<(ResponsesEndpoint, &'a str, &'static str), ProviderError> {
    let runtime = matches!(
        model,
        "us.openai.gpt-6-astra"
            | "global.openai.gpt-6-astra"
            | "us.openai.gpt-6-sol"
            | "global.openai.gpt-6-sol"
            | "us.openai.gpt-6-luna"
            | "global.openai.gpt-6-luna"
            | "us.openai.gpt-6.1-sol"
            | "global.openai.gpt-6.1-sol"
    );
    if runtime {
        let source_supported = if model.ends_with("gpt-6.1-sol") {
            // This launch's model card confirms the profiles and the us-east-1
            // example, but does not enumerate other source Regions.
            region == "us-east-1"
        } else if model.ends_with("gpt-6-astra") {
            matches!(
                region,
                "us-east-1"
                    | "us-east-2"
                    | "us-west-1"
                    | "us-west-2"
                    | "ca-central-1"
                    | "eu-central-1"
                    | "eu-north-1"
                    | "eu-west-1"
                    | "eu-west-2"
                    | "eu-west-3"
                    | "ap-northeast-1"
                    | "ap-northeast-2"
                    | "ap-northeast-3"
                    | "ap-south-1"
                    | "ap-southeast-1"
                    | "ap-southeast-2"
                    | "sa-east-1"
            )
        } else {
            matches!(
                region,
                "us-east-1"
                    | "us-east-2"
                    | "us-west-1"
                    | "us-west-2"
                    | "ca-central-1"
                    | "ca-west-1"
                    | "eu-central-1"
                    | "eu-central-2"
                    | "eu-north-1"
                    | "eu-south-1"
                    | "eu-south-2"
                    | "eu-west-1"
                    | "eu-west-2"
                    | "eu-west-3"
                    | "ap-east-2"
                    | "ap-northeast-1"
                    | "ap-northeast-2"
                    | "ap-northeast-3"
                    | "ap-south-1"
                    | "ap-south-2"
                    | "ap-southeast-1"
                    | "ap-southeast-2"
                    | "ap-southeast-3"
                    | "ap-southeast-4"
                    | "ap-southeast-5"
                    | "ap-southeast-6"
                    | "ap-southeast-7"
                    | "il-central-1"
                    | "af-south-1"
                    | "sa-east-1"
                    | "mx-central-1"
            )
        };
        if !source_supported {
            return Err(ProviderError::not_supported(
                "bedrock",
                "No verified Responses profile in this source Region",
            ));
        }
        // US profiles can be invoked from the documented US/Canada source Regions.
        // Account permissions and each profile's current destination availability
        // remain AWS-owned, and are returned as native upstream errors.
        if model.starts_with("us.")
            && !matches!(
                region,
                "us-east-1"
                    | "us-east-2"
                    | "us-west-1"
                    | "us-west-2"
                    | "ca-central-1"
                    | "ca-west-1"
            )
        {
            return Err(ProviderError::not_supported(
                "bedrock",
                "US Responses profile in this source Region",
            ));
        }
        return Ok((ResponsesEndpoint::Runtime, model, "/openai/v1/responses"));
    }
    let (wire, path, available) = match model {
        "openai.gpt-6-astra" => (
            model,
            "/openai/v1/responses",
            matches!(region, "us-east-1" | "us-west-2"),
        ),
        "openai.gpt-6-sol" | "openai.gpt-6-luna" | "openai.gpt-6.1-sol" => {
            (model, "/openai/v1/responses", region == "us-east-1")
        }
        "openai.gpt-oss-120b-1:0" | "openai.gpt-oss-20b-1:0" => {
            let wire = if model == "openai.gpt-oss-120b-1:0" {
                "openai.gpt-oss-120b"
            } else {
                "openai.gpt-oss-20b"
            };
            (
                wire,
                "/v1/responses",
                matches!(
                    region,
                    "us-east-1"
                        | "us-east-2"
                        | "us-west-2"
                        | "ap-southeast-3"
                        | "ap-south-1"
                        | "ap-southeast-2"
                        | "ap-northeast-1"
                        | "eu-central-1"
                        | "eu-west-1"
                        | "eu-west-2"
                        | "eu-south-1"
                        | "eu-north-1"
                        | "sa-east-1"
                        | "us-gov-west-1"
                ),
            )
        }
        _ => {
            return Err(ProviderError::not_supported(
                "bedrock",
                format!("Model {model} has no verified Responses endpoint"),
            ));
        }
    };
    if !available {
        return Err(ProviderError::not_supported(
            "bedrock",
            format!("Model {model} is not available on Mantle in {region}"),
        ));
    }
    Ok((ResponsesEndpoint::Mantle, wire, path))
}

impl BedrockClient {
    pub(crate) fn supports_responses_model(&self, model: &str) -> bool {
        response_target(model, &self.auth.credentials().region).is_ok()
    }

    pub(crate) async fn native_response(&self, mut body: Value) -> Result<Response, ProviderError> {
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(|| ProviderError::invalid_request("bedrock", "model must be a string"))?;
        let credentials = self.auth.credentials();
        let (endpoint, wire, path) = response_target(model, &credentials.region)?;
        body["model"] = Value::String(wire.to_string());
        if endpoint == ResponsesEndpoint::Runtime {
            if body.get("background") == Some(&Value::Bool(true)) {
                return Err(ProviderError::not_supported(
                    "bedrock",
                    "Runtime Responses background processing",
                ));
            }
            if body
                .get("tools")
                .and_then(Value::as_array)
                .is_some_and(|tools| {
                    tools.iter().any(|tool| {
                        !matches!(
                            tool.get("type").and_then(Value::as_str),
                            Some("function" | "custom")
                        )
                    })
                })
            {
                return Err(ProviderError::not_supported(
                    "bedrock",
                    "Runtime Responses server-side tools",
                ));
            }
        }
        let api_base = match endpoint {
            ResponsesEndpoint::Runtime => format!(
                "https://bedrock-runtime.{}.amazonaws.com",
                credentials.region
            ),
            ResponsesEndpoint::Mantle => {
                format!("https://bedrock-mantle.{}.api.aws", credentials.region)
            }
        };
        let config = BaseConfig {
            api_base: Some(api_base.clone()),
            ..self.runtime_client.config().clone()
        };
        let client = if body.get("stream") == Some(&Value::Bool(true)) {
            BaseHttpClient::new_for_provider_streaming_no_redirect("bedrock", config)?
        } else {
            BaseHttpClient::new_for_provider_no_redirect("bedrock", config)?
        };
        self.send_responses_request(&client, &format!("{api_base}{path}"), endpoint, body)
            .await
    }

    async fn send_responses_request(
        &self,
        client: &BaseHttpClient,
        url: &str,
        endpoint: ResponsesEndpoint,
        body: Value,
    ) -> Result<Response, ProviderError> {
        let credentials = self.auth.credentials();
        let signer = SigV4Signer::new_for_service(
            credentials.access_key_id.clone(),
            credentials.secret_access_key.clone(),
            credentials.session_token.clone(),
            credentials.region.clone(),
            if endpoint == ResponsesEndpoint::Runtime {
                "bedrock"
            } else {
                "bedrock-mantle"
            },
        );
        let body = serde_json::to_string(&body)
            .map_err(|error| ProviderError::serialization("bedrock", error.to_string()))?;
        let headers = HashMap::from([("content-type".to_string(), "application/json".to_string())]);
        let signed = signer
            .sign_request("POST", url, &headers, &body, chrono::Utc::now())
            .map_err(|error| {
                ProviderError::configuration("bedrock", format!("Signing failed: {error}"))
            })?;
        let mut request = client.post(url)?;
        for (name, value) in signed {
            request = request.header(name, value);
        }
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(client.config().timeout),
            request.body(body).send(),
        )
        .await
        .map_err(|_| ProviderError::timeout("bedrock", "Responses upstream timed out"))?
        .map_err(|error| client.map_preserved_request_error(error))?;
        if response.status().as_u16() == 429 {
            let retry = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse().ok());
            let body = read_streaming_error_body(response)
                .await
                .unwrap_or_else(|_| "Bedrock rate limited".into());
            return Err(ProviderError::rate_limit_with_retry("bedrock", body, retry));
        }
        self.ensure_successful_response(response).await
    }
}

#[cfg(test)]
#[path = "responses_tests.rs"]
mod tests;
