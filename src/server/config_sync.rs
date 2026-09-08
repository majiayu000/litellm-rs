//! Authoritative encrypted runtime snapshots. Redis carries only revision IDs.
use super::{runtime::build_runtime_revision, state::AppState};
use crate::config::{
    Config,
    models::{cache::CacheConfig, provider::ProviderConfig, router::GatewayRouterConfig},
};
use crate::core::guardrails::GuardrailConfig;
use crate::utils::{
    auth::crypto::encryption::{decrypt_data, encrypt_data},
    error::gateway_error::{GatewayError, Result},
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc, time::Duration};

pub(crate) const REVISION_CHANNEL: &str = "litellm-rs:config:revisions";
const RECONCILE_INTERVAL: Duration = Duration::from_secs(1);

/// Public diagnostics contain no configuration values or backend error bodies.
#[derive(Clone, Default, Serialize)]
pub struct ConfigSyncStatus {
    /// Revision currently serving requests on this node.
    pub active_revision: u64,
    /// Most recent authoritative revision read by this node.
    pub observed_revision: u64,
    /// Failed apply stage, kept until an authoritative snapshot applies successfully.
    pub last_apply_error: Option<String>,
    /// Failed storage/notification stage, cleared on successful synchronization.
    pub last_sync_error: Option<String>,
}

pub(crate) struct ConfigSync {
    key: Vec<u8>,
    pub(crate) status: parking_lot::RwLock<ConfigSyncStatus>,
}

// Node-local listener/auth/storage/telemetry settings are deliberately excluded.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRuntime {
    revision: u64,
    providers: Vec<ProviderConfig>,
    router: GatewayRouterConfig,
    model_aliases: HashMap<String, String>,
    guardrails: GuardrailConfig,
    cache: CacheConfig,
}

impl ConfigSync {
    pub(crate) fn from_config(config: &Config) -> Result<Option<Arc<Self>>> {
        let Some(env_name) = &config.gateway.storage.config_sync_key_env else {
            return Ok(None);
        };
        let key = std::env::var(env_name).map_err(|_| {
            GatewayError::Config(
                "Configuration sync encryption key environment variable is missing".into(),
            )
        })?;
        if key.len() < 32 {
            return Err(GatewayError::Config(
                "Configuration sync requires a dedicated encryption key of at least 32 bytes"
                    .into(),
            ));
        }
        Ok(Some(Arc::new(Self {
            key: key.into_bytes(),
            status: Default::default(),
        })))
    }

    pub(crate) fn encrypt(&self, config: &Config, revision: u64) -> Result<String> {
        let payload = StoredRuntime {
            revision,
            providers: config.gateway.providers.clone(),
            router: config.gateway.router.clone(),
            model_aliases: config.gateway.model_aliases.clone(),
            guardrails: config.gateway.guardrails.clone(),
            cache: config.gateway.cache.clone(),
        };
        let json = serde_json::to_string(&payload)
            .map_err(|_| GatewayError::Config("Runtime snapshot serialization failed".into()))?;
        encrypt_data(&self.key, &json)
    }

    fn candidate(&self, base: &Config, revision: u64, ciphertext: &str) -> Result<Config> {
        let json = decrypt_data(&self.key, ciphertext).map_err(|_| {
            GatewayError::Config(
                "Runtime snapshot decryption failed; check the shared encryption key".into(),
            )
        })?;
        let payload: StoredRuntime = serde_json::from_str(&json)
            .map_err(|_| GatewayError::Config("Runtime snapshot decoding failed".into()))?;
        if payload.revision != revision {
            return Err(GatewayError::Config(
                "Runtime snapshot revision does not match its authenticated payload".into(),
            ));
        }
        let mut candidate = base.clone();
        candidate.gateway.providers = payload.providers;
        candidate.gateway.router = payload.router;
        candidate.gateway.model_aliases = payload.model_aliases;
        candidate.gateway.guardrails = payload.guardrails;
        candidate.gateway.cache = payload.cache;
        Ok(candidate)
    }
}

impl AppState {
    /// Current node's revision and sanitized synchronization diagnostics.
    pub fn config_sync_status(&self) -> ConfigSyncStatus {
        let mut status = self
            .config_sync
            .as_ref()
            .map(|sync| sync.status.read().clone())
            .unwrap_or_default();
        status.active_revision = self.pin_runtime().generation;
        status
    }

    pub(crate) async fn reconcile_runtime_config(&self) -> Result<()> {
        let Some(sync) = &self.config_sync else {
            return Ok(());
        };
        let _guard = self.apply_lock.lock().await;
        let row = self.storage.database.runtime_config().await.map_err(|_| {
            sync.status.write().last_sync_error =
                Some("Authoritative configuration read failed".into());
            GatewayError::Storage("Authoritative configuration read failed".into())
        })?;
        let revision = u64::try_from(row.revision).map_err(|_| {
            GatewayError::Storage("Invalid authoritative configuration revision".into())
        })?;
        sync.status.write().observed_revision = revision;
        if revision <= self.pin_runtime().generation {
            return Ok(());
        }
        let apply = async {
            let ciphertext = row.encrypted_payload.as_deref().ok_or_else(|| {
                GatewayError::Storage("Authoritative configuration payload is missing".into())
            })?;
            let candidate = sync.candidate(&self.config(), revision, ciphertext)?;
            let built = build_runtime_revision(
                candidate,
                revision,
                self.pricing.clone(),
                self.storage.redis.clone(),
            )
            .await
            .map_err(|_| {
                GatewayError::Config("Authoritative runtime validation/build failed".into())
            })?;
            self.publish_runtime(built);
            Ok::<_, GatewayError>(())
        }
        .await;
        let mut status = sync.status.write();
        match apply {
            Ok(()) => {
                status.last_apply_error = None;
                Ok(())
            }
            Err(error) => {
                status.last_apply_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    async fn observe_revision(&self, message: redis::Msg) -> Result<()> {
        let revision = message.get_payload::<u64>().map_err(|_| {
            GatewayError::Storage("Invalid configuration revision notification".into())
        })?;
        if revision > self.pin_runtime().generation {
            self.reconcile_runtime_config().await?;
        }
        Ok(())
    }
}

/// Owns the subscriber so dropping/stopping a server also stops its worker.
pub(super) struct ConfigSyncTask(tokio::task::JoinHandle<()>);
impl Drop for ConfigSyncTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(super) fn start(state: AppState) -> Option<ConfigSyncTask> {
    let sync = state.config_sync.clone()?;
    Some(ConfigSyncTask(tokio::spawn(async move {
        loop {
            let result: Result<()> = async {
                let mut subscription = state
                    .storage
                    .redis
                    .subscribe(&[REVISION_CHANNEL.to_string()])
                    .await?;
                // Subscribe before loading, closing the reconnect read/subscribe race.
                state.reconcile_runtime_config().await?;
                loop {
                    match tokio::time::timeout(RECONCILE_INTERVAL, subscription.next_message())
                        .await
                    {
                        Ok(Ok(message)) => state.observe_revision(message).await?,
                        Ok(Err(error)) => return Err(error),
                        Err(_) => state.reconcile_runtime_config().await?,
                    }
                    sync.status.write().last_sync_error = None;
                }
            }
            .await;
            if let Err(error) = result {
                sync.status.write().last_sync_error = Some(error.to_string());
                tracing::error!(error = %error, "Configuration synchronization failed");
            }
            tokio::time::sleep(RECONCILE_INTERVAL).await;
        }
    })))
}

#[cfg(all(test, feature = "sqlite"))]
#[path = "config_sync_tests.rs"]
mod tests;
