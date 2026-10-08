use crate::core::providers::create_provider;
use crate::core::providers::factory::CONSTRUCTION_ENV_LOCK as ENV_LOCK;
use std::sync::MutexGuard;
#[rustfmt::skip]
const ENVS: &[&str] = &[
    "MIMO_API_KEY", "XIAOMI_API_KEY", "CLOUDFLARE_API_TOKEN",
    "REPLICATE_API_TOKEN", "REPLICATE_API_KEY", "FAL_AI_API_KEY",
    "COHERE_API_KEY", "GEMINI_API_KEY", "GOOGLE_API_KEY",
    "GITHUB_TOKEN", "AI21_API_KEY", "HF_TOKEN",
    "BASETEN_API_KEY", "HEROKU_API_KEY", "INFERENCE_KEY", "EMBEDDING_KEY",
    "OVHCLOUD_API_KEY", "OVH_AI_ENDPOINTS_ACCESS_TOKEN",
    "DEEPGRAM_API_KEY", "DEEPGRAM_API_BASE",
    "ELEVENLABS_API_KEY", "ELEVENLABS_API_BASE",
    "AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN",
    "AWS_REGION", "AWS_DEFAULT_REGION",
    "GOOGLE_CLOUD_PROJECT", "GOOGLE_PROJECT_ID", "GCP_PROJECT", "GCLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION", "VERTEX_AI_LOCATION", "GOOGLE_APPLICATION_CREDENTIALS",
    "GITHUB_COPILOT_TOKEN_DIR", "GITHUB_COPILOT_ACCESS_TOKEN_FILE", "GITHUB_COPILOT_API_KEY_FILE",
];
pub(super) struct EnvScope {
    previous: Vec<(&'static str, Option<String>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvScope {
    pub(super) fn new(values: &[(&str, &str)]) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let previous = ENVS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect();
        for key in ENVS {
            unsafe { std::env::remove_var(key) };
        }
        for &(key, value) in values {
            unsafe { std::env::set_var(key, value) };
        }
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvScope {
    fn drop(&mut self) {
        for (key, value) in self.previous.drain(..).rev() {
            match value {
                Some(value) => unsafe { std::env::set_var(key, value) },
                None => unsafe { std::env::remove_var(key) },
            }
        }
    }
}

#[cfg(all(feature = "providers-extended", feature = "gateway"))]
#[tokio::test]
async fn copilot_captured_credentials_isolate_reloaded_runtime_state() {
    use super::*;
    use std::sync::atomic::Ordering;
    let first = tempfile::tempdir().unwrap();
    let second = tempfile::tempdir().unwrap();
    let write = |directory: &std::path::Path, access: &str, key: &str| {
        std::fs::write(directory.join("access-token"), access).unwrap();
        std::fs::write(
            directory.join("api-key.json"),
            serde_json::json!({
                "token": key, "expires_at": u64::MAX,
                "endpoints": {"api": "https://copilot.fixture.invalid"}
            })
            .to_string(),
        )
        .unwrap();
    };
    write(first.path(), "fixture-access-a", "fixture-key-a");
    write(second.path(), "fixture-access-a", "fixture-key-a");
    let config = ProviderConfig {
        name: "copilot-fixture".into(),
        provider_type: "github_copilot".into(),
        models: vec!["gpt-4o".into()],
        ..Default::default()
    };
    let _env = EnvScope::new(&[
        ("GITHUB_COPILOT_TOKEN_DIR", first.path().to_str().unwrap()),
        ("GITHUB_COPILOT_ACCESS_TOKEN_FILE", "access-token"),
        ("GITHUB_COPILOT_API_KEY_FILE", "api-key.json"),
    ]);
    let original = Router::from_gateway_config(std::slice::from_ref(&config), None)
        .await
        .unwrap();
    let id = "copilot-fixture-gpt-4o";
    let original_deployment = original.get_deployment(id).unwrap();
    original_deployment.record_success(3, 1);
    let replica = Router::from_gateway_config(std::slice::from_ref(&config), None)
        .await
        .unwrap();
    replica.inherit_runtime_state(&original);
    assert_eq!(
        replica
            .get_deployment(id)
            .unwrap()
            .state
            .total_requests
            .load(Ordering::Relaxed),
        1
    );
    assert_eq!(
        replica.get_deployment(id).unwrap().state.runtime_identity,
        original_deployment.state.runtime_identity
    );

    // A changed effective path isolates even equal captured tokens; same-path
    // access/endpoint changes also isolate the reconstructed resource. An API
    // token refresh with the same access authority and endpoint retains state.
    for variant in [
        "directory",
        "access-file",
        "key-file",
        "access-content",
        "endpoint",
        "key-content",
    ] {
        unsafe {
            std::env::set_var("GITHUB_COPILOT_TOKEN_DIR", first.path());
        }
        unsafe {
            std::env::set_var("GITHUB_COPILOT_ACCESS_TOKEN_FILE", "access-token");
        }
        unsafe {
            std::env::set_var("GITHUB_COPILOT_API_KEY_FILE", "api-key.json");
        }
        write(first.path(), "fixture-access-a", "fixture-key-a");
        match variant {
            "directory" => unsafe {
                std::env::set_var("GITHUB_COPILOT_TOKEN_DIR", second.path());
            },
            "access-file" => {
                std::fs::copy(
                    first.path().join("access-token"),
                    first.path().join("alternate-access"),
                )
                .unwrap();
                unsafe {
                    std::env::set_var("GITHUB_COPILOT_ACCESS_TOKEN_FILE", "alternate-access");
                }
            }
            "key-file" => {
                std::fs::copy(
                    first.path().join("api-key.json"),
                    first.path().join("alternate-key.json"),
                )
                .unwrap();
                unsafe {
                    std::env::set_var("GITHUB_COPILOT_API_KEY_FILE", "alternate-key.json");
                }
            }
            "access-content" => write(first.path(), "fixture-access-b", "fixture-key-a"),
            "endpoint" => {
                let path = first.path().join("api-key.json");
                let mut key: serde_json::Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                key["endpoints"]["api"] = serde_json::json!("https://rotated.fixture.invalid");
                std::fs::write(path, serde_json::to_vec(&key).unwrap()).unwrap();
            }
            "key-content" => write(first.path(), "fixture-access-a", "fixture-key-b"),
            _ => unreachable!(),
        }
        let replacement = Router::from_gateway_config(std::slice::from_ref(&config), None)
            .await
            .unwrap();
        replacement.inherit_runtime_state(&original);
        let replaced = replacement.get_deployment(id).unwrap();
        if variant == "key-content" {
            assert_eq!(
                replaced.state.runtime_identity, original_deployment.state.runtime_identity,
                "API refresh must retain the same access authority"
            );
            assert_eq!(replaced.state.total_requests.load(Ordering::Relaxed), 1);
        } else {
            assert_ne!(
                replaced.state.runtime_identity, original_deployment.state.runtime_identity,
                "{variant}"
            );
            assert_eq!(
                replaced.state.total_requests.load(Ordering::Relaxed),
                0,
                "{variant}"
            );
        }
        assert_eq!(
            original_deployment
                .state
                .total_requests
                .load(Ordering::Relaxed),
            1
        );
    }

    // Existing explicit settings override environment fallbacks, including
    // file names. Changing unused environment credentials must preserve state.
    write(first.path(), "fixture-access-a", "fixture-key-a");
    let mut explicit = config.clone();
    explicit.settings = serde_json::from_value(serde_json::json!({
        "token_dir": first.path().to_str().unwrap(),
        "access_token_file": "access-token", "api_key_file": "api-key.json"
    }))
    .unwrap();
    let before = Router::from_gateway_config(std::slice::from_ref(&explicit), None)
        .await
        .unwrap();
    before.get_deployment(id).unwrap().record_success(3, 1);
    unsafe {
        std::env::set_var("GITHUB_COPILOT_TOKEN_DIR", second.path());
    }
    unsafe {
        std::env::set_var("GITHUB_COPILOT_ACCESS_TOKEN_FILE", "unused-access");
    }
    unsafe {
        std::env::set_var("GITHUB_COPILOT_API_KEY_FILE", "unused-api-key");
    }
    let after = Router::from_gateway_config(std::slice::from_ref(&explicit), None)
        .await
        .unwrap();
    after.inherit_runtime_state(&before);
    assert_eq!(
        after.get_deployment(id).unwrap().state.runtime_identity,
        before.get_deployment(id).unwrap().state.runtime_identity
    );
    assert_eq!(
        after
            .get_deployment(id)
            .unwrap()
            .state
            .total_requests
            .load(Ordering::Relaxed),
        1
    );
}

#[tokio::test]
async fn native_audio_environment_rotations_change_runtime_resource_identity() {
    use super::*;

    for (selector, key_env, base_env, base) in [
        (
            "deepgram",
            "DEEPGRAM_API_KEY",
            "DEEPGRAM_API_BASE",
            "https://api.deepgram.com",
        ),
        (
            "elevenlabs",
            "ELEVENLABS_API_KEY",
            "ELEVENLABS_API_BASE",
            "https://api.elevenlabs.io",
        ),
    ] {
        let config = ProviderConfig {
            name: selector.to_string(),
            provider_type: selector.to_string(),
            api_key: String::new(),
            models: vec!["fixture-model".to_string()],
            ..ProviderConfig::default()
        };
        let mut identities = Vec::new();
        for (key, suffix) in [
            ("audio-fixture-key-a", "a"),
            ("audio-fixture-key-b", "a"),
            ("audio-fixture-key-b", "b"),
        ] {
            let endpoint = format!("{base}/fixture-{suffix}");
            let _env = EnvScope::new(&[(key_env, key), (base_env, &endpoint)]);
            let normalized = normalize_provider_construction(&config);
            assert_eq!(normalized.config.api_key, key);
            assert_eq!(
                normalized.config.base_url.as_deref(),
                Some(endpoint.as_str())
            );
            let router = Router::from_gateway_config(std::slice::from_ref(&config), None)
                .await
                .unwrap();
            let deployment = router
                .get_deployment(&format!("{selector}-fixture-model"))
                .unwrap();
            identities.push(deployment.state.runtime_identity.clone().unwrap());
            assert!(
                router
                    .load_routing_snapshot()
                    .resolve_legacy_credential("fixture-model", key)
                    .is_ok()
            );

            for blank_key in ["", " "] {
                let mut blank = config.clone();
                blank
                    .settings
                    .insert("api_key".to_string(), blank_key.into());
                let result = Router::from_gateway_config(&[blank], None).await;
                assert!(
                    matches!(result, Err(RouterError::DeploymentNotFound(message)) if message.contains("API key is required")),
                    "explicit blank settings must preserve the provider error"
                );
            }

            let mut null_endpoint = config.clone();
            null_endpoint
                .settings
                .insert("base_url".to_string(), serde_json::Value::Null);
            assert_eq!(
                normalize_provider_construction(&null_endpoint)
                    .config
                    .base_url
                    .as_deref(),
                Some(endpoint.as_str())
            );

            let mut explicit = config.clone();
            explicit.api_key = "explicit-audio-fixture".to_string();
            explicit
                .settings
                .insert("api_base".to_string(), format!("{base}/explicit").into());
            let normalized = normalize_provider_construction(&explicit);
            assert_eq!(normalized.config.api_key, explicit.api_key);
            assert!(normalized.config.base_url.is_none());
            assert_eq!(
                normalized.config.settings["api_base"],
                explicit.settings["api_base"]
            );
        }
        assert_ne!(
            identities[0], identities[1],
            "credential rotation must retire the prior resource"
        );
        assert_ne!(
            identities[1], identities[2],
            "endpoint rotation must retire the prior resource"
        );
    }
}

#[cfg(feature = "providers-extra")]
#[tokio::test]
async fn vertex_environment_rotations_change_runtime_resource_identity() {
    use super::*;
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("fixture-a.json");
    let second = directory.path().join("fixture-b.json");
    let contents = r#"{"type":"authorized_user","client_id":"fixture-client","client_secret":"fixture-secret","refresh_token":"fixture-refresh"}"#;
    std::fs::write(&first, contents).unwrap();
    std::fs::write(&second, contents).unwrap();
    let config = ProviderConfig {
        name: "vertex-fixture".to_string(),
        provider_type: "vertex_ai".to_string(),
        models: vec!["gemini-2.5-flash".to_string()],
        ..ProviderConfig::default()
    };
    let mut identities = Vec::new();
    for (project, location, file) in [
        ("fixture-project-a", "us-central1", &first),
        ("fixture-project-b", "us-central1", &first),
        ("fixture-project-b", "europe-west1", &first),
        ("fixture-project-b", "europe-west1", &second),
    ] {
        let file = file.to_str().unwrap();
        let _env = EnvScope::new(&[
            ("GOOGLE_CLOUD_PROJECT", project),
            ("GOOGLE_CLOUD_LOCATION", location),
            ("GOOGLE_APPLICATION_CREDENTIALS", file),
        ]);
        let normalized = normalize_provider_construction(&config);
        assert_eq!(normalized.config.settings["project_id"], project);
        assert_eq!(normalized.config.settings["location"], location);
        assert_eq!(normalized.config.settings["credentials_file"], file);
        let router = Router::from_gateway_config(std::slice::from_ref(&config), None)
            .await
            .unwrap();
        identities.push(
            router
                .get_deployment("vertex-fixture-gemini-2.5-flash")
                .unwrap()
                .state
                .runtime_identity
                .clone()
                .unwrap(),
        );
        let mut explicit = config.clone();
        explicit.settings = serde_json::from_value(serde_json::json!({"project":"explicit-project", "region":"asia-east1", "access_token":"fixture-direct-token"})).unwrap();
        let normalized = normalize_provider_construction(&explicit);
        assert_eq!(normalized.config.settings["project_id"], "explicit-project");
        assert_eq!(normalized.config.settings["location"], "asia-east1");
        assert!(
            !normalized.config.settings.contains_key("credentials_file"),
            "direct tokens must not acquire an unused environment credential file"
        );
    }
    {
        let _env = EnvScope::new(&[("GOOGLE_CLOUD_PROJECT", "environment-project")]);
        let mut explicit = config.clone();
        explicit.project = Some("top-level-project".into());
        explicit
            .settings
            .insert("access_token".into(), "fixture-direct-token".into());
        assert_eq!(
            normalize_provider_construction(&explicit).config.settings["project_id"],
            "top-level-project"
        );
        explicit
            .settings
            .insert("project".into(), "settings-project".into());
        // The direct factory inserts top-level project before merging settings
        // with entry.or_insert. Compare its effective input with normalization.
        let direct = crate::core::providers::factory::vertex_resource_config_from_factory(
            &serde_json::json!({"project": "top-level-project", "access_token": "fixture-direct-token"}),
        ).unwrap();
        assert_eq!(
            normalize_provider_construction(&explicit).config.settings["project_id"],
            direct.project_id
        );
        assert_eq!(direct.project_id, "top-level-project");
        // The explicit canonical key precedes project in the factory resolver.
        explicit
            .settings
            .insert("project_id".into(), "canonical-project".into());
        let direct = crate::core::providers::factory::vertex_resource_config_from_factory(
            &serde_json::json!({"project_id": "canonical-project", "project": "top-level-project", "access_token": "fixture-direct-token"}),
        ).unwrap();
        assert_eq!(
            normalize_provider_construction(&explicit).config.settings["project_id"],
            direct.project_id
        );
        assert_eq!(direct.project_id, "canonical-project");
    }
    // The selected path remains the same; credentials held by the new provider change.
    let _env = EnvScope::new(&[
        ("GOOGLE_CLOUD_PROJECT", "fixture-project-b"),
        ("GOOGLE_CLOUD_LOCATION", "europe-west1"),
        ("GOOGLE_APPLICATION_CREDENTIALS", second.to_str().unwrap()),
    ]);
    std::fs::write(
        &second,
        contents.replace("fixture-refresh", "fixture-refresh-rotated"),
    )
    .unwrap();
    let router = Router::from_gateway_config(std::slice::from_ref(&config), None)
        .await
        .unwrap();
    let rotated = router
        .get_deployment("vertex-fixture-gemini-2.5-flash")
        .unwrap()
        .state
        .runtime_identity
        .clone()
        .unwrap();
    assert_ne!(identities.last().unwrap(), &rotated);

    // Reconstructed replicas must agree even though parsed WIF headers use HashMap.
    let wif = r#"{"type":"external_account","audience":"fixture-audience","subject_token_type":"urn:ietf:params:oauth:token-type:jwt","token_url":"https://sts.googleapis.com/v1/token","credential_source":{"url":"https://fixture.invalid/token","headers":{"fixture-a":"a","fixture-b":"b"}}}"#;
    let mut wif_identity = None;
    for index in 0..6 {
        let contents = if index % 2 == 0 {
            wif.to_owned()
        } else {
            wif.replace(
                "\"fixture-a\":\"a\",\"fixture-b\":\"b\"",
                "\"fixture-b\":\"b\",\"fixture-a\":\"a\"",
            )
        };
        std::fs::write(&second, contents).unwrap();
        let router = Router::from_gateway_config(std::slice::from_ref(&config), None)
            .await
            .unwrap();
        let identity = router
            .get_deployment("vertex-fixture-gemini-2.5-flash")
            .unwrap()
            .state
            .runtime_identity
            .clone()
            .unwrap();
        if let Some(previous) = &wif_identity {
            assert_eq!(previous, &identity);
        }
        wif_identity = Some(identity);
    }
    // A selected file failure keeps the factory's read/parse error; higher-priority
    // credentials never inspect an unused file.
    let missing = directory.path().join("missing.json");
    unsafe { std::env::set_var("GOOGLE_APPLICATION_CREDENTIALS", &missing) };
    let error = Router::from_gateway_config(std::slice::from_ref(&config), None)
        .await
        .err()
        .unwrap();
    assert!(
        matches!(error, RouterError::DeploymentNotFound(message) if message.contains("failed to read credentials file"))
    );
    std::fs::write(&missing, "{invalid").unwrap();
    let error = Router::from_gateway_config(std::slice::from_ref(&config), None)
        .await
        .err()
        .unwrap();
    assert!(
        matches!(error, RouterError::DeploymentNotFound(message) if message.contains("invalid credentials file"))
    );
    std::fs::remove_file(&missing).unwrap();
    for credentials in [
        serde_json::json!({"access_token":"fixture-direct-token"}),
        serde_json::json!({"credentials_json":contents}),
    ] {
        let mut explicit = config.clone();
        explicit.settings = serde_json::from_value(credentials).unwrap();
        assert!(Router::from_gateway_config(&[explicit], None).await.is_ok());
    }
    for pair in identities.windows(2) {
        assert_ne!(pair[0], pair[1]);
    }
}

#[tokio::test]
async fn bedrock_environment_rotations_change_runtime_resource_identity() {
    use super::*;
    let config = ProviderConfig {
        name: "bedrock-fixture".to_string(),
        provider_type: "bedrock".to_string(),
        api_key: String::new(),
        models: vec!["anthropic.claude-3-sonnet-20240229-v1:0".to_string()],
        ..ProviderConfig::default()
    };
    let mut identities = Vec::new();
    for (key, secret, token, region) in [
        (
            "AKIA-fixture-a",
            "fixture-secret-a",
            "fixture-session-a",
            "us-east-1",
        ),
        (
            "AKIA-fixture-b",
            "fixture-secret-a",
            "fixture-session-a",
            "us-east-1",
        ),
        (
            "AKIA-fixture-b",
            "fixture-secret-b",
            "fixture-session-a",
            "us-east-1",
        ),
        (
            "AKIA-fixture-b",
            "fixture-secret-b",
            "fixture-session-b",
            "us-east-1",
        ),
        (
            "AKIA-fixture-b",
            "fixture-secret-b",
            "fixture-session-b",
            "us-west-2",
        ),
    ] {
        let _env = EnvScope::new(&[
            ("AWS_ACCESS_KEY_ID", key),
            ("AWS_SECRET_ACCESS_KEY", secret),
            ("AWS_SESSION_TOKEN", token),
            ("AWS_DEFAULT_REGION", region),
        ]);
        let normalized = normalize_provider_construction(&config);
        let resource = crate::core::providers::factory::bedrock_resource_config_from_factory(
            &serde_json::json!(normalized.config.settings),
        );
        assert_eq!(resource.aws_access_key_id, key);
        assert_eq!(resource.aws_secret_access_key, secret);
        assert_eq!(resource.aws_session_token.as_deref(), Some(token));
        assert_eq!(resource.aws_region, region);
        let router = Router::from_gateway_config(std::slice::from_ref(&config), None)
            .await
            .unwrap();
        identities.push(
            router
                .get_deployment("bedrock-fixture-anthropic.claude-3-sonnet-20240229-v1:0")
                .unwrap()
                .state
                .runtime_identity
                .clone()
                .unwrap(),
        );
        let mut explicit = config.clone();
        explicit.settings = serde_json::from_value(serde_json::json!({"access_key":"explicit-fixture", "secret_key":"explicit-secret", "session_token":"explicit-session", "region":"eu-west-1"})).unwrap();
        let normalized = normalize_provider_construction(&explicit);
        let resource = crate::core::providers::factory::bedrock_resource_config_from_factory(
            &serde_json::json!(normalized.config.settings),
        );
        assert_eq!(resource.aws_access_key_id, "explicit-fixture");
        assert_eq!(resource.aws_secret_access_key, "explicit-secret");
        assert_eq!(
            resource.aws_session_token.as_deref(),
            Some("explicit-session")
        );
        assert_eq!(resource.aws_region, "eu-west-1");
    }
    for pair in identities.windows(2) {
        assert_ne!(
            pair[0], pair[1],
            "changed effective AWS resource must retire prior identity"
        );
    }
}

#[tokio::test]
async fn bedrock_identity_binds_credentials_read_after_normalization() {
    use super::*;
    let _env = EnvScope::new(&[
        ("AWS_ACCESS_KEY_ID", "AKIA-fixture-key"),
        ("AWS_SECRET_ACCESS_KEY", "fixture-secret"),
    ]);
    let config = ProviderConfig {
        name: "bedrock-fixture".into(),
        provider_type: "bedrock".into(),
        ..ProviderConfig::default()
    };
    let normalized = normalize_provider_construction(&config).config;
    assert!(normalized.settings["aws_session_token"].is_null());
    let base_identity = GatewayRuntimeIdentity::for_provider(&normalized);
    let Provider::Bedrock(first) = create_provider(normalized.clone()).await.unwrap() else {
        panic!("expected Bedrock provider");
    };
    let first_digest = first.credential_resource_identity();

    // The public factory's existing fallback can observe a token appearing
    // after normalization. Identity must bind what this provider actually uses.
    unsafe { std::env::set_var("AWS_SESSION_TOKEN", "late-fixture-session") };
    let Provider::Bedrock(second) = create_provider(normalized.clone()).await.unwrap() else {
        panic!("expected Bedrock provider");
    };
    let second_digest = second.credential_resource_identity();
    assert_eq!(first_digest, first.credential_resource_identity());
    assert_ne!(
        base_identity.with_credential_digest(first_digest),
        base_identity.with_credential_digest(second_digest),
        "same normalized config must not mask different constructed credentials"
    );

    // The digest is stable once constructed, even if the environment changes again.
    unsafe { std::env::remove_var("AWS_SESSION_TOKEN") };
    assert_eq!(second_digest, second.credential_resource_identity());
    let Provider::Bedrock(third) = create_provider(normalized).await.unwrap() else {
        panic!("expected Bedrock provider");
    };
    assert_eq!(first_digest, third.credential_resource_identity());
}

#[cfg(feature = "providers-extended")]
mod matrix {
    use super::super::*;
    use super::{ENV_LOCK, ENVS, EnvScope};
    use crate::core::providers::unified_provider::ProviderError;
    const GEM_TOP: &str = "gem-top-test-api-key-12345678901234567890";
    const GEM_SETTINGS: &str = "gem-settings-test-api-key-12345678901234567890";
    const GEM_GOOGLE: &str = "gem-google-test-api-key-12345678901234567890";
    const GEM_SETTING: &str = "gem-setting-test-api-key-12345678901234567890";
    const GEM_ENV: &str = "gem-env-test-api-key-12345678901234567890";
    const GEM_GOOGLE_ENV: &str = "gem-google-env-test-api-key-12345678901234567890";
    const _: () =
        assert!(GEM_TOP.len() >= 20 && GEM_SETTINGS.len() >= 20 && GEM_GOOGLE.len() >= 20);
    const _: () =
        assert!(GEM_SETTING.len() >= 20 && GEM_ENV.len() >= 20 && GEM_GOOGLE_ENV.len() >= 20);

    struct Case {
        name: &'static str,
        selector: &'static str,
        top: &'static str,
        settings: &'static [(&'static str, &'static str)],
        env: &'static [(&'static str, &'static str)],
        selected: Option<&'static str>,
        shadowed: &'static [&'static str],
    }

    fn provider(name: &str, api_key: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            api_key: api_key.to_string(),
            models: vec!["credential-model".to_string()],
            ..ProviderConfig::default()
        }
    }

