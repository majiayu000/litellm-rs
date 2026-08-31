#!/usr/bin/env bash
set -euo pipefail

readonly OHA_VERSION="1.16.0"
readonly CONCURRENCY=64
readonly WARMUP_SECONDS=10
readonly DURATION_SECONDS=60
readonly BUILD_FLAGS=(--release --bin gateway)

repo_root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
output_path=${1:-}
if [[ -z "$output_path" ]]; then
  echo "usage: $0 <artifact.json>" >&2
  exit 2
fi

for command in cargo curl git jq oha python3 rustc; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required command is unavailable: $command" >&2
    exit 1
  fi
done

oha_version=$(oha --version)
if [[ "$oha_version" != *"$OHA_VERSION"* ]]; then
  echo "oha $OHA_VERSION is required; found: $oha_version" >&2
  exit 1
fi

if [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
  echo "benchmark evidence requires a clean Git worktree" >&2
  exit 1
fi

case "$(uname -s)" in
  Darwin)
    cpu_model=$(sysctl -n machdep.cpu.brand_string)
    logical_cpus=$(sysctl -n hw.logicalcpu)
    memory_bytes=$(sysctl -n hw.memsize)
    ;;
  Linux)
    cpu_model=$(lscpu | awk -F: '/Model name/ {sub(/^[[:space:]]+/, "", $2); print $2; exit}')
    logical_cpus=$(getconf _NPROCESSORS_ONLN)
    memory_bytes=$(awk '/MemTotal/ {print $2 * 1024; exit}' /proc/meminfo)
    ;;
  *)
    echo "unsupported benchmark host OS: $(uname -s)" >&2
    exit 1
    ;;
esac
if [[ -z "$cpu_model" || -z "$logical_cpus" || -z "$memory_bytes" ]]; then
  echo "failed to collect required hardware metadata" >&2
  exit 1
fi

tmp_dir=$(mktemp -d)
mock_pid=""
gateway_pid=""
cleanup() {
  if [[ -n "$gateway_pid" ]]; then kill "$gateway_pid" 2>/dev/null || true; fi
  if [[ -n "$mock_pid" ]]; then kill "$mock_pid" 2>/dev/null || true; fi
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

readonly request_body='{"model":"benchmark-model","messages":[{"role":"user","content":"ping"}],"stream":false}'
request_file="$tmp_dir/request.json"
printf '%s' "$request_body" >"$request_file"

cd "$repo_root"
cargo build "${BUILD_FLAGS[@]}"
target/release/gateway --config scripts/bench/gateway-overhead.yaml validate-config

python3 scripts/bench/mock_openai.py >"$tmp_dir/mock.log" 2>&1 &
mock_pid=$!
for _ in {1..100}; do
  if curl --fail --silent http://127.0.0.1:18000/health >/dev/null; then break; fi
  sleep 0.05
done
if ! curl --fail --silent http://127.0.0.1:18000/health >/dev/null; then
  echo "mock upstream failed to start; log follows" >&2
  sed -n '1,120p' "$tmp_dir/mock.log" >&2
  exit 1
fi

target/release/gateway \
  --config scripts/bench/gateway-overhead.yaml \
  --log-level error serve >"$tmp_dir/gateway.log" 2>&1 &
gateway_pid=$!
for _ in {1..200}; do
  if curl --fail --silent http://127.0.0.1:18080/health >/dev/null; then break; fi
  sleep 0.05
done
if ! curl --fail --silent http://127.0.0.1:18080/health >/dev/null; then
  echo "gateway failed to start; log follows" >&2
  sed -n '1,160p' "$tmp_dir/gateway.log" >&2
  exit 1
fi

response_file="$tmp_dir/response.json"
curl --fail --silent \
  -H 'content-type: application/json' \
  --data-binary "@$request_file" \
  http://127.0.0.1:18080/v1/chat/completions >"$response_file"
jq -e '.choices[0].message.content == "pong"' "$response_file" >/dev/null

oha_args=(
  --no-tui
  -w
  --output-format json
  -m POST
  -H 'content-type: application/json'
  -d "$request_body"
  -c "$CONCURRENCY"
)

oha "${oha_args[@]}" \
  -z "${WARMUP_SECONDS}s" \
  http://127.0.0.1:18080/v1/chat/completions >"$tmp_dir/warmup.json"
jq -e '.summary.successRate == 1 and (.errorDistribution | length == 0)' \
  "$tmp_dir/warmup.json" >/dev/null

oha "${oha_args[@]}" \
  -z "${DURATION_SECONDS}s" \
  http://127.0.0.1:18080/v1/chat/completions >"$tmp_dir/oha.json"
jq -e '.summary.successRate == 1 and (.errorDistribution | length == 0)' \
  "$tmp_dir/oha.json" >/dev/null

mkdir -p "$(dirname "$output_path")"
jq -n \
  --arg captured_at "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
  --arg git_sha "$(git rev-parse HEAD)" \
  --arg os_name "$(uname -s)" \
  --arg os_release "$(uname -r)" \
  --arg architecture "$(uname -m)" \
  --arg cpu_model "$cpu_model" \
  --argjson logical_cpus "$logical_cpus" \
  --argjson memory_bytes "$memory_bytes" \
  --arg rust "$(rustc -Vv)" \
  --arg cargo "$(cargo -V)" \
  --arg oha "$oha_version" \
  --argjson concurrency "$CONCURRENCY" \
  --argjson warmup_seconds "$WARMUP_SECONDS" \
  --argjson duration_seconds "$DURATION_SECONDS" \
  --argjson request_bytes "$(wc -c <"$request_file" | tr -d ' ')" \
  --argjson response_bytes "$(wc -c <"$response_file" | tr -d ' ')" \
  --slurpfile raw "$tmp_dir/oha.json" \
  '{
    schema_version: 1,
    captured_at: $captured_at,
    source: {
      git_sha: $git_sha,
      git_dirty: false,
      build_flags: ["--release", "--bin", "gateway"]
    },
    environment: {
      hardware: {
        cpu_model: $cpu_model,
        logical_cpus: $logical_cpus,
        memory_bytes: $memory_bytes
      },
      os: {name: $os_name, release: $os_release, architecture: $architecture},
      rust: $rust,
      cargo: $cargo,
      oha: $oha
    },
    workload: {
      concurrency: $concurrency,
      warmup_seconds: $warmup_seconds,
      duration_seconds: $duration_seconds,
      request_bytes: $request_bytes,
      response_bytes: $response_bytes,
      protocol: "HTTP/1.1 keep-alive",
      route: "POST /v1/chat/completions",
      upstream: "deterministic local mock, fixed response, zero injected delay"
    },
    results: {
      requests_per_second: $raw[0].summary.requestsPerSec,
      latency_ms: {
        p50: ($raw[0].latencyPercentiles.p50 * 1000),
        p95: ($raw[0].latencyPercentiles.p95 * 1000),
        p99: ($raw[0].latencyPercentiles.p99 * 1000)
      },
      error_rate: (1 - $raw[0].summary.successRate)
    },
    oha_raw: $raw[0]
  }' >"$output_path"

jq -e '
  .source.git_sha and
  .environment.hardware.cpu_model and
  .environment.os.name and
  .environment.rust and
  .workload.request_bytes and
  .workload.response_bytes and
  .results.requests_per_second and
  .results.latency_ms.p50 and
  .results.latency_ms.p95 and
  .results.latency_ms.p99 and
  (.results.error_rate >= 0)
' "$output_path" >/dev/null

echo "wrote benchmark artifact: $output_path"
jq '.results' "$output_path"
