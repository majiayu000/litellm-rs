//! Shared lifecycle for asynchronous media generation providers.

use std::time::Duration;

use reqwest::Method;
use serde_json::Value;
use tokio::time::{sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, HttpErrorMapper, apply_provider_headers,
};
use crate::core::providers::unified_provider::ProviderError;

#[path = "media_config.rs"]
pub(crate) mod config_boundary;
#[path = "media_factory.rs"]
mod factory;
#[cfg(feature = "providers-extended")]
pub(crate) use factory::build_image_provider;

/// Normalized terminal output from an asynchronous generation task.
#[derive(Debug, Clone, PartialEq)]
pub struct GenerationOutput {
    pub urls: Vec<String>,
    pub credits_used: Option<f64>,
}

/// Provider-specific decoding result for one poll response.
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationPoll {
    Pending,
    Succeeded(GenerationOutput),
    Failed(String),
    Rejected(String),
    Canceled,
}

/// Timing policy for an asynchronous generation task.
#[derive(Debug, Clone, Copy)]
pub struct PollPolicy {
    initial_delay: Duration,
    max_delay: Duration,
    timeout: Duration,
}

impl PollPolicy {
    pub fn new(initial_delay: Duration, max_delay: Duration, timeout: Duration) -> Self {
        Self {
            initial_delay,
            max_delay,
            timeout,
        }
    }

    pub fn from_millis(initial_delay: u64, max_delay: u64, timeout: u64) -> Self {
        Self::new(
            Duration::from_millis(initial_delay),
            Duration::from_millis(max_delay),
            Duration::from_millis(timeout),
        )
    }
}

impl Default for PollPolicy {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(1),
            Duration::from_secs(5),
            Duration::from_secs(600),
        )
    }
}

/// Cancellable, timeout-bounded polling lifecycle shared by BFL and Runway.
#[derive(Debug, Clone)]
pub struct GenerationLifecycle {
    provider: &'static str,
    client: BaseHttpClient,
    policy: PollPolicy,
}

impl GenerationLifecycle {
    pub fn new(
        provider: &'static str,
        config: BaseConfig,
        policy: PollPolicy,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            provider,
            client: BaseHttpClient::new_for_provider(provider, config)?,
            policy,
        })
    }

    /// Create a polling lifecycle that never follows provider redirects.
    pub fn new_no_redirect(
        provider: &'static str,
        config: BaseConfig,
        policy: PollPolicy,
    ) -> Result<Self, ProviderError> {
        Ok(Self {
            provider,
            client: BaseHttpClient::new_for_provider_no_redirect(provider, config)?,
            policy,
        })
    }

    /// Poll a provider-owned URL until the decoder reports a terminal state.
    pub async fn wait_for_json<F>(
        &self,
        polling_url: String,
        headers: Vec<HeaderPair>,
        cancellation: &CancellationToken,
        decode: F,
    ) -> Result<GenerationOutput, ProviderError>
    where
        F: Fn(Value) -> Result<GenerationPoll, ProviderError>,
    {
        if cancellation.is_cancelled() {
            return Err(self.cancelled());
        }

        tokio::select! {
            _ = cancellation.cancelled() => Err(self.cancelled()),
            result = timeout(
                self.policy.timeout,
                self.poll(polling_url, headers, decode),
            ) => match result {
                Ok(result) => result,
                Err(_) => Err(ProviderError::timeout(
                    self.provider,
                    "media generation lifecycle timed out",
                )),
            },
        }
    }

    async fn poll<F>(
        &self,
        polling_url: String,
        headers: Vec<HeaderPair>,
        decode: F,
    ) -> Result<GenerationOutput, ProviderError>
    where
        F: Fn(Value) -> Result<GenerationPoll, ProviderError>,
    {
        let mut delay = self.policy.initial_delay;
        loop {
            sleep(delay).await;
            match decode(self.get_json(&polling_url, headers.clone()).await?)? {
                GenerationPoll::Pending => {
                    delay = next_delay(delay, self.policy.max_delay);
                }
                GenerationPoll::Succeeded(output) => return Ok(output),
                GenerationPoll::Failed(message) => {
                    return Err(ProviderError::api_error(self.provider, 502, message));
                }
                GenerationPoll::Rejected(message) => {
                    return Err(ProviderError::invalid_request(self.provider, message));
                }
                GenerationPoll::Canceled => return Err(self.cancelled()),
            }
        }
    }

    async fn get_json(
        &self,
        polling_url: &str,
        headers: Vec<HeaderPair>,
    ) -> Result<Value, ProviderError> {
        let request = self
            .client
            .request_preserving_endpoint_policy(Method::GET, polling_url)?;
        let response = apply_provider_headers(request, headers)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        let status = response.status();
        let body = match response.text().await {
            Ok(body) => body,
            Err(_) if !status.is_success() => {
                return Err(HttpErrorMapper::map_status_without_body(
                    self.provider,
                    status.as_u16(),
                ));
            }
            Err(error) => return Err(self.client.map_preserved_request_error(error)),
        };
        if !status.is_success() {
            return Err(HttpErrorMapper::map_status_code(
                self.provider,
                status.as_u16(),
                &body,
            ));
        }
        serde_json::from_str(&body).map_err(|error| {
            ProviderError::response_parsing(
                self.provider,
                format!("invalid generation status response: {error}"),
            )
        })
    }

    fn cancelled(&self) -> ProviderError {
        ProviderError::cancelled(
            self.provider,
            "media generation",
            Some("cancellation requested".to_string()),
        )
    }
}

