# LiteLLM Rust Helm chart

Install the gateway using existing configuration and secret resources in the release namespace.
The ConfigMap must contain the ordinary `gateway.yaml` file. The Secret supplies its environment
references. Use the validated examples in `deployment/kubernetes/` as the starting point.
Configure HTTP on 8000, matching the container port and health probes. `/metrics` uses
the same HTTP listener and requires gateway authentication when enabled. Configure your
Prometheus scraper with a gateway credential from its Secret; the chart does not advertise
an unauthenticated annotation-based scrape or create a separate metrics listener.
For multiple replicas, configure shared PostgreSQL and Redis; the chart does not deploy databases.

```bash
helm upgrade --install gateway deployment/helm/litellm-rs \
  --namespace litellm-gateway --create-namespace \
  --set image=registry.example.com/litellm-rs:your-promoted-tag \
  --set configMapName=litellm-gateway-config \
  --set secretName=litellm-gateway-secrets
```

`values.yaml` lists the supported parameters. Fixed replicas default to one. With
`autoscaling.enabled=true`, HPA owns replicas and the Deployment omits `spec.replicas`.
HPA requires Metrics Server and CPU/memory resource requests. PDB allows one unavailable
pod. The ingress is opt-in and exposes only HTTP; configure its class, host and TLS Secret.
Services and PDBs select only pods belonging to their Helm release.

Configuration is maintained outside Helm; restart the deployment after changing its ConfigMap
or environment Secret. The writable data directory is ephemeral, so use PostgreSQL for durable data.

Validate and package from the repository root:

```bash
bash scripts/test/helm-chart.sh
helm package deployment/helm/litellm-rs --destination /tmp
```

Validation uses Python with PyYAML, Helm and kubeconform with Kubernetes 1.35.0 schemas. It renders the single,
fixed multi-replica (including ingress/TLS), and HPA fixtures before strict schema validation.
