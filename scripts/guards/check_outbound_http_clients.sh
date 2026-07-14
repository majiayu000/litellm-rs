#!/usr/bin/env bash
# Provider/runtime outbound HTTP boundary guard.
#
# The Rust AST guard scans all production provider and AI runtime route sources,
# validates exact path + violation + purpose exceptions, and runs red/green
# fixtures for every forbidden raw-client spelling.

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

cargo test provider_runtime_http_boundary_ --lib --locked -- --nocapture
