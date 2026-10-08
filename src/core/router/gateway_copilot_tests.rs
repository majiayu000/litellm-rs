//! Exercise native credential identity through the actual gateway factory.

use super::gateway_config_tests::EnvScope;
use super::*;
use std::path::Path;

const MODEL: &str = "gpt-4o";
const DEPLOYMENT: &str = "copilot-fixture-gpt-4o";
const ACCESS: &str = "copilot-access-fixture-secret";
const API_TOKEN: &str = "copilot-api-fixture-secret";
const ROTATED_ACCESS: &str = "copilot-rotated-access-fixture-secret";
const ROTATED_API_TOKEN: &str = "copilot-rotated-api-fixture-secret";

fn provider_config() -> ProviderConfig {
    ProviderConfig {
        name: "copilot-fixture".into(),
        provider_type: "github_copilot".into(),
        models: vec![MODEL.into()],
        ..ProviderConfig::default()
    }
}

fn write_api_key(directory: &Path, name: &str, token: &str, endpoint: &str) {
    std::fs::write(
        directory.join(name),
        serde_json::to_vec(&serde_json::json!({
            "token": token,
            "expires_at": 4_000_000_000_u64,
            "endpoints": {"api": endpoint}
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write_credentials(directory: &Path) {
    std::fs::create_dir_all(directory).unwrap();
    for name in ["access-a", "access-b"] {
        std::fs::write(directory.join(name), ACCESS).unwrap();
    }
    for name in ["api-a.json", "api-b.json"] {
        write_api_key(
            directory,
            name,
            API_TOKEN,
            "https://copilot-fixture.invalid/a",
        );
    }
}

async fn construct(
    config: &ProviderConfig,
    directory: &Path,
    access_file: &str,
    api_file: &str,
) -> Router {
    let _environment = EnvScope::new(&[
        ("GITHUB_COPILOT_TOKEN_DIR", directory.to_str().unwrap()),
        ("GITHUB_COPILOT_ACCESS_TOKEN_FILE", access_file),
        ("GITHUB_COPILOT_API_KEY_FILE", api_file),
    ]);
    // Factory construction and local lease selection must never authenticate or probe.
    Router::from_gateway_config(
        std::slice::from_ref(config),
        Some(RouterConfig {
            enable_pre_call_checks: false,
            ..RouterConfig::default()
        }),
    )
    .await
    .unwrap()
}

fn identity(router: &Router) -> GatewayRuntimeIdentity {
    router
        .get_deployment(DEPLOYMENT)
        .unwrap()
        .state
        .runtime_identity
        .clone()
        .unwrap()
}

fn assert_credentials_private(router: &Router) {
    let deployment = router.get_deployment(DEPLOYMENT).unwrap();
    let debug = format!("{deployment:?} {:?}", identity(router));
    let snapshot = router.load_routing_snapshot();
    for secret in [ACCESS, API_TOKEN, ROTATED_ACCESS, ROTATED_API_TOKEN] {
        assert!(
            !debug.contains(secret),
            "native credentials must be redacted"
        );
        assert!(
            snapshot.resolve_legacy_credential(MODEL, secret).is_err(),
            "native file credentials must not become legacy API-key selectors"
        );
    }
}

#[tokio::test]
async fn copilot_gateway_identity_tracks_paths_refresh_authority_and_endpoint() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    write_credentials(&first);
    write_credentials(&second);
    let config = provider_config();
    let original = construct(&config, &first, "access-a", "api-a.json").await;
    let original_identity = identity(&original);
    let same = construct(&config, &first, "access-a", "api-a.json").await;
    assert_eq!(original_identity, identity(&same));
    assert_credentials_private(&original);
    assert_credentials_private(&same);

    for change in [
        "token directory",
        "access filename",
        "API filename",
        "access bytes",
        "API endpoint",
    ] {
        write_credentials(&first);
        let mut selected_directory = first.as_path();
        let mut access_file = "access-a";
        let mut api_file = "api-a.json";
        match change {
            "token directory" => selected_directory = &second,
            "access filename" => access_file = "access-b",
            "API filename" => api_file = "api-b.json",
            "access bytes" => {
                std::fs::write(first.join(access_file), ROTATED_ACCESS).unwrap();
            }
            "API endpoint" => write_api_key(
                &first,
                api_file,
                API_TOKEN,
                "https://copilot-fixture.invalid/b",
            ),
            _ => unreachable!(),
        }
        let rotated = construct(&config, selected_directory, access_file, api_file).await;
        assert_ne!(original_identity, identity(&rotated), "{change}");
        assert_eq!(
            original_identity,
            identity(&original),
            "old snapshot: {change}"
        );
        assert_credentials_private(&rotated);
    }
}

#[tokio::test]
async fn copilot_gateway_explicit_paths_override_all_three_environment_paths() {
    let directory = tempfile::tempdir().unwrap();
    let configured = directory.path().join("configured");
    let environment_a = directory.path().join("environment-a");
    let environment_b = directory.path().join("environment-b");
    for path in [&configured, &environment_a, &environment_b] {
        write_credentials(path);
    }
    std::fs::write(environment_b.join("access-b"), ROTATED_ACCESS).unwrap();
    write_api_key(
        &environment_b,
        "api-b.json",
        ROTATED_API_TOKEN,
        "https://copilot-fixture.invalid/rotated",
    );
    let mut config = provider_config();
    config.settings = serde_json::from_value(serde_json::json!({
        "token_dir": configured.to_str().unwrap(),
        "access_token_file": "access-a",
        "api_key_file": "api-a.json"
    }))
    .unwrap();
    let first = construct(&config, &environment_a, "access-a", "api-a.json").await;
    let second = construct(&config, &environment_b, "access-b", "api-b.json").await;
    assert_eq!(identity(&first), identity(&second));
    assert_credentials_private(&first);
    assert_credentials_private(&second);
}

#[cfg(feature = "gateway")]
#[tokio::test]
async fn copilot_gateway_rotation_isolates_registry_usage_cooldown_and_live_leases() {
    use crate::core::router::runtime_state::RuntimeStateRegistry;
    use crate::core::types::model::ProviderCapability;
    use std::sync::atomic::Ordering;

    let directory = tempfile::tempdir().unwrap();
    write_credentials(directory.path());
    let config = provider_config();
    let original = construct(&config, directory.path(), "access-a", "api-a.json").await;
    let old_snapshot = original.load_routing_snapshot();
    let old = original.get_deployment(DEPLOYMENT).unwrap();
    let old_lease = original
        .select_deployment_lease_for_capability_matching_async(
            MODEL,
            &ProviderCapability::ChatCompletion,
            |_| true,
        )
        .await
        .unwrap();
    old.record_success(17, 1);
    old.enter_cooldown(600);
    let cooldown = old.state.cooldown_until.load(Ordering::Acquire);
    let mut registry = RuntimeStateRegistry::default();
    registry.remember(&old_snapshot);

    let same = construct(&config, directory.path(), "access-a", "api-a.json").await;
    let same_deployment = same.get_deployment(DEPLOYMENT).unwrap();
    let retained = registry.find(DEPLOYMENT, &same_deployment.state).unwrap();
    assert!(std::ptr::eq(&retained.tpm_current, &old.state.tpm_current));
    assert_eq!(retained.tpm_current.load(Ordering::Acquire), 17);
    assert_eq!(retained.active_requests.load(Ordering::Acquire), 1);
    assert_eq!(retained.cooldown_until.load(Ordering::Acquire), cooldown);
    assert_eq!(old.shared_state_id(), same_deployment.shared_state_id());

    for change in [
        "API token",
        "expiry",
        "JSON formatting",
        "access whitespace",
    ] {
        write_credentials(directory.path());
        let mut refreshed = serde_json::json!({
            "token": API_TOKEN,
            "expires_at": 4_000_000_000_u64,
            "endpoints": {"api": "https://copilot-fixture.invalid/a"}
        });
        match change {
            "API token" => refreshed["token"] = ROTATED_API_TOKEN.into(),
            "expiry" => refreshed["expires_at"] = 4_000_000_001_u64.into(),
            "JSON formatting" => {}
            "access whitespace" => {
                std::fs::write(
                    directory.path().join("access-a"),
                    format!(" \t{ACCESS}\r\n"),
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        let bytes = if change == "JSON formatting" {
            serde_json::to_vec_pretty(&refreshed).unwrap()
        } else {
            serde_json::to_vec(&refreshed).unwrap()
        };
        std::fs::write(directory.path().join("api-a.json"), bytes).unwrap();
        let reloaded = construct(&config, directory.path(), "access-a", "api-a.json").await;
        let deployment = reloaded.get_deployment(DEPLOYMENT).unwrap();
        assert_eq!(identity(&original), identity(&reloaded), "{change}");
        assert_eq!(
            old.shared_state_id(),
            deployment.shared_state_id(),
            "{change}"
        );
        let retained = registry.find(DEPLOYMENT, &deployment.state).unwrap();
        assert!(std::ptr::eq(&retained.tpm_current, &old.state.tpm_current));
        assert_eq!(retained.rpm_current.load(Ordering::Acquire), 1, "{change}");
        assert_eq!(retained.tpm_current.load(Ordering::Acquire), 17, "{change}");
        assert_eq!(
            retained.active_requests.load(Ordering::Acquire),
            1,
            "{change}"
        );
        assert_eq!(
            retained.cooldown_until.load(Ordering::Acquire),
            cooldown,
            "{change}"
        );
        assert_credentials_private(&reloaded);
    }

    std::fs::write(directory.path().join("access-a"), ROTATED_ACCESS).unwrap();
    let rotated = construct(&config, directory.path(), "access-a", "api-a.json").await;
    let new = rotated.get_deployment(DEPLOYMENT).unwrap();
    assert!(registry.find(DEPLOYMENT, &new.state).is_none());
    assert_ne!(old.shared_state_id(), new.shared_state_id());
    assert_eq!(new.state.tpm_current.load(Ordering::Acquire), 0);
    assert_eq!(new.state.cooldown_until.load(Ordering::Acquire), 0);
    registry.remember(&rotated.load_routing_snapshot());
    registry.prune_retired();
    let new_lease = rotated
        .select_deployment_lease_for_capability_matching_async(
            MODEL,
            &ProviderCapability::ChatCompletion,
            |_| true,
        )
        .await
        .unwrap();
    assert_eq!(old.state.active_requests.load(Ordering::Acquire), 1);
    assert_eq!(new.state.active_requests.load(Ordering::Acquire), 1);
    drop(new_lease);
    assert_eq!(new.state.active_requests.load(Ordering::Acquire), 0);
    assert_eq!(old.state.active_requests.load(Ordering::Acquire), 1);
    assert_eq!(old.state.tpm_current.load(Ordering::Acquire), 17);
    assert_eq!(old.state.cooldown_until.load(Ordering::Acquire), cooldown);
    assert!(registry.find(DEPLOYMENT, &old.state).is_some());
    assert!(registry.find(DEPLOYMENT, &new.state).is_some());
    assert_eq!(
        old_lease.deployment().shared_state_id(),
        old.shared_state_id()
    );
    drop(old_lease);
    assert_eq!(old.state.active_requests.load(Ordering::Acquire), 0);
    assert_eq!(new.state.tpm_current.load(Ordering::Acquire), 0);
    assert_credentials_private(&original);
    assert_credentials_private(&rotated);
}
