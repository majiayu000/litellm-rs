//! Native response handles are scoped to their gateway owner and upstream account.
use actix_web::{HttpRequest, HttpResponse};
use serde_json::Value;

use super::{AppState, MAX_RESPONSE_BYTES, ProviderError};
use crate::core::providers::{Provider, base::HttpMethod};
use crate::server::guardrails::{self, GuardrailDecisionSink};
use crate::storage::database::{Database, entities::response::Model as ResponseRecord};
use crate::utils::error::gateway_error::GatewayError;

pub(in crate::server::routes::ai) struct NativeResponseStorage {
    pub(in crate::server::routes::ai) owner: String,
    input_json: String,
    pub(in crate::server::routes::ai) deployment: String,
    pub(in crate::server::routes::ai) binding: String,
    record: Option<ResponseRecord>,
    pub(in crate::server::routes::ai) settlement_id: Option<String>,
    background: bool,
    retain_content: bool,
}

impl NativeResponseStorage {
    pub(super) fn new(owner: String, input: &Value, deployment: String, binding: String) -> Self {
        Self {
            owner,
            input_json: if input.get("store") == Some(&Value::Bool(false)) {
                serde_json::json!({"store":false,"stream":input.get("stream").and_then(Value::as_bool).unwrap_or(false)}).to_string()
            } else {
                input.to_string()
            },
            deployment,
            binding,
            record: None,
            settlement_id: None,
            background: input.get("background") == Some(&Value::Bool(true)),
            retain_content: input.get("store") != Some(&Value::Bool(false)),
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
        if let Some(settlement_id) = &self.settlement_id {
            database.bind_response_settlement(settlement_id, id).await?;
        }
        let status = value.get("status").and_then(Value::as_str).ok_or_else(|| {
            ProviderError::response_parsing("responses", "Stored response is missing its status")
        })?;
        let now = chrono::Utc::now().timestamp();
        let stored_json = stored_value(value, self.retain_content).to_string();
        if let Some(record) = self.record.as_mut() {
            if record.id != id {
                return Err(ProviderError::response_parsing(
                    "responses",
                    "Response ID changed during streaming",
                )
                .into());
            }
            if !database
                .finish_owned_response(record, stored_json.clone(), status, now)
                .await?
            {
                let current = database.owned_response(id, &self.owner, now).await?;
                // A lifecycle read/cancel may have cached the same upstream terminal
                // state first. It does not own this request's billing reservation.
                if let Some(current) = current
                    && self.background
                    && super::background::is_terminal(&current.status)
                    && super::background::is_terminal(status)
                {
                    *record = current;
                    return Ok(());
                }
                return Err(GatewayError::conflict(
                    "Stored response was deleted or finalized concurrently",
                ));
            }
            record.response_json = stored_json;
            record.status = status.into();
            record.revision += 1;
        } else {
            let record = ResponseRecord {
                id: id.into(),
                owner: self.owner.clone(),
                response_json: stored_json,
                input_json: self.input_json.clone(),
                deployment_id: Some(self.deployment.clone()),
                deployment_binding: Some(self.binding.clone()),
                background: self.background,
                status: status.into(),
                expires_at: now + if self.retain_content { 86_400 } else { 600 },
                lease_until: None,
                revision: 0,
            };
            database.insert_response(record.clone(), now).await?;
            self.record = Some(record);
        }
        Ok(())
    }
    pub(super) fn is_background(&self) -> bool {
        self.background
    }

    pub(super) fn record(&self) -> Option<&ResponseRecord> {
        self.record.as_ref()
    }
}

fn stored_value(value: &Value, retain_content: bool) -> Value {
    if retain_content {
        value.clone()
    } else {
        serde_json::json!({"id":value.get("id"),"object":"response","status":value.get("status"),"store":false})
    }
}

pub(super) async fn previous_response(
    database: &Database,
    body: &Value,
    owner: Option<&crate::server::routes::ai::responses::ResponseOwner>,
) -> Result<Option<(ResponseRecord, u32)>, GatewayError> {
    let id = match body.get("previous_response_id") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(id)) if !id.is_empty() => id,
        _ => {
            return Err(GatewayError::validation(
                "previous_response_id must be a nonempty string",
            ));
        }
    };
    let missing = || GatewayError::not_found(format!("Response '{id}' not found"));
    let owner = owner.ok_or_else(missing)?;
    let record = database
        .owned_response(id, &owner.0, chrono::Utc::now().timestamp())
        .await?
        .ok_or_else(missing)?;
    let stored_input: Value = serde_json::from_str(&record.input_json)?;
    if stored_input.get("store") == Some(&Value::Bool(false)) {
        return Err(GatewayError::validation(
            "A response created with store=false cannot be continued",
        ));
    }
    if record.deployment_id.is_none() || record.deployment_binding.is_none() {
        return Err(GatewayError::validation(
            "A chat-adapted response cannot be continued through a native deployment",
        ));
    }
    if matches!(record.status.as_str(), "queued" | "in_progress") {
        return Err(GatewayError::conflict(
            "The previous response has not finished",
        ));
    }
    let value: Value = serde_json::from_str(&record.response_json)?;
    // The previous response's total includes its retained input and generated
    // output, including opaque reasoning that cannot be tokenized from JSON.
    let usage = super::response_usage(&value).ok_or_else(|| {
        GatewayError::conflict(
            "The previous response has no verified usage for context reservation",
        )
    })?;
    Ok(Some((record, usage.total_tokens)))
}

