use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseInput, ResponseInputContent, ResponseInputItem, ResponseInputMessage,
    ResponseOutputContent, ResponseOutputItem, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::core::types::codex::wire::CodexFunctionCall;
use crate::server::routes::ai::chat::handle_chat_completion_after_input_guardrail;
use crate::server::state::AppState;
use crate::storage::database::{Database, entities::response::Model as ResponseRecord};
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use serde::ser::Error as SerializeError;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc, time::Duration};
use tracing::error;

use super::super::responses_native::lifecycle as native_lifecycle;
use super::{convert_to_responses_api, current_unix_ts, uuid_v4_hex};
use crate::core::providers::base::HttpMethod;
use crate::server::routes::ai::openai_errors;

const RESPONSE_STORE_TTL_SECS: i64 = 86_400;

#[derive(Clone)]
pub(super) struct StoredResponse {
    response: ResponsesApiResponse,
    input: ResponseInput,
    record: ResponseRecord,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResponseOwner(pub(crate) String);

#[derive(Serialize)]
struct DeletedResponse {
    id: String,
    object: &'static str,
    deleted: bool,
}

#[derive(Debug, Serialize)]
struct ResponseInputItemsList {
    object: &'static str,
    data: Vec<ResponseInputListItem>,
    first_id: Option<String>,
    last_id: Option<String>,
    has_more: bool,
}

#[derive(Debug)]
struct ResponseInputListItem {
    id: String,
    item: ResponseInputItem,
}

impl Serialize for ResponseInputListItem {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let Value::Object(mut item) = serde_json::to_value(&self.item).map_err(S::Error::custom)?
        else {
            return Err(S::Error::custom("response input item must be an object"));
        };
        item.insert("id".to_string(), Value::String(self.id.clone()));
        item.serialize(serializer)
    }
}

#[derive(Deserialize)]
pub struct InputItemsQuery {
    after: Option<String>,
    include: Option<Vec<String>>,
    limit: Option<usize>,
    order: Option<String>,
}

pub async fn get_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    let result = async {
        let record =
            get_owned_record(&state.storage.database, response_id.as_str(), &owner).await?;
        if record.deployment_id.is_some() {
            native_lifecycle::lifecycle(&state, &req, record, HttpMethod::GET, None).await
        } else {
            Ok(HttpResponse::Ok().json(decode_stored_response(record)?.response))
        }
    }
    .await;
    Ok(result.unwrap_or_else(|error| openai_errors::gateway_error_response(&error)))
}

pub async fn delete_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    let result = async {
        let record =
            get_owned_record(&state.storage.database, response_id.as_str(), &owner).await?;
        if record.deployment_id.is_some() {
            return native_lifecycle::lifecycle(&state, &req, record, HttpMethod::DELETE, None)
                .await;
        }
        if !state
            .storage
            .database
            .delete_owned_response(&record.id, &record.owner, current_unix_ts())
            .await?
        {
            return Err(response_not_found(&record.id));
        }
        Ok(HttpResponse::Ok().json(DeletedResponse {
            id: record.id,
            object: "response.deleted",
            deleted: true,
        }))
    }
    .await;
    Ok(result.unwrap_or_else(|error| openai_errors::gateway_error_response(&error)))
}

pub async fn cancel_response(
    state: web::Data<AppState>,
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    let result = async {
        let record =
            get_owned_record(&state.storage.database, response_id.as_str(), &owner).await?;
        if record.deployment_id.is_some() {
            if !record.background {
                return Err(GatewayError::conflict(
                    "Only background Responses tasks can be canceled",
                ));
            }
            return native_lifecycle::lifecycle(
                &state,
                &req,
                record,
                HttpMethod::POST,
                Some("cancel"),
            )
            .await;
        }
        cancel_background_record(
            &state.storage.database,
            decode_stored_response(record)?,
            &owner,
        )
        .await
        .map(|response| HttpResponse::Ok().json(response))
    }
    .await;
    Ok(result.unwrap_or_else(|error| openai_errors::gateway_error_response(&error)))
}