fn next_delay(current: Duration, maximum: Duration) -> Duration {
    current.saturating_mul(2).min(maximum)
}

/// Narrow Runway video contract, separately gated because the gateway has no video route.
#[cfg(feature = "runway-media")]
pub mod runway {
    use std::time::Duration;

    use reqwest::StatusCode;
    use serde::{Deserialize, Serialize};
    use serde_json::{Map, Value};
    use tokio_util::sync::CancellationToken;

    use crate::core::providers::ProviderError;
    use crate::core::providers::base::{
        BaseConfig, BaseHttpClient, HeaderPair, HttpErrorMapper, ProviderRequestBuilder,
        apply_provider_headers, header_owned, header_static,
    };
    use crate::core::providers::media::{
        GenerationLifecycle, GenerationOutput, GenerationPoll, PollPolicy,
    };
    use crate::core::traits::provider::ProviderConfig;

    const PROVIDER: &str = "runwayml";
    const DEFAULT_API_BASE: &str = "https://api.dev.runwayml.com/v1";
    const API_VERSION: &str = "2024-11-06";

    #[derive(Clone, Serialize, Deserialize)]
    pub struct RunwayConfig {
        #[serde(flatten)]
        pub base: BaseConfig,
        #[serde(skip, default = "default_runway_poll_policy")]
        pub poll_policy: PollPolicy,
    }

    fn default_runway_poll_policy() -> PollPolicy {
        PollPolicy::new(
            Duration::from_secs(5),
            Duration::from_secs(20),
            Duration::from_secs(600),
        )
    }

    impl Default for RunwayConfig {
        fn default() -> Self {
            Self {
                base: BaseConfig {
                    api_base: Some(DEFAULT_API_BASE.to_string()),
                    ..BaseConfig::default()
                },
                poll_policy: default_runway_poll_policy(),
            }
        }
    }

    impl RunwayConfig {
        pub fn with_api_key(api_key: impl Into<String>) -> Self {
            let mut config = Self::default();
            config.base.api_key = Some(api_key.into());
            config
        }

        pub fn from_env() -> Self {
            let mut config = Self::default();
            let env = BaseConfig::from_env(PROVIDER);
            config.base.api_key = std::env::var("RUNWAYML_API_SECRET")
                .ok()
                .and_then(trimmed_credential)
                .or_else(|| env.api_key.and_then(trimmed_credential));
            config.base.timeout = env.timeout;
            config.base.max_retries = env.max_retries;
            if env.api_base.is_some() {
                config.base.api_base = env.api_base;
            }
            config
        }
    }

