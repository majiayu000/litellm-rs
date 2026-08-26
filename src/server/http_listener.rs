use crate::config::models::server::ServerConfig;
use crate::server::http::HttpServer;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_web::{HttpServer as ActixHttpServer, web};
use std::net::ToSocketAddrs;
use validator::Validate;

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ListenerSettings {
    pub(super) configured_workers: usize,
    pub(super) effective_workers: usize,
    pub(super) first_request_head_timeout: std::time::Duration,
    pub(super) max_connections_per_worker: Option<usize>,
}

pub(super) fn validated_listener_settings(config: &ServerConfig) -> Result<ListenerSettings> {
    Validate::validate(config)
        .map_err(|error| GatewayError::Config(format!("Invalid server configuration: {error}")))?;

    let configured_workers = config.worker_count();
    let first_request_head_timeout = std::time::Duration::from_secs(config.timeout);
    let Some(total_connections) = config.max_connections else {
        return Ok(ListenerSettings {
            configured_workers,
            effective_workers: configured_workers,
            first_request_head_timeout,
            max_connections_per_worker: None,
        });
    };

    // Actix's max_connections setting is per worker, and actix-server 2.6
    // cannot re-enable a worker after a limit of 1 is released. Keep each
    // worker at 2 or more connections and round down so the effective
    // server-wide capacity never exceeds the configured total.
    let workers = configured_workers.min(total_connections / 2).max(1);
    Ok(ListenerSettings {
        configured_workers,
        effective_workers: workers,
        first_request_head_timeout,
        max_connections_per_worker: Some(total_connections / workers),
    })
}

pub(super) fn build_actix_server(
    state: web::Data<AppState>,
    settings: &ListenerSettings,
    addresses: impl ToSocketAddrs,
) -> std::io::Result<(actix_web::dev::Server, std::net::SocketAddr)> {
    let mut last_error = None;
    for address in addresses.to_socket_addrs()? {
        let app_state = state.clone();
        let mut builder = ActixHttpServer::new(move || HttpServer::create_app(app_state.clone()))
            .workers(settings.effective_workers)
            .client_request_timeout(settings.first_request_head_timeout);
        if let Some(per_worker) = settings.max_connections_per_worker {
            builder = builder.max_connections(per_worker);
        }
        match builder.bind(address) {
            Ok(builder) => {
                let bound_addresses = builder.addrs();
                let [selected_address] = bound_addresses.as_slice() else {
                    return Err(std::io::Error::other(
                        "Actix must bind exactly one resolved address",
                    ));
                };
                return Ok((builder.run(), *selected_address));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("Could not bind to address")))
}
