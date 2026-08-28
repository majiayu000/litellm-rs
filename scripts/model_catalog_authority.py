"""Strict, deterministic model catalog classification authority generation."""

from __future__ import annotations

import hashlib
import json
from collections import Counter, defaultdict
from typing import Any


CATALOG_SCHEMA_VERSION = 1
ENFORCED_CATALOG_PROVIDERS = frozenset(("openai", "azure", "azure_ai"))
NON_MODEL_KEYS = frozenset(("_metadata", "fallback_generalizations", "sample_spec"))
CATALOG_DECISIONS = frozenset(("callable", "pricing_only", "unreviewed"))
CATALOG_ENDPOINTS = frozenset(
    (
        "chat_completions",
        "responses",
        "embeddings",
        "image_generation",
        "image_edit",
        "image_variation",
        "audio_transcription",
        "audio_translation",
        "text_to_speech",
        "moderation",
        "rerank",
        "realtime",
        "video_generation",
    )
)
CATALOG_CAPABILITIES = frozenset(
    (
        "chat_completion",
        "chat_completion_stream",
        "embeddings",
        "image_generation",
        "image_edit",
        "image_variation",
        "audio_transcription",
        "audio_translation",
        "text_to_speech",
        "moderation",
        "rerank",
        "tool_calling",
        "function_calling",
        "code_execution",
        "file_upload",
        "fine_tuning",
        "batch_processing",
        "realtime_api",
        "gemini_generate_content",
    )
)
DOCUMENT_FIELDS = frozenset(
    (
        "schema_version",
        "revision",
        "sources",
        "provider_aliases",
        "enforced_providers",
        "entries",
    )
)
SOURCE_FIELDS = frozenset(("kind", "location", "reviewed_on", "revision", "sha256"))
COMMON_ENTRY_FIELDS = frozenset(
    ("provider", "pricing_key", "decision", "evidence_sources")
)
CALLABLE_FIELDS = COMMON_ENTRY_FIELDS | frozenset(
    (
        "catalog_model_id",
        "endpoints",
        "capabilities",
        "supported_parameters",
        "aliases",
    )
)
PRICING_ONLY_FIELDS = COMMON_ENTRY_FIELDS | frozenset(("reason",))


def _fail_unknown_fields(context: str, value: dict[str, Any], allowed: frozenset[str]) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise SystemExit(f"{context} has unknown fields: {', '.join(unknown)}")


def _non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str) or not value:
        raise SystemExit(f"{context} must be a non-empty string")
    return value


def _string_list(value: Any, context: str, *, non_empty: bool = False) -> list[str]:
    if not isinstance(value, list) or any(not isinstance(item, str) or not item for item in value):
        raise SystemExit(f"{context} must be a string list")
    if non_empty and not value:
        raise SystemExit(f"{context} must be non-empty")
    if len(value) != len(set(value)):
        raise SystemExit(f"{context} contains duplicates")
    return value


def _validate_sources(value: Any) -> dict[str, dict[str, str]]:
    if not isinstance(value, dict) or not value:
        raise SystemExit("catalog decisions sources must be a non-empty object")
    sources: dict[str, dict[str, str]] = {}
    for source_id, source in value.items():
        source_id = _non_empty_string(source_id, "catalog source id")
        if not isinstance(source, dict):
            raise SystemExit(f"catalog source {source_id!r} must be an object")
        _fail_unknown_fields(f"catalog source {source_id!r}", source, SOURCE_FIELDS)
        required = ("kind", "location", "reviewed_on", "revision")
        sources[source_id] = {
            field: _non_empty_string(source.get(field), f"catalog source {source_id!r}.{field}")
            for field in required
        }
        if "sha256" in source:
            digest = _non_empty_string(
                source["sha256"], f"catalog source {source_id!r}.sha256"
            )
            if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
                raise SystemExit(
                    f"catalog source {source_id!r}.sha256 must be lowercase hex"
                )
            sources[source_id]["sha256"] = digest
    return sources


def _validate_provider_aliases(value: Any) -> dict[str, list[str]]:
    if not isinstance(value, dict):
        raise SystemExit("provider_aliases must be an object")
    result: dict[str, list[str]] = {}
    owner: dict[str, str] = {}
    canonical = set(value)
    for provider, aliases in value.items():
        provider = _non_empty_string(provider, "canonical provider")
        aliases = _string_list(aliases, f"provider_aliases.{provider}")
        for alias in aliases:
            if alias in canonical:
                raise SystemExit(
                    f"provider alias collision: {alias!r} is also a canonical provider"
                )
            previous = owner.setdefault(alias, provider)
            if previous != provider:
                raise SystemExit(
                    f"provider alias collision: {alias!r} belongs to {previous!r} and {provider!r}"
                )
        result[provider] = sorted(aliases)
    return dict(sorted(result.items()))