pub async fn list_response_input_items(
    state: web::Data<AppState>,
    req: HttpRequest,
    response_id: web::Path<String>,
    query: web::Query<InputItemsQuery>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    let result = async {
        let record =
            get_owned_record(&state.storage.database, response_id.as_str(), &owner).await?;
        if record.deployment_id.is_some() {
            return native_lifecycle::lifecycle(
                &state,
                &req,
                record,
                HttpMethod::GET,
                Some("input_items"),
            )
            .await;
        }
        let stored = decode_stored_response(record)?;
        input_items_page(&stored.input, &query).map(|page| HttpResponse::Ok().json(page))
    }
    .await;
    Ok(result.unwrap_or_else(|error| openai_errors::gateway_error_response(&error)))
}

pub(super) async fn handle_background_response(
    state: AppState,
    chat_request: ChatCompletionRequest,
    guarded_request: ResponsesApiRequest,
    mut context: crate::core::types::context::RequestContext,
    owner: Option<ResponseOwner>,
) -> HttpResponse {
    let Some(owner) = owner else {
        return openai_errors::validation_error(
            "background responses require an authenticated owner",
        );
    };
    let response = queued_background_response(&guarded_request);
    let record = match response_record(&guarded_request, &response, &owner, true) {
        Ok(record) => record,
        Err(error) => return openai_errors::gateway_error_response(&error),
    };
    let database = Arc::clone(&state.storage.database);
    if let Err(error) = database
        .insert_response(record.clone(), current_unix_ts())
        .await
    {
        return openai_errors::gateway_error_response(&error);
    }
    super::super::response_cache::bypass_chat_response_cache(&mut context);
    let mut running = response.clone();
    running.status = "in_progress".into();
    let running_json = match serde_json::to_string(&running) {
        Ok(json) => json,
        Err(error) => return openai_errors::gateway_error_response(&error.into()),
    };
    tokio::spawn(async move {
        let mut record = record;
        match database
            .start_response_worker(&record, running_json, current_unix_ts())
            .await
        {
            Ok(true) => {
                record.revision += 1;
                record.status = "in_progress".into();
            }
            Ok(false) => return,
            Err(error) => {
                error!("Responses worker could not start: {error}");
                return;
            }
        }

        // The shared record is also the cancellation signal. Dropping this future
        // stops local upstream work when any replica cancels/deletes the response.
        let work = handle_chat_completion_after_input_guardrail(
            &state,
            Arc::new(chat_request),
            Arc::new(context),
        );
        tokio::pin!(work);
        let mut heartbeat = tokio::time::interval(Duration::from_secs(1));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        heartbeat.tick().await;
        let remaining = record
            .lease_until
            .unwrap_or_default()
            .saturating_sub(current_unix_ts())
            .max(0) as u64;
        let mut lease_deadline = tokio::time::Instant::now() + Duration::from_secs(remaining);
        let result = loop {
            tokio::select! {
                biased;
                _ = tokio::time::sleep_until(lease_deadline) => return,
                result = &mut work => break result,
                (started, renewal) = async {
                    heartbeat.tick().await;
                    let started = tokio::time::Instant::now();
                    let renewal = database.renew_response_lease(&record.id, &record.owner, current_unix_ts()).await;
                    (started, renewal)
                } => match renewal {
                    Ok(true) => lease_deadline = started + Duration::from_secs(60),
                    Ok(false) => return,
                    Err(error) => {
                        // Keep polling the paid request during a transient database
                        // failure, but never execute beyond the last confirmed lease.
                        error!("Responses worker lease renewal failed: {error}");
                    }
                }
            }
        };
        let mut response = match result {
            Ok(chat_resp) => convert_to_responses_api(chat_resp, &guarded_request),
            Err(error) => {
                error!("Background Responses API error: {error}");
                let mut response = queued_background_response(&guarded_request);
                response.status = "failed".into();
                response.error = Some(
                    crate::core::models::openai::responses_api::ResponseApiError {
                        code: "server_error".into(),
                        message: "Background response execution failed".into(),
                    },
                );
                response
            }
        };
        response.id = record.id.clone();
        if let Err(error) = finish_background_response(&database, &record, &response).await {
            error!("Responses completion could not be stored: {error}");
        }
    });
    HttpResponse::Ok().json(response)
}

