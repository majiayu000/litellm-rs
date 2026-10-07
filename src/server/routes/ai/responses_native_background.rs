//! One creation owns one settlement. Lifecycle reads never create a billing task.
use actix_web::HttpResponse;
use serde_json::Value;
use std::time::Duration;

use super::{AppState, NativeCall, RequestContext, lifecycle, response_usage, settle};
use crate::core::providers::{Provider, base::HttpMethod};
use crate::server::guardrails::{self, GuardrailDecisionSink};
use crate::server::routes::ai::execution::StreamingDeploymentLease;
use crate::utils::error::gateway_error::GatewayError;

pub(in crate::server::routes::ai) fn is_terminal(status: &str) -> bool {
    matches!(status, "completed" | "incomplete" | "failed" | "cancelled")
}

pub(super) async fn response(
    state: AppState,
    context: RequestContext,
    mut call: NativeCall,
    mut lease: StreamingDeploymentLease,
    value: Result<Value, GatewayError>,
    already_stored: bool,
    facts: Option<crate::core::request_ledger::SharedRequestLedgerFacts>,
) -> Result<HttpResponse, GatewayError> {
    let durable_id = call
        .storage
        .as_ref()
        .and_then(|storage| storage.settlement_id.clone());
    let sink = GuardrailDecisionSink::from_state(
        &state,
        Some(&call.model),
        Some(&call.provider),
        Some(&call.deployment),
    );
    let prepared = async {
        let value = value?;
        let value =
            guardrails::apply_native_responses(state.guardrails().as_ref(), value, true, &sink)
                .await?;
        let storage = call.storage.as_mut().ok_or_else(|| {
            GatewayError::validation("Background Responses require an authenticated owner")
        })?;
        if !already_stored {
            storage.save(&state.storage.database, &value).await?;
        }
        let record = storage
            .record()
            .ok_or_else(|| GatewayError::validation("Missing background response handle"))?;
        let provider = lifecycle::bound_provider(&state, record)?;
        Ok::<_, GatewayError>((value, provider))
    }
    .await;
    let (initial, native_provider) = match prepared {
        Ok(value) => value,
        Err(error) => {
            if durable_id.is_none() {
                lease
                    .settle_interrupted(
                        0,
                        None,
                        settle(
                            &state,
                            &context,
                            &call.provider,
                            &call.model,
                            call.pricing,
                            None,
                            None,
                            call.reservation,
                            call.key_reservation,
                            facts,
                        ),
                    )
                    .await;
            }
            call.callback.fail(error.to_string(), "background_error");
            // A local storage/guardrail failure is not an upstream health failure.
            lease.finish_success(0).await;
            return Err(error);
        }
    };
    let reply = initial.clone();
    tokio::spawn(async move {
        let NativeCall {
            callback,
            response: _,
            provider,
            model,
            deployment,
            pricing,
            file_search_calls: _,
            reservation,
            key_reservation,
            mut storage,
            started,
        } = call;
        // Settle before the shared budget backend's ten-minute lease expires.
        // Missing terminal usage uses the same conservative fallback as disconnects.
        let result = tokio::time::timeout_at(
            started + Duration::from_secs(540),
            poll(&native_provider, initial),
        )
        .await
        .unwrap_or_else(|_| {
            Err(GatewayError::timeout(
                "Background Responses usage polling timed out",
            ))
        });
        let usage = result.as_ref().ok().and_then(response_usage);
        let tokens_used = usage
            .as_ref()
            .map_or(0, |usage| u64::from(usage.total_tokens));
        let settlement = async {
            if let Some(id) = durable_id {
                if let Err(error) =
                    super::super::responses_settlement::submit_usage(&state, &id, usage.as_ref())
                        .await
                {
                    tracing::error!(%error, "Response settlement remains pending for recovery");
                }
            } else {
                settle(
                    &state,
                    &context,
                    &provider,
                    &model,
                    pricing,
                    usage.as_ref(),
                    usage
                        .as_ref()
                        .map(crate::core::pricing_service::PricingUsage::from),
                    reservation,
                    key_reservation,
                    facts,
                )
                .await;
            }
        };
        // Polling/guardrail/storage failures stay neutral for upstream health.
        if result
            .as_ref()
            .is_ok_and(|value| value.get("status").and_then(Value::as_str) == Some("completed"))
        {
            lease.settle_terminal(tokens_used, None, settlement).await;
        } else {
            lease
                .settle_interrupted(tokens_used, None, settlement)
                .await;
        }
        // GET/cancel/delete on another replica cannot cancel this settlement owner.
        lease
            .finish_success(
                usage
                    .as_ref()
                    .map_or(0, |usage| u64::from(usage.total_tokens)),
            )
            .await;
        match result {
            Ok(value) => {
                let failed = value.get("status").and_then(Value::as_str) == Some("failed");
                let sink = GuardrailDecisionSink::from_state(
                    &state,
                    Some(&model),
                    Some(&provider),
                    Some(&deployment),
                );
                match guardrails::apply_native_responses(
                    state.guardrails().as_ref(),
                    value,
                    true,
                    &sink,
                )
                .await
                {
                    Ok(value) => {
                        if let Some(storage) = storage.as_mut()
                            && let Err(error) = storage.save(&state.storage.database, &value).await
                        {
                            // Deletion may win the race; never resurrect its row or rebill.
                            tracing::warn!("Background response cache was not updated: {error}");
                        }
                        if failed {
                            callback.fail("Upstream background response failed", "provider_error");
                        } else {
                            callback.complete_usage(usage.as_ref(), "success");
                        }
                    }
                    Err(error) => {
                        callback.fail(error.to_string(), "guardrail_error");
                    }
                }
            }
            Err(error) => {
                callback.fail(error.to_string(), "background_error");
            }
        }
    });
    Ok(HttpResponse::Ok().json(reply))
}

async fn poll(provider: &Provider, mut value: Value) -> Result<Value, GatewayError> {
    let id = value
        .get("id")
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| GatewayError::validation("Background response is missing its ID"))?
        .to_string();
    let mut delay = 1;
    loop {
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if is_terminal(status) {
            return Ok(value);
        }
        if !matches!(status, "queued" | "in_progress") {
            return Err(GatewayError::validation(
                "Invalid background response status",
            ));
        }
        tokio::time::sleep(Duration::from_secs(delay)).await;
        delay = (delay * 2).min(10);
        let result = async {
            let mut response = provider
                .native_response_lifecycle(&id, HttpMethod::GET, None, "")
                .await?;
            lifecycle::read_json(&mut response).await
        }
        .await;
        match result {
            Ok(current) => {
                if current.get("id").and_then(Value::as_str) != Some(id.as_str()) {
                    return Err(GatewayError::validation(
                        "Background poll returned a different response ID",
                    ));
                }
                value = current;
            }
            // Polling failure does not mean generation failed. Retry within the
            // creation's bounded polling window, without replaying POST /responses.
            Err(error) => tracing::warn!("Background response usage poll failed: {error}"),
        }
    }
}
