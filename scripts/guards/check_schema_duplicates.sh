#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

SEARCH_PATHS=(
  "src/config"
  "src/core"
  "src/sdk"
)

search_matches() {
  local pattern="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -n --no-heading --color never "$pattern" "${SEARCH_PATHS[@]}" || true
    return
  fi

  if command -v grep >/dev/null 2>&1; then
    grep -R -n -E -- "$pattern" "${SEARCH_PATHS[@]}" || true
    return
  fi

  echo "Schema guard failed: neither 'rg' nor 'grep' is available in PATH." >&2
  exit 1
}

assert_single_definition() {
  local schema_name="$1"
  local pattern="$2"
  local expected_path="$3"
  local matches count actual_path

  matches="$(search_matches "$pattern")"
  count="$(printf '%s\n' "$matches" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"

  if [[ "$count" -ne 1 ]]; then
    echo "Schema guard failed: '$schema_name' expected exactly 1 definition, found $count."
    printf '%s\n' "$matches"
    exit 1
  fi

  actual_path="$(printf '%s\n' "$matches" | head -n 1 | cut -d: -f1)"
  if [[ "$actual_path" != "$expected_path" ]]; then
    echo "Schema guard failed: '$schema_name' must be defined in '$expected_path', found '$actual_path'."
    exit 1
  fi
}

assert_allowed_definition_set() {
  local schema_name="$1"
  local pattern="$2"
  shift 2

  local matches count actual_paths expected_paths

  matches="$(search_matches "$pattern")"
  count="$(printf '%s\n' "$matches" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
  if [[ "$count" -lt 1 ]]; then
    echo "Schema guard failed: '$schema_name' has no definition in guarded paths."
    exit 1
  fi

  actual_paths="$(printf '%s\n' "$matches" | sed '/^[[:space:]]*$/d' | cut -d: -f1 | sort -u)"
  expected_paths="$(printf '%s\n' "$@" | sort -u)"

  if [[ "$actual_paths" != "$expected_paths" ]]; then
    echo "Schema guard failed: '$schema_name' definitions are not in the allowed set."
    echo "Expected paths:"
    printf '%s\n' "$expected_paths"
    echo "Actual paths:"
    printf '%s\n' "$actual_paths"
    exit 1
  fi
}

# Canonical single-definition schemas
assert_single_definition "GatewayConfig" "pub[[:space:]]+struct[[:space:]]+GatewayConfig\\b" "src/config/models/gateway.rs"
assert_single_definition "ProviderConfig" "pub[[:space:]]+struct[[:space:]]+ProviderConfig\\b" "src/config/models/provider.rs"
assert_single_definition "GatewayRouterConfig" "pub[[:space:]]+struct[[:space:]]+GatewayRouterConfig\\b" "src/config/models/router.rs"
assert_single_definition "RouterConfig" "pub[[:space:]]+struct[[:space:]]+RouterConfig\\b" "src/core/router/config.rs"

# ProviderType is intentionally defined in core + sdk; no third location is allowed.
assert_allowed_definition_set \
  "ProviderType" \
  "pub[[:space:]]+enum[[:space:]]+ProviderType\\b" \
  "src/core/providers/provider_type.rs" \
  "src/sdk/config.rs"

echo "Schema duplication guard passed."