fn response_record(
    original: &ResponsesApiRequest,
    response: &ResponsesApiResponse,
    owner: &ResponseOwner,
    background: bool,
) -> Result<ResponseRecord, GatewayError> {
    Ok(ResponseRecord {
        id: response.id.clone(),
        owner: owner.0.clone(),
        response_json: serde_json::to_string(response)?,
        input_json: serde_json::to_string(&original.input)?,
        deployment_id: None,
        deployment_binding: None,
        background,
        status: response.status.clone(),
        expires_at: current_unix_ts() + RESPONSE_STORE_TTL_SECS,
        lease_until: background.then(|| current_unix_ts() + 60),
        revision: 0,
    })
}

pub(crate) async fn store_response_if_requested(
    database: &Database,
    original: &ResponsesApiRequest,
    response: &ResponsesApiResponse,
    owner: Option<ResponseOwner>,
) -> Result<(), GatewayError> {
    if original.store.unwrap_or(true)
        && let Some(owner) = owner
    {
        database
            .insert_response(
                response_record(original, response, &owner, false)?,
                current_unix_ts(),
            )
            .await?;
    }
    Ok(())
}

pub(super) async fn resolve_previous_response_context(
    database: &Database,
    mut request: ResponsesApiRequest,
    owner: &Option<ResponseOwner>,
) -> Result<ResponsesApiRequest, GatewayError> {
    let Some(previous_response_id) = request.previous_response_id.as_deref() else {
        return Ok(request);
    };
    let stored = get_owned_response(database, previous_response_id, owner).await?;
    request.input = append_previous_context(&stored, &request.input);
    Ok(request)
}

pub(crate) fn response_owner(
    context: &crate::core::types::context::RequestContext,
) -> Option<ResponseOwner> {
    if let Some(api_key_id) = context.api_key_id() {
        return Some(ResponseOwner(format!("api_key:{api_key_id}")));
    }
    if let Some(user_id) = context
        .user_id
        .as_deref()
        .filter(|user_id| !user_id.is_empty())
    {
        return Some(ResponseOwner(format!("user:{user_id}")));
    }
    None
}

pub(super) fn validate_storage_owner(
    request: &ResponsesApiRequest,
    owner: &Option<ResponseOwner>,
) -> Result<(), GatewayError> {
    if owner.is_none() && request.store != Some(false) {
        return Err(GatewayError::validation(
            "Responses lifecycle storage requires authentication; set store=false for anonymous requests",
        ));
    }
    Ok(())
}

fn queued_background_response(original: &ResponsesApiRequest) -> ResponsesApiResponse {
    ResponsesApiResponse {
        id: format!("resp_bg_{}", uuid_v4_hex()),
        object: "response".to_string(),
        created_at: current_unix_ts(),
        status: "queued".to_string(),
        model: original.model.clone(),
        output: vec![],
        usage: None,
        error: None,
        previous_response_id: original.previous_response_id.clone(),
        metadata: original.metadata.clone(),
    }
}

async fn finish_background_response(
    database: &Database,
    record: &ResponseRecord,
    response: &ResponsesApiResponse,
) -> Result<bool, GatewayError> {
    database
        .finish_owned_response(
            record,
            serde_json::to_string(response)?,
            &response.status,
            current_unix_ts(),
        )
        .await
}

#[cfg(test)]
async fn cancel_stored_background_response(
    database: &Database,
    response_id: &str,
    owner: &Option<ResponseOwner>,
) -> Result<ResponsesApiResponse, GatewayError> {
    let stored = get_owned_response(database, response_id, owner).await?;
    cancel_background_record(database, stored, owner).await
}

