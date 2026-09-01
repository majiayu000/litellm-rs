use super::Config;
use crate::config::models::gateway::GatewayConfig;

#[derive(serde::Deserialize)]
struct ConfigMapManifest {
    data: std::collections::BTreeMap<String, String>,
}

#[test]
fn kubernetes_manifests_match_runtime_contract() {
    let manifest_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("deployment/kubernetes");
    let config_map = std::fs::read_to_string(manifest_dir.join("configmap.yaml")).unwrap();
    let config_map: ConfigMapManifest = serde_yml::from_str(&config_map).unwrap();
    let gateway_yaml = config_map.data.get("gateway.yaml").unwrap();
    let gateway_yaml = gateway_yaml
        .replace("${OPENAI_API_KEY}", "sk-test-openai")
        .replace("${ANTHROPIC_API_KEY}", "sk-ant-test")
        .replace("${DATABASE_URL}", "postgresql://localhost/litellm")
        .replace("${REDIS_URL}", "redis://localhost:6379")
        .replace(
            "${LITELLM_JWT_SECRET}",
            "StrongJwtSecretWithMixedCaseAndNumbers1234!",
        );
    let gateway: GatewayConfig = serde_yml::from_str(&gateway_yaml).unwrap();
    Config { gateway }.validate().unwrap();

    let deployment = std::fs::read_to_string(manifest_dir.join("deployment.yaml")).unwrap();
    let deployment: serde_json::Value = serde_yml::from_str(&deployment).unwrap();
    assert!(
        deployment
            .pointer("/spec/template/spec/serviceAccountName")
            .is_none(),
        "the gateway does not require Kubernetes API permissions"
    );
    assert_eq!(
        deployment.pointer("/spec/template/spec/containers/0/livenessProbe/httpGet/path"),
        Some(&serde_json::json!("/health"))
    );
    assert_eq!(
        deployment.pointer("/spec/template/spec/containers/0/readinessProbe/httpGet/path"),
        Some(&serde_json::json!("/health/ready"))
    );
    assert_eq!(
        deployment.pointer("/spec/template/spec/containers/0/image"),
        Some(&serde_json::json!("litellm-rs:latest"))
    );
    assert_eq!(
        deployment.pointer("/spec/template/spec/containers/0/imagePullPolicy"),
        Some(&serde_json::json!("Always"))
    );

    let pdb = std::fs::read_to_string(manifest_dir.join("poddisruptionbudget.yaml")).unwrap();
    let pdb: serde_json::Value = serde_yml::from_str(&pdb).unwrap();
    assert_eq!(
        pdb.pointer("/apiVersion"),
        Some(&serde_json::json!("policy/v1"))
    );
    assert_eq!(
        pdb.pointer("/spec/selector"),
        deployment.pointer("/spec/selector")
    );
    assert_eq!(
        pdb.pointer("/spec/maxUnavailable"),
        Some(&serde_json::json!(1)),
        "one unavailable pod preserves availability with multiple replicas without blocking a single-replica override"
    );
    assert!(
        pdb.pointer("/spec/minAvailable").is_none(),
        "minAvailable would block voluntary disruption for a single replica"
    );
}
