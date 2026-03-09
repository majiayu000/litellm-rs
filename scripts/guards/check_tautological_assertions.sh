#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

GUARDED_PATHS=(
  "src/server/tests.rs"
  "tests/integration/router_tests.rs"
  "src/core/cost/providers/anthropic.rs"
)

search_matches() {
  local pattern="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -n --no-heading --color never "$pattern" "${GUARDED_PATHS[@]}" || true
    return
  fi

  if command -v grep >/dev/null 2>&1; then
    grep -n -E -- "$pattern" "${GUARDED_PATHS[@]}" || true
    return
  fi

  echo "Tautology guard failed: neither 'rg' nor 'grep' is available in PATH." >&2
  exit 1
}

assert_no_match() {
  local title="$1"
  local pattern="$2"
  local matches

  matches="$(search_matches "$pattern")"
  if [[ -n "${matches//[[:space:]]/}" ]]; then
    echo "Tautology guard failed: ${title}"
    printf '%s\n' "$matches"
    exit 1
  fi
}

assert_no_match \
  "found assert!(x.is_some() || x.is_none()) pattern" \
  'assert![[:space:]]*\([[:space:]]*[^)]*\.is_some\(\)[[:space:]]*\|\|[[:space:]]*[^)]*\.is_none\(\)[[:space:]]*\)'

assert_no_match \
  "found assert!(x.is_ok() || x.is_err()) pattern" \
  'assert![[:space:]]*\([[:space:]]*[^)]*\.is_ok\(\)[[:space:]]*\|\|[[:space:]]*[^)]*\.is_err\(\)[[:space:]]*\)'

echo "Tautological assertion guard passed."