async fn cancel_background_record(
    database: &Database,
    mut stored: StoredResponse,
    owner: &Option<ResponseOwner>,
) -> Result<ResponsesApiResponse, GatewayError> {
    let response_id = stored.record.id.clone();
    if !stored.record.background {
        return Err(GatewayError::conflict(
            "Only background Responses tasks can be canceled",
        ));
    }
    match stored.response.status.as_str() {
        "cancelled" => Ok(stored.response),
        "queued" | "in_progress" => {
            stored.response.status = "cancelled".into();
            if finish_background_response(database, &stored.record, &stored.response).await? {
                return Ok(stored.response);
            }
            let latest = get_owned_response(database, &response_id, owner).await?;
            if latest.response.status == "cancelled" {
                Ok(latest.response)
            } else {
                Err(GatewayError::conflict(
                    "Background response finished before cancellation",
                ))
            }
        }
        status => Err(GatewayError::conflict(format!(
            "Cannot cancel background response with status {status}"
        ))),
    }
}

async fn get_owned_response(
    database: &Database,
    response_id: &str,
    owner: &Option<ResponseOwner>,
) -> Result<StoredResponse, GatewayError> {
    decode_stored_response(get_owned_record(database, response_id, owner).await?)
}

fn decode_stored_response(record: ResponseRecord) -> Result<StoredResponse, GatewayError> {
    if record.deployment_id.is_some() {
        return Err(GatewayError::validation(
            "A native response cannot be continued through the chat adapter",
        ));
    }
    Ok(StoredResponse {
        response: serde_json::from_str(&record.response_json)?,
        input: serde_json::from_str(&record.input_json)?,
        record,
    })
}

async fn get_owned_record(
    database: &Database,
    response_id: &str,
    owner: &Option<ResponseOwner>,
) -> Result<ResponseRecord, GatewayError> {
    let Some(owner) = owner else {
        return Err(response_not_found(response_id));
    };
    let now = current_unix_ts();
    let mut record = database
        .owned_response(response_id, &owner.0, now)
        .await?
        .ok_or_else(|| response_not_found(response_id))?;
    if record.lease_until.is_some_and(|deadline| deadline <= now)
        && matches!(record.status.as_str(), "queued" | "in_progress")
    {
        let mut response: ResponsesApiResponse = serde_json::from_str(&record.response_json)?;
        response.status = "failed".into();
        response.error = Some(
            crate::core::models::openai::responses_api::ResponseApiError {
                code: "server_error".into(),
                message: "Background response worker stopped before completion".into(),
            },
        );
        database
            .fail_abandoned_response(&record, serde_json::to_string(&response)?, now)
            .await?;
        record = database
            .owned_response(response_id, &owner.0, now)
            .await?
            .ok_or_else(|| response_not_found(response_id))?;
    }
    Ok(record)
}

fn append_previous_context(
    stored: &StoredResponse,
    current_input: &ResponseInput,
) -> ResponseInput {
    let mut items = input_items_from_response_input(&stored.input);
    items.extend(output_items_as_input_context(&stored.response.output));
    items.extend(non_empty_input_items(current_input));
    ResponseInput::Items(items)
}

fn non_empty_input_items(input: &ResponseInput) -> Vec<ResponseInputItem> {
    match input {
        ResponseInput::Text(text) if text.trim().is_empty() => vec![],
        ResponseInput::Text(_) | ResponseInput::Items(_) => input_items_from_response_input(input),
    }
}

fn input_items_from_response_input(input: &ResponseInput) -> Vec<ResponseInputItem> {
    match input {
        ResponseInput::Text(text) => vec![ResponseInputItem::Message(ResponseInputMessage {
            id: None,
            phase: None,
            internal_chat_message_metadata_passthrough: None,
            role: "user".to_string(),
            content: ResponseInputContent::Text(text.clone()),
        })],
        ResponseInput::Items(items) => items.clone(),
    }
}

