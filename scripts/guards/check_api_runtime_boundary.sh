#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

GUARDED_PATHS=(
  "src/config"
  "src/sdk"
)

search_matches() {
  local pattern="$1"
  if command -v rg >/dev/null 2>&1; then
    rg -n --no-heading --color never -g '*.rs' "$pattern" "${GUARDED_PATHS[@]}" || true
    return
  fi

  if command -v grep >/dev/null 2>&1; then
    find "${GUARDED_PATHS[@]}" -type f -name '*.rs' -print0 \
      | xargs -0 grep -n -E -- "$pattern" || true
    return
  fi

  echo "Boundary guard failed: neither 'rg' nor 'grep' is available in PATH." >&2
  exit 1
}

assert_no_match() {
  local title="$1"
  local pattern="$2"
  local matches

  matches="$(search_matches "$pattern")"
  if [[ -n "${matches//[[:space:]]/}" ]]; then
    echo "Boundary guard failed: ${title}"
    printf '%s\n' "$matches"
    exit 1
  fi
}

assert_no_match \
  "config/sdk must not import runtime crates (actix-web/sea-orm/redis)" \
  '^[[:space:]]*(pub[[:space:]]+)?use[[:space:]]+(actix_web|sea_orm|redis)\b'

assert_no_match \
  "config/sdk must not depend on server/storage modules" \
  '^[[:space:]]*(pub[[:space:]]+)?use[[:space:]]+crate[[:space:]]*::[[:space:]]*(server|storage)\b'

assert_no_match \
  "config/sdk must not reference runtime crates via fully-qualified paths" \
  '^[[:space:]]*[^/"#*].*\b(actix_web|sea_orm|redis)[[:space:]]*::'

assert_no_match \
  "config/sdk must not reference server/storage modules via fully-qualified paths" \
  '^[[:space:]]*[^/"#*].*\bcrate[[:space:]]*::[[:space:]]*(server|storage)[[:space:]]*::'

echo "API/runtime boundary guard passed."
