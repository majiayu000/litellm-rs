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
if [[ -e "$output_path" || -L "$output_path" ]]; then
  echo "benchmark artifact already exists: $output_path" >&2
  exit 1
fi

for command in cargo curl git jq oha python3 rustc; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "required command is unavailable: $command" >&2
    exit 1
  fi
done

oha_version=$(oha --version)
if [[ "$oha_version" != "oha $OHA_VERSION" ]]; then
  echo "exact oha version 'oha $OHA_VERSION' is required; found: $oha_version" >&2
  exit 1
fi

for variable in \
  RUSTFLAGS \
  RUSTDOCFLAGS \
  RUSTC_WRAPPER \
  RUSTC_WORKSPACE_WRAPPER \
  CARGO_ENCODED_RUSTFLAGS \
  CARGO_BUILD_RUSTFLAGS \
  CARGO_BUILD_RUSTC_WRAPPER \
  CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER \
  CARGO_BUILD_TARGET \
  CARGO_TARGET_DIR \
  CARGO_INCREMENTAL; do
  if [[ ${!variable+x} ]]; then
    echo "build override is not allowed: $variable" >&2
    exit 1
  fi
done
while IFS='=' read -r variable _; do
  case "$variable" in
    CARGO_PROFILE_RELEASE_* | CARGO_TARGET_*_RUSTFLAGS | CARGO_TARGET_*_LINKER)
      echo "build override is not allowed: $variable" >&2
      exit 1
      ;;
  esac
done < <(env)

cargo_home_dir=${CARGO_HOME:-${HOME:-}/.cargo}
for cargo_config in "$cargo_home_dir/config.toml" "$cargo_home_dir/config"; do
  if [[ -f "$cargo_config" ]]; then
    echo "external Cargo configuration is not allowed: $cargo_config" >&2
    exit 1
  fi
done
ancestor_dir=$(dirname "$repo_root")
while :; do
  for cargo_config in "$ancestor_dir/.cargo/config.toml" "$ancestor_dir/.cargo/config"; do
    if [[ -f "$cargo_config" ]]; then
      echo "external Cargo configuration is not allowed: $cargo_config" >&2
      exit 1
    fi
  done
  if [[ "$ancestor_dir" == / ]]; then
    break
  fi
  ancestor_dir=$(dirname "$ancestor_dir")
done

