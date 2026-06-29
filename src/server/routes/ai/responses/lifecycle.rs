use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::responses_api::{
    ResponseInput, ResponseInputContent, ResponseInputItem, ResponseInputMessage,
    ResponseOutputContent, ResponseOutputItem, ResponsesApiRequest, ResponsesApiResponse,
};
use crate::server::routes::ai::chat::handle_chat_completion_with_state;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::{HttpRequest, HttpResponse, Result as ActixResult, web};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use tokio::task::JoinHandle;
use tracing::error;

use super::{convert_to_responses_api, current_unix_ts, uuid_v4_hex};
use crate::server::routes::ai::openai_errors;

static RESPONSE_STORE: LazyLock<DashMap<String, StoredResponse>> = LazyLock::new(DashMap::new);
static BACKGROUND_TASKS: LazyLock<DashMap<String, JoinHandle<()>>> = LazyLock::new(DashMap::new);
const RESPONSE_STORE_LIMIT: usize = 1024;
const RESPONSE_STORE_TTL_SECS: i64 = 86_400;

#[derive(Clone)]
pub(super) struct StoredResponse {
    response: ResponsesApiResponse,
    input: ResponseInput,
    background: bool,
    owner: ResponseOwner,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResponseOwner(String);

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

#[derive(Debug, Serialize)]
struct ResponseInputListItem {
    id: String,
    #[serde(flatten)]
    item: ResponseInputItem,
}

#[derive(Deserialize)]
pub struct InputItemsQuery {
    after: Option<String>,
    include: Option<Vec<String>>,
    limit: Option<usize>,
    order: Option<String>,
}

struct BackgroundTaskCleanup {
    response_id: String,
}

impl Drop for BackgroundTaskCleanup {
    fn drop(&mut self) {
        BACKGROUND_TASKS.remove(&self.response_id);
    }
}

pub async fn get_response(
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    match get_owned_response(response_id.as_str(), &owner) {
        Ok(stored) => Ok(HttpResponse::Ok().json(stored.response)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

pub async fn delete_response(
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    let response_id = response_id.into_inner();
    match get_owned_response(&response_id, &owner) {
        Ok(_) => {
            RESPONSE_STORE.remove(&response_id);
            if let Some((_, task)) = BACKGROUND_TASKS.remove(&response_id) {
                task.abort();
            }
            Ok(HttpResponse::Ok().json(DeletedResponse {
                id: response_id,
                object: "response",
                deleted: true,
            }))
        }
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

pub async fn cancel_response(
    req: HttpRequest,
    response_id: web::Path<String>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    match cancel_stored_background_response(response_id.as_str(), &owner) {
        Ok(response) => Ok(HttpResponse::Ok().json(response)),
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

pub async fn list_response_input_items(
    req: HttpRequest,
    response_id: web::Path<String>,
    query: web::Query<InputItemsQuery>,
) -> ActixResult<HttpResponse> {
    let owner = response_owner(&super::super::context::get_request_context(&req)?);
    match get_owned_response(response_id.as_str(), &owner) {
        Ok(stored) => match input_items_page(&stored.input, &query) {
            Ok(page) => Ok(HttpResponse::Ok().json(page)),
            Err(error) => Ok(openai_errors::gateway_error_response(&error)),
        },
        Err(error) => Ok(openai_errors::gateway_error_response(&error)),
    }
}

pub(super) fn handle_background_response(
    state: AppState,
    chat_request: ChatCompletionRequest,
    original: ResponsesApiRequest,
    context: crate::core::types::context::RequestContext,
    owner: Option<ResponseOwner>,
) -> HttpResponse {
    let Some(owner) = owner else {
        return openai_errors::validation_error(
            "background responses require an authenticated owner",
        );
    };
    let response = queued_background_response(&original);
    let response_id = response.id.clone();
    insert_stored_response(
        response_id.clone(),
        StoredResponse {
            response: response.clone(),
            input: original.input.clone(),
            background: true,
            owner,
        },
    );
    let task_response_id = response_id.clone();
    let handle = tokio::spawn(async move {
        let _cleanup = BackgroundTaskCleanup {
            response_id: response_id.clone(),
        };
        set_background_status(&response_id, "in_progress");
        match handle_chat_completion_with_state(&state, chat_request, context).await {
            Ok(chat_resp) => {
                let mut response = convert_to_responses_api(chat_resp, &original);
                response.id = response_id.clone();
                finish_background_response(&response_id, original.input.clone(), response);
            }
            Err(error) => {
                error!("Background Responses API error: {}", error);
                set_background_status(&response_id, "failed");
            }
        }
    });
    BACKGROUND_TASKS.insert(task_response_id, handle);
    HttpResponse::Ok().json(response)
}

pub(crate) fn store_response_if_requested(
    original: &ResponsesApiRequest,
    response: &ResponsesApiResponse,
    owner: Option<ResponseOwner>,
) {
    if original.store.unwrap_or(true)
        && let Some(owner) = owner
    {
        insert_stored_response(
            response.id.clone(),
            StoredResponse {
                response: response.clone(),
                input: original.input.clone(),
                background: false,
                owner,
            },
        );
    }
}

pub(super) fn resolve_previous_response_context(
    mut request: ResponsesApiRequest,
    owner: &Option<ResponseOwner>,
) -> Result<ResponsesApiRequest, GatewayError> {
    let Some(previous_response_id) = request.previous_response_id.as_deref() else {
        return Ok(request);
    };
    let stored = get_owned_response(previous_response_id, owner)?;
    request.input = append_previous_context(&stored, &request.input);
    Ok(request)
}

pub(super) fn response_owner(
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

fn set_background_status(response_id: &str, status: &str) {
    if let Some(mut stored) = RESPONSE_STORE.get_mut(response_id)
        && stored.background
        && stored.response.status != "cancelled"
    {
        stored.response.status = status.to_string();
    }
}

fn finish_background_response(
    response_id: &str,
    input: ResponseInput,
    response: ResponsesApiResponse,
) {
    if let Some(mut stored) = RESPONSE_STORE.get_mut(response_id)
        && stored.response.status != "cancelled"
    {
        stored.response = response;
        stored.input = input;
    }
}

fn cancel_stored_background_response(
    response_id: &str,
    owner: &Option<ResponseOwner>,
) -> Result<ResponsesApiResponse, GatewayError> {
    let Some(mut stored) = RESPONSE_STORE.get_mut(response_id) else {
        return Err(response_not_found(response_id));
    };
    let Some(owner) = owner else {
        return Err(response_not_found(response_id));
    };
    if &stored.owner != owner {
        return Err(response_not_found(response_id));
    }
    if !stored.background {
        return Err(GatewayError::conflict(
            "Only background Responses tasks can be canceled",
        ));
    }
    match stored.response.status.as_str() {
        "queued" | "in_progress" | "cancelled" => {
            if let Some((_, task)) = BACKGROUND_TASKS.remove(response_id) {
                task.abort();
            }
            stored.response.status = "cancelled".to_string();
            Ok(stored.response.clone())
        }
        status => Err(GatewayError::conflict(format!(
            "Cannot cancel background response with status {status}"
        ))),
    }
}

fn get_owned_response(
    response_id: &str,
    owner: &Option<ResponseOwner>,
) -> Result<StoredResponse, GatewayError> {
    let stored = RESPONSE_STORE
        .get(response_id)
        .ok_or_else(|| response_not_found(response_id))?;
    let Some(owner) = owner else {
        return Err(response_not_found(response_id));
    };
    if &stored.owner != owner {
        return Err(response_not_found(response_id));
    }
    Ok(stored.clone())
}

fn insert_stored_response(response_id: String, stored: StoredResponse) {
    cleanup_response_store();
    RESPONSE_STORE.insert(response_id, stored);
}

fn cleanup_response_store() {
    let now = current_unix_ts();
    let mut entries = RESPONSE_STORE
        .iter()
        .map(|entry| (entry.key().clone(), entry.value().response.created_at))
        .collect::<Vec<_>>();

    for (response_id, created_at) in &entries {
        if now.saturating_sub(*created_at) > RESPONSE_STORE_TTL_SECS {
            remove_response_and_task(response_id);
        }
    }

    entries.retain(|(_, created_at)| now.saturating_sub(*created_at) <= RESPONSE_STORE_TTL_SECS);
    let overflow = entries
        .len()
        .saturating_add(1)
        .saturating_sub(RESPONSE_STORE_LIMIT);
    if overflow == 0 {
        return;
    }

    entries.sort_by_key(|(_, created_at)| *created_at);
    for (response_id, _) in entries.into_iter().take(overflow) {
        remove_response_and_task(&response_id);
    }
}

fn remove_response_and_task(response_id: &str) {
    RESPONSE_STORE.remove(response_id);
    if let Some((_, task)) = BACKGROUND_TASKS.remove(response_id) {
        task.abort();
    }
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
        .map(|(index, item)| (stable_input_item_id(index, &item), item))
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

    let mut start = 0;
    if let Some(after) = &query.after {
        let index = items
            .iter()
            .position(|(id, _)| id == after)
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
    let first_id = data.first().map(|(id, _)| id.clone());
    let last_id = data.last().map(|(id, _)| id.clone());
    let data = data
        .into_iter()
        .map(|(id, item)| ResponseInputListItem { id, item })
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

    let value = serde_json::to_string(&(index, item)).unwrap_or_else(|_| index.to_string());
    let digest = Sha256::digest(value.as_bytes());
    format!("item_{}", hex::encode(&digest[..8]))
}

fn output_items_as_input_context(output: &[ResponseOutputItem]) -> Vec<ResponseInputItem> {
    output
        .iter()
        .filter_map(|item| {
            let ResponseOutputItem::Message(message) = item else {
                return None;
            };
            let text = output_text_content(&message.content);
            if text.trim().is_empty() {
                return None;
            }
            Some(ResponseInputItem::Message(ResponseInputMessage {
                role: "assistant".to_string(),
                content: ResponseInputContent::Text(text),
            }))
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
