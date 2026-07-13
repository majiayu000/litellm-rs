use crate::core::providers::unified_provider::ProviderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BedrockService {
    Runtime,
    Control,
    AgentRuntime,
}

impl BedrockService {
    pub(super) fn base_url(self, region: &str) -> String {
        let service = match self {
            Self::Runtime => "bedrock-runtime",
            Self::Control => "bedrock",
            Self::AgentRuntime => "bedrock-agent-runtime",
        };
        format!("https://{service}.{region}.amazonaws.com")
    }
}

pub(super) struct BedrockRequestTarget {
    pub(super) service: BedrockService,
    pub(super) url: String,
}

pub(super) fn request_target(
    region: &str,
    model_id: &str,
    operation: &str,
) -> Result<BedrockRequestTarget, ProviderError> {
    let operation = operation.trim_matches('/');
    let (service, path) = match operation {
        "invoke" | "invoke-with-response-stream" | "converse" | "converse-stream" => {
            if model_id.is_empty() {
                return Err(ProviderError::invalid_request(
                    "bedrock",
                    format!("model ID is required for operation '{operation}'"),
                ));
            }
            (
                BedrockService::Runtime,
                format!(
                    "model/{}/{}",
                    encode_model_id_path_segment(model_id),
                    operation
                ),
            )
        }
        "list-foundation-models" => (BedrockService::Control, "foundation-models".to_string()),
        path if path == "model-invocation-job" || path.starts_with("model-invocation-job/") => {
            (BedrockService::Control, path.to_string())
        }
        path if path.starts_with("agents/") || path.starts_with("knowledgebases/") => {
            (BedrockService::AgentRuntime, path.to_string())
        }
        path if path.starts_with("guardrail/") => (BedrockService::Runtime, path.to_string()),
        _ => {
            return Err(ProviderError::invalid_request(
                "bedrock",
                format!("unsupported Bedrock operation '{operation}'"),
            ));
        }
    };

    Ok(BedrockRequestTarget {
        service,
        url: format!("{}/{}", service.base_url(region), path),
    })
}

fn encode_model_id_path_segment(model_id: &str) -> String {
    url::form_urlencoded::byte_serialize(model_id.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_matrix_selects_the_matching_service_authority() {
        let cases = [
            (
                "model",
                "invoke",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/model/invoke",
            ),
            (
                "",
                "list-foundation-models",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/foundation-models",
            ),
            (
                "",
                "model-invocation-job/job-1",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/model-invocation-job/job-1",
            ),
            (
                "",
                "agents/a/agentAliases/b/sessions/c/text",
                BedrockService::AgentRuntime,
                "https://bedrock-agent-runtime.us-east-1.amazonaws.com/agents/a/agentAliases/b/sessions/c/text",
            ),
            (
                "",
                "knowledgebases/kb/retrieve",
                BedrockService::AgentRuntime,
                "https://bedrock-agent-runtime.us-east-1.amazonaws.com/knowledgebases/kb/retrieve",
            ),
            (
                "",
                "guardrail/g/version/1/apply",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/guardrail/g/version/1/apply",
            ),
        ];

        for (model_id, operation, expected_service, expected_url) in cases {
            let target = request_target("us-east-1", model_id, operation)
                .unwrap_or_else(|error| panic!("target should build: {error}"));
            assert_eq!(target.service, expected_service);
            assert_eq!(target.url, expected_url);
        }
    }

    #[test]
    fn unknown_or_incomplete_operations_fail_closed() {
        assert!(request_target("us-east-1", "model", "custom-operation").is_err());
        assert!(request_target("us-east-1", "", "invoke").is_err());
        assert!(request_target("us-east-1", "", "https://example.com/path").is_err());
    }

    #[test]
    fn model_ids_are_encoded_as_one_path_segment() {
        let arn = "arn:aws:bedrock:us-east-1:123456789012:inference-profile/us.model:0";
        let target = request_target("us-east-1", arn, "invoke")
            .unwrap_or_else(|error| panic!("ARN target should build: {error}"));

        assert!(target.url.contains("inference-profile%2Fus.model%3A0"));
        assert!(!target.url.contains("/inference-profile/"));
    }
}