fn input_items_page(
    input: &ResponseInput,
    query: &InputItemsQuery,
) -> Result<ResponseInputItemsList, GatewayError> {
    if let Some(include) = &query.include
        && !include.is_empty()
    {
        return Err(GatewayError::validation(
            "input_items include is not supported",
        ));
    }

    let mut items = input_items_from_response_input(input)
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            let id = stable_input_item_id(index, &item);
            (id.clone(), id, item)
        })
        .collect::<Vec<_>>();
    match query.order.as_deref().unwrap_or("desc") {
        "asc" => {}
        "desc" => items.reverse(),
        other => {
            return Err(GatewayError::validation(format!(
                "Unsupported input_items order: {other}"
            )));
        }
    }
    let ids: HashSet<_> = items.iter().map(|item| item.1.clone()).collect();
    let mut cursors = HashSet::new();
    for (index, (cursor, id, _)) in items.iter_mut().enumerate() {
        while !cursors.insert(cursor.clone()) || (cursor != id && ids.contains(cursor.as_str())) {
            *cursor = format!("{cursor}:{index}");
        }
    }

    let mut start = 0;
    if let Some(after) = &query.after {
        let index = items
            .iter()
            .position(|(cursor, _, _)| cursor == after)
            .ok_or_else(|| {
                GatewayError::validation(format!("Unknown input_items cursor: {after}"))
            })?;
        start = index + 1;
    }

    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    let has_more = start + limit < items.len();
    let data = items
        .into_iter()
        .skip(start)
        .take(limit)
        .collect::<Vec<_>>();
    let first_id = data.first().map(|(cursor, _, _)| cursor.clone());
    let last_id = data.last().map(|(cursor, _, _)| cursor.clone());
    let data = data
        .into_iter()
        .map(|(_, id, item)| ResponseInputListItem { id, item })
        .collect();

    Ok(ResponseInputItemsList {
        object: "list",
        data,
        first_id,
        last_id,
        has_more,
    })
}

fn stable_input_item_id(index: usize, item: &ResponseInputItem) -> String {
    use sha2::{Digest, Sha256};

    if let Ok(Value::Object(payload)) = serde_json::to_value(item)
        && let Some(id) = payload.get("id").and_then(Value::as_str)
        && !id.is_empty()
    {
        return id.to_string();
    }
    let value = serde_json::to_string(&(index, item)).unwrap_or_else(|_| index.to_string());
    let digest = Sha256::digest(value.as_bytes());
    format!("item_{}", hex::encode(&digest[..8]))
}

fn output_items_as_input_context(output: &[ResponseOutputItem]) -> Vec<ResponseInputItem> {
    output
        .iter()
        .filter_map(|item| match item {
            ResponseOutputItem::Message(message) => {
                let text = output_text_content(&message.content);
                if text.trim().is_empty() {
                    return None;
                }
                Some(ResponseInputItem::Message(ResponseInputMessage {
                    id: Some(message.id.clone()),
                    phase: None,
                    internal_chat_message_metadata_passthrough: None,
                    role: "assistant".to_string(),
                    content: ResponseInputContent::Text(text),
                }))
            }
            ResponseOutputItem::FunctionCall(call) => {
                Some(ResponseInputItem::FunctionCall(CodexFunctionCall {
                    id: Some(call.id.clone()),
                    call_id: call.call_id.clone().unwrap_or_else(|| call.id.clone()),
                    name: call.name.clone(),
                    namespace: None,
                    arguments: call.arguments.clone(),
                    status: Some(call.status.clone()),
                    internal_chat_message_metadata_passthrough: None,
                }))
            }
            ResponseOutputItem::CustomToolCall(call) => {
                Some(ResponseInputItem::CustomToolCall(call.clone()))
            }
            _ => None,
        })
        .collect()
}

fn output_text_content(content: &[ResponseOutputContent]) -> String {
    content
        .iter()
        .map(|part| match part {
            ResponseOutputContent::OutputText { text, .. } => text.as_str(),
            ResponseOutputContent::Refusal { refusal } => refusal.as_str(),
        })
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn response_not_found(response_id: &str) -> GatewayError {
    GatewayError::not_found(format!("Response not found: {response_id}"))
}

#[cfg(test)]
#[path = "lifecycle_tests.rs"]
mod tests;
