#!/usr/bin/env bash
# PR Overlap Guard — detects when open PRs modify the same files
#
# Prevents the "7 PRs touching 14 shared files" problem.
# Run before creating a new PR to check for conflicts with existing PRs.
#
# Requires: gh CLI authenticated
# Usage: ./scripts/guards/check_pr_overlap.sh

set -euo pipefail

RED='\033[0;31m'
YELLOW='\033[1;33m'
GREEN='\033[0;32m'
NC='\033[0m'

echo "=== PR Overlap Guard ==="

# Get current branch's changed files
CURRENT_BRANCH=$(git branch --show-current)
CURRENT_FILES=$(git diff --name-only origin/main...HEAD 2>/dev/null | sort)

if [ -z "$CURRENT_FILES" ]; then
    echo "No changes detected on current branch."
    exit 0
fi

echo "Current branch: $CURRENT_BRANCH"
echo "Changed files: $(echo "$CURRENT_FILES" | wc -l | tr -d ' ')"
echo ""

# Get all open PRs
OPEN_PRS=$(gh pr list --state open --json number,title,headRefName --jq '.[] | "\(.number)|\(.headRefName)|\(.title)"' 2>/dev/null || echo "")

if [ -z "$OPEN_PRS" ]; then
    echo -e "${GREEN}No open PRs — no overlap possible.${NC}"
    exit 0
fi

OVERLAP_FOUND=0

while IFS='|' read -r PR_NUM PR_BRANCH PR_TITLE; do
    # Skip current branch
    if [ "$PR_BRANCH" = "$CURRENT_BRANCH" ]; then
        continue
    fi

    # Get PR's changed files
    PR_FILES=$(gh pr diff "$PR_NUM" --name-only 2>/dev/null | sort)

    # Find overlap
    COMMON=$(comm -12 <(echo "$CURRENT_FILES") <(echo "$PR_FILES") 2>/dev/null | grep -v '^$' || true)

    if [ -n "$COMMON" ]; then
        OVERLAP_COUNT=$(echo "$COMMON" | wc -l | tr -d ' ')
        echo -e "${YELLOW}OVERLAP with PR #$PR_NUM ($PR_TITLE):${NC}"
        echo "  $OVERLAP_COUNT shared files:"
        echo "$COMMON" | head -5 | sed 's/^/    /'
        if [ "$OVERLAP_COUNT" -gt 5 ]; then
            echo "    ... and $((OVERLAP_COUNT - 5)) more"
        fi
        echo ""
        OVERLAP_FOUND=1
    fi
done <<< "$OPEN_PRS"

if [ "$OVERLAP_FOUND" -eq 1 ]; then
    echo -e "${RED}ACTION REQUIRED: File overlaps detected with open PRs.${NC}"
    echo "Options:"
    echo "  1. Coordinate merge order with the other PR authors"
    echo "  2. Consolidate overlapping PRs into one"
    echo "  3. Rebase after the conflicting PR is merged"
    exit 1
else
    echo -e "${GREEN}PASS: No file overlaps with open PRs.${NC}"
fi
