#!/usr/bin/env bash
# Log PII guard.
#
# Baseline-aware during the audit remediation campaign: C9 will remove the
# existing request/response body logs, then this threshold should be tightened.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

MAX_ALLOWED="${LITELLM_LOG_PII_BASELINE_MAX:-8}"

if ! command -v rg >/dev/null 2>&1; then
    echo "Log PII guard failed: 'rg' is required." >&2
    exit 1
fi

pattern='(debug!|info!|warn!|error!|tracing::debug!|tracing::info!|tracing::warn!|tracing::error!).*\b(body|body_str|error_body|response_text|request_body|response_body)\b'

matches="$(
    rg -n --no-heading --color never \
        -g '*.rs' \
        "$pattern" \
        src/ || true
)"

count="$(printf '%s\n' "$matches" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"

echo "Log PII guard: $count suspicious body-log hits (baseline max: $MAX_ALLOWED)."

if [[ "$count" -gt "$MAX_ALLOWED" ]]; then
    echo "FAIL: suspicious body-log count increased above the audit baseline."
    printf '%s\n' "$matches"
    echo
    echo "Avoid logging raw request/response bodies. Log request id, provider, model, status, size, or redacted snippets instead."
    exit 1
fi

if [[ "$count" -gt 0 ]]; then
    echo "WARN: existing C9/M12 logging debt remains. This guard currently blocks regressions, not the baseline debt."
    printf '%s\n' "$matches"
fi

exit 0
