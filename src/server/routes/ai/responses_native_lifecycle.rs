//! Native response handles are scoped to their gateway owner and upstream account.
use actix_web::{HttpRequest, HttpResponse};
use futures::StreamExt;
use serde_json::Value;

use super::{AppState, MAX_RESPONSE_BYTES, ProviderError};
use crate::core::providers::{Provider, base::HttpMethod};
use crate::server::guardrails::{self, GuardrailDecisionSink};
use crate::storage::database::{Database, entities::response::Model as ResponseRecord};
use crate::utils::error::gateway_error::GatewayError;

pub(super) struct NativeResponseStorage {
    owner: String,
    input_json: String,
    deployment: String,
    binding: String,
    record: Option<ResponseRecord>,
}

impl NativeResponseStorage {
    pub(super) fn new(owner: String, input: &Value, deployment: String, binding: String) -> Self {
        Self {
            owner,
            input_json: input.to_string(),
            deployment,
            binding,
            record: None,
        }
    }

    pub(super) async fn save(
        &mut self,
        database: &Database,
        value: &Value,
    ) -> Result<(), GatewayError> {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                ProviderError::response_parsing("responses", "Stored response is missing its ID")
            })?;
        let status = value.get("status").and_then(Value::as_str).ok_or_else(|| {
            ProviderError::response_parsing("responses", "Stored response is missing its status")
        })?;
        let now = chrono::Utc::now().timestamp();
        if let Some(record) = self.record.as_mut() {
            if record.id != id {
                return Err(ProviderError::response_parsing(
                    "responses",
                    "Response ID changed during streaming",
                )
                .into());
            }
            if !database
                .finish_owned_response(record, value.to_string(), status, now)
                .await?
            {
                return Err(GatewayError::conflict(
                    "Stored response was deleted or finalized concurrently",
                ));
            }
            record.response_json = value.to_string();
            record.status = status.into();
            record.revision += 1;
        } else {
            let record = ResponseRecord {
                id: id.into(),
                owner: self.owner.clone(),
                response_json: value.to_string(),
                input_json: self.input_json.clone(),
                deployment_id: Some(self.deployment.clone()),
                deployment_binding: Some(self.binding.clone()),
                background: false,
                status: status.into(),
                expires_at: now + 86_400,
                lease_until: None,
                revision: 0,
            };
            database.insert_response(record.clone(), now).await?;
            self.record = Some(record);
        }
        Ok(())
    }
}

fn bound_provider(state: &AppState, record: &ResponseRecord) -> Result<Provider, GatewayError> {
    let deployment = record
        .deployment_id
        .as_deref()
        .and_then(|id| state.unified_router().get_deployment(id))
        .ok_or_else(|| {
            GatewayError::conflict("The response's upstream deployment is unavailable")
        })?;
    if record.deployment_binding.is_none()
        || deployment.provider.native_response_binding() != record.deployment_binding
    {
        return Err(GatewayError::conflict(
            "The response's upstream account configuration has changed",
        ));
    }
    Ok(deployment.provider.clone())
}

pub(in crate::server::routes::ai) async fn lifecycle(
    state: &AppState,
    req: &HttpRequest,
    record: ResponseRecord,
    method: HttpMethod,
    suffix: Option<&str>,
) -> Result<HttpResponse, GatewayError> {
    if url::form_urlencoded::parse(req.query_string().as_bytes())
        .any(|(key, value)| key == "stream" && value == "true")
    {
        return Err(GatewayError::validation(
            "Resuming a stored native response stream is not yet supported",
        ));
    }
    let provider = bound_provider(state, &record)?;
    let response = provider
        .native_response_lifecycle(&record.id, method.clone(), suffix, req.query_string())
        .await?;
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            ProviderError::network("responses", "Responses lifecycle body interrupted")
        })?;
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ProviderError::response_parsing(
                "responses",
                "Responses lifecycle body exceeds size limit",
            )
            .into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes).map_err(|_| {
        ProviderError::response_parsing("responses", "Invalid Responses lifecycle JSON")
    })?;
    if suffix.is_none() && value.get("id").and_then(Value::as_str) != Some(record.id.as_str()) {
        return Err(ProviderError::response_parsing(
            "responses",
            "Responses lifecycle returned a different ID",
        )
        .into());
    }
    if matches!(method, HttpMethod::DELETE) {
        if value.get("deleted") != Some(&Value::Bool(true)) {
            return Err(ProviderError::response_parsing(
                "responses",
                "Upstream did not confirm response deletion",
            )
            .into());
        }
        state
            .storage
            .database
            .delete_owned_response(&record.id, &record.owner, chrono::Utc::now().timestamp())
            .await?;
        return Ok(HttpResponse::Ok().json(value));
    }
    let sink = GuardrailDecisionSink::from_state(
        state,
        None,
        Some(provider.name()),
        record.deployment_id.as_deref(),
    );
    let value =
        guardrails::apply_native_responses(state.guardrails().as_ref(), value, true, &sink).await?;
    Ok(HttpResponse::Ok().json(value))
}
