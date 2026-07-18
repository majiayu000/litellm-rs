#!/usr/bin/env bash
# Sync vendored SpecRail gate scripts and schemas from an upstream specrail checkout.
#
# Usage:
#   scripts/sync-specrail-checks.sh /path/to/specrail   # copy files, rewrite lock
#   scripts/sync-specrail-checks.sh --verify            # check vendored files against lock
#
# Repo-specific Python checks are local-owned and explicitly excluded from
# upstream sync. Every tracked checks/*.py file and repo-local import must be
# classified as either upstream-managed or local-owned.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOCK_FILE="$REPO_ROOT/checks/specrail-sync.lock.json"

SYNCED_FILES=(
  "checks/duplicate_work_gate.py"
  "checks/github_duplicate_evidence.py"
  "checks/github_evidence_common.py"
  "checks/github_issue_evidence.py"
  "checks/github_issue_reference.py"
  "checks/github_pr_evidence.py"
  "checks/pr_gate.py"
  "checks/review_json_gate.py"
  "checks/route_gate.py"
  "checks/runtime_gate_rules.py"
  "checks/runtime_ledger_gate.py"
  "checks/specrail_lib.py"
  "schemas/duplicate_work_evidence.schema.json"
  "schemas/pr_review_gate.schema.json"
  "schemas/review_result.schema.json"
  "schemas/runtime_checkpoint.schema.json"
)

LOCAL_OWNED_FILES=(
  "checks/check_workflow.py"
  "checks/schema_contract.py"
)

write_lock() {
  local upstream_sha="$1"
  python3 - "$LOCK_FILE" "$upstream_sha" "${SYNCED_FILES[@]}" -- "${LOCAL_OWNED_FILES[@]}" <<'PY'
import hashlib
import json
import os
import sys

lock_path, upstream_sha, *classified = sys.argv[1:]
separator = classified.index("--")
files = classified[:separator]
excluded = classified[separator + 1:]
repo_root = os.path.dirname(os.path.dirname(os.path.abspath(lock_path)))
entries = []
for rel in files:
    path = os.path.join(repo_root, rel)
    digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
    entries.append({"path": rel, "sha256": digest})
lock = {
    "upstream_repo": "https://github.com/majiayu000/specrail",
    "upstream_sha": upstream_sha,
    "excluded": excluded,
    "files": entries,
}
with open(lock_path, "w") as fh:
    json.dump(lock, fh, indent=2)
    fh.write("\n")
print(f"lock written: {lock_path} @ upstream {upstream_sha}")
PY
}

verify_lock() {
  python3 - "$LOCK_FILE" "${SYNCED_FILES[@]}" -- "${LOCAL_OWNED_FILES[@]}" <<'PY'
import hashlib
import json
import os
import sys

lock_path, *classified = sys.argv[1:]
separator = classified.index("--")
expected_files = classified[:separator]
expected_excluded = classified[separator + 1:]
repo_root = os.path.dirname(os.path.dirname(os.path.abspath(lock_path)))
with open(lock_path) as fh:
    lock = json.load(fh)
failed = False
entries = lock.get("files")
if not isinstance(entries, list):
    print("INVALID: lock files must be a list")
    sys.exit(1)
lock_files = [entry.get("path") for entry in entries if isinstance(entry, dict)]
if len(lock_files) != len(entries) or lock_files != expected_files:
    print("INVALID: sync managed file list does not match lock")
    print(f"script: {expected_files}")
    print(f"lock:   {lock_files}")
    failed = True
excluded = lock.get("excluded")
if excluded != expected_excluded:
    print("INVALID: local-owned excluded file list does not match lock")
    print(f"script: {expected_excluded}")
    print(f"lock:   {excluded}")
    failed = True
for entry in entries:
    if not isinstance(entry, dict):
        continue
    path = os.path.join(repo_root, entry["path"])
    if not os.path.exists(path):
        print(f"MISSING: {entry['path']}")
        failed = True
        continue
    digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
    if digest != entry["sha256"]:
        print(f"DRIFT: {entry['path']}")
        failed = True
if failed:
    print(f"vendored SpecRail files drifted from lock (upstream {lock['upstream_sha']}); "
          "re-run scripts/sync-specrail-checks.sh <specrail-repo> or restore the files")
    sys.exit(1)
print(f"ok: {len(lock['files'])} upstream-managed files match lock "
      f"(upstream {lock['upstream_sha']})")
print(f"ok: {len(expected_excluded)} local-owned files explicitly excluded from upstream sync")
PY
}