if [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
  echo "benchmark evidence requires a clean Git worktree" >&2
  exit 1
fi
source_git_sha=$(git -C "$repo_root" rev-parse HEAD)
readonly source_git_sha
python_version=$(python3 -VV 2>&1)
readonly python_version

verify_source_unchanged() {
  if [[ "$(git -C "$repo_root" rev-parse HEAD)" != "$source_git_sha" ]] ||
    [[ -n "$(git -C "$repo_root" status --porcelain)" ]]; then
    echo "Git HEAD or worktree changed after the benchmark source was captured" >&2
    return 1
  fi
}

require_port_available() {
  local port=$1
  if ! python3 - "$port" <<'PY'
import socket
import sys

with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
    listener.bind(("127.0.0.1", int(sys.argv[1])))
PY
  then
    echo "benchmark port is already in use: 127.0.0.1:$port" >&2
    return 1
  fi
}

wait_for_child_service() {
  local pid=$1
  local url=$2
  local log_path=$3
  local attempts=$4
  local label=$5
  local attempt

  for ((attempt = 0; attempt < attempts; attempt += 1)); do
    if ! kill -0 "$pid" 2>/dev/null; then
      echo "$label process exited before becoming ready; log follows" >&2
      sed -n '1,160p' "$log_path" >&2
      return 1
    fi
    if curl --fail --silent --connect-timeout 0.2 --max-time 0.5 "$url" >/dev/null; then
      if ! kill -0 "$pid" 2>/dev/null; then
        echo "$label process exited during its readiness check; log follows" >&2
        sed -n '1,160p' "$log_path" >&2
        return 1
      fi
      return 0
    fi
    sleep 0.05
  done

  echo "$label failed to become ready; log follows" >&2
  sed -n '1,160p' "$log_path" >&2
  return 1
}

case "$(uname -s)" in
  Darwin)
    cpu_model=$(sysctl -n machdep.cpu.brand_string)
    logical_cpus=$(sysctl -n hw.logicalcpu)
    memory_bytes=$(sysctl -n hw.memsize)
    ;;
  Linux)
    cpu_model=""
    if command -v lscpu >/dev/null 2>&1; then
      cpu_model=$(lscpu 2>/dev/null | awk -F: '/Model name/ {sub(/^[[:space:]]+/, "", $2); print $2; exit}' || true)
    fi
    if [[ -z "$cpu_model" && -r /proc/cpuinfo ]]; then
      cpu_model=$(awk -F: '
        tolower($1) ~ /model name|hardware/ {
          sub(/^[[:space:]]+/, "", $2); print $2; exit
        }
      ' /proc/cpuinfo)
    fi
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
artifact_tmp="$tmp_dir/artifact.json"
artifact_reserved=false
cleanup() {
  if [[ -n "$gateway_pid" ]]; then kill "$gateway_pid" 2>/dev/null || true; fi
  if [[ -n "$mock_pid" ]]; then kill "$mock_pid" 2>/dev/null || true; fi
  if [[ "$artifact_reserved" == true ]]; then rm -f "$output_path"; fi
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

readonly request_body='{"model":"benchmark-model","messages":[{"role":"user","content":"ping"}],"stream":false}'
request_file="$tmp_dir/request.json"
printf '%s' "$request_body" >"$request_file"

export CARGO_TARGET_DIR="$tmp_dir/cargo-target"
mkdir -p "$CARGO_TARGET_DIR"
gateway_bin="$CARGO_TARGET_DIR/release/gateway"

cd "$repo_root"
cargo build "${BUILD_FLAGS[@]}"
if [[ ! -x "$gateway_bin" ]]; then
  echo "benchmark gateway binary was not produced: $gateway_bin" >&2
  exit 1
fi
"$gateway_bin" --config scripts/bench/gateway-overhead.yaml validate-config

require_port_available 18000
python3 scripts/bench/mock_openai.py >"$tmp_dir/mock.log" 2>&1 &
mock_pid=$!
wait_for_child_service \
  "$mock_pid" http://127.0.0.1:18000/health "$tmp_dir/mock.log" 100 "mock upstream"

require_port_available 18080
"$gateway_bin" \
  --config scripts/bench/gateway-overhead.yaml \
  --log-level error serve >"$tmp_dir/gateway.log" 2>&1 &
gateway_pid=$!
wait_for_child_service \
  "$gateway_pid" http://127.0.0.1:18080/health "$tmp_dir/gateway.log" 200 gateway

response_file="$tmp_dir/response.json"
curl --fail --silent \
  --connect-timeout 1 \
  --max-time 5 \
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
jq -e '
  .summary.successRate == 1 and
  (.errorDistribution | length == 0) and
  (.statusCodeDistribution | keys == ["200"])
' \
  "$tmp_dir/warmup.json" >/dev/null

oha "${oha_args[@]}" \
  -z "${DURATION_SECONDS}s" \
  http://127.0.0.1:18080/v1/chat/completions >"$tmp_dir/oha.json"
jq -e '
  .summary.successRate == 1 and
  (.errorDistribution | length == 0) and
  (.statusCodeDistribution | keys == ["200"])
' \
  "$tmp_dir/oha.json" >/dev/null

artifact_dir=$(dirname "$output_path")
mkdir -p "$artifact_dir"
jq -n \
  --arg captured_at "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
  --arg git_sha "$source_git_sha" \
  --arg os_name "$(uname -s)" \
  --arg os_release "$(uname -r)" \
  --arg architecture "$(uname -m)" \
  --arg cpu_model "$cpu_model" \
  --argjson logical_cpus "$logical_cpus" \
  --argjson memory_bytes "$memory_bytes" \
  --arg rust "$(rustc -Vv)" \
  --arg cargo "$(cargo -V)" \
  --arg python "$python_version" \
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
      python: $python,
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
  }' >"$artifact_tmp"

jq -e '
  .source.git_sha and
  .environment.hardware.cpu_model and
  .environment.os.name and
  .environment.rust and
  .environment.python and
  .workload.request_bytes and
  .workload.response_bytes and
  .results.requests_per_second and
  .results.latency_ms.p50 and
  .results.latency_ms.p95 and
  .results.latency_ms.p99 and
  (.results.error_rate >= 0)
' "$artifact_tmp" >/dev/null

verify_source_unchanged
set -o noclobber
if ! exec 3>"$output_path"; then
  set +o noclobber
  echo "benchmark artifact already exists: $output_path" >&2
  exit 1
fi
set +o noclobber
artifact_reserved=true
if ! cat "$artifact_tmp" >&3; then
  exec 3>&-
  echo "failed to publish benchmark artifact: $output_path" >&2
  exit 1
fi
exec 3>&-
artifact_reserved=false

echo "wrote benchmark artifact: $output_path"
jq '.results' "$output_path"