def _validate_entry(
    value: Any,
    sources: dict[str, dict[str, str]],
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise SystemExit("catalog decision entry must be an object")
    decision = value.get("decision")
    if decision not in CATALOG_DECISIONS:
        raise SystemExit(f"catalog decision must be one of {sorted(CATALOG_DECISIONS)}")
    allowed = {
        "callable": CALLABLE_FIELDS,
        "pricing_only": PRICING_ONLY_FIELDS,
        "unreviewed": COMMON_ENTRY_FIELDS,
    }[decision]
    _fail_unknown_fields(f"catalog {decision} entry", value, allowed)

    provider = _non_empty_string(value.get("provider"), "catalog entry provider")
    pricing_key = _non_empty_string(value.get("pricing_key"), "catalog entry pricing_key")
    if pricing_key in NON_MODEL_KEYS:
        raise SystemExit(f"catalog decision cannot classify control key {pricing_key!r}")
    evidence = _string_list(
        value.get("evidence_sources"),
        f"catalog entry {provider}/{pricing_key} evidence_sources",
        non_empty=True,
    )
    missing_sources = sorted(set(evidence) - set(sources))
    if missing_sources:
        raise SystemExit(
            f"catalog entry {provider}/{pricing_key} has unknown evidence sources: "
            + ", ".join(missing_sources)
        )

    result: dict[str, Any] = {
        "provider": provider,
        "pricing_key": pricing_key,
        "decision": decision,
        "evidence_sources": sorted(evidence),
    }
    if decision == "unreviewed":
        return result
    if decision == "pricing_only":
        result["reason"] = _non_empty_string(
            value.get("reason"), f"pricing_only {provider}/{pricing_key}.reason"
        )
        return result

    result["catalog_model_id"] = _non_empty_string(
        value.get("catalog_model_id"), f"callable {provider}/{pricing_key}.catalog_model_id"
    )
    endpoints = _string_list(
        value.get("endpoints", []), f"callable {provider}/{pricing_key}.endpoints"
    )
    invalid_endpoints = sorted(set(endpoints) - CATALOG_ENDPOINTS)
    if invalid_endpoints:
        raise SystemExit(
            f"callable {provider}/{pricing_key}.endpoints contains unknown values: "
            + ", ".join(invalid_endpoints)
        )
    capabilities = _string_list(
        value.get("capabilities", []), f"callable {provider}/{pricing_key}.capabilities"
    )
    invalid_capabilities = sorted(set(capabilities) - CATALOG_CAPABILITIES)
    if invalid_capabilities:
        raise SystemExit(
            f"callable {provider}/{pricing_key}.capabilities contains unknown values: "
            + ", ".join(invalid_capabilities)
        )
    if endpoints:
        result["endpoints"] = sorted(endpoints)
    if capabilities:
        result["capabilities"] = sorted(capabilities)
    supported_parameters = _string_list(
        value.get("supported_parameters", []),
        f"callable {provider}/{pricing_key}.supported_parameters",
    )
    if supported_parameters:
        result["supported_parameters"] = sorted(supported_parameters)
    result["aliases"] = sorted(
        _string_list(value.get("aliases", []), f"callable {provider}/{pricing_key}.aliases")
    )
    return result


def _semantic_sha256(value: Any) -> str:
    payload = json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(payload.encode("utf-8")).hexdigest()


def build_catalog_authority(
    pricing_entries: dict[str, dict[str, Any]],
    decision_document: dict[str, Any],
) -> dict[str, Any]:
    """Validate a closed decision ledger and return canonical runtime authority data."""
    if not isinstance(decision_document, dict):
        raise SystemExit("catalog decisions must be a JSON object")
    _fail_unknown_fields("catalog decisions", decision_document, DOCUMENT_FIELDS)
    if decision_document.get("schema_version") != CATALOG_SCHEMA_VERSION:
        raise SystemExit(f"catalog decisions schema_version must be {CATALOG_SCHEMA_VERSION}")
    revision = _non_empty_string(decision_document.get("revision"), "catalog revision")
    sources = _validate_sources(decision_document.get("sources"))
    enforced_providers = _string_list(
        decision_document.get("enforced_providers"),
        "enforced_providers",
        non_empty=True,
    )
    if set(enforced_providers) != ENFORCED_CATALOG_PROVIDERS:
        raise SystemExit(
            "enforced_providers must be exactly: "
            + ", ".join(sorted(ENFORCED_CATALOG_PROVIDERS))
        )
    provider_aliases = _validate_provider_aliases(
        decision_document.get("provider_aliases", {})
    )
    raw_entries = decision_document.get("entries")
    if not isinstance(raw_entries, list):
        raise SystemExit("catalog decisions entries must be a list")

    entries: list[dict[str, Any]] = []
    by_tuple: dict[tuple[str, str], dict[str, Any]] = {}
    for raw_entry in raw_entries:
        entry = _validate_entry(raw_entry, sources)
        identity = (entry["provider"], entry["pricing_key"])
        if identity in by_tuple:
            raise SystemExit(
                f"duplicate classification for {identity[0]!r}/{identity[1]!r}"
            )
        by_tuple[identity] = entry
        entries.append(entry)

    price_tuples = {
        (entry.get("litellm_provider"), key)
        for key, entry in pricing_entries.items()
    }
    malformed_providers = sorted(
        key for provider, key in price_tuples if not isinstance(provider, str) or not provider
    )
    if malformed_providers:
        raise SystemExit(
            "pricing rows have invalid providers: " + ", ".join(malformed_providers)
        )
    decision_tuples = set(by_tuple)
    missing = sorted(price_tuples - decision_tuples)
    if missing:
        provider, key = missing[0]
        raise SystemExit(
            f"missing classification for {provider!r}/{key!r} ({len(missing)} missing)"
        )
    stale = sorted(decision_tuples - price_tuples)
    if stale:
        provider, key = stale[0]
        raise SystemExit(f"stale classification for {provider!r}/{key!r} ({len(stale)} stale)")

    catalog_owners: dict[tuple[str, str], str] = {}
    alias_owners: dict[tuple[str, str], str] = {}
    for entry in entries:
        if entry["decision"] != "callable":
            continue
        provider = entry["provider"]
        catalog_id = entry["catalog_model_id"]
        for candidate in (catalog_id, *entry["aliases"]):
            ledger_entry = by_tuple.get((provider, candidate))
            if ledger_entry is not None and ledger_entry["decision"] != "callable":
                raise SystemExit(
                    f"callable identity {provider!r}/{candidate!r} collides with "
                    f"non-callable {ledger_entry['decision']!r} pricing key"
                )
        identity = (provider, catalog_id)
        if identity in alias_owners:
            raise SystemExit(
                f"callable canonical ID collides with alias {provider!r}/{catalog_id!r}"
            )
        previous = catalog_owners.setdefault(identity, entry["pricing_key"])
        if previous != entry["pricing_key"]:
            raise SystemExit(
                f"callable catalog collision for {provider!r}/{catalog_id!r}: "
                f"{previous!r} and {entry['pricing_key']!r}"
            )
        for alias in entry["aliases"]:
            alias_identity = (provider, alias)
            if alias_identity in catalog_owners:
                raise SystemExit(f"callable alias collides with canonical ID {provider!r}/{alias!r}")
            alias_previous = alias_owners.setdefault(alias_identity, catalog_id)
            if alias_previous != catalog_id:
                raise SystemExit(
                    f"callable alias collision for {provider!r}/{alias!r}: "
                    f"{alias_previous!r} and {catalog_id!r}"
                )

    entries.sort(key=lambda entry: (entry["provider"], entry["pricing_key"]))
    coverage: dict[str, Counter[str]] = defaultdict(Counter)
    for entry in entries:
        coverage[entry["provider"]][entry["decision"]] += 1
    provider_coverage = {
        provider: {decision: counts.get(decision, 0) for decision in sorted(CATALOG_DECISIONS)}
        for provider, counts in sorted(coverage.items())
    }
    semantic = {
        "schema_version": CATALOG_SCHEMA_VERSION,
        "revision": revision,
        "enforced_providers": sorted(enforced_providers),
        "provider_aliases": provider_aliases,
        "entries": entries,
    }
    normalized_decisions = {
        **semantic,
        "sources": sources,
    }
    return {
        "_metadata": {
            "schema_version": CATALOG_SCHEMA_VERSION,
            "revision": revision,
            "decision_source_sha256": _semantic_sha256(normalized_decisions),
            "pricing_universe_sha256": _semantic_sha256(sorted(price_tuples)),
            "classification_sha256": _semantic_sha256(semantic),
            "total_entry_count": len(entries),
            "enforced_providers": sorted(enforced_providers),
            "provider_coverage": provider_coverage,
        },
        "provider_aliases": provider_aliases,
        "entries": entries,
    }
