#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
chart=deployment/helm/litellm-rs
output=$(mktemp -d)
trap 'rm -rf "$output"' EXIT
for fixture in single multi hpa; do
  helm lint --strict "$chart" -f "$chart/ci/$fixture.yaml"
  helm template gateway "$chart" --namespace litellm-gateway \
    -f "$chart/ci/$fixture.yaml" > "$output/$fixture.yaml"
  kubeconform -strict -summary -kubernetes-version 1.35.0 "$output/$fixture.yaml"
done
