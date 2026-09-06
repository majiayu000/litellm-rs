# 🚀 Deployment Guide

This directory contains all deployment-related files for the Rust LiteLLM Gateway.

## 📁 Directory Structure

```
deployment/
├── 📁 docker/              # Docker deployment
│   ├── Dockerfile          # Main Docker image
│   ├── docker-compose.yml  # Production compose
│   └── docker-compose.dev.yml # Development compose
├── 📁 kubernetes/          # Kubernetes manifests
│   └── (K8s YAML files)
├── 📁 systemd/             # System service files
│   └── rust-litellm-gateway.service
├── 📁 scripts/             # Deployment scripts
│   ├── start.sh            # Quick start script
│   ├── setup.sh            # Environment setup
│   ├── docker-start.sh     # Docker startup
│   └── init-*.sql          # Database initialization
└── 📁 configs/             # Deployment configurations
    └── monitoring/         # Monitoring configs
```

## 🚀 Quick Deployment Options

### 1. Local Development
```bash
# Quick start
./deployment/scripts/start.sh

# Or manually
cargo run
```

### 2. Docker
```bash
# Build and run
cd deployment/docker
docker-compose up -d
```

### 3. Production (systemd)
```bash
# Install service
sudo cp deployment/systemd/rust-litellm-gateway.service /etc/systemd/system/
sudo systemctl enable rust-litellm-gateway
sudo systemctl start rust-litellm-gateway
```

### 4. Kubernetes
```bash
# Deploy to K8s
kubectl apply -f deployment/kubernetes/
```

#### Horizontal pod autoscaling

`kubernetes/hpa.yaml` targets the `apps/v1` Deployment `litellm-gateway` in
the `litellm-gateway` namespace using `autoscaling/v2`. It maintains 3–10 replicas,
with average CPU utilization of 70% and memory utilization of 80%. Utilization is
relative to requests, not limits: the existing `250m` CPU and `256Mi` memory
requests give targets of `175m` and `204.8Mi` per pod. Keep both requests set on
every container, including any added sidecars. The HPA chooses the larger replica
recommendation from the two metrics.

Scale-up permits at most two additional pods per 60 seconds with no stabilization
delay. Scale-down uses a 300-second stabilization window and removes at most one
pod per 60 seconds. Tune these defaults for measured load and cluster capacity.

The cluster must run the HPA controller and expose CPU and memory resource metrics
through `metrics.k8s.io`, usually via [Metrics Server](https://github.com/kubernetes-sigs/metrics-server),
with API aggregation enabled and working kubelet metric collection. The gateway's
Prometheus `/metrics` endpoint does not supply these resource metrics. Missing
metrics prevent normal scaling; inspect HPA conditions and Metrics Server health.
See the [Kubernetes HPA documentation](https://kubernetes.io/docs/concepts/workloads/autoscaling/horizontal-pod-autoscale/).

The Deployment omits `spec.replicas` so subsequent applies respect HPA ownership.
New Deployments initially default to one replica until the HPA reconciles. If
deploying without the HPA, explicitly set the desired fixed replica count.

Validate all raw manifests without contacting a cluster using
[kubeconform](https://github.com/yannh/kubeconform) and Kubernetes 1.35.0 schemas
(the supported [1.35 release line](https://kubernetes.io/releases/)):

```bash
kubeconform -strict -summary -kubernetes-version 1.35.0 deployment/kubernetes/*.yaml
```

## 📋 Prerequisites

- **Rust 1.85+** for local builds
- **Docker & Docker Compose** for containerized deployment
- **PostgreSQL & Redis** for data storage
- **Kubernetes** for cluster deployment

## 🔧 Configuration

1. **Edit main config**: `config/gateway.yaml`
2. **Set environment variables** as needed
3. **Choose deployment method** from above

## 📚 Detailed Guides

- [Docker Deployment](docker/README.md)
- [Kubernetes Deployment](kubernetes/README.md)
- [Production Setup](scripts/README.md)

## 🆘 Troubleshooting

- Check logs: `journalctl -u rust-litellm-gateway -f`
- Verify config: `./deployment/scripts/start.sh`
- Test API: `curl http://localhost:8000/health`