verify_python_imports() {
  local tracking_mode="${1:-strict}"
  shift || true
  python3 - "$LOCK_FILE" "$REPO_ROOT" "$tracking_mode" "$@" <<'PY'
import ast
import importlib
import importlib.machinery
import json
import subprocess
import sys
from pathlib import Path

lock_path = Path(sys.argv[1])
repo_root = Path(sys.argv[2])
tracking_mode = sys.argv[3]
if tracking_mode not in {"strict", "allow-untracked-managed"}:
    print(f"INVALID TRACKING MODE: {tracking_mode}", file=sys.stderr)
    raise SystemExit(1)
allowed_untracked_managed = {Path(path) for path in sys.argv[4:]}
if tracking_mode == "strict" and allowed_untracked_managed:
    print("INVALID: strict tracking mode cannot allow untracked files", file=sys.stderr)
    raise SystemExit(1)
with lock_path.open(encoding="utf-8") as fh:
    lock = json.load(fh)

checks_dir = repo_root / "checks"
managed = {Path(entry["path"]) for entry in lock["files"]}
excluded = {Path(path) for path in lock["excluded"]}
unknown_allowed = sorted(allowed_untracked_managed - managed)
if unknown_allowed:
    for path in unknown_allowed:
        print(f"INVALID UNTRACKED MANAGED ALLOWANCE: {path}", file=sys.stderr)
    raise SystemExit(1)
managed_python = {path for path in managed if path.suffix == ".py"}
excluded_python = {path for path in excluded if path.suffix == ".py"}
classified_python = managed_python | excluded_python
managed_checks_python = {
    path for path in managed_python if path.parts and path.parts[0] == "checks"
}
excluded_checks_python = {
    path for path in excluded_python if path.parts and path.parts[0] == "checks"
}
classified_checks_python = managed_checks_python | excluded_checks_python

tracked = subprocess.run(
    ["git", "-C", str(repo_root), "ls-files"],
    capture_output=True,
    text=True,
    check=False,
)
if tracked.returncode != 0:
    print(f"TRACKING FAILED: {tracked.stderr.strip()}", file=sys.stderr)
    raise SystemExit(1)
tracked_files = {Path(line) for line in tracked.stdout.splitlines() if line}
tracked_python = {path for path in tracked_files if path.suffix == ".py"}
tracked_checks_python = {
    path for path in tracked_python if path.parts and path.parts[0] == "checks"
}
unclassified = sorted(tracked_checks_python - classified_checks_python)
untracked_managed = sorted(managed - tracked_files)
untracked_excluded = sorted(excluded - tracked_files)
disallowed_untracked_managed = sorted(
    set(untracked_managed) - allowed_untracked_managed
)
if unclassified or untracked_excluded or disallowed_untracked_managed:
    for path in unclassified:
        print(f"UNCLASSIFIED TRACKED PYTHON: {path}", file=sys.stderr)
    for path in untracked_excluded:
        print(f"CLASSIFIED FILE IS NOT TRACKED: {path}", file=sys.stderr)
    for path in disallowed_untracked_managed:
        print(f"CLASSIFIED FILE IS NOT TRACKED: {path}", file=sys.stderr)
    raise SystemExit(1)

repo_root_resolved = repo_root.resolve()
checks_dir_resolved = checks_dir.resolve()


def candidate_paths(base):
    return (base.with_suffix(".py"), base / "__init__.py")


SOURCELESS_SUFFIXES = tuple(
    importlib.machinery.BYTECODE_SUFFIXES + importlib.machinery.EXTENSION_SUFFIXES
)


def reject_sourceless_candidates(module, base):
    # A repo-local import that resolves to bytecode or an extension module has
    # no .py source to classify, so it must fail closed instead of being
    # treated as an external import.
    source_py = base.with_suffix(".py")
    init_py = base / "__init__.py"
    for suffix in SOURCELESS_SUFFIXES:
        flat = base.with_name(base.name + suffix)
        if flat.is_file() and not source_py.is_file():
            print(f"SOURCELESS LOCAL IMPORT: {module} resolves to {flat}", file=sys.stderr)
            raise SystemExit(1)
        package_init = base / f"__init__{suffix}"
        if package_init.is_file() and not init_py.is_file():
            print(
                f"SOURCELESS LOCAL IMPORT: {module} resolves to {package_init}",
                file=sys.stderr,
            )
            raise SystemExit(1)


def existing_local_paths(module):
    parts = module.split(".")
    if not parts or any(not part or part in {".", ".."} for part in parts):
        return []
    bases = []
    if parts[0] == "checks":
        bases.append((checks_dir.joinpath(*parts[1:]), checks_dir_resolved))
    else:
        bases.append((checks_dir.joinpath(*parts), checks_dir_resolved))
        bases.append((repo_root.joinpath(*parts), repo_root_resolved))
    resolved_paths = []
    for base, allowed_root in bases:
        reject_sourceless_candidates(module, base)
        for candidate in candidate_paths(base):
            if not candidate.is_file():
                continue
            resolved = candidate.resolve()
            try:
                resolved.relative_to(allowed_root)
                relative = resolved.relative_to(repo_root_resolved)
            except ValueError:
                print(f"LOCAL IMPORT PATH ESCAPE: {candidate}", file=sys.stderr)
                raise SystemExit(1)
            if relative not in resolved_paths:
                resolved_paths.append(relative)
    return resolved_paths


def require_classified_import(source, module):
    for candidate in existing_local_paths(module):
        if candidate not in classified_python:
            print(
                f"UNCLASSIFIED LOCAL IMPORT: {source} imports {candidate}",
                file=sys.stderr,
            )
            raise SystemExit(1)


for relative_path in sorted(classified_checks_python):
    source_path = repo_root / relative_path
    try:
        tree = ast.parse(source_path.read_text(encoding="utf-8"), filename=str(relative_path))
    except (OSError, SyntaxError) as exc:
        print(f"AST FAILED: {relative_path}: {type(exc).__name__}: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
    importlib_aliases = {"importlib"}
    import_module_aliases = set()
    builtins_aliases = {"builtins"}
    builtin_import_aliases = set()
    sys_aliases = {"sys"}
    sys_path_aliases = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "importlib":
                    importlib_aliases.add(alias.asname or alias.name)
                elif alias.name == "builtins":
                    builtins_aliases.add(alias.asname or alias.name)
                elif alias.name == "sys":
                    sys_aliases.add(alias.asname or alias.name)
        elif isinstance(node, ast.ImportFrom) and not node.level:
            if node.module in {"importlib", "builtins"} and any(
                alias.name == "*" for alias in node.names
            ):
                print(
                    f"DYNAMIC IMPORT ALIAS: {relative_path}: star import of "
                    f"{node.module} cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if node.module == "importlib":
                for alias in node.names:
                    if alias.name == "import_module":
                        import_module_aliases.add(alias.asname or alias.name)
            elif node.module == "builtins":
                for alias in node.names:
                    if alias.name == "__import__":
                        builtin_import_aliases.add(alias.asname or alias.name)
            elif node.module == "sys":
                for alias in node.names:
                    if alias.name in {"path", "*"}:
                        sys_path_aliases.add(alias.asname or alias.name)

    # Fail closed on aliased dynamic-import callables and sys.path mutation:
    # only direct, literal importlib.import_module / __import__ calls are
    # classified above, so any other reference to those callables (assignment,
    # getattr base, star import) or any sys.path access escapes classification
    # and must be rejected before the check modules execute.
    if "*" in sys_path_aliases:
        print(
            f"UNSUPPORTED SYS PATH ACCESS: {relative_path}: "
            "star import of sys cannot be classified safely",
            file=sys.stderr,
        )
        raise SystemExit(1)
    dynamic_callable_names = import_module_aliases | builtin_import_aliases | {"__import__"}
    module_alias_names = importlib_aliases | builtins_aliases
    call_funcs = {id(node.func) for node in ast.walk(tree) if isinstance(node, ast.Call)}
    attribute_bases = {
        id(node.value) for node in ast.walk(tree) if isinstance(node, ast.Attribute)
    }
    allowed_module_refs = set()
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Attribute)
            and id(node) in call_funcs
            and node.attr in {"import_module", "__import__"}
            and isinstance(node.value, ast.Name)
            and node.value.id in module_alias_names
        ):
            allowed_module_refs.add(id(node.value))
    for node in ast.walk(tree):
        if isinstance(node, ast.Name):
            if node.id in dynamic_callable_names and id(node) not in call_funcs:
                print(
                    f"DYNAMIC IMPORT ALIAS: {relative_path}: {node.id} "
                    "referenced outside a direct classified call",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if node.id in module_alias_names and id(node) not in allowed_module_refs:
                print(
                    f"DYNAMIC IMPORT ALIAS: {relative_path}: {node.id} "
                    "referenced outside a direct classified call",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if node.id in sys_path_aliases:
                print(
                    f"UNSUPPORTED SYS PATH ACCESS: {relative_path}: {node.id} "
                    "import search path use cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if node.id in sys_aliases and id(node) not in attribute_bases:
                print(
                    f"UNSUPPORTED SYS PATH ACCESS: {relative_path}: {node.id} "
                    "module reference outside direct attribute access cannot "
                    "be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
        elif isinstance(node, ast.Attribute):
            if node.attr in {"import_module", "__import__"} and not (
                id(node) in call_funcs
                and isinstance(node.value, ast.Name)
                and node.value.id in module_alias_names
            ):
                print(
                    f"DYNAMIC IMPORT ALIAS: {relative_path}: attribute "
                    f"{node.attr} cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if (
                node.attr == "path"
                and isinstance(node.value, ast.Name)
                and node.value.id in sys_aliases
            ):
                print(
                    f"UNSUPPORTED SYS PATH ACCESS: {relative_path}: "
                    "sys.path use cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)

    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for alias in node.names:
                require_classified_import(relative_path, alias.name)
        elif isinstance(node, ast.ImportFrom):
            if node.level:
                reason = (
                    "LOCAL IMPORT PATH ESCAPE"
                    if node.level > 1
                    else "UNSUPPORTED RELATIVE LOCAL IMPORT"
                )
                print(
                    f"{reason}: {relative_path}: checks/ is a flat non-package layout",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            module = node.module or ""
            require_classified_import(relative_path, module)
            for alias in node.names:
                if alias.name != "*":
                    qualified = f"{module}.{alias.name}" if module else alias.name
                    require_classified_import(relative_path, qualified)
        elif isinstance(node, ast.Call):
            function = node.func
            dynamic_name = None
            if (
                isinstance(function, ast.Name)
                and function.id in ({"__import__"} | builtin_import_aliases)
            ):
                dynamic_name = "__import__"
            elif isinstance(function, ast.Name) and function.id in import_module_aliases:
                dynamic_name = "importlib.import_module"
            elif (
                isinstance(function, ast.Attribute)
                and function.attr == "import_module"
                and isinstance(function.value, ast.Name)
                and function.value.id in importlib_aliases
            ):
                dynamic_name = "importlib.import_module"
            elif (
                isinstance(function, ast.Attribute)
                and function.attr == "__import__"
                and isinstance(function.value, ast.Name)
                and function.value.id in builtins_aliases
            ):
                dynamic_name = "builtins.__import__"
            if dynamic_name is None:
                continue
            target = node.args[0] if node.args else next(
                (keyword.value for keyword in node.keywords if keyword.arg == "name"),
                None,
            )
            if not isinstance(target, ast.Constant) or not isinstance(target.value, str):
                print(
                    f"NON-LITERAL DYNAMIC IMPORT: {relative_path}: "
                    f"{dynamic_name} target cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            if target.value.startswith("."):
                print(
                    f"UNSUPPORTED RELATIVE LOCAL IMPORT: {relative_path}: "
                    "checks/ is a flat non-package layout",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            require_classified_import(relative_path, target.value)
            if dynamic_name == "importlib.import_module":
                continue
            level = node.args[4] if len(node.args) > 4 else next(
                (keyword.value for keyword in node.keywords if keyword.arg == "level"),
                None,
            )
            if level is not None:
                if not isinstance(level, ast.Constant) or not isinstance(level.value, int):
                    print(
                        f"NON-LITERAL DYNAMIC IMPORT: {relative_path}: "
                        f"{dynamic_name} level cannot be classified safely",
                        file=sys.stderr,
                    )
                    raise SystemExit(1)
                if level.value:
                    print(
                        f"UNSUPPORTED RELATIVE LOCAL IMPORT: {relative_path}: "
                        "checks/ is a flat non-package layout",
                        file=sys.stderr,
                    )
                    raise SystemExit(1)
            fromlist = node.args[3] if len(node.args) > 3 else next(
                (keyword.value for keyword in node.keywords if keyword.arg == "fromlist"),
                None,
            )
            if fromlist is None or (
                isinstance(fromlist, ast.Constant) and fromlist.value is None
            ):
                continue
            if not isinstance(fromlist, (ast.List, ast.Tuple)):
                print(
                    f"NON-LITERAL DYNAMIC IMPORT: {relative_path}: "
                    f"{dynamic_name} fromlist cannot be classified safely",
                    file=sys.stderr,
                )
                raise SystemExit(1)
            for element in fromlist.elts:
                if (
                    not isinstance(element, ast.Constant)
                    or not isinstance(element.value, str)
                    or element.value == "*"
                ):
                    print(
                        f"NON-LITERAL DYNAMIC IMPORT: {relative_path}: "
                        f"{dynamic_name} fromlist entry cannot be classified safely",
                        file=sys.stderr,
                    )
                    raise SystemExit(1)
                require_classified_import(
                    relative_path, f"{target.value}.{element.value}"
                )

sys.path.insert(0, str(repo_root))
sys.path.insert(0, str(checks_dir))
for relative_path in sorted(classified_checks_python):
    try:
        importlib.import_module(relative_path.stem)
    except BaseException as exc:
        print(
            f"IMPORT FAILED: {relative_path}: {type(exc).__name__}: {exc}",
            file=sys.stderr,
        )
        raise SystemExit(1) from exc

print(f"ok: {len(managed_python)} upstream-managed Python files classified")
print(f"ok: {len(excluded_python)} local-owned excluded Python files classified")
if untracked_managed:
    print(
        f"ok: {len(untracked_managed)} newly copied upstream-managed files "
        "pending tracking"
    )
print("ok: classified SpecRail Python import closure")
PY
}

verify_upstream_sources() {
  local upstream="$1"
  local upstream_sha="$2"
  local failed=0
  local rel
  local mode
  for rel in "${SYNCED_FILES[@]}"; do
    if ! git -C "$upstream" cat-file -e "${upstream_sha}:${rel}" 2>/dev/null; then
      echo "UPSTREAM HEAD DOES NOT TRACK: $rel" >&2
      failed=1
      continue
    fi
    mode="$(git -C "$upstream" ls-tree "$upstream_sha" -- "$rel" | awk 'NR==1{print $1}')"
    if [[ "$mode" != "100644" && "$mode" != "100755" ]]; then
      echo "UPSTREAM HEAD PATH IS NOT A REGULAR FILE (mode ${mode:-missing}): $rel" >&2
      failed=1
      continue
    fi
    if ! git -C "$upstream" diff --cached --quiet "$upstream_sha" -- "$rel"; then
      echo "UPSTREAM INDEX DRIFT: $rel" >&2
      failed=1
    fi
    if ! git -C "$upstream" diff --quiet -- "$rel"; then
      echo "UPSTREAM WORKTREE DRIFT: $rel" >&2
      failed=1
    fi
  done
  if [[ "$failed" -ne 0 ]]; then
    echo "error: synced files must match tracked content in upstream HEAD" >&2
    return 1
  fi
}

verify_workflow() {
  python3 "$REPO_ROOT/checks/check_workflow.py" --repo "$REPO_ROOT"
}

# Print SYNCED_FILES entries that are not present in the prior lock. Only these
# genuinely new sync entries may be untracked after a sync; a previously locked
# managed file that disappeared from the index (e.g. git rm --cached) must
# still fail closed as CLASSIFIED FILE IS NOT TRACKED.
new_sync_entries() {
  if [[ ! -f "$LOCK_FILE" ]]; then
    printf '%s\n' "${SYNCED_FILES[@]}"
    return
  fi
  python3 - "$LOCK_FILE" "${SYNCED_FILES[@]}" <<'PY'
import json
import sys

lock_path, *synced = sys.argv[1:]
with open(lock_path, encoding="utf-8") as fh:
    lock = json.load(fh)
entries = lock.get("files")
if not isinstance(entries, list):
    print("INVALID: lock files must be a list", file=sys.stderr)
    raise SystemExit(1)
prior = {entry.get("path") for entry in entries if isinstance(entry, dict)}
for rel in synced:
    if rel not in prior:
        print(rel)
PY
}

if [[ "${1:-}" == "--verify" ]]; then
  verify_lock
  verify_python_imports strict
  verify_workflow
  exit 0
fi

UPSTREAM="${1:?usage: sync-specrail-checks.sh /path/to/specrail | --verify}"
if [[ ! -d "$UPSTREAM/checks" ]]; then
  echo "error: $UPSTREAM does not look like a specrail checkout (no checks/)" >&2
  exit 1
fi

upstream_sha="$(git -C "$UPSTREAM" rev-parse --verify 'HEAD^{commit}')"
verify_upstream_sources "$UPSTREAM" "$upstream_sha"
new_entries_output="$(new_sync_entries)"
NEW_SYNC_ENTRIES=()
while IFS= read -r rel; do
  [[ -n "$rel" ]] && NEW_SYNC_ENTRIES+=("$rel")
done <<< "$new_entries_output"
for rel in "${SYNCED_FILES[@]}"; do
  # Copy the blob recorded at upstream_sha instead of the worktree path so a
  # symlinked or otherwise substituted worktree entry can never be vendored,
  # then restore the tree mode that shell redirection drops (100755 vs 100644;
  # verify_upstream_sources already rejected every other mode).
  git -C "$UPSTREAM" show "${upstream_sha}:${rel}" > "$REPO_ROOT/$rel"
  entry_mode="$(git -C "$UPSTREAM" ls-tree "$upstream_sha" -- "$rel" | awk 'NR==1{print $1}')"
  if [[ "$entry_mode" == "100755" ]]; then
    chmod 755 "$REPO_ROOT/$rel"
  else
    chmod 644 "$REPO_ROOT/$rel"
  fi
done
write_lock "$upstream_sha"
verify_lock
verify_python_imports allow-untracked-managed ${NEW_SYNC_ENTRIES[@]+"${NEW_SYNC_ENTRIES[@]}"}
verify_workflow
