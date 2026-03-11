#!/bin/sh
# Diagnostic wrapper: logs full claude invocation to debug Harness integration
LOG="/tmp/harness-claude-debug.log"
echo "=== $(date) ===" >> "$LOG"
echo "ARGS: $*" >> "$LOG"
echo "ENV ANTHROPIC_API_KEY set: $([ -n "$ANTHROPIC_API_KEY" ] && echo yes || echo no)" >> "$LOG"
echo "ENV CLAUDE_API_KEY set: $([ -n "$CLAUDE_API_KEY" ] && echo yes || echo no)" >> "$LOG"
echo "PARENT PID: $PPID" >> "$LOG"
ps -p $PPID -o comm= >> "$LOG" 2>&1

/opt/homebrew/bin/claude "$@" 2>> "$LOG"
EXIT_CODE=$?
echo "EXIT CODE: $EXIT_CODE" >> "$LOG"
exit $EXIT_CODE
