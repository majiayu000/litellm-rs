//! Credentials owned by one provider construction and its clones.

use super::super::config::GITHUB_COPILOT_API_BASE;
use super::*;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

pub(super) struct CapturedCredentials {
    state: Mutex<CapturedState>,
    identity: [u8; 32],
    pub(super) access_token_gate: tokio::sync::Mutex<()>,
    pub(super) api_key_gate: tokio::sync::Mutex<()>,
}

impl std::fmt::Debug for CapturedCredentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapturedCredentials")
            .field("credentials", &"[redacted]")
            .finish_non_exhaustive()
    }
}

struct CapturedState {
    access_token: Option<String>,
    api_key: Option<ApiKeyInfo>,
    access_file: Option<Vec<u8>>,
    api_key_file: Option<Vec<u8>>,
    allow_device_flow: bool,
}

#[derive(Clone, Copy)]
pub(super) enum CredentialUpdate<'a> {
    AccessToken(&'a str),
    ApiKey(&'a ApiKeyInfo),
}

impl CopilotAuthenticator {
    /// Capture the paths and bytes actually consumed by a constructed provider.
    /// Later file changes require a new provider; refresh uses its bound access token.
    pub(crate) fn capture(config: &GitHubCopilotConfig) -> (Self, [u8; 32]) {
        let mut authenticator = Self::from_paths(config);
        if let Ok(directory) = std::env::current_dir() {
            authenticator.token_dir = directory.join(&authenticator.token_dir);
            authenticator.access_token_path = directory.join(&authenticator.access_token_path);
            authenticator.api_key_path = directory.join(&authenticator.api_key_path);
        }
        let access_file = fs::read(&authenticator.access_token_path).ok();
        let api_key_file = fs::read(&authenticator.api_key_path).ok();
        let access_token = access_file
            .as_deref()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_owned);
        let api_key: Option<ApiKeyInfo> = api_key_file
            .as_deref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok());

        let mut digest = Sha256::new();
        digest.update(b"github-copilot-credential-resource-v2");
        let directory = authenticator.token_dir.as_os_str().as_encoded_bytes();
        digest.update((directory.len() as u64).to_be_bytes());
        digest.update(directory);
        for path in [
            &authenticator.access_token_path,
            &authenticator.api_key_path,
        ] {
            let path = path.as_os_str().as_encoded_bytes();
            digest.update((path.len() as u64).to_be_bytes());
            digest.update(path);
        }
        let endpoint = api_key
            .as_ref()
            .and_then(|info| info.endpoints.api.as_deref())
            .or(config.api_base.as_deref())
            .unwrap_or(GITHUB_COPILOT_API_BASE);
        digest.update((endpoint.len() as u64).to_be_bytes());
        digest.update(endpoint.as_bytes());
        match (access_token.as_deref(), api_key.as_ref()) {
            (Some(token), _) => {
                // A short-lived API key refresh must retain this account's state.
                // Raw file bytes remain captured below for rotation-safe writeback.
                digest.update([1]);
                digest.update((token.len() as u64).to_be_bytes());
                digest.update(token.as_bytes());
            }
            (None, Some(info)) => {
                // With no refresh authority, the entire opaque API token is the
                // known credential resource; expiry and JSON formatting are not.
                digest.update([2]);
                digest.update((info.token.len() as u64).to_be_bytes());
                digest.update(info.token.as_bytes());
            }
            (None, None) => {
                // Independent unbound device flows must not inherit each other's
                // admission/circuit state before their account is known.
                digest.update([0]);
                digest.update(uuid::Uuid::new_v4().as_bytes());
            }
        }
        let identity = digest.finalize().into();
        let allow_device_flow = api_key.is_none();
        authenticator.captured = Some(Arc::new(CapturedCredentials {
            identity,
            state: Mutex::new(CapturedState {
                access_token,
                api_key,
                access_file,
                api_key_file,
                allow_device_flow,
            }),
            access_token_gate: tokio::sync::Mutex::new(()),
            api_key_gate: tokio::sync::Mutex::new(()),
        }));
        (authenticator, identity)
    }

    pub(crate) fn credential_resource_identity(&self) -> [u8; 32] {
        self.captured
            .as_ref()
            .expect("public constructors capture their credential resource")
            .identity
    }

    pub(super) fn cached_access_token(&self) -> Option<String> {
        if let Some(captured) = &self.captured {
            return captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .access_token
                .clone();
        }
        fs::read_to_string(&self.access_token_path)
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
    }

    pub(super) fn cached_api_key(&self) -> Option<ApiKeyInfo> {
        if let Some(captured) = &self.captured {
            return captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .api_key
                .clone();
        }
        fs::read_to_string(&self.api_key_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
    }

    pub(super) fn device_flow_allowed(&self) -> bool {
        self.captured.as_ref().is_none_or(|captured| {
            captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .allow_device_flow
        })
    }

    pub(crate) fn invalidate_api_key(&self) {
        if let Some(captured) = &self.captured {
            captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .api_key = None;
        }
    }

    pub(super) fn cache_credential(
        &self,
        update: CredentialUpdate<'_>,
    ) -> Result<(), GitHubCopilotError> {
        let (path, contents, label) = match update {
            CredentialUpdate::AccessToken(token) => (
                &self.access_token_path,
                token.as_bytes().to_vec(),
                "access token",
            ),
            CredentialUpdate::ApiKey(info) => {
                let Ok(contents) = serde_json::to_vec(info) else {
                    return Ok(());
                };
                (&self.api_key_path, contents, "API key")
            }
        };
        if let Some(captured) = &self.captured {
            let mut state = captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match update {
                CredentialUpdate::AccessToken(token) => state.access_token = Some(token.to_owned()),
                CredentialUpdate::ApiKey(info) => state.api_key = Some(info.clone()),
            }
        }
        self.ensure_token_dir()?;
        if let Err(error) = self.persist_credential(path, &contents) {
            warn!("Failed to cache {label}: {error}");
        }
        Ok(())
    }

    /// Update only files still owned by this captured resource. A read error is
    /// not evidence of absence, and must never authorize overwriting the cache.
    pub(super) fn persist_credential(
        &self,
        path: &std::path::Path,
        contents: &[u8],
    ) -> std::io::Result<()> {
        let mut captured = self.captured.as_ref().map(|captured| {
            captured
                .state
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        });
        if let Some(state) = captured.as_ref() {
            for (captured_path, expected) in [
                (&self.access_token_path, &state.access_file),
                (&self.api_key_path, &state.api_key_file),
            ] {
                let changed = match fs::read(captured_path) {
                    Ok(bytes) => expected.as_ref() != Some(&bytes),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        expected.is_some()
                    }
                    Err(error) => return Err(error),
                };
                if changed {
                    // This is a guarded writeback, not an inter-process atomic write.
                    debug!(
                        "Copilot credential files changed; keeping refreshed credentials in memory"
                    );
                    return Ok(());
                }
            }
        }
        fs::write(path, contents)?;
        if let Some(state) = captured.as_mut() {
            if path == self.access_token_path {
                state.access_file = Some(contents.to_vec());
            }
            if path == self.api_key_path {
                state.api_key_file = Some(contents.to_vec());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "credential_snapshot_tests.rs"]
mod tests;