    impl std::fmt::Debug for RunwayConfig {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RunwayConfig")
                .field("endpoint_access", &self.base.endpoint_access)
                .field("has_api_key", &self.base.api_key.is_some())
                .field("custom_header_count", &self.base.headers.len())
                .finish()
        }
    }

    fn trimmed_credential(value: String) -> Option<String> {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    }

    impl ProviderConfig for RunwayConfig {
        fn validate(&self) -> Result<(), String> {
            self.base.validate(PROVIDER)
        }

        fn api_key(&self) -> Option<&str> {
            self.base.api_key.as_deref()
        }

        fn api_base(&self) -> Option<&str> {
            self.base.api_base.as_deref()
        }

        fn timeout(&self) -> Duration {
            self.base.timeout_duration()
        }

        fn max_retries(&self) -> u32 {
            self.base.max_retries
        }
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RunwayImageToVideoRequest {
        pub model: String,
        pub prompt_image: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub prompt_text: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ratio: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub duration: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub seed: Option<u64>,
        #[serde(flatten)]
        pub extra: Map<String, Value>,
    }

    #[derive(Debug, Clone, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RunwayTextToVideoRequest {
        pub model: String,
        pub prompt_text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ratio: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub duration: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub seed: Option<u64>,
        #[serde(flatten)]
        pub extra: Map<String, Value>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub struct RunwayTaskRef {
        pub id: String,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RunwayTaskStatus {
        Pending,
        Running,
        Succeeded,
        Failed,
        Canceled,
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct RunwayTask {
        pub id: String,
        pub status: RunwayTaskStatus,
        pub output: Vec<String>,
        pub failure: Option<String>,
    }

    #[derive(Clone)]
    pub struct RunwayProvider {
        config: RunwayConfig,
        client: BaseHttpClient,
        lifecycle: GenerationLifecycle,
    }

    impl RunwayProvider {
        pub fn new(mut config: RunwayConfig) -> Result<Self, ProviderError> {
            super::config_boundary::validate_media_config(
                PROVIDER,
                &mut config.base,
                super::config_boundary::MediaCredential::Bearer,
                &["authorization", "x-runway-version"],
            )?;
            config
                .validate()
                .map_err(|error| ProviderError::configuration(PROVIDER, error))?;
            Ok(Self {
                client: BaseHttpClient::new_for_provider_no_redirect(
                    PROVIDER,
                    config.base.clone(),
                )?,
                lifecycle: GenerationLifecycle::new_no_redirect(
                    PROVIDER,
                    config.base.clone(),
                    config.poll_policy,
                )?,
                config,
            })
        }

        pub fn from_env() -> Result<Self, ProviderError> {
            Self::new(RunwayConfig::from_env())
        }

        pub async fn submit_image_to_video(
            &self,
            request: RunwayImageToVideoRequest,
        ) -> Result<RunwayTaskRef, ProviderError> {
            self.submit("image_to_video", &request).await
        }

        pub async fn submit_text_to_video(
            &self,
            request: RunwayTextToVideoRequest,
        ) -> Result<RunwayTaskRef, ProviderError> {
            self.submit("text_to_video", &request).await
        }

        pub async fn get_task(&self, task_id: &str) -> Result<RunwayTask, ProviderError> {
            let request = self.authenticated(self.client.get(self.task_url(task_id)?)?)?;
            let response = request
                .send()
                .await
                .map_err(|error| self.client.map_preserved_request_error(error))?;
            self.parse_task_response(task_id, response).await
        }

        pub async fn cancel_task(&self, task_id: &str) -> Result<(), ProviderError> {
            let request = self.authenticated(
                self.client
                    .request(reqwest::Method::DELETE, self.task_url(task_id)?)?,
            )?;
            let response = request
                .send()
                .await
                .map_err(|error| self.client.map_preserved_request_error(error))?;
            let (status, body) = self.read_response_body(response).await?;
            if status.is_success() {
                Ok(())
            } else {
                Err(HttpErrorMapper::map_status_code(
                    PROVIDER,
                    status.as_u16(),
                    &body,
                ))
            }
        }

        pub async fn wait_for_task(
            &self,
            task_id: &str,
            cancellation: &CancellationToken,
        ) -> Result<GenerationOutput, ProviderError> {
            let api_key = self.api_key()?;
            let expected_id = task_id.to_string();
            self.lifecycle
                .wait_for_json(
                    self.task_url(task_id)?,
                    self.request_headers(api_key),
                    cancellation,
                    move |value| decode_task_poll(value, &expected_id),
                )
                .await
        }

        async fn submit<T: Serialize + ?Sized>(
            &self,
            endpoint: &str,
            request: &T,
        ) -> Result<RunwayTaskRef, ProviderError> {
            let request_builder = self.authenticated(
                self.client
                    .post(format!("{}/{endpoint}", self.api_base()))?,
            )?;
            let response = request_builder
                .json(request)
                .send()
                .await
                .map_err(|error| self.client.map_preserved_request_error(error))?;
            let (status, body) = self.read_response_body(response).await?;
            if !status.is_success() {
                return Err(HttpErrorMapper::map_status_code(
                    PROVIDER,
                    status.as_u16(),
                    &body,
                ));
            }
            let task: RunwayTaskRef = serde_json::from_str(&body).map_err(|error| {
                ProviderError::response_parsing(
                    PROVIDER,
                    format!("invalid Runway submit response: {error}"),
                )
            })?;
            self.task_url(&task.id).map_err(|_| {
                ProviderError::response_parsing(
                    PROVIDER,
                    "Runway submit response contained an invalid task ID",
                )
            })?;
            Ok(task)
        }

        async fn parse_task_response(
            &self,
            expected_id: &str,
            response: reqwest::Response,
        ) -> Result<RunwayTask, ProviderError> {
            let (status, body) = self.read_response_body(response).await?;
            if !status.is_success() {
                return Err(HttpErrorMapper::map_status_code(
                    PROVIDER,
                    status.as_u16(),
                    &body,
                ));
            }
            let value: Value = serde_json::from_str(&body).map_err(|error| {
                ProviderError::response_parsing(
                    PROVIDER,
                    format!("invalid Runway task response: {error}"),
                )
            })?;
            decode_task(&value, expected_id)
        }

        async fn read_response_body(
            &self,
            response: reqwest::Response,
        ) -> Result<(StatusCode, String), ProviderError> {
            let status = response.status();
            match response.text().await {
                Ok(body) => Ok((status, body)),
                Err(error) if status.is_success() => {
                    Err(self.client.map_preserved_request_error(error))
                }
                Err(_) => Err(HttpErrorMapper::map_status_without_body(
                    PROVIDER,
                    status.as_u16(),
                )),
            }
        }

        fn authenticated(
            &self,
            request: ProviderRequestBuilder,
        ) -> Result<ProviderRequestBuilder, ProviderError> {
            Ok(apply_provider_headers(
                request,
                self.request_headers(self.api_key()?),
            ))
        }

        fn request_headers(&self, api_key: &str) -> Vec<HeaderPair> {
            let mut headers = self
                .config
                .base
                .headers
                .iter()
                .filter(|(key, _)| {
                    !key.eq_ignore_ascii_case("authorization")
                        && !key.eq_ignore_ascii_case("x-runway-version")
                })
                .map(|(key, value)| header_owned(key.clone(), value.clone()))
                .collect::<Vec<_>>();
            headers.push(header_owned(
                "authorization".to_string(),
                format!("Bearer {api_key}"),
            ));
            headers.push(header_static("x-runway-version", API_VERSION));
            headers
        }

        fn api_key(&self) -> Result<&str, ProviderError> {
            self.config
                .base
                .api_key
                .as_deref()
                .ok_or_else(|| ProviderError::authentication(PROVIDER, "API key is required"))
        }

        fn api_base(&self) -> &str {
            self.config
                .base
                .api_base
                .as_deref()
                .unwrap_or(DEFAULT_API_BASE)
                .trim_end_matches('/')
        }

        fn task_url(&self, task_id: &str) -> Result<String, ProviderError> {
            if !valid_task_id(task_id) {
                return Err(ProviderError::invalid_request(
                    PROVIDER,
                    "invalid Runway task ID",
                ));
            }
            Ok(format!("{}/tasks/{task_id}", self.api_base()))
        }
    }

    impl std::fmt::Debug for RunwayProvider {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("RunwayProvider").finish_non_exhaustive()
        }
    }

    fn valid_task_id(id: &str) -> bool {
        !id.is_empty()
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    }

    fn decode_task(value: &Value, expected_id: &str) -> Result<RunwayTask, ProviderError> {
        let id = value["id"]
            .as_str()
            .ok_or_else(|| ProviderError::response_parsing(PROVIDER, "Runway task omitted id"))?;
        if !valid_task_id(id) || id != expected_id {
            return Err(ProviderError::response_parsing(
                PROVIDER,
                "Runway task response contained an invalid or mismatched id",
            ));
        }
        let status = match value["status"].as_str() {
            Some("PENDING" | "THROTTLED") => RunwayTaskStatus::Pending,
            Some("RUNNING") => RunwayTaskStatus::Running,
            Some("SUCCEEDED") => RunwayTaskStatus::Succeeded,
            Some("FAILED") => RunwayTaskStatus::Failed,
            Some("CANCELED" | "CANCELLED") => RunwayTaskStatus::Canceled,
            status => {
                return Err(ProviderError::response_parsing(
                    PROVIDER,
                    format!("unknown Runway task status: {status:?}"),
                ));
            }
        };
        let output = if status == RunwayTaskStatus::Succeeded {
            let values = value["output"]
                .as_array()
                .filter(|values| !values.is_empty())
                .ok_or_else(|| {
                    ProviderError::response_parsing(
                        PROVIDER,
                        "Runway succeeded task requires non-empty output",
                    )
                })?;
            values
                .iter()
                .map(|item| {
                    let output = item.as_str().ok_or_else(|| {
                        ProviderError::response_parsing(
                            PROVIDER,
                            "Runway succeeded task output must contain only URLs",
                        )
                    })?;
                    let parsed = url::Url::parse(output).map_err(|_| {
                        ProviderError::response_parsing(
                            PROVIDER,
                            "Runway succeeded task output contained an invalid URL",
                        )
                    })?;
                    if !matches!(parsed.scheme(), "http" | "https") {
                        return Err(ProviderError::response_parsing(
                            PROVIDER,
                            "Runway succeeded task output contained an invalid URL",
                        ));
                    }
                    Ok(output.to_string())
                })
                .collect::<Result<Vec<_>, ProviderError>>()?
        } else {
            Vec::new()
        };
        Ok(RunwayTask {
            id: id.to_string(),
            status,
            output,
            failure: value["failure"].as_str().map(str::to_string),
        })
    }

    fn decode_task_poll(value: Value, expected_id: &str) -> Result<GenerationPoll, ProviderError> {
        let task = decode_task(&value, expected_id)?;
        match task.status {
            RunwayTaskStatus::Pending | RunwayTaskStatus::Running => Ok(GenerationPoll::Pending),
            RunwayTaskStatus::Succeeded => Ok(GenerationPoll::Succeeded(GenerationOutput {
                urls: task.output,
                credits_used: None,
            })),
            RunwayTaskStatus::Failed => Ok(GenerationPoll::Failed(
                task.failure
                    .unwrap_or_else(|| "Runway generation failed".to_string()),
            )),
            RunwayTaskStatus::Canceled => Ok(GenerationPoll::Canceled),
        }
    }
}

#[cfg(all(test, feature = "runway-media"))]
mod tests {
    use std::time::Duration;

    use super::runway::RunwayConfig;

    #[test]
    fn runway_default_polling_respects_official_interval() {
        let policy = RunwayConfig::default().poll_policy;

        assert!(policy.initial_delay >= Duration::from_secs(5));
        assert!(policy.max_delay >= policy.initial_delay);
    }

    #[test]
    fn runway_deserialization_preserves_official_poll_interval() {
        let config: RunwayConfig = serde_json::from_value(serde_json::json!({
            "api_key": "runway-secret"
        }))
        .expect("Runway config should deserialize");

        assert_eq!(config.poll_policy.initial_delay, Duration::from_secs(5));
        assert_eq!(config.poll_policy.max_delay, Duration::from_secs(20));
    }
}
