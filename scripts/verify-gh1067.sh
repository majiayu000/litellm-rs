#!/usr/bin/env bash
set -uo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"
HEAD_SHA="$(git rev-parse HEAD)"
if [[ ! "$HEAD_SHA" =~ ^[0-9a-f]{40}$ ]]; then
  echo "Expected a 40-character Git HEAD SHA, got: $HEAD_SHA" >&2
  exit 2
fi
ARTIFACT_DIR="$ROOT_DIR/artifacts/logs/gh1067/$HEAD_SHA"
if ! mkdir -p "$ARTIFACT_DIR"; then
  echo "Failed to create evidence directory: $ARTIFACT_DIR" >&2
  exit 3
fi
if ! rm -f "$ARTIFACT_DIR/_SUCCESS"; then
  echo "Failed to remove prior _SUCCESS: $ARTIFACT_DIR/_SUCCESS" >&2
  exit 4
fi
if ! rm -f "$ARTIFACT_DIR/manifest.json" \
  "$ARTIFACT_DIR/admin_dashboard_dom.log" \
  "$ARTIFACT_DIR/checksums.sha256" \
  "$ARTIFACT_DIR/node-version.log" \
  "$ARTIFACT_DIR/npm-ci.log"; then
  echo "Failed to reset generated GH-1067 evidence files" >&2
  exit 5
fi
LOG_PATH="$ARTIFACT_DIR/admin_dashboard_dom.log"
if ! : >"$LOG_PATH"; then
  echo "Failed to create combined command log: $LOG_PATH" >&2
  exit 6
fi
STARTED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
NODE_VERSION_COMMAND=(node --version)
NPM_VERSION_COMMAND=(npm --version)
NODE_VERSION="$("${NODE_VERSION_COMMAND[@]}" 2>&1)"
node_command_exit=$?
NPM_VERSION="$("${NPM_VERSION_COMMAND[@]}" 2>&1)"
npm_version_exit=$?
version_exit=0
if [[ $node_command_exit -ne 0 || "$NODE_VERSION" != "v24.14.0" ]]; then
  version_exit=1
fi
{
  printf 'command: node --version\nactual: %s\nexpected: v24.14.0\n' "$NODE_VERSION"
  if [[ "$version_exit" -ne 0 ]]; then
    printf 'result: FAIL\n'
  else
    printf 'result: PASS\n'
  fi
} | tee -a "$LOG_PATH"
run_logged() {
  local label="$1"
  local command_dir="$2"
  shift 2
  local -a command=("$@")
  {
    printf '\n[%s]\ncwd: %s\ncommand:' "$label" "$command_dir"
    printf ' %q' "${command[@]}"
    printf '\n'
    (cd "$command_dir" && "${command[@]}")
  } 2>&1 | tee -a "$LOG_PATH"
  return "${PIPESTATUS[0]}"
}
npm_ci_command=(npm ci --ignore-scripts)
run_logged "npm-ci" "$ROOT_DIR/tests/admin_dashboard" "${npm_ci_command[@]}"
npm_ci_exit=$?
dom_test_command=(node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs)
run_logged "admin_dashboard_dom" "$ROOT_DIR" "${dom_test_command[@]}"
dom_test_exit=$?
COMPLETED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
node - "$ARTIFACT_DIR" "$HEAD_SHA" "$STARTED_AT" "$COMPLETED_AT" \
  "$NODE_VERSION" "$NPM_VERSION" "$version_exit" "$npm_ci_exit" \
  "$dom_test_exit" "$npm_version_exit" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");
const [artifactDir, head, startedAt, completedAt, nodeVersion, npmVersion,
  versionExit, npmCiExit, domTestExit, npmVersionExit] = process.argv.slice(2);
const root = process.cwd();
const relative = (value) => path.relative(root, value).replaceAll(path.sep, "/");
const sha256 = (value) =>
  crypto.createHash("sha256").update(fs.readFileSync(value)).digest("hex");
const trackedInputs = [
  ".gitignore", ".github/workflows/admin-dashboard-verification.yml",
  "scripts/verify-gh1067.sh", "tests/admin_dashboard/package.json",
  "tests/admin_dashboard/package-lock.json", "tests/admin_dashboard/admin_dashboard_dom.test.mjs",
  "src/server/routes/admin_dashboard/index.html", "src/server/routes/admin_dashboard/app.js",
  "src/server/routes/admin_dashboard/provider_health.js",
  "src/server/routes/admin_dashboard/routing_inventory.js",
];
const log = path.join(artifactDir, "admin_dashboard_dom.log");
const files = [...trackedInputs.map((name) => path.join(root, name)), log]
  .filter((name) => fs.existsSync(name))
  .map((name) => ({ path: relative(name), sha256: sha256(name) }));
const manifest = {
  issue: "GH-1067",
  head_sha: head,
  started_at_utc: startedAt,
  completed_at_utc: completedAt,
  tools: {
    node: { version: nodeVersion, expected: "v24.14.0", exit_code: +versionExit },
    npm: { version: npmVersion, version_exit_code: +npmVersionExit },
    jsdom: "29.1.1" },
  commands: [
    { cwd: ".", command: "node --version", exit_code: +versionExit, log: relative(log) },
    { cwd: "tests/admin_dashboard", command: "npm ci --ignore-scripts", exit_code: +npmCiExit, log: relative(log) },
    { cwd: ".", command: "node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs", exit_code: +domTestExit, log: relative(log) },
  ],
  paths: files,
  evidence: { directory: relative(artifactDir),
    checksums: `${relative(artifactDir)}/checksums.sha256`,
    success_marker: `${relative(artifactDir)}/_SUCCESS` },
};
const manifestPath = path.join(artifactDir, "manifest.json");
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
const checksumFiles = [manifestPath, log].sort();
const sums = checksumFiles.map((name) =>
  `${sha256(name)}  ${path.basename(name)}`).join("\n");
fs.writeFileSync(path.join(artifactDir, "checksums.sha256"), `${sums}\n`);
NODE
manifest_exit=$?
final_exit=0
for result in \
  "$version_exit" "$npm_version_exit" "$npm_ci_exit" "$dom_test_exit" "$manifest_exit"; do
  if [[ "$result" -ne 0 ]]; then
    final_exit=1
  fi
done
if [[ "$final_exit" -eq 0 ]] && ! touch "$ARTIFACT_DIR/_SUCCESS"; then
  echo "Failed to create final _SUCCESS marker" >&2
  final_exit=1
fi
evidence_file_count="$(find "$ARTIFACT_DIR" -mindepth 1 -maxdepth 1 \
  -type f | wc -l | tr -d ' ')"
if [[ "$final_exit" -eq 0 && "$evidence_file_count" -eq 4 ]]; then
  echo "GH-1067 verification passed: $ARTIFACT_DIR"
else
  rm -f "$ARTIFACT_DIR/_SUCCESS"
  if [[ "$evidence_file_count" -ne 4 ]]; then
    echo "Expected exactly four evidence files, found $evidence_file_count" >&2
  fi
  echo "GH-1067 verification failed; _SUCCESS was not created: $ARTIFACT_DIR" >&2
  final_exit=1
fi
exit "$final_exit"
