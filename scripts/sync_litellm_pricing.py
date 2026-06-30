#!/usr/bin/env python3
"""Refresh the embedded LiteLLM pricing catalog.

The gateway default pricing source embeds config/model_prices_extended.json.
This script imports LiteLLM's model_prices_and_context_window.json format
directly, validates the fields this crate relies on, and writes the bundled
catalog used at runtime.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path
from typing import Any
from urllib.error import URLError
from urllib.request import urlopen


DEFAULT_SOURCE_COMMIT = "0ade44f4da6171e7222c8e4273c3703a6258f972"
DEFAULT_SOURCE_URL = (
    "https://raw.githubusercontent.com/BerriAI/litellm/"
    f"{DEFAULT_SOURCE_COMMIT}/model_prices_and_context_window.json"
)
DEFAULT_OUTPUT = Path("config/model_prices_extended.json")
DEFAULT_MIN_MODELS = 2500
TOKEN_LIMIT_FIELDS = ("max_tokens", "max_input_tokens", "max_output_tokens")


def is_metadata_key(key: str) -> bool:
    return key == "sample_spec" or key.startswith("_") or "example" in key


def load_url(url: str) -> dict[str, Any]:
    try:
        with urlopen(url, timeout=30) as response:
            payload = response.read().decode("utf-8")
    except URLError as error:
        raise SystemExit(f"failed to fetch {url}: {error}") from error

    data = json.loads(payload)
    if not isinstance(data, dict):
        raise SystemExit("pricing source must be a JSON object")
    return data


def model_entries(data: dict[str, Any]) -> dict[str, dict[str, Any]]:
    entries: dict[str, dict[str, Any]] = {}
    for key, value in data.items():
        if is_metadata_key(key):
            continue
        if not isinstance(value, dict):
            raise SystemExit(f"pricing entry {key!r} must be a JSON object")
        entries[key] = value
    return entries


def validate_entries(entries: dict[str, dict[str, Any]], min_models: int) -> None:
    if len(entries) < min_models:
        raise SystemExit(
            f"pricing catalog has {len(entries)} model entries; expected at least {min_models}"
        )

    for key, entry in entries.items():
        provider = entry.get("litellm_provider")
        if not isinstance(provider, str) or not provider:
            raise SystemExit(f"pricing entry {key!r} is missing litellm_provider")

        for field in TOKEN_LIMIT_FIELDS:
            value = entry.get(field)
            if value is None:
                continue
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise SystemExit(f"{key!r}.{field} must be a JSON number")
            if not math.isfinite(value) or value < 0 or int(value) != value:
                raise SystemExit(f"{key!r}.{field} must be a non-negative integer")
            if value > 2**32 - 1:
                raise SystemExit(f"{key!r}.{field} exceeds u32::MAX")


def load_overlay_entries(paths: list[Path]) -> dict[str, dict[str, Any]]:
    overlay: dict[str, dict[str, Any]] = {}
    for path in paths:
        if not path.exists():
            continue
        data = json.loads(path.read_text(encoding="utf-8"))
        if not isinstance(data, dict):
            raise SystemExit(f"overlay file {path} must be a JSON object")
        overlay_keys = data.get("_metadata", {}).get("compatibility_overlay_keys")
        if overlay_keys is not None:
            if not isinstance(overlay_keys, list) or not all(
                isinstance(key, str) for key in overlay_keys
            ):
                raise SystemExit(
                    f"_metadata.compatibility_overlay_keys in {path} must be a string list"
                )
            key_filter = set(overlay_keys)
        else:
            key_filter = None
        for key, value in data.items():
            if is_metadata_key(key):
                continue
            if key_filter is not None and key not in key_filter:
                continue
            if not isinstance(value, dict):
                raise SystemExit(f"overlay entry {key!r} in {path} must be a JSON object")
            overlay[key] = value
    return overlay


def render_catalog(
    source_data: dict[str, Any],
    source_entries: dict[str, dict[str, Any]],
    overlay_entries: dict[str, dict[str, Any]],
    source_url: str,
    source_commit: str,
) -> tuple[dict[str, Any], int]:
    overlay_override_count = len(set(source_entries) & set(overlay_entries))
    overlay_add_count = len(set(overlay_entries) - set(source_entries))
    data: dict[str, Any] = {
        "_metadata": {
            "source": "LiteLLM model_prices_and_context_window.json",
            "source_repo": "https://github.com/BerriAI/litellm",
            "source_url": source_url,
            "source_commit": source_commit,
            "generated_by": "scripts/sync_litellm_pricing.py",
            "upstream_model_count": len(source_entries),
            "compatibility_overlay_count": len(overlay_entries),
            "compatibility_overlay_override_count": overlay_override_count,
            "compatibility_overlay_add_count": overlay_add_count,
            "compatibility_overlay_keys": sorted(overlay_entries),
        }
    }

    for key, value in source_data.items():
        if key == "_metadata":
            continue
        data[key] = value

    for key in sorted(overlay_entries):
        data[key] = overlay_entries[key]

    data["_metadata"]["total_model_count"] = len(source_entries) + overlay_add_count
    return data, len(overlay_entries)


def write_catalog(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-url", default=DEFAULT_SOURCE_URL)
    parser.add_argument("--source-commit", default=DEFAULT_SOURCE_COMMIT)
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--overlay-file",
        action="append",
        type=Path,
        default=[],
        help=(
            "existing LiteLLM-format catalog whose local-only rows should be "
            "preserved as compatibility overlay"
        ),
    )
    parser.add_argument("--min-models", type=int, default=DEFAULT_MIN_MODELS)
    parser.add_argument(
        "--check",
        action="store_true",
        help="validate and compare without writing the output file",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    source_data = load_url(args.source_url)
    source_entries = model_entries(source_data)
    validate_entries(source_entries, args.min_models)
    overlay_paths = args.overlay_file or [args.output]
    overlay_entries = load_overlay_entries(overlay_paths)
    data, overlay_count = render_catalog(
        source_data,
        source_entries,
        overlay_entries,
        args.source_url,
        args.source_commit,
    )
    merged_entries = model_entries(data)
    validate_entries(merged_entries, args.min_models)

    if args.check:
        if not args.output.exists():
            print(f"{args.output} does not exist", file=sys.stderr)
            return 1
        current = json.loads(args.output.read_text(encoding="utf-8"))
        if current != data:
            print(
                f"{args.output} is out of sync with {args.source_url}",
                file=sys.stderr,
            )
            return 1
    else:
        write_catalog(args.output, data)

    print(
        (
            f"validated {len(source_entries)} upstream LiteLLM pricing entries "
            f"and {overlay_count} local compatibility entries from {args.source_url}"
        ),
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
