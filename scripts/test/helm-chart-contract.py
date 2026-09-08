"""Check relationships that Kubernetes per-resource schemas cannot validate."""
import sys
import yaml

resources = list(yaml.safe_load_all(open(sys.argv[1], encoding="utf-8")))
by_kind = {item["kind"]: item for item in resources}
deployment = by_kind["Deployment"]
pod = deployment["spec"]["template"]
labels = pod["metadata"]["labels"]
service = by_kind["Service"]
selectors = [
    deployment["spec"]["selector"]["matchLabels"],
    service["spec"]["selector"],
    by_kind["PodDisruptionBudget"]["spec"]["selector"]["matchLabels"],
    pod["spec"]["affinity"]["podAntiAffinity"]
    ["preferredDuringSchedulingIgnoredDuringExecution"][0]
    ["podAffinityTerm"]["labelSelector"]["matchLabels"],
]
for selector in selectors:
    assert selector.items() <= labels.items(), (selector, labels)
    assert "app.kubernetes.io/instance" in selector
assert pod["spec"]["containers"][0]["ports"] == [
    {"name": "http", "containerPort": 8000, "protocol": "TCP"}
], "gateway exposes one actual HTTP listener"
assert all(port["targetPort"] == "http" for port in service["spec"]["ports"])
for metadata in [pod["metadata"], service["metadata"]]:
    assert "prometheus.io/scrape" not in metadata.get("annotations", {})
hpa = by_kind.get("HorizontalPodAutoscaler")
if hpa:
    assert "replicas" not in deployment["spec"], "HPA owns replicas"
    assert hpa["spec"]["scaleTargetRef"]["name"] == deployment["metadata"]["name"]
else:
    assert deployment["spec"]["replicas"] == int(sys.argv[2])
ingress = by_kind.get("Ingress")
if ingress:
    backend = ingress["spec"]["rules"][0]["http"]["paths"][0]["backend"]["service"]
    assert backend["name"] == service["metadata"]["name"]
    assert backend["port"]["number"] == service["spec"]["ports"][0]["port"]
print("Helm resource relationships verified")