    async fn run(case: &Case) {
        let before = {
            let _lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
            ENVS.iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>()
        };
        {
            let _env = EnvScope::new(case.env);
            let mut config = provider(case.name, case.top);
            config.provider_type = case.selector.to_string();
            if matches!(
                case.selector.parse::<ProviderType>(),
                Ok(ProviderType::Cloudflare)
            ) {
                config.organization = Some("fixture-account".to_string());
            }
            for &(key, value) in case.settings {
                config.settings.insert(key.to_string(), value.into());
            }
            let router = Router::from_gateway_config(&[config], None).await;
            if let Some(selected) = case.selected {
                let snapshot = router
                    .unwrap_or_else(|error| panic!("{}: {error}", case.name))
                    .load_routing_snapshot();
                assert!(
                    matches!(
                        snapshot.resolve_legacy_credential("credential-model", selected),
                        Ok(deployment) if deployment == format!("{}-credential-model", case.name)
                    ),
                    "{} selected wrong credential",
                    case.name
                );
                for shadowed in case.shadowed {
                    assert!(
                        matches!(
                            snapshot.resolve_legacy_credential("credential-model", shadowed),
                            Err(ProviderError::ModelNotFound { .. })
                        ),
                        "{} accepted shadowed credential",
                        case.name
                    );
                }
            } else {
                if let Ok(router) = router {
                    assert!(
                        matches!(
                            router.load_routing_snapshot().resolve_legacy_credential(
                                "credential-model",
                                "unresolved-fixture"
                            ),
                            Err(ProviderError::ModelNotFound { .. })
                        ),
                        "{} published unresolved provenance",
                        case.name
                    );
                }
            }
        }
        let after = {
            let _lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
            ENVS.iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>()
        };
        assert_eq!(before, after);
    }

