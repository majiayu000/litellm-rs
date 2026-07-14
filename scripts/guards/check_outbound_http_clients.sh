#!/usr/bin/env bash
# Outbound HTTP client guard.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

guard_tests="$(cargo test provider_runtime_http_boundary_ --lib --locked -- --list \
    | sed -n 's/: test$//p')"
guard_count="$(printf '%s\n' "$guard_tests" | sed '/^$/d' | wc -l | tr -d ' ')"
if [[ "$guard_count" != "3" ]]; then
    echo "expected exactly 3 provider/runtime HTTP boundary tests, found $guard_count" >&2
    exit 1
fi

while IFS= read -r guard_test; do
    [[ -n "$guard_test" ]] || continue
    cargo test "$guard_test" --lib --locked -- --exact --include-ignored --nocapture
done <<< "$guard_tests"
