#!/usr/bin/env bash
# Log PII guard.
#
# Blocks raw request/response body logging in normal tracing/log macros.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

MAX_ALLOWED="${LITELLM_LOG_PII_BASELINE_MAX:-0}"

if ! command -v rg >/dev/null 2>&1; then
    echo "Log PII guard failed: 'rg' is required." >&2
    exit 1
fi

log_macro='(debug!|info!|warn!|error!|tracing::debug!|tracing::info!|tracing::warn!|tracing::error!)'
raw_body_names='(body_str|error_body|error_text|response_text|request_body|response_body)'
raw_body_value_names='(body|body_str|error_body|error_text|response_text|request_body|response_body)'
raw_body_value_suffix='(\.(as_str|clone|to_string)\(\))?'
raw_body_label='"[^"]*\b(request|response|error)?[ _-]?body\s*:\s*\{[^"]*"'
raw_named_arg="&?\s*${raw_body_names}\b${raw_body_value_suffix}\s*(,|\))"
raw_body_arg=",\s*&?\s*body\b${raw_body_value_suffix}\s*(,|\))"
raw_body_field="\b(body|request_body|response_body|error_body|error_text)\s*=\s*[%?]?\s*&?\s*${raw_body_value_names}\b${raw_body_value_suffix}\s*(,|\))"
pattern="(?s)${log_macro}\\s*\\([^;]{0,1200}(${raw_body_label}|${raw_named_arg}|${raw_body_arg}|${raw_body_field})[^;]{0,1200};"

matches="$(
    rg -n -U --no-heading --color never \
        -g '*.rs' \
        "$pattern" \
        src/ || true
)"

count="$(printf '%s\n' "$matches" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"

echo "Log PII guard: $count suspicious body-log hits (baseline max: $MAX_ALLOWED)."

if [[ "$count" -gt "$MAX_ALLOWED" ]]; then
    echo "FAIL: suspicious body-log count exceeds the allowed maximum."
    printf '%s\n' "$matches"
    echo
    echo "Avoid logging raw request/response bodies. Log request id, provider, model, status, size, or redacted snippets instead."
    exit 1
fi

if [[ "$count" -gt 0 ]]; then
    echo "WARN: suspicious body-log hits remain."
    printf '%s\n' "$matches"
fi

exit 0
