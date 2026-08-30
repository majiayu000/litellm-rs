#!/usr/bin/env python3
"""Refresh the embedded LiteLLM pricing catalog.

The gateway default pricing source embeds config/model_prices_extended.json.
This script imports LiteLLM's model_prices_and_context_window.json format
directly, validates the fields this crate relies on, and writes the bundled
catalog used at runtime.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import sys
import tempfile
from datetime import date, datetime, timezone
from pathlib import Path
from typing import Any
from urllib.error import URLError
from urllib.request import urlopen

from model_catalog_authority import build_catalog_authority


DEFAULT_SOURCE_COMMIT = "ec94a1f82aa9066dbf205773abf71595d3208388"
DEFAULT_SOURCE_URL = (
    "https://raw.githubusercontent.com/BerriAI/litellm/"
    f"{DEFAULT_SOURCE_COMMIT}/model_prices_and_context_window.json"
)
DEFAULT_OUTPUT = Path("config/model_prices_extended.json")
DEFAULT_CATALOG_DECISIONS = Path("config/model_catalog_decisions.json")
DEFAULT_CATALOG_AUTHORITY = Path("config/model_catalog_authority.json")
DEFAULT_MIN_MODELS = 2500
TOKEN_LIMIT_FIELDS = ("max_tokens", "max_input_tokens", "max_output_tokens")
SOURCE_COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SOURCE_URL_PATTERN = re.compile(
    r"^https://raw\.githubusercontent\.com/BerriAI/litellm/"
    r"(?P<commit>[0-9a-f]{40})/model_prices_and_context_window\.json$"
)
NON_MODEL_KEYS = frozenset(("_metadata", "fallback_generalizations", "sample_spec"))

MISTRAL_SOURCE = "https://docs.mistral.ai/inference/pricing"
COHERE_SOURCE = "https://docs.cohere.com/changelog/command-gets-refreshed"
GEMINI_SOURCE = "https://ai.google.dev/gemini-api/docs/pricing"
OPENAI_SOURCE = "https://developers.openai.com/api/docs/models/gpt-5.5-pro"
OPENAI_REALTIME_2_SOURCE = (
    "https://developers.openai.com/api/docs/models/gpt-realtime-2"
)
XAI_SOURCE = "https://docs.x.ai/developers/pricing"
GPT_PRO_TIER_FIELDS = (
    "input_cost_per_token_above_272k_tokens",
    "output_cost_per_token_above_272k_tokens",
    "cache_read_input_token_cost_above_272k_tokens",
)
GEMINI_BATCH_RATES = {
    "input_cost_per_token_batches": 0.000000375,
    "output_cost_per_token_batches": 0.000001875,
    "cache_read_input_token_cost_batches": 0.0000000375,
}
GEMINI_PROMO_VALID_THROUGH = date(2026, 12, 31)
GEMINI_PROMO_MODELS = frozenset(
    (
        "gemini-3.6-flash",
        "gemini-3.7-flash",
        "gemini/gemini-3.6-flash",
        "gemini/gemini-3.7-flash",
    )
)
OFFICIAL_OVERRIDE_REMOVALS = {
    "gpt-5.5-pro": GPT_PRO_TIER_FIELDS,
    "gpt-5.5-pro-2026-04-23": GPT_PRO_TIER_FIELDS,
}
FORBIDDEN_PRICING_FIELDS = OFFICIAL_OVERRIDE_REMOVALS

# These rows intentionally override upstream or compatibility-overlay values
# when an official first-party source is newer or more expressive. Keep this
# mapping narrow: upstream remains authoritative for every unlisted field/row.
OFFICIAL_OVERRIDE_PATCHES: dict[str, dict[str, Any]] = {
    "mistral-small-2603": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "cache_read_input_token_cost": 0.000000015,
        "source": MISTRAL_SOURCE,
    },
    "mistral-small-4": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "cache_read_input_token_cost": 0.000000015,
        "source": MISTRAL_SOURCE,
    },
    "mistral-small-latest": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "cache_read_input_token_cost": 0.000000015,
        "source": MISTRAL_SOURCE,
    },
    "mistral/mistral-small-2603": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "cache_read_input_token_cost": 0.000000015,
        "source": MISTRAL_SOURCE,
    },
    "mistral/mistral-small-latest": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "cache_read_input_token_cost": 0.000000015,
        "source": MISTRAL_SOURCE,
    },
    "command-r-08-2024": {
        "input_cost_per_token": 0.00000015,
        "output_cost_per_token": 0.00000060,
        "source": COHERE_SOURCE,
    },
    "command-r-plus-08-2024": {
        "input_cost_per_token": 0.0000025,
        "output_cost_per_token": 0.000010,
        "source": COHERE_SOURCE,
    },
    "gemini-3.6-flash": {
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
        **GEMINI_BATCH_RATES,
        "pricing_valid_through": "2026-12-31",
        "source": GEMINI_SOURCE,
    },
    "gemini-3.7-flash": {
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
        **GEMINI_BATCH_RATES,
        "pricing_valid_through": "2026-12-31",
        "source": GEMINI_SOURCE,
    },
    "gemini/gemini-3.6-flash": {
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
        **GEMINI_BATCH_RATES,
        "pricing_valid_through": "2026-12-31",
        "source": GEMINI_SOURCE,
    },
    "gemini/gemini-3.7-flash": {
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
        **GEMINI_BATCH_RATES,
        "pricing_valid_through": "2026-12-31",
        "source": GEMINI_SOURCE,
    },
    "gpt-5.5-pro": {
        "input_cost_per_token": 0.000030,
        "output_cost_per_token": 0.000180,
        "cache_read_input_token_cost": 0.000030,
        "source": OPENAI_SOURCE,
    },
    "gpt-5.5-pro-2026-04-23": {
        "input_cost_per_token": 0.000030,
        "output_cost_per_token": 0.000180,
        "cache_read_input_token_cost": 0.000030,
        "source": OPENAI_SOURCE,
    },
    "gpt-realtime-2": {
        "input_cost_per_token": 0.000004,
        "output_cost_per_token": 0.000024,
        "source": OPENAI_REALTIME_2_SOURCE,
    },
    # Both runtime tier consumers use `prompt_tokens > threshold`. The raw
    # 199999 threshold therefore encodes xAI's documented inclusive >=200k
    # boundary; the upstream 200k fields remain for source-schema fidelity.
    "xai/grok-4.5": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000003,
        "input_cost_per_token_above_199999_tokens": 0.000004,
        "output_cost_per_token_above_199999_tokens": 0.000012,
        "cache_read_input_token_cost_above_199999_tokens": 0.0000006,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.0000006,
        "source": XAI_SOURCE,
    },
    "xai/grok-4.5-latest": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000003,
        "input_cost_per_token_above_199999_tokens": 0.000004,
        "output_cost_per_token_above_199999_tokens": 0.000012,
        "cache_read_input_token_cost_above_199999_tokens": 0.0000006,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.0000006,
        "source": XAI_SOURCE,
    },
    "xai/grok-4.6": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000005,
        "input_cost_per_token_above_199999_tokens": 0.000004,
        "output_cost_per_token_above_199999_tokens": 0.000012,
        "cache_read_input_token_cost_above_199999_tokens": 0.000001,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.000001,
        "source": XAI_SOURCE,
    },
}

# Fail the update if official exact-ID, cache, or tier coverage disappears.
# This prevents a source-format change from silently degrading to scalar rates.
OFFICIAL_PRICING_CONTRACTS: dict[str, dict[str, Any]] = {
    "claude-fable-5": {
        "input_cost_per_token": 0.000010,
        "output_cost_per_token": 0.000050,
        "cache_read_input_token_cost": 0.000001,
        "cache_creation_input_token_cost": 0.0000125,
        "cache_creation_input_token_cost_above_1hr": 0.000020,
    },
    "claude-opus-5": {
        "input_cost_per_token": 0.000005,
        "output_cost_per_token": 0.000025,
        "cache_read_input_token_cost": 0.0000005,
        "cache_creation_input_token_cost": 0.00000625,
        "cache_creation_input_token_cost_above_1hr": 0.000010,
    },
    "claude-sonnet-5": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000010,
        "cache_read_input_token_cost": 0.0000002,
        "cache_creation_input_token_cost": 0.0000025,
        "cache_creation_input_token_cost_above_1hr": 0.000004,
    },
    "gemini/gemini-3.6-flash": {
        "litellm_provider": "gemini",
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
    },
    "gemini/gemini-3.7-flash": {
        "litellm_provider": "gemini",
        "input_cost_per_token": 0.00000075,
        "output_cost_per_token": 0.00000375,
        "cache_read_input_token_cost": 0.000000075,
    },
    "xai/grok-4.5": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000003,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.0000006,
    },
    "xai/grok-4.5-latest": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000003,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.0000006,
    },
    "xai/grok-4.6": {
        "input_cost_per_token": 0.000002,
        "output_cost_per_token": 0.000006,
        "cache_read_input_token_cost": 0.0000005,
        "input_cost_per_token_above_200k_tokens": 0.000004,
        "output_cost_per_token_above_200k_tokens": 0.000012,
        "cache_read_input_token_cost_above_200k_tokens": 0.000001,
    },
    "deepseek-v4-flash": {
        "input_cost_per_token": 0.00000022,
        "output_cost_per_token": 0.00000066,
        "cache_read_input_token_cost": 0.000000007,
    },
    "deepseek/deepseek-v4-flash": {
        "input_cost_per_token": 0.00000022,
        "output_cost_per_token": 0.00000066,
        "cache_read_input_token_cost": 0.000000007,
    },
}
for model, patch in OFFICIAL_OVERRIDE_PATCHES.items():
    OFFICIAL_PRICING_CONTRACTS.setdefault(model, {}).update(patch)


def is_metadata_key(key: str) -> bool:
    return key in NON_MODEL_KEYS


def validate_control_block(key: str, value: Any) -> None:
    if not isinstance(value, dict):
        raise SystemExit(f"control block {key!r} must be a JSON object")


def reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key!r}")
        result[key] = value
    return result


def parse_json(payload: str, source: str) -> dict[str, Any]:
    try:
        data = json.loads(payload, object_pairs_hook=reject_duplicate_keys)
    except (json.JSONDecodeError, ValueError) as error:
        raise SystemExit(f"failed to parse {source}: {error}") from error
    if not isinstance(data, dict):
        raise SystemExit(f"{source} must be a JSON object")
    return data


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise SystemExit(f"failed to read {path}: {error}") from error
    return parse_json(payload, str(path))


def validate_source_identity(source_url: str, source_commit: str) -> None:
    if not SOURCE_COMMIT_PATTERN.fullmatch(source_commit):
        raise SystemExit("source commit must be a full 40-character lowercase Git SHA")
    match = SOURCE_URL_PATTERN.fullmatch(source_url)
    if match is None:
        raise SystemExit("source URL must be an immutable raw GitHub URL")
    if match.group("commit") != source_commit:
        raise SystemExit("source URL commit does not match --source-commit")


def load_catalog_source_identity(path: Path) -> tuple[str, str]:
    """Load and validate the immutable upstream identity recorded by a catalog."""
    catalog = load_json(path)
    metadata = catalog.get("_metadata")
    if not isinstance(metadata, dict):
        raise SystemExit(f"{path} is missing object _metadata")
    source_url = metadata.get("source_url")
    source_commit = metadata.get("source_commit")
    if not isinstance(source_url, str) or not isinstance(source_commit, str):
        raise SystemExit(f"{path} is missing string source_url/source_commit metadata")
    validate_source_identity(source_url, source_commit)
    return source_url, source_commit


def load_url(url: str) -> tuple[dict[str, Any], str]:
    try:
        with urlopen(url, timeout=30) as response:
            payload_bytes = response.read()
    except (OSError, URLError) as error:
        raise SystemExit(f"failed to fetch {url}: {error}") from error
    try:
        payload = payload_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise SystemExit(f"failed to decode {url} as UTF-8: {error}") from error
    return parse_json(payload, f"pricing source {url}"), hashlib.sha256(payload_bytes).hexdigest()


def model_entries(data: dict[str, Any]) -> dict[str, dict[str, Any]]:
    entries: dict[str, dict[str, Any]] = {}
    for key, value in data.items():
        if is_metadata_key(key):
            validate_control_block(key, value)
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

        for field, value in entry.items():
            is_token_cost = (
                "cost_per_token" in field
                or field.endswith("_token_cost")
                or "_token_cost_" in field
            )
            if not is_token_cost:
                continue
            if (
                isinstance(value, bool)
                or not isinstance(value, (int, float))
                or not math.isfinite(value)
                or value < 0
            ):
                raise SystemExit(f"{key!r}.{field} must be a finite non-negative number")

        time_of_use = entry.get("time_of_use_pricing")
        if time_of_use is not None:
            validate_time_of_use_pricing(key, entry, time_of_use)


def validate_time_of_use_pricing(
    model: str, entry: dict[str, Any], time_of_use: Any
) -> None:
    if not isinstance(time_of_use, dict):
        raise SystemExit(f"{model!r}.time_of_use_pricing must be a JSON object")
    if time_of_use.get("timezone") != "UTC":
        raise SystemExit(f"{model!r}.time_of_use_pricing.timezone must be 'UTC'")
    windows = time_of_use.get("peak_windows")
    if not isinstance(windows, list) or not windows:
        raise SystemExit(f"{model!r}.time_of_use_pricing.peak_windows must be non-empty")
    for index, window in enumerate(windows):
        if not isinstance(window, dict):
            raise SystemExit(f"{model!r}.time_of_use_pricing.peak_windows[{index}] must be an object")
        weekdays = window.get("weekdays")
        start_hour = window.get("start_hour")
        end_hour = window.get("end_hour")
        if (
            not isinstance(weekdays, list)
            or not weekdays
            or any(isinstance(day, bool) or not isinstance(day, int) or day < 1 or day > 7 for day in weekdays)
        ):
            raise SystemExit(
                f"{model!r}.time_of_use_pricing.peak_windows[{index}].weekdays must use 1..7"
            )
        if (
            isinstance(start_hour, bool)
            or isinstance(end_hour, bool)
            or not isinstance(start_hour, int)
            or not isinstance(end_hour, int)
            or start_hour < 0
            or end_hour > 24
            or start_hour >= end_hour
        ):
            raise SystemExit(
                f"{model!r}.time_of_use_pricing.peak_windows[{index}] has invalid hours"
            )

    peak_rates = time_of_use.get("peak_rates")
    if not isinstance(peak_rates, dict):
        raise SystemExit(f"{model!r}.time_of_use_pricing.peak_rates must be an object")
    required_rates = [
        "input_cost_per_token",
        "output_cost_per_token",
        "cache_read_input_token_cost",
    ]
    for field in required_rates:
        value = peak_rates.get(field)
        if (
            isinstance(value, bool)
            or not isinstance(value, (int, float))
            or not math.isfinite(value)
            or value < 0
        ):
            raise SystemExit(
                f"{model!r}.time_of_use_pricing.peak_rates.{field} must be a finite non-negative number"
            )


def validate_contract_entry(
    model: str, entry: dict[str, Any], contract: dict[str, Any]
) -> None:
    for field, expected in contract.items():
        if field not in entry:
            raise SystemExit(f"official pricing contract {model!r} is missing {field}")
        if entry[field] != expected:
            raise SystemExit(
                f"official pricing contract {model!r}.{field} expected {expected!r}, "
                f"found {entry[field]!r}"
            )


def validate_official_contracts(
    entries: dict[str, dict[str, Any]], as_of_date: date | None = None
) -> None:
    as_of_date = as_of_date or datetime.now(timezone.utc).date()
    for model, contract in OFFICIAL_PRICING_CONTRACTS.items():
        entry = entries.get(model)
        if entry is None:
            raise SystemExit(f"official pricing contract is missing exact model ID {model!r}")
        if model not in GEMINI_PROMO_MODELS or as_of_date <= GEMINI_PROMO_VALID_THROUGH:
            validate_contract_entry(model, entry, contract)
        validate_forbidden_fields(model, entry)

    for model in ("deepseek-v4-flash", "deepseek/deepseek-v4-flash"):
        if "time_of_use_pricing" not in entries[model]:
            raise SystemExit(f"official pricing contract {model!r} is missing time_of_use_pricing")


def validate_forbidden_fields(model: str, entry: dict[str, Any]) -> None:
    for field in FORBIDDEN_PRICING_FIELDS.get(model, ()):
        if field in entry:
            raise SystemExit(f"official pricing contract {model!r} contains forbidden field {field}")


def load_overlay_entries(paths: list[Path]) -> dict[str, dict[str, Any]]:
    overlay: dict[str, dict[str, Any]] = {}
    for path in paths:
        if not path.exists():
            raise SystemExit(f"declared overlay file {path} does not exist")
        data = load_json(path)
        for key in NON_MODEL_KEYS & data.keys():
            validate_control_block(key, data[key])
        metadata = data.get("_metadata", {})
        if not isinstance(metadata, dict):
            raise SystemExit(f"_metadata in {path} must be a JSON object")
        overlay_keys = metadata.get("compatibility_overlay_keys")
        if overlay_keys is not None:
            if not isinstance(overlay_keys, list) or not all(
                isinstance(key, str) for key in overlay_keys
            ):
                raise SystemExit(
                    f"_metadata.compatibility_overlay_keys in {path} must be a string list"
                )
            key_filter = set(overlay_keys)
            if len(key_filter) != len(overlay_keys):
                raise SystemExit(
                    f"_metadata.compatibility_overlay_keys in {path} contains duplicates"
                )
            missing_keys = sorted(key_filter - set(data))
            if missing_keys:
                raise SystemExit(
                    f"overlay file {path} is missing declared keys: {', '.join(missing_keys)}"
                )
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


def apply_official_overrides(
    source_entries: dict[str, dict[str, Any]],
    overlay_entries: dict[str, dict[str, Any]],
    as_of_date: date | None = None,
) -> dict[str, dict[str, Any]]:
    as_of_date = as_of_date or datetime.now(timezone.utc).date()
    patched = {model: dict(entry) for model, entry in overlay_entries.items()}
    for model, patch in OFFICIAL_OVERRIDE_PATCHES.items():
        promo_expired = (
            model in GEMINI_PROMO_MODELS and as_of_date > GEMINI_PROMO_VALID_THROUGH
        )
        overlay = patched.get(model)
        overlay_has_promo_signature = overlay is not None and all(
            overlay.get(field) == expected for field, expected in patch.items()
        )
        original = overlay if overlay is not None else source_entries.get(model)
        if promo_expired and overlay_has_promo_signature:
            original = source_entries.get(model)
        if original is None:
            raise SystemExit(
                f"official override target {model!r} is missing from source and compatibility overlay"
            )
        entry = dict(original)
        for field in OFFICIAL_OVERRIDE_REMOVALS.get(model, ()):
            entry.pop(field, None)
        if promo_expired and overlay_has_promo_signature:
            entry.pop("pricing_valid_through", None)
        elif not promo_expired:
            entry.update(patch)
        patched[model] = entry
    return patched


def render_catalog(
    source_data: dict[str, Any],
    source_entries: dict[str, dict[str, Any]],
    overlay_entries: dict[str, dict[str, Any]],
    source_url: str,
    source_commit: str,
    source_sha256: str,
) -> tuple[dict[str, Any], int]:
    overlay_override_count = len(set(source_entries) & set(overlay_entries))
    overlay_add_count = len(set(overlay_entries) - set(source_entries))
    data: dict[str, Any] = {
        "_metadata": {
            "source": "LiteLLM model_prices_and_context_window.json",
            "source_repo": "https://github.com/BerriAI/litellm",
            "source_url": source_url,
            "source_commit": source_commit,
            "source_sha256": source_sha256,
            "generated_by": "scripts/sync_litellm_pricing.py",
            "upstream_model_count": len(source_entries),
            "compatibility_overlay_count": len(overlay_entries),
            "compatibility_overlay_override_count": overlay_override_count,
            "compatibility_overlay_add_count": overlay_add_count,
            "compatibility_overlay_keys": sorted(overlay_entries),
        }
    }

    for key, value in source_data.items():
        if is_metadata_key(key):
            continue
        data[key] = value

    for key in sorted(overlay_entries):
        data[key] = overlay_entries[key]

    data["_metadata"]["total_model_count"] = len(source_entries) + overlay_add_count
    return data, len(overlay_entries)


def write_catalog(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(data, indent=2, ensure_ascii=False) + "\n"
    temporary_path: str | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="w", encoding="utf-8", dir=path.parent, prefix=f".{path.name}.", delete=False
        ) as temporary:
            temporary_path = temporary.name
            temporary.write(payload)
            temporary.flush()
            os.fsync(temporary.fileno())
        os.replace(temporary_path, path)
    except OSError as error:
        if temporary_path is not None:
            try:
                os.unlink(temporary_path)
            except FileNotFoundError:
                pass
        raise SystemExit(f"failed to atomically write {path}: {error}") from error


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-url")
    parser.add_argument("--source-commit")
    parser.add_argument(
        "--source-catalog",
        type=Path,
        help="derive the immutable source identity from committed catalog metadata",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--catalog-decisions", type=Path, default=DEFAULT_CATALOG_DECISIONS
    )
    parser.add_argument(
        "--catalog-authority-output", type=Path, default=DEFAULT_CATALOG_AUTHORITY
    )
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
    as_of_date = datetime.now(timezone.utc).date()
    if args.source_catalog is not None:
        if not args.check:
            raise SystemExit("--source-catalog requires --check")
        if args.source_url is not None or args.source_commit is not None:
            raise SystemExit(
                "--source-catalog cannot be combined with --source-url/--source-commit"
            )
        source_url, source_commit = load_catalog_source_identity(args.source_catalog)
    elif args.source_url is None and args.source_commit is None:
        if args.check:
            raise SystemExit(
                "--check requires --source-catalog or an explicit source URL/commit"
            )
        source_url, source_commit = DEFAULT_SOURCE_URL, DEFAULT_SOURCE_COMMIT
    elif args.source_url is None or args.source_commit is None:
        raise SystemExit("--source-url and --source-commit must be provided together")
    else:
        source_url, source_commit = args.source_url, args.source_commit

    validate_source_identity(source_url, source_commit)
    source_data, source_sha256 = load_url(source_url)
    source_entries = model_entries(source_data)
    validate_entries(source_entries, args.min_models)
    overlay_paths = args.overlay_file or ([args.output] if args.output.exists() else [])
    overlay_entries = apply_official_overrides(
        source_entries, load_overlay_entries(overlay_paths), as_of_date
    )
    data, overlay_count = render_catalog(
        source_data,
        source_entries,
        overlay_entries,
        source_url,
        source_commit,
        source_sha256,
    )
    merged_entries = model_entries(data)
    validate_entries(merged_entries, args.min_models)
    validate_official_contracts(merged_entries, as_of_date)
    decisions = load_json(args.catalog_decisions)
    catalog_authority = build_catalog_authority(merged_entries, decisions)
    authority_metadata = catalog_authority["_metadata"]
    data["_metadata"].update(
        {
            "catalog_decision_revision": authority_metadata["revision"],
            "catalog_decision_source_sha256": authority_metadata[
                "decision_source_sha256"
            ],
            "catalog_authority_sha256": authority_metadata["classification_sha256"],
            "catalog_authority_entry_count": authority_metadata["total_entry_count"],
            "catalog_authority_enforced_providers": authority_metadata[
                "enforced_providers"
            ],
            "catalog_authority_provider_coverage": authority_metadata[
                "provider_coverage"
            ],
        }
    )

    if args.check:
        if not args.output.exists():
            print(f"{args.output} does not exist", file=sys.stderr)
            return 1
        current = load_json(args.output)
        if current != data:
            print(
                f"{args.output} is out of sync with {source_url}",
                file=sys.stderr,
            )
            return 1
        if not args.catalog_authority_output.exists():
            print(f"{args.catalog_authority_output} does not exist", file=sys.stderr)
            return 1
        current_authority = load_json(args.catalog_authority_output)
        if current_authority != catalog_authority:
            print(
                f"{args.catalog_authority_output} is out of sync with {args.catalog_decisions}",
                file=sys.stderr,
            )
            return 1
    else:
        write_catalog(args.catalog_authority_output, catalog_authority)
        write_catalog(args.output, data)

    print(
        (
            f"validated {len(source_entries)} upstream LiteLLM pricing entries "
            f"and {overlay_count} local compatibility entries from {source_url}"
            f"; classified {authority_metadata['total_entry_count']} exact pricing rows"
        ),
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
