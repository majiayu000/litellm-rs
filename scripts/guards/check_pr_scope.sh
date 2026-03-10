#!/usr/bin/env bash
# PR Scope Guard — prevents oversized PRs and file ownership conflicts
#
# Checks:
# 1. PR changes too many files (> threshold)
# 2. PR changes too many lines (> threshold)
# 3. PR touches files from multiple ownership domains (potential scope creep)
#
# Usage: ./scripts/guards/check_pr_scope.sh [base_branch]

set -euo pipefail

BASE="${1:-origin/main}"
MAX_FILES=15
MAX_LINES=800

# Colors
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "=== PR Scope Guard ==="

# Deepen fetch if merge base not found (shallow clone)
if ! git merge-base "$BASE" HEAD >/dev/null 2>&1; then
    echo "Shallow clone detected, fetching full history..."
    git fetch origin main --unshallow 2>/dev/null || git fetch origin main --deepen=100 2>/dev/null || true
    if ! git merge-base "$BASE" HEAD >/dev/null 2>&1; then
        echo "WARN: Cannot determine merge base — skipping scope check."
        exit 0
    fi
fi

# Count changed files (excluding auto-generated)
CHANGED_FILES=$(git diff --name-only "$BASE"...HEAD -- \
    ':!Cargo.lock' \
    ':!*.md' \
    ':!docs/**' \
    | wc -l | tr -d ' ')

# Count changed lines
CHANGED_LINES=$(git diff --stat "$BASE"...HEAD -- \
    ':!Cargo.lock' \
    ':!*.md' \
    ':!docs/**' \
    | tail -1 | grep -oE '[0-9]+ insertion|[0-9]+ deletion' \
    | grep -oE '[0-9]+' | paste -sd+ - | bc 2>/dev/null || echo "0")

echo "Files changed: $CHANGED_FILES (threshold: $MAX_FILES)"
echo "Lines changed: $CHANGED_LINES (threshold: $MAX_LINES)"

# Check file count
WARN=0
if [ "$CHANGED_FILES" -gt "$MAX_FILES" ]; then
    echo -e "${RED}FAIL: PR touches $CHANGED_FILES files (max $MAX_FILES)${NC}"
    echo "  Consider splitting into smaller, focused PRs."
    WARN=1
fi

# Check line count
if [ "$CHANGED_LINES" -gt "$MAX_LINES" ]; then
    echo -e "${RED}FAIL: PR changes $CHANGED_LINES lines (max $MAX_LINES)${NC}"
    echo "  Consider splitting into smaller, focused PRs."
    WARN=1
fi

# Check ownership domains — detect scope creep
DOMAINS=""
for file in $(git diff --name-only "$BASE"...HEAD -- ':!Cargo.lock' ':!*.md' ':!docs/**'); do
    case "$file" in
        src/auth/*) DOMAINS="$DOMAINS auth" ;;
        src/core/router/*) DOMAINS="$DOMAINS router" ;;
        src/core/cache/*) DOMAINS="$DOMAINS cache" ;;
        src/core/rate_limiter/*) DOMAINS="$DOMAINS rate_limiter" ;;
        src/core/providers/*) DOMAINS="$DOMAINS providers" ;;
        src/core/mcp/*) DOMAINS="$DOMAINS mcp" ;;
        src/core/a2a/*) DOMAINS="$DOMAINS a2a" ;;
        src/server/*) DOMAINS="$DOMAINS server" ;;
        src/storage/*) DOMAINS="$DOMAINS storage" ;;
        src/core/secret_managers/*) DOMAINS="$DOMAINS secrets" ;;
        tests/*) ;; # tests don't count as domain
        scripts/*) ;; # scripts don't count
    esac
done

UNIQUE_DOMAINS=$(echo "$DOMAINS" | tr ' ' '\n' | sort -u | grep -v '^$' | wc -l | tr -d ' ' || echo "0")
if [ "$UNIQUE_DOMAINS" -gt 3 ]; then
    echo -e "${YELLOW}WARN: PR touches $UNIQUE_DOMAINS ownership domains${NC}"
    echo "  Domains: $(echo "$DOMAINS" | tr ' ' '\n' | sort -u | grep -v '^$' | tr '\n' ', ')"
    echo "  This suggests scope creep — consider splitting by domain."
    WARN=1
fi

if [ "$WARN" -eq 0 ]; then
    echo "PASS: PR scope is within limits."
fi

# Exit with warning (non-blocking) — change to exit 1 for hard block
exit 0
