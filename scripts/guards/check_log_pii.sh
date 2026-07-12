#!/usr/bin/env bash
# Log PII guard.
#
# Blocks raw request/response body and session identifier logging in normal
# tracing/log macros.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

BODY_MAX_ALLOWED="${LITELLM_LOG_PII_BASELINE_MAX:-0}"
SESSION_IDENTIFIER_MAX_ALLOWED="${LITELLM_LOG_SESSION_IDENTIFIER_BASELINE_MAX:-0}"

if ! command -v rg >/dev/null 2>&1; then
    echo "Log PII guard failed: 'rg' is required." >&2
    exit 1
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "Log PII guard failed: 'python3' is required." >&2
    exit 1
fi

log_macro='(trace!|debug!|info!|warn!|error!|tracing::trace!|tracing::debug!|tracing::info!|tracing::warn!|tracing::error!)'
raw_body_names='(body_str|error_body|error_text|response_text|request_body|response_body)'
raw_body_value_names='(body|body_str|error_body|error_text|response_text|request_body|response_body)'
raw_body_value_suffix='(\.(as_str|clone|to_string)\(\))?'
raw_body_label='"[^"]*\b(request|response|error)?[ _-]?body\s*:\s*\{[^"]*"'
raw_named_arg="&?\s*${raw_body_names}\b${raw_body_value_suffix}\s*(,|\))"
raw_body_arg=",\s*&?\s*body\b${raw_body_value_suffix}\s*(,|\))"
raw_body_field="\b(body|request_body|response_body|error_body|error_text)\s*=\s*[%?]?\s*&?\s*${raw_body_value_names}\b${raw_body_value_suffix}\s*(,|\))"
raw_body_pattern="(?s)${log_macro}\\s*\\([^;]{0,1200}(${raw_body_label}|${raw_named_arg}|${raw_body_arg}|${raw_body_field})[^;]{0,1200};"
session_identifier_scanner="scripts/guards/check_log_session_identifiers.py"

scan_logs() {
    local pattern="$1"
    local output
    local status

    set +e
    output="$(rg -n -U --no-heading --color never \
        -g '*.rs' \
        "$pattern" \
        src/)"
    status=$?
    set -e

    if [[ "$status" -gt 1 ]]; then
        echo "Log PII guard failed while scanning source (rg exit $status)." >&2
        return "$status"
    fi

    printf '%s' "$output"
}

count_matches() {
    printf '%s\n' "$1" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' '
}

scan_session_identifiers() {
    local output
    local status

    set +e
    output="$(python3 "$session_identifier_scanner" src/)"
    status=$?
    set -e

    if [[ "$status" -ne 0 ]]; then
        echo "Log PII guard failed while scanning session identifiers (scanner exit $status)." >&2
        return "$status"
    fi

    printf '%s' "$output"
}

if ! python3 "$session_identifier_scanner" --self-test; then
    echo "Log PII guard failed: session identifier scanner self-test failed." >&2
    exit 1
fi

body_matches="$(scan_logs "$raw_body_pattern")"
session_identifier_matches="$(scan_session_identifiers)"
body_count="$(count_matches "$body_matches")"
session_identifier_count="$(count_matches "$session_identifier_matches")"

echo "Log PII guard: $body_count suspicious body-log hits (baseline max: $BODY_MAX_ALLOWED)."
echo "Log PII guard: $session_identifier_count session-identifier log hits (baseline max: $SESSION_IDENTIFIER_MAX_ALLOWED)."

if [[ "$body_count" -gt "$BODY_MAX_ALLOWED" ]]; then
    echo "FAIL: suspicious body-log count exceeds the allowed maximum."
    printf '%s\n' "$body_matches"
    echo
    echo "Avoid logging raw request/response bodies. Log request id, provider, model, status, size, or redacted snippets instead."
    exit 1
fi

if [[ "$body_count" -gt 0 ]]; then
    echo "WARN: suspicious body-log hits remain."
    printf '%s\n' "$body_matches"
fi

if [[ "$session_identifier_count" -gt "$SESSION_IDENTIFIER_MAX_ALLOWED" ]]; then
    echo "FAIL: session-identifier log count exceeds the allowed maximum."
    printf '%s\n' "$session_identifier_matches"
    echo
    echo "Log only the session event/outcome; never log session IDs, tokens, hashes, prefixes, suffixes, or lengths."
    exit 1
fi

if [[ "$session_identifier_count" -gt 0 ]]; then
    echo "WARN: session-identifier log hits remain."
    printf '%s\n' "$session_identifier_matches"
fi

exit 0
