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
mkdir -p "$ARTIFACT_DIR"
rm -f "$ARTIFACT_DIR/_SUCCESS"
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
} | tee "$ARTIFACT_DIR/node-version.log"

run_logged() {
  local label="$1"
  local command_dir="$2"
  shift 2
  local -a command=("$@")
  local log_path="$ARTIFACT_DIR/$label.log"

  {
    printf 'cwd: %s\ncommand:' "$command_dir"
    printf ' %q' "${command[@]}"
    printf '\n'
    (cd "$command_dir" && "${command[@]}")
  } 2>&1 | tee "$log_path"
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

const [
  artifactDir,
  head,
  startedAt,
  completedAt,
  nodeVersion,
  npmVersion,
  versionExit,
  npmCiExit,
  domTestExit,
  npmVersionExit,
] = process.argv.slice(2);
const root = process.cwd();
const relative = (value) => path.relative(root, value).replaceAll(path.sep, "/");
const sha256 = (value) =>
  crypto.createHash("sha256").update(fs.readFileSync(value)).digest("hex");
const trackedInputs = [
  ".gitignore",
  ".github/workflows/admin-dashboard-verification.yml",
  "scripts/verify-gh1067.sh",
  "tests/admin_dashboard/package.json",
  "tests/admin_dashboard/package-lock.json",
  "tests/admin_dashboard/admin_dashboard_dom.test.mjs",
  "src/server/routes/admin_dashboard/index.html",
  "src/server/routes/admin_dashboard/app.js",
];
const logs = ["node-version.log", "npm-ci.log", "admin_dashboard_dom.log"]
  .map((name) => path.join(artifactDir, name));
const files = [...trackedInputs.map((name) => path.join(root, name)), ...logs]
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
    jsdom: "29.1.1",
  },
  commands: [
    { cwd: ".", command: "node --version", exit_code: +versionExit, log: relative(logs[0]) },
    { cwd: "tests/admin_dashboard", command: "npm ci --ignore-scripts", exit_code: +npmCiExit, log: relative(logs[1]) },
    { cwd: ".", command: "node --test tests/admin_dashboard/admin_dashboard_dom.test.mjs", exit_code: +domTestExit, log: relative(logs[2]) },
  ],
  paths: files,
  evidence: {
    directory: relative(artifactDir),
    checksums: `${relative(artifactDir)}/SHA256SUMS`,
    success_marker: `${relative(artifactDir)}/_SUCCESS`,
  },
};
const manifestPath = path.join(artifactDir, "manifest.json");
fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
const checksumFiles = [manifestPath, ...logs].sort();
const sums = checksumFiles
  .map((name) => `${sha256(name)}  ${path.basename(name)}`)
  .join("\n");
fs.writeFileSync(path.join(artifactDir, "SHA256SUMS"), `${sums}\n`);
NODE
manifest_exit=$?

final_exit=0
for result in \
  "$version_exit" "$npm_version_exit" "$npm_ci_exit" "$dom_test_exit" "$manifest_exit"; do
  if [[ "$result" -ne 0 ]]; then
    final_exit=1
  fi
done
if [[ "$final_exit" -eq 0 ]]; then
  touch "$ARTIFACT_DIR/_SUCCESS"
  echo "GH-1067 verification passed: $ARTIFACT_DIR"
else
  echo "GH-1067 verification failed; _SUCCESS was not created: $ARTIFACT_DIR" >&2
fi
exit "$final_exit"