pub(super) async fn retained_input(
    state: &AppState,
    body: &Value,
    previous: Option<&ResponseRecord>,
    sink: &GuardrailDecisionSink,
) -> Result<Value, GatewayError> {
    let Some(previous) = previous else {
        return Ok(body.clone());
    };
    let input: Value = serde_json::from_str(&previous.input_json)?;
    let output: Value = serde_json::from_str(&previous.response_json)?;
    let mut history = Vec::new();
    append_input(&mut history, input.get("input"));
    append_input(&mut history, output.get("output"));
    let retained = serde_json::json!({"input":history});
    let checked = guardrails::apply_native_responses(
        state.guardrails().as_ref(),
        retained.clone(),
        false,
        sink,
    )
    .await?;
    if checked != retained {
        return Err(GatewayError::validation(
            "Stored upstream context cannot be masked; submit explicit input instead of previous_response_id",
        ));
    }
    append_input(&mut history, body.get("input"));
    let mut stored_input = body.clone();
    stored_input["input"] = Value::Array(history);
    if stored_input.to_string().len() > MAX_RESPONSE_BYTES {
        return Err(GatewayError::validation(
            "Stored Responses context exceeds the storage size limit",
        ));
    }
    Ok(stored_input)
}

fn append_input(items: &mut Vec<Value>, value: Option<&Value>) {
    match value {
        Some(Value::Array(values)) => items.extend(values.iter().cloned()),
        Some(Value::String(text)) => items.push(serde_json::json!({"role":"user", "content":text})),
        Some(Value::Null) | None => {}
        Some(value) => items.push(value.clone()),
    }
}

pub(in crate::server::routes::ai) fn bound_provider(
    state: &AppState,
    record: &ResponseRecord,
) -> Result<Provider, GatewayError> {
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
    let streaming = url::form_urlencoded::parse(req.query_string().as_bytes())
        .any(|(key, value)| key == "stream" && value == "true");
    if streaming {
        let input: Value = serde_json::from_str(&record.input_json)?;
        if !matches!(method, HttpMethod::GET)
            || suffix.is_some()
            || !record.background
            || input.get("stream") != Some(&Value::Bool(true))
        {
            return Err(GatewayError::validation(
                "Only background responses created with stream=true can resume streaming",
            ));
        }
        guardrails::reject_unsupported_streaming_mask(state)?;
    }
    let provider = bound_provider(state, &record)?;
    let response = provider
        .native_response_lifecycle(&record.id, method.clone(), suffix, req.query_string())
        .await?;
    if streaming {
        return Ok(super::stream::resume(
            state.clone(),
            response,
            record.id.clone(),
            provider.name().to_string(),
            record.deployment_id.clone(),
        ));
    }
    let mut response = response;
    let value = read_json(&mut response).await?;
    if suffix != Some("input_items")
        && value.get("id").and_then(Value::as_str) != Some(record.id.as_str())
    {
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
    if (suffix.is_none() && matches!(method, HttpMethod::GET) || suffix == Some("cancel"))
        && let Some(status) = value.get("status").and_then(Value::as_str)
        && matches!(status, "completed" | "incomplete" | "failed" | "cancelled")
    {
        state
            .storage
            .database
            .finish_owned_response(
                &record,
                stored_value(
                    &value,
                    serde_json::from_str::<Value>(&record.input_json)?.get("store")
                        != Some(&Value::Bool(false)),
                )
                .to_string(),
                status,
                chrono::Utc::now().timestamp(),
            )
            .await?;
    }
    Ok(HttpResponse::Ok().json(value))
}

/// Bound both successful create/poll bodies, including providers that omit Content-Length.
pub(in crate::server::routes::ai) async fn read_json(
    response: &mut reqwest::Response,
) -> Result<Value, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ProviderError::network("responses", "Responses body interrupted"))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(ProviderError::response_parsing(
                "responses",
                "Responses body exceeds size limit",
            )
            .into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| ProviderError::response_parsing("responses", "Invalid Responses JSON").into())
}