    #[tokio::test]
    async fn direct_factory_skips_blank_primary_and_alternate_credentials() {
        for (selector, values, selected) in [
            (
                "heroku",
                vec![
                    ("HEROKU_API_KEY", " "),
                    ("INFERENCE_KEY", ""),
                    ("EMBEDDING_KEY", "embedding-fixture"),
                ],
                "embedding-fixture",
            ),
            (
                "ovhcloud",
                vec![
                    ("OVHCLOUD_API_KEY", ""),
                    ("OVH_AI_ENDPOINTS_ACCESS_TOKEN", "ovh-fixture"),
                ],
                "ovh-fixture",
            ),
        ] {
            let _env = EnvScope::new(&values);
            let mut config = provider(selector, "");
            config.provider_type = selector.to_owned();
            let created = crate::core::providers::create_provider(config)
                .await
                .unwrap();
            let crate::core::providers::Provider::OpenAILike(created) = created else {
                panic!("expected catalog provider");
            };
            assert_eq!(created.config().base.api_key.as_deref(), Some(selected));
        }
    }

    #[tokio::test]
    async fn complete_construction_credential_precedence_matrix() {
        #[rustfmt::skip]
    let cases = [
        Case { name: "native-top", selector: "openai", top: "sk-top-fixture", settings: &[("api_key","sk-settings-fixture")], env: &[], selected: Some("sk-top-fixture"), shadowed: &["sk-settings-fixture"] },
        Case { name: "native-settings", selector: "openai", top: " ", settings: &[("api_key","sk-settings-fixture")], env: &[], selected: Some("sk-settings-fixture"), shadowed: &[] },
        Case { name: "catalog-explicit", selector: "xiaomi_mimo", top: "explicit", settings: &[], env: &[("MIMO_API_KEY","primary"),("XIAOMI_API_KEY","alternate")], selected: Some("explicit"), shadowed: &["primary","alternate"] },
        Case { name: "catalog-primary", selector: "xiaomi_mimo", top: " ", settings: &[], env: &[("MIMO_API_KEY","primary"),("XIAOMI_API_KEY","alternate")], selected: Some("primary"), shadowed: &["alternate"] },
        Case { name: "catalog-alternate", selector: "xiaomi_mimo", top: "", settings: &[], env: &[("MIMO_API_KEY"," "),("XIAOMI_API_KEY","alternate")], selected: Some("alternate"), shadowed: &[] },
        Case { name: "catalog-blank", selector: "xiaomi_mimo", top: " ", settings: &[], env: &[("MIMO_API_KEY"," "),("XIAOMI_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "catalog-alias-ai21", selector: "ai21-chat", top: "", settings: &[], env: &[("AI21_API_KEY","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "catalog-ai21-env", selector: "ai21_chat", top: "", settings: &[], env: &[("AI21_API_KEY","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "catalog-huggingface-env", selector: "hugging_face", top: "", settings: &[], env: &[("HF_TOKEN","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "heroku-explicit", selector: "heroku", top: "explicit", settings: &[], env: &[("HEROKU_API_KEY","primary"),("INFERENCE_KEY","native"),("EMBEDDING_KEY","embedding")], selected: Some("explicit"), shadowed: &["primary","native","embedding"] },
        Case { name: "heroku-primary", selector: "heroku", top: "", settings: &[], env: &[("HEROKU_API_KEY","primary"),("INFERENCE_KEY","native")], selected: Some("primary"), shadowed: &["native"] },
        Case { name: "heroku-native", selector: "heroku", top: "", settings: &[], env: &[("INFERENCE_KEY","native")], selected: Some("native"), shadowed: &[] },
        Case { name: "heroku-embedding", selector: "heroku", top: "", settings: &[], env: &[("INFERENCE_KEY"," "),("EMBEDDING_KEY","embedding")], selected: Some("embedding"), shadowed: &[] },
        Case { name: "ovh-native", selector: "ovhcloud", top: "", settings: &[], env: &[("OVH_AI_ENDPOINTS_ACCESS_TOKEN","native")], selected: Some("native"), shadowed: &[] },
        Case { name: "ovh-explicit", selector: "ovhcloud", top: "explicit", settings: &[], env: &[("OVHCLOUD_API_KEY","primary"),("OVH_AI_ENDPOINTS_ACCESS_TOKEN","native")], selected: Some("explicit"), shadowed: &["primary","native"] },
        Case { name: "catalog-baseten-env", selector: "baseten", top: "", settings: &[], env: &[("BASETEN_API_KEY","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "cf-settings", selector: "cf", top: "top", settings: &[("api_token","settings")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("settings"), shadowed: &["top","env"] },
        Case { name: "cf-top", selector: "cloudflare", top: "top", settings: &[("api_token"," ")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("top"), shadowed: &["env"] },
        Case { name: "cf-env", selector: "workers-ai", top: " ", settings: &[("api_token","")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "cf-blank", selector: "cloudflare", top: " ", settings: &[("api_token","")], env: &[("CLOUDFLARE_API_TOKEN"," ")], selected: None, shadowed: &[] },
        Case { name: "cf-api-key", selector: "cloudflare", top: " ", settings: &[("api_token",""),("api_key","settings")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "bedrock-api-key-ignored", selector: "bedrock", top: "stray", settings: &[("aws_access_key_id","AKIATEST123456789012"),("aws_secret_access_key","test-secret-key")], env: &[], selected: None, shadowed: &[] },
        Case { name: "rep-top", selector: "replicate", top: "top", settings: &[("api_key","settings"),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1"),("REPLICATE_API_KEY","env2")], selected: Some("top"), shadowed: &["settings","token","env1","env2"] },
        Case { name: "rep-settings", selector: "replicate-ai", top: " ", settings: &[("api_key","settings"),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1")], selected: Some("settings"), shadowed: &["token","env1"] },
        Case { name: "rep-token", selector: "replicate", top: "", settings: &[("api_key"," "),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1")], selected: Some("token"), shadowed: &["env1"] },
        Case { name: "rep-env1", selector: "replicate", top: "", settings: &[], env: &[("REPLICATE_API_TOKEN","env1"),("REPLICATE_API_KEY","env2")], selected: Some("env1"), shadowed: &["env2"] },
        Case { name: "rep-env2", selector: "replicate", top: "", settings: &[], env: &[("REPLICATE_API_TOKEN"," "),("REPLICATE_API_KEY","env2")], selected: Some("env2"), shadowed: &[] },
        Case { name: "rep-blank", selector: "replicate", top: " ", settings: &[("api_key"," "),("api_token","")], env: &[("REPLICATE_API_TOKEN"," "),("REPLICATE_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "fal-top", selector: "fal-ai", top: "top", settings: &[("api_key","settings")], env: &[("FAL_AI_API_KEY","env")], selected: Some("top"), shadowed: &["settings","env"] },
        Case { name: "fal-settings", selector: "fal", top: " ", settings: &[("api_key","settings")], env: &[("FAL_AI_API_KEY","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "fal-env", selector: "fal_ai", top: "", settings: &[("api_key"," ")], env: &[("FAL_AI_API_KEY","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "fal-blank", selector: "fal_ai", top: " ", settings: &[("api_key","")], env: &[("FAL_AI_API_KEY"," ")], selected: None, shadowed: &[] },
        Case { name: "cohere-top", selector: "cohere", top: "top", settings: &[("api_key","settings")], env: &[("COHERE_API_KEY","env")], selected: Some("top"), shadowed: &["settings","env"] },
        Case { name: "cohere-settings", selector: "cohere-ai", top: " ", settings: &[("api_key","settings")], env: &[("COHERE_API_KEY","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "cohere-env", selector: "cohere", top: "", settings: &[], env: &[("COHERE_API_KEY","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "cohere-blank", selector: "cohere", top: " ", settings: &[("api_key","")], env: &[("COHERE_API_KEY"," ")], selected: None, shadowed: &[] },
        Case { name: "gem-top", selector: "gemini", top: GEM_TOP, settings: &[("api_key",GEM_SETTINGS)], env: &[("GEMINI_API_KEY",GEM_ENV)], selected: Some(GEM_TOP), shadowed: &[GEM_SETTINGS,GEM_ENV] },
        Case { name: "gem-settings", selector: "google-gemini", top: " ", settings: &[("api_key",GEM_SETTINGS),("google_api_key",GEM_GOOGLE)], env: &[], selected: Some(GEM_SETTINGS), shadowed: &[GEM_GOOGLE] },
        Case { name: "gem-google", selector: "google_ai", top: "", settings: &[("api_key"," "),("google_api_key",GEM_GOOGLE),("gemini_api_key",GEM_SETTING)], env: &[], selected: Some(GEM_GOOGLE), shadowed: &[GEM_SETTING] },
        Case { name: "gem-setting", selector: "google-ai", top: "", settings: &[("google_api_key"," "),("gemini_api_key",GEM_SETTING)], env: &[("GEMINI_API_KEY",GEM_ENV)], selected: Some(GEM_SETTING), shadowed: &[GEM_ENV] },
        Case { name: "gem-env1", selector: "gemini", top: "", settings: &[], env: &[("GEMINI_API_KEY",GEM_ENV),("GOOGLE_API_KEY",GEM_GOOGLE_ENV)], selected: Some(GEM_ENV), shadowed: &[GEM_GOOGLE_ENV] },
        Case { name: "gem-env2", selector: "gemini", top: "", settings: &[], env: &[("GEMINI_API_KEY"," "),("GOOGLE_API_KEY",GEM_GOOGLE_ENV)], selected: Some(GEM_GOOGLE_ENV), shadowed: &[] },
        Case { name: "gem-blank", selector: "gemini", top: " ", settings: &[("api_key"," "),("google_api_key",""),("gemini_api_key"," ")], env: &[("GEMINI_API_KEY"," "),("GOOGLE_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "unknown", selector: "not-a-provider", top: "unknown", settings: &[], env: &[], selected: None, shadowed: &[] },
    ];
        for case in cases {
            run(&case).await;
        }
    }
}

use super::Router;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::model::ProviderCapability;
use std::collections::HashMap;

fn function<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing {signature}"));
    let body = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap();
    let mut depth = 0;
    for (offset, byte) in source[body..].bytes().enumerate() {
        depth += usize::from(byte == b'{');
        depth -= usize::from(byte == b'}');
        if depth == 0 {
            return &source[start..=body + offset];
        }
    }
    panic!("unclosed {signature}");
}

#[test]
fn credential_resolver_and_construction_source_guards() {
    let unified = include_str!("unified.rs");
    for signature in [
        "fn resolve_legacy_credential(",
        "fn resolve_legacy_credential_with(",
    ] {
        let resolver = function(unified, signature);
        for forbidden in [
            "std::env",
            "ProviderConfig",
            "create_provider",
            "factory",
            "add_deployment",
        ] {
            assert!(
                !resolver.contains(forbidden),
                "{signature} contains {forbidden}"
            );
        }
    }
    let gateway = include_str!("gateway_config.rs");
    let identity = include_str!("gateway_identity.rs");
    let canonical = function(
        gateway,
        "pub(super) async fn from_gateway_config_with_identity(",
    );
    assert_eq!(
        canonical.matches("create_provider_with_resources(").count(),
        1
    );

    let construction_entry_points = [
        function(gateway, "pub async fn from_gateway_config("),
        function(gateway, "pub async fn from_gateway_config_with_aliases("),
        canonical,
        function(identity, "pub async fn from_gateway_config_with_pricing("),
        function(
            identity,
            "pub async fn from_gateway_config_with_aliases_and_pricing(",
        ),
    ];
    assert_eq!(
        construction_entry_points
            .iter()
            .map(|entry_point| entry_point
                .matches("create_provider_with_resources(")
                .count())
            .sum::<usize>(),
        1
    );
}

#[test]
fn ambiguous_native_probe_capabilities_require_custom_endpoint_at_config_boundaries() {
    for provider_type in [
        "openai",
        "bedrock",
        "openrouter",
        "vertex_ai",
        "gemini",
        "fal_ai",
        "mistral",
        "cloudflare",
        "azure",
        "azure_ai",
        "ollama",
        "cohere",
        "replicate",
    ] {
        let mut provider = crate::config::models::provider::ProviderConfig {
            name: format!("{provider_type}-primary"),
            provider_type: provider_type.to_string(),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig {
                interval: 15,
                ..Default::default()
            },
            ..crate::config::models::provider::ProviderConfig::default()
        };

        let error = provider
            .validate_health_check_runtime()
            .expect_err("multi-capability providers require an explicit probe endpoint");
        assert!(
            error.contains("require a custom health_check.endpoint"),
            "{error}"
        );

        provider.health_check.endpoint = Some("https://8.8.8.8/health".to_string());
        assert!(provider.validate_health_check_runtime().is_ok());
    }
}

#[test]
fn fal_ai_name_selector_cannot_bypass_native_probe_validation() {
    let provider = crate::config::models::provider::ProviderConfig {
        name: "fal_ai".to_string(),
        provider_type: String::new(),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            interval: 15,
            ..Default::default()
        },
        ..crate::config::models::provider::ProviderConfig::default()
    };

    let error = provider
        .validate_health_check_runtime()
        .expect_err("the effective name selector must be validated");
    assert!(
        error.contains("require a custom health_check.endpoint"),
        "{error}"
    );
}

#[test]
fn unambiguously_chat_only_provider_can_opt_into_native_probe() {
    let provider = crate::config::models::provider::ProviderConfig {
        name: "anthropic-primary".to_string(),
        provider_type: "anthropic".to_string(),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            interval: 15,
            ..Default::default()
        },
        ..crate::config::models::provider::ProviderConfig::default()
    };

    assert!(provider.validate_health_check_runtime().is_ok());
}

fn identity_provider_config(
    model: &str,
    mapping: Option<crate::core::providers::model_identity::ModelIdentityMapping>,
) -> crate::config::models::provider::ProviderConfig {
    let mut config = crate::config::models::provider::ProviderConfig {
        name: "identity-openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        models: vec![model.to_string()],
        ..Default::default()
    };
    if let Some(mapping) = mapping {
        config.settings.insert(
            crate::core::providers::model_identity::MODEL_IDENTITY_MAPPINGS_KEY.to_string(),
            serde_json::json!({model: mapping}),
        );
    }
    config
}

fn openai_like_identity_config(
    name: &str,
    provider_type: &str,
    model: &str,
    mapping: Option<crate::core::providers::model_identity::ModelIdentityMapping>,
) -> crate::config::models::provider::ProviderConfig {
    let mut config = crate::config::models::provider::ProviderConfig {
        name: name.to_string(),
        provider_type: provider_type.to_string(),
        api_key: "test-key".to_string(),
        models: vec![model.to_string()],
        ..Default::default()
    };
    if provider_type == "openai_compatible" {
        config.settings.insert(
            "base_url".to_string(),
            serde_json::json!("https://vertex.example.com/v1"),
        );
        config.settings.insert(
            "provider_name".to_string(),
            serde_json::json!("vertex_publisher"),
        );
    }
    if let Some(mapping) = mapping {
        config.settings.insert(
            crate::core::providers::model_identity::MODEL_IDENTITY_MAPPINGS_KEY.to_string(),
            serde_json::json!({model: mapping}),
        );
    }
    config
}

fn runtime_price(provider: &str) -> crate::core::pricing_service::LiteLLMModelInfo {
    crate::core::pricing_service::LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(1024),
        input_cost_per_token: Some(0.01),
        output_cost_per_token: Some(0.02),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    }
}

#[tokio::test]
async fn pricing_aware_default_openai_publishes_only_callable_catalog_models() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let provider = crate::config::models::provider::ProviderConfig {
        name: "default-openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        models: Vec::new(),
        ..Default::default()
    };

    let router = Router::from_gateway_config_with_pricing(&[provider], None, pricing)
        .await
        .expect("default OpenAI catalog should publish only callable deployments");

    assert!(!router.get_deployments_for_model("gpt-4").is_empty());
    for non_callable in [
        "high/1536-x-1024/gpt-image-1",
        "openai/container",
        "daybreak-blue-latest",
        "openai/sora-2",
        "GPT-4",
    ] {
        assert!(
            router.get_deployments_for_model(non_callable).is_empty(),
            "non-callable or non-exact catalog identity {non_callable:?} became routable"
        );
    }
}

#[tokio::test]
async fn explicit_fine_tune_mapping_preserves_wire_id_and_uses_exact_openai_metadata() {
    let wire_model = "ft:tenant:custom-chat";
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    pricing.add_custom_model("runtime-only-price".to_string(), runtime_price("openai"));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("runtime-only-price".to_string()),
    );

    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config(wire_model, Some(mapping))],
        None,
        pricing.clone(),
    )
    .await
    .expect("injected runtime target should validate");
    let deployment = router
        .get_deployment("identity-openai-ft:tenant:custom-chat")
        .expect("deployment should be published");
    let bound = deployment
        .provider
        .runtime_pricing()
        .expect("managed provider should retain injected authority");
    assert!(std::sync::Arc::ptr_eq(&pricing, &bound));
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("configured fine-tune must retain typed identity");
    assert_eq!(identity.wire_model(), wire_model);
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
    assert_eq!(identity.pricing_model(), Some("runtime-only-price"));
    let crate::core::providers::Provider::OpenAI(provider) = &deployment.provider else {
        panic!("fine-tune deployment must use the OpenAI provider");
    };
    let model = provider
        .get_model_info(wire_model)
        .expect("bound fine-tune metadata must resolve");
    assert_eq!(model.id, "gpt-4");
    assert_eq!(model.max_context_length, 8192);
    assert_eq!(model.max_output_length, Some(8192));
    assert_eq!(
        provider
            .get_model_context_window(wire_model)
            .expect("bound fine-tune context must resolve"),
        8192
    );
    assert_eq!(crate::utils::ModelUtils::get_base_model(&model.id), "gpt-4");
    assert!(deployment.provider.supports_capability_for_model(
        wire_model,
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ));
    assert!(!deployment.provider.supports_capability_for_model(
        wire_model,
        &crate::core::types::model::ProviderCapability::Embeddings,
    ));
}

#[tokio::test]
async fn unmapped_fine_tune_stays_unprivileged_and_reports_mapping_hint() {
    let wire_model = "ft:tenant:custom-chat";
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config(wire_model, None)],
        None,
        pricing,
    )
    .await
    .expect("an exact configured deployment must survive without semantic mappings");

    let deployment = router
        .get_deployment("identity-openai-ft:tenant:custom-chat")
        .expect("configured deployment should be published");
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("configured deployment should retain a typed identity");
    assert_eq!(identity.wire_model(), wire_model);
    assert_eq!(identity.capability_catalog_model(), None);
    assert_eq!(identity.pricing_model(), None);

    let error = match router.select_deployment_lease_for_capability(
        wire_model,
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ) {
        Ok(_) => panic!("an unmapped deployment must not gain catalog capabilities"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("model_identity_mappings.ft:tenant:custom-chat.capability_catalog_model"),
        "{error}"
    );
}

#[tokio::test]
async fn pricing_only_mapping_never_grants_provider_capability() {
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    pricing.add_custom_model("runtime-only-price".to_string(), runtime_price("openai"));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        None,
        Some("runtime-only-price".to_string()),
    );
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
        pricing,
    )
    .await
    .expect("pricing-only deployment may be retained for non-capability consumers");
    let deployment = router
        .get_deployment("identity-openai-wire-deployment")
        .expect("deployment should be published");
    assert!(!deployment.provider.supports_capability_for_model(
        "wire-deployment",
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ));
}

#[tokio::test]
async fn explicit_unpriced_mapping_never_falls_back_to_raw_catalog_price() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        None,
    );
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("gpt-4", Some(mapping))],
        None,
        pricing.clone(),
    )
    .await
    .expect("explicit unpriced mapping should remain routable by capability");
    let deployment = router
        .get_deployment("identity-openai-gpt-4")
        .expect("deployment should be published");
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("validated deployment identity should bind");
    assert_eq!(identity.pricing_model(), None);
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
}

#[tokio::test]
async fn compatibility_constructor_rejects_runtime_mapping_without_authority() {
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("runtime-only-price".to_string()),
    );
    let error = Router::from_gateway_config(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
    )
    .await
    .expect_err("mapping without injected runtime pricing must fail");
    let text = error.to_string();
    assert!(text.contains("model_identity_mappings") && text.contains("pricing"));
}

#[tokio::test]
async fn pricing_aware_constructor_fails_absent_target_before_snapshot_publication() {
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("missing-runtime-price".to_string()),
    );
    let error = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
        pricing,
    )
    .await
    .expect_err("missing runtime target must fail startup");
    let text = error.to_string();
    assert!(
        text.contains("identity-openai")
            && text.contains("wire-deployment")
            && text.contains("pricing_model")
            && text.contains("missing-runtime-price"),
        "{text}"
    );
}

#[tokio::test]
async fn xai_identity_is_automatic_only_for_native_or_explicitly_mapped_publishers() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let qualified = "xai/grok-4.6";
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some(qualified.to_string()),
        Some(qualified.to_string()),
    );
    let mut custom = openai_like_identity_config("custom-xai", "xai", "custom/grok-4.6", None);
    custom.models = [
        "custom/grok-4.6",
        "custom/xai/grok-4.6",
        "custom/custom/grok-4.6",
        "customized/grok-4.6",
        "wrong/grok-4.6",
        "custom/grok-4.6-latest",
    ]
    .map(str::to_string)
    .to_vec();
    custom
        .settings
        .insert("model_prefix".to_string(), serde_json::json!("custom/"));
    let providers = vec![
        openai_like_identity_config("native-xai", "xai", qualified, None),
        openai_like_identity_config(
            "mapped-vertex",
            "openai_compatible",
            qualified,
            Some(mapping),
        ),
        openai_like_identity_config("unmapped-vertex", "openai_compatible", qualified, None),
        custom,
    ];
    let router = Router::from_gateway_config_with_pricing(&providers, None, pricing)
        .await
        .expect("exact native and explicitly mapped xAI identities should validate");
    let ids = router.get_deployments_for_model(qualified);
    let deployment = |name: &str| {
        ids.iter()
            .find(|id| id.starts_with(name))
            .and_then(|id| router.get_deployment(id))
            .unwrap_or_else(|| panic!("missing {name} deployment in {ids:?}"))
    };

    let native = deployment("native-xai");
    let crate::core::providers::Provider::OpenAILike(native_provider) = &native.provider else {
        panic!("xAI catalog provider must use OpenAI-compatible transport");
    };
    let native_json = LLMProvider::transform_request(
        native_provider,
        crate::core::types::chat::ChatRequest {
            model: qualified.to_string(),
            messages: vec![],
            ..Default::default()
        },
        crate::core::types::context::RequestContext::default(),
    )
    .await
    .expect("native xAI transform");
    assert_eq!(native_json["model"], "grok-4.6");

    let mapped = deployment("mapped-vertex");
    assert!(
        mapped
            .provider
            .supports_capability_for_model(qualified, &ProviderCapability::ChatCompletion)
    );
    assert!(
        !mapped
            .provider
            .supports_capability_for_model(qualified, &ProviderCapability::BatchProcessing)
    );
    let crate::core::providers::Provider::OpenAILike(mapped_provider) = &mapped.provider else {
        panic!("mapped publisher must use OpenAI-compatible transport");
    };
    let mapped_json = LLMProvider::transform_request(
        mapped_provider,
        crate::core::types::chat::ChatRequest {
            model: qualified.to_string(),
            messages: vec![],
            extra_params: HashMap::from([(
                "reasoning_effort".to_string(),
                serde_json::json!("xhigh"),
            )]),
            ..Default::default()
        },
        crate::core::types::context::RequestContext::default(),
    )
    .await
    .expect("mapped publisher transform");
    assert_eq!(mapped_json["model"], qualified);
    assert_eq!(mapped_json["reasoning_effort"], "xhigh");
    for field in ["stop", "presence_penalty", "frequency_penalty"] {
        let request = crate::core::types::chat::ChatRequest {
            model: qualified.to_string(),
            messages: vec![],
            extra_params: HashMap::from([
                ("reasoning_effort".to_string(), serde_json::json!("high")),
                (field.to_string(), serde_json::json!(1)),
            ]),
            ..Default::default()
        };
        assert!(
            LLMProvider::transform_request(
                mapped_provider,
                request,
                crate::core::types::context::RequestContext::default(),
            )
            .await
            .is_err(),
            "{field} must be rejected after the final extra-body merge"
        );
    }

    let unmapped = deployment("unmapped-vertex");
    assert!(
        !unmapped
            .provider
            .supports_capability_for_model(qualified, &ProviderCapability::ChatCompletion)
    );
    assert!(
        unmapped
            .provider
            .calculate_cost(qualified, 1, 1)
            .await
            .is_err(),
        "an unmapped publisher must not turn absent pricing into zero"
    );

    let short = native
        .provider
        .calculate_cost(qualified, 199_999, 0)
        .await
        .expect("short-context xAI pricing");
    let long = native
        .provider
        .calculate_cost(qualified, 200_000, 0)
        .await
        .expect("long-context xAI pricing");
    assert!((short - 0.399_998).abs() < 1e-12);
    assert!((long - 0.8).abs() < 1e-12);

    for (raw, wire) in [
        ("custom/grok-4.6", "grok-4.6"),
        ("custom/xai/grok-4.6", "xai/grok-4.6"),
    ] {
        let id = router
            .get_deployments_for_model(raw)
            .into_iter()
            .next()
            .expect("custom xAI deployment id");
        let custom = router.get_deployment(&id).expect("custom xAI deployment");
        let identity = custom
            .provider
            .deployment_model_identity()
            .expect("custom xAI identity");
        assert_eq!(identity.wire_model(), wire);
        assert_eq!(identity.capability_catalog_model(), Some("grok-4.6"));
        assert_eq!(identity.pricing_model(), Some(qualified));
    }
    for invalid in [
        "custom/custom/grok-4.6",
        "customized/grok-4.6",
        "wrong/grok-4.6",
        "custom/grok-4.6-latest",
    ] {
        assert!(
            router
                .select_deployment_lease_for_capability(
                    invalid,
                    &ProviderCapability::ChatCompletion,
                )
                .is_err(),
            "{invalid} must not acquire xAI capability"
        );
    }
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn cohere_catalog_modes_select_only_implemented_endpoints() {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::types::model::ProviderCapability;
    let config = ProviderConfig {
        name: "cohere".into(),
        provider_type: "cohere".into(),
        api_key: "test-key".into(),
        models: vec![
            "command-a-03-2025".into(),
            "command-r".into(),
            "command-a-vision-07-2025".into(),
            "embed-v4.0".into(),
            "rerank-v4.0-pro".into(),
            "cohere-transcribe-03-2026".into(),
        ],
        ..Default::default()
    };
    let router = Router::from_gateway_config(&[config], None).await.unwrap();
    for (model, allowed) in [
        ("command-a-03-2025", ProviderCapability::ChatCompletion),
        ("command-r", ProviderCapability::ChatCompletion),
        ("embed-v4.0", ProviderCapability::Embeddings),
        ("rerank-v4.0-pro", ProviderCapability::Rerank),
    ] {
        assert!(
            router
                .select_deployment_lease_for_capability(model, &allowed)
                .is_ok(),
            "{model}"
        );
    }
    for model in ["embed-v4.0", "rerank-v4.0-pro", "cohere-transcribe-03-2026"] {
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability(model, &capability)
                    .is_err(),
                "{model}: {capability:?}"
            );
        }
    }
    assert!(
        router
            .select_deployment_lease_for_capability(
                "cohere-transcribe-03-2026",
                &ProviderCapability::AudioTranscription
            )
            .is_err()
    );
    assert!(
        router
            .select_deployment_lease_for_capability(
                "command-a-vision-07-2025",
                &ProviderCapability::ToolCalling
            )
            .is_err()
    );
}

#[tokio::test]
async fn xai_multi_agent_is_not_a_chat_deployment() {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::types::model::ProviderCapability;
    let config = ProviderConfig {
        name: "xai".into(),
        provider_type: "xai".into(),
        api_key: "test-key".into(),
        models: vec![
            "grok-4.20-multi-agent-0309".into(),
            "grok-4.20-0309-reasoning".into(),
        ],
        ..Default::default()
    };
    let router = Router::from_gateway_config(&[config], None).await.unwrap();
    assert!(
        router
            .select_deployment_lease_for_capability(
                "grok-4.20-0309-reasoning",
                &ProviderCapability::ChatCompletion
            )
            .is_ok()
    );
    for capability in [
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
        ProviderCapability::ToolCalling,
    ] {
        assert!(
            router
                .select_deployment_lease_for_capability("grok-4.20-multi-agent-0309", &capability)
                .is_err()
        );
    }
}

#[cfg(feature = "providers-extra")]
#[tokio::test]
async fn audited_azure_ai_tools_are_routable_only_for_tool_models() {
    use crate::config::models::provider::ProviderConfig;
    use crate::core::types::model::ProviderCapability;
    let config = ProviderConfig {
        name: "azure-tool-test".into(),
        provider_type: "azure_ai".into(),
        api_key: "test-key".into(),
        base_url: Some("https://test.services.ai.azure.com/models".into()),
        models: vec![
            "gpt-5.4".into(),
            "gpt-5.5".into(),
            "grok-4".into(),
            "Phi-4".into(),
        ],
        ..Default::default()
    };
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default().unwrap(),
    );
    let router = Router::from_gateway_config_with_pricing(&[config], None, pricing)
        .await
        .unwrap();
    for model in ["gpt-5.4", "gpt-5.5", "grok-4", "Phi-4"] {
        for capability in [
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
        ] {
            assert_eq!(
                router
                    .select_deployment_lease_for_capability(model, &capability)
                    .is_ok(),
                model != "Phi-4",
                "{model}: {capability:?}"
            );
        }
    }
}

#[cfg(not(feature = "providers-extra"))]
#[tokio::test]
async fn model_less_azure_ai_compatible_route_keeps_dynamic_chat() {
    use crate::core::types::model::ProviderCapability;
    for (models, route) in [
        (vec![], "customer-azure-endpoint"),
        (vec!["team-chat".into()], "team-chat"),
    ] {
        let config = crate::config::models::provider::ProviderConfig {
            name: "customer-azure-endpoint".into(),
            provider_type: "azure_ai".into(),
            api_key: "test-key".into(),
            base_url: Some("https://test.services.ai.azure.com/models".into()),
            models,
            ..Default::default()
        };
        let pricing = std::sync::Arc::new(
            crate::core::pricing_service::PricingService::with_embedded_default().unwrap(),
        );
        let router = Router::from_gateway_config_with_pricing(&[config], None, pricing)
            .await
            .unwrap();
        for capability in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ] {
            assert!(
                router
                    .select_deployment_lease_for_capability(route, &capability)
                    .is_ok()
            );
        }
    }
}

#[test]
fn bedrock_absent_session_token_is_frozen_before_factory_handoff() {
    use super::*;
    let _env = EnvScope::new(&[
        ("AWS_ACCESS_KEY_ID", "fixture-access"),
        ("AWS_SECRET_ACCESS_KEY", "fixture-secret"),
    ]);
    let config = ProviderConfig {
        name: "bedrock".into(),
        provider_type: "bedrock".into(),
        ..Default::default()
    };
    let normalized = normalize_provider_construction(&config);
    let identity = GatewayRuntimeIdentity::for_provider(&normalized.config);
    unsafe { std::env::set_var("AWS_SESSION_TOKEN", "later-fixture-token") };
    assert!(
        normalized
            .bedrock_resource
            .unwrap()
            .aws_session_token
            .is_none()
    );
    assert_eq!(
        identity,
        GatewayRuntimeIdentity::for_provider(&normalized.config)
    );
    assert_ne!(
        identity,
        GatewayRuntimeIdentity::for_provider(&normalize_provider_construction(&config).config)
    );
}

#[cfg(feature = "providers-extra")]
#[test]
fn vertex_effective_environment_and_file_contents_are_frozen_for_identity() {
    use super::*;
    use crate::core::providers::vertex_ai::VertexCredentials;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("credentials.json");
    let credentials = |secret: &str| {
        serde_json::json!({"type":"authorized_user", "client_id":"fixture-client", "client_secret":secret, "refresh_token":"fixture-refresh"}).to_string()
    };
    std::fs::write(&path, credentials("fixture-secret-a")).unwrap();
    let _env = EnvScope::new(&[
        ("GOOGLE_CLOUD_PROJECT", "fixture-project-a"),
        ("GOOGLE_CLOUD_LOCATION", "us-east1"),
        ("GOOGLE_APPLICATION_CREDENTIALS", path.to_str().unwrap()),
    ]);
    let config = ProviderConfig {
        name: "vertex_ai".into(),
        provider_type: "vertex_ai".into(),
        ..Default::default()
    };
    let normalized = normalize_provider_construction(&config);
    let identity = GatewayRuntimeIdentity::for_provider(&normalized.config);
    std::fs::write(&path, credentials("fixture-secret-b")).unwrap();
    unsafe {
        std::env::set_var("GOOGLE_CLOUD_PROJECT", "fixture-project-b");
        std::env::set_var("GOOGLE_CLOUD_LOCATION", "us-west1");
    }
    let resource = normalized.vertex_resource.unwrap().unwrap();
    assert_eq!(resource.project_id, "fixture-project-a");
    assert_eq!(resource.location, "us-east1");
    let VertexCredentials::AuthorizedUser(value) = resource.credentials else {
        panic!("expected file credential")
    };
    assert_eq!(value.client_secret, "fixture-secret-a");
    assert_eq!(
        identity,
        GatewayRuntimeIdentity::for_provider(&normalized.config)
    );
    assert_ne!(
        identity,
        GatewayRuntimeIdentity::for_provider(&normalize_provider_construction(&config).config)
    );
    unsafe {
        std::env::set_var("GOOGLE_CLOUD_PROJECT", "fixture-project-a");
        std::env::set_var("GOOGLE_CLOUD_LOCATION", "us-east1");
    }
    assert_ne!(
        identity,
        GatewayRuntimeIdentity::for_provider(&normalize_provider_construction(&config).config),
        "same credential path with changed contents must rotate identity"
    );
}
