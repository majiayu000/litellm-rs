#!/usr/bin/env python3
"""Regression tests for the deterministic LiteLLM pricing synchronizer."""

from __future__ import annotations

import importlib.util
import json
import math
import pathlib
import sys
import tempfile
import unittest


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "sync_litellm_pricing.py"
CATALOG_PATH = REPO_ROOT / "config" / "model_prices_extended.json"
CATALOG_DECISIONS_PATH = REPO_ROOT / "config" / "model_catalog_decisions.json"
CATALOG_AUTHORITY_PATH = REPO_ROOT / "config" / "model_catalog_authority.json"
CI_WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "ci.yml"
sys.path.insert(0, str(SCRIPT_PATH.parent))

spec = importlib.util.spec_from_file_location("sync_litellm_pricing", SCRIPT_PATH)
assert spec and spec.loader
sync = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = sync
spec.loader.exec_module(sync)


class SyncPricingTests(unittest.TestCase):
    def test_rejects_mutable_or_mismatched_source_identity(self) -> None:
        with self.assertRaisesRegex(SystemExit, "immutable raw GitHub URL"):
            sync.validate_source_identity(
                "https://raw.githubusercontent.com/BerriAI/litellm/staging/model_prices_and_context_window.json",
                "a" * 40,
            )

        with self.assertRaisesRegex(SystemExit, "does not match"):
            sync.validate_source_identity(
                "https://raw.githubusercontent.com/BerriAI/litellm/"
                + "b" * 40
                + "/model_prices_and_context_window.json",
                "a" * 40,
            )

    def test_load_json_rejects_duplicate_keys(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            path = pathlib.Path(temp_dir) / "duplicate.json"
            path.write_text('{"model": {}, "model": {}}', encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "duplicate JSON key"):
                sync.load_json(path)

    def test_overlay_key_validation_is_fail_closed(self) -> None:
        catalogs = (
            {
                "_metadata": {"compatibility_overlay_keys": ["local", "local"]},
                "local": {},
            },
            {
                "_metadata": {"compatibility_overlay_keys": ["missing"]},
                "local": {},
            },
        )
        for catalog in catalogs:
            with self.subTest(catalog=catalog), tempfile.TemporaryDirectory() as temp_dir:
                path = pathlib.Path(temp_dir) / "overlay.json"
                path.write_text(json.dumps(catalog), encoding="utf-8")
                with self.assertRaises(SystemExit):
                    sync.load_overlay_entries([path])

    def test_missing_declared_overlay_is_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            missing = pathlib.Path(temp_dir) / "missing-overlay.json"
            with self.assertRaisesRegex(SystemExit, "does not exist"):
                sync.load_overlay_entries([missing])

    def test_only_exact_control_blocks_are_filtered(self) -> None:
        source = {
            "_metadata": {"source": "upstream"},
            "fallback_generalizations": {"foo": "bar"},
            "sample_spec": {"litellm_provider": "sample"},
            "example-real-model": {"litellm_provider": "sample"},
            "model-with-example-suffix": {"litellm_provider": "sample"},
            "real-model": {
                "litellm_provider": "openai",
                "input_cost_per_token": 1.0,
                "output_cost_per_token": 2.0,
            },
        }
        rendered, _ = sync.render_catalog(
            source,
            sync.model_entries(source),
            {},
            "https://example.invalid",
            "a" * 40,
            "d" * 64,
        )
        self.assertNotIn("fallback_generalizations", rendered)
        self.assertNotIn("sample_spec", rendered)
        self.assertIn("example-real-model", rendered)
        self.assertIn("model-with-example-suffix", rendered)
        self.assertIn("real-model", rendered)
        self.assertEqual(rendered["_metadata"]["upstream_model_count"], 3)

    def test_malformed_control_block_is_not_silently_filtered(self) -> None:
        with self.assertRaisesRegex(SystemExit, "control block.*JSON object"):
            sync.model_entries({"sample_spec": []})

        with tempfile.TemporaryDirectory() as temp_dir:
            path = pathlib.Path(temp_dir) / "overlay.json"
            path.write_text(
                json.dumps(
                    {
                        "_metadata": {"compatibility_overlay_keys": []},
                        "fallback_generalizations": [],
                    }
                ),
                encoding="utf-8",
            )
            with self.assertRaisesRegex(SystemExit, "control block.*JSON object"):
                sync.load_overlay_entries([path])

    def test_official_overrides_win_and_are_tracked(self) -> None:
        source = {
            "gpt-5.5-pro": {
                "litellm_provider": "openai",
                "input_cost_per_token": 99.0,
                "output_cost_per_token": 99.0,
            }
        }
        overlay = {
            "gpt-5.5-pro": {
                "litellm_provider": "openai",
                "input_cost_per_token": 0.000030,
                "output_cost_per_token": 0.000180,
            }
        }
        rendered, _ = sync.render_catalog(
            source,
            sync.model_entries(source),
            overlay,
            "https://example.invalid",
            "a" * 40,
            "d" * 64,
        )
        self.assertEqual(rendered["gpt-5.5-pro"]["input_cost_per_token"], 0.000030)
        self.assertEqual(rendered["_metadata"]["compatibility_overlay_keys"], ["gpt-5.5-pro"])
        self.assertEqual(rendered["_metadata"]["source_sha256"], "d" * 64)


class PricingSchemaValidationTests(unittest.TestCase):
    def test_rejects_non_finite_or_negative_costs(self) -> None:
        for bad_value in (math.nan, math.inf, -0.1):
            with self.subTest(value=bad_value):
                with self.assertRaisesRegex(SystemExit, "finite non-negative"):
                    sync.validate_entries(
                        {
                            "bad-model": {
                                "litellm_provider": "test",
                                "input_cost_per_token": bad_value,
                            }
                        },
                        min_models=1,
                    )

    def test_rejects_incomplete_time_of_use_pricing(self) -> None:
        row = {
            "litellm_provider": "deepseek",
            "input_cost_per_token": 0.00000022,
            "output_cost_per_token": 0.00000066,
            "time_of_use_pricing": {
                "timezone": "UTC",
                "peak_windows": [{"weekdays": [1, 2, 3, 4, 5], "start_hour": 1, "end_hour": 4}],
                "peak_rates": {"input_cost_per_token": 0.00000044},
            },
        }
        with self.assertRaisesRegex(SystemExit, "peak_rates.*output_cost_per_token"):
            sync.validate_entries({"deepseek-v4-flash": row}, min_models=1)

    def test_required_tier_contract_rejects_partial_row(self) -> None:
        with self.assertRaisesRegex(SystemExit, "cache_read_input_token_cost_above_200k_tokens"):
            sync.validate_contract_entry(
                "xai/grok-4.5",
                {
                    "litellm_provider": "xai",
                    "input_cost_per_token": 0.000002,
                    "output_cost_per_token": 0.000006,
                    "cache_read_input_token_cost": 0.0000003,
                    "input_cost_per_token_above_200k_tokens": 0.000004,
                    "output_cost_per_token_above_200k_tokens": 0.000012,
                },
                sync.OFFICIAL_PRICING_CONTRACTS["xai/grok-4.5"],
            )

    def test_forbidden_runtime_fields_fail_contract_validation(self) -> None:
        with self.assertRaisesRegex(SystemExit, "forbidden.*above_272k"):
            sync.validate_forbidden_fields(
                "gpt-5.5-pro",
                {"input_cost_per_token_above_272k_tokens": 0.000060},
            )


class CatalogAuthorityTests(unittest.TestCase):
    def source(self) -> dict[str, object]:
        return {
            "review": {
                "kind": "issue_review",
                "location": "https://github.com/majiayu000/litellm-rs/issues/1241",
                "reviewed_on": "2026-08-28",
                "revision": "test-review-1",
            }
        }

    def decision(
        self,
        provider: str,
        key: str,
        decision: str,
        **fields: object,
    ) -> dict[str, object]:
        return {
            "provider": provider,
            "pricing_key": key,
            "decision": decision,
            "evidence_sources": ["review"],
            **fields,
        }

    def document(self, entries: list[dict[str, object]]) -> dict[str, object]:
        return {
            "schema_version": 1,
            "revision": "test-ledger-1",
            "sources": self.source(),
            "enforced_providers": ["openai", "azure", "azure_ai"],
            "provider_aliases": {
                "openai": [],
                "azure": ["azure-openai"],
                "azure_ai": ["azure-ai", "azureai"],
                "together_ai": ["together"],
            },
            "entries": entries,
        }

    def test_missing_and_duplicate_decisions_fail_closed(self) -> None:
        prices = {
            "known": {"litellm_provider": "other"},
            "second": {"litellm_provider": "other"},
        }
        missing = self.document([self.decision("other", "known", "unreviewed")])
        with self.assertRaisesRegex(SystemExit, "missing classification.*other.*second"):
            sync.build_catalog_authority(prices, missing)

        duplicate = self.document(
            [
                self.decision("other", "known", "unreviewed"),
                self.decision("other", "known", "pricing_only", reason="dimension_variant"),
                self.decision("other", "second", "unreviewed"),
            ]
        )
        with self.assertRaisesRegex(SystemExit, "duplicate classification.*other.*known"):
            sync.build_catalog_authority(prices, duplicate)

    def test_enforced_provider_unreviewed_is_explicit_and_reported(self) -> None:
        prices = {"gpt-test": {"litellm_provider": "openai"}}
        document = self.document(
            [self.decision("openai", "gpt-test", "unreviewed")]
        )
        authority = sync.build_catalog_authority(prices, document)
        self.assertEqual(authority["entries"][0]["decision"], "unreviewed")
        self.assertEqual(
            authority["_metadata"]["provider_coverage"]["openai"],
            {"callable": 0, "pricing_only": 0, "unreviewed": 1},
        )

    def test_callable_and_pricing_only_contracts_are_strict(self) -> None:
        prices = {
            "gpt-test": {"litellm_provider": "openai"},
            "openai/container": {"litellm_provider": "openai"},
        }
        malformed_callable = self.document(
            [
                self.decision(
                    "openai",
                    "gpt-test",
                    "callable",
                    catalog_model_id="gpt-test",
                    endpoints=["invented_endpoint"],
                    capabilities=["chat_completion"],
                    supported_parameters=["messages"],
                    aliases=[],
                ),
                self.decision(
                    "openai",
                    "openai/container",
                    "pricing_only",
                    reason="tool_or_session_charge",
                ),
            ]
        )
        with self.assertRaisesRegex(SystemExit, "callable.*endpoints.*unknown"):
            sync.build_catalog_authority(prices, malformed_callable)

        malformed_pricing = self.document(
            [
                self.decision(
                    "openai",
                    "gpt-test",
                    "callable",
                    catalog_model_id="gpt-test",
                    endpoints=["chat_completions"],
                    capabilities=["chat_completion"],
                    supported_parameters=["messages"],
                    aliases=[],
                ),
                self.decision(
                    "openai",
                    "openai/container",
                    "pricing_only",
                    reason="tool_or_session_charge",
                    capabilities=["chat_completion"],
                ),
            ]
        )
        with self.assertRaisesRegex(SystemExit, "pricing_only.*capabilities"):
            sync.build_catalog_authority(prices, malformed_pricing)

    def test_callable_aliases_are_required_by_the_strict_schema(self) -> None:
        prices = {"gpt-test": {"litellm_provider": "openai"}}
        missing_aliases = self.document(
            [
                self.decision(
                    "openai",
                    "gpt-test",
                    "callable",
                    catalog_model_id="gpt-test",
                )
            ]
        )
        with self.assertRaisesRegex(SystemExit, "aliases.*string list"):
            sync.build_catalog_authority(prices, missing_aliases)

    def test_callable_alias_cannot_repeat_its_own_exact_pricing_key(self) -> None:
        prices = {"gpt-test": {"litellm_provider": "openai"}}
        self_alias = self.document(
            [
                self.decision(
                    "openai",
                    "gpt-test",
                    "callable",
                    catalog_model_id="catalog-only",
                    aliases=["gpt-test"],
                )
            ]
        )
        with self.assertRaisesRegex(SystemExit, "alias.*own exact pricing row"):
            sync.build_catalog_authority(prices, self_alias)

    def test_exact_case_and_native_slash_keys_are_preserved_deterministically(self) -> None:
        prices = {
            "together_ai/BAAI/bge-base-en-v1.5": {
                "litellm_provider": "together_ai"
            },
            "together_ai/baai/bge-base-en-v1.5": {
                "litellm_provider": "together_ai"
            },
        }
        entries = [
            self.decision(
                "together_ai",
                key,
                "callable",
                catalog_model_id=key.removeprefix("together_ai/"),
                endpoints=["embeddings"],
                capabilities=["embeddings"],
                supported_parameters=["input"],
                aliases=[],
            )
            for key in reversed(list(prices))
        ]
        first = sync.build_catalog_authority(prices, self.document(entries))
        second = sync.build_catalog_authority(prices, self.document(list(reversed(entries))))
        self.assertEqual(first, second)
        self.assertEqual(
            [entry["catalog_model_id"] for entry in first["entries"]],
            ["BAAI/bge-base-en-v1.5", "baai/bge-base-en-v1.5"],
        )

    def test_unknown_fields_and_control_keys_fail(self) -> None:
        prices = {"known": {"litellm_provider": "other"}}
        unknown = self.decision("other", "known", "unreviewed", future=True)
        with self.assertRaisesRegex(SystemExit, "unknown fields.*future"):
            sync.build_catalog_authority(prices, self.document([unknown]))

        control = self.decision("other", "_metadata", "unreviewed")
        with self.assertRaisesRegex(SystemExit, "control key"):
            sync.build_catalog_authority(prices, self.document([control]))

    def test_alias_and_canonical_identity_collisions_fail_in_either_order(self) -> None:
        prices = {
            "first": {"litellm_provider": "openai"},
            "second": {"litellm_provider": "openai"},
        }
        entries = [
            self.decision(
                "openai",
                "first",
                "callable",
                catalog_model_id="first",
                aliases=["second"],
            ),
            self.decision(
                "openai",
                "second",
                "callable",
                catalog_model_id="second",
                aliases=[],
            ),
        ]
        for ordered in (entries, list(reversed(entries))):
            with self.subTest(ordered=ordered), self.assertRaisesRegex(
                SystemExit, "collid"
            ):
                sync.build_catalog_authority(prices, self.document(ordered))

    def test_callable_identities_cannot_upgrade_non_callable_exact_ledger_keys(self) -> None:
        for field in ("catalog_model_id", "aliases"):
            for target_decision in ("callable", "pricing_only", "unreviewed"):
                for target_key in ("restricted", "BAAI/restricted-model"):
                    with self.subTest(
                        field=field,
                        target_decision=target_decision,
                        target_key=target_key,
                    ):
                        prices = {
                            "callable-price": {"litellm_provider": "openai"},
                            target_key: {"litellm_provider": "openai"},
                        }
                        callable_fields: dict[str, object] = {
                            "catalog_model_id": "callable-model",
                            "aliases": [],
                        }
                        callable_fields[field] = (
                            [target_key] if field == "aliases" else target_key
                        )
                        target_fields = {
                            "callable": {
                                "catalog_model_id": f"target/{target_key}",
                                "aliases": [],
                            },
                            "pricing_only": {"reason": "non_callable_charge"},
                            "unreviewed": {},
                        }[target_decision]
                        document = self.document(
                            [
                                self.decision(
                                    "openai",
                                    "callable-price",
                                    "callable",
                                    **callable_fields,
                                ),
                                self.decision(
                                    "openai",
                                    target_key,
                                    target_decision,
                                    **target_fields,
                                ),
                            ]
                        )
                        with self.assertRaisesRegex(
                            SystemExit, "callable (?:identity|alias).*different pricing row"
                        ):
                            sync.build_catalog_authority(prices, document)

    def test_callable_identity_ledger_checks_are_provider_and_case_sensitive(self) -> None:
        prices = {
            "source-price": {"litellm_provider": "openai"},
            "restricted": {"litellm_provider": "other"},
            "Restricted": {"litellm_provider": "openai"},
            "callable-target": {"litellm_provider": "openai"},
        }
        document = self.document(
            [
                self.decision(
                    "openai",
                    "source-price",
                    "callable",
                    catalog_model_id="source-price",
                    aliases=["friendly-alias"],
                ),
                self.decision("other", "restricted", "unreviewed"),
                self.decision("openai", "Restricted", "unreviewed"),
                self.decision(
                    "openai",
                    "callable-target",
                    "callable",
                    catalog_model_id="target-model",
                    aliases=[],
                ),
            ]
        )

        authority = sync.build_catalog_authority(prices, document)
        self.assertEqual(len(authority["entries"]), 4)
        source = next(
            entry
            for entry in authority["entries"]
            if entry["pricing_key"] == "source-price"
        )
        self.assertEqual(source["aliases"], ["friendly-alias"])

    def test_repository_ledger_has_reviewed_target_counts_without_inferred_capabilities(self) -> None:
        prices = sync.model_entries(sync.load_json(CATALOG_PATH))
        decisions = sync.load_json(CATALOG_DECISIONS_PATH)
        authority = sync.build_catalog_authority(prices, decisions)

        target_counts = {"callable": 0, "pricing_only": 0, "unreviewed": 0}
        target_providers = {"openai", "azure", "azure_ai"}
        callable_with_explicit_contract = 0
        for entry in authority["entries"]:
            if entry["provider"] in target_providers:
                target_counts[entry["decision"]] += 1
            if entry["decision"] == "callable" and any(
                field in entry
                for field in ("endpoints", "capabilities", "supported_parameters")
            ):
                callable_with_explicit_contract += 1

        self.assertEqual(authority["_metadata"]["total_entry_count"], 3474)
        self.assertEqual(
            target_counts,
            {"callable": 179, "pricing_only": 293, "unreviewed": 87},
        )
        self.assertEqual(callable_with_explicit_contract, 0)
        historical = next(
            entry
            for entry in authority["entries"]
            if entry["provider"] == "openai"
            and entry["pricing_key"] == "chatgpt-4o-latest"
        )
        self.assertEqual(historical["decision"], "pricing_only")
        self.assertIn("removed", historical["reason"])
        source_id = historical["evidence_sources"][0]
        self.assertEqual(
            decisions["sources"][source_id]["location"],
            "https://developers.openai.com/api/docs/models/chatgpt-4o-latest",
        )

    def test_repository_authority_matches_pricing_set_and_embedded_digests(self) -> None:
        catalog = sync.load_json(CATALOG_PATH)
        prices = sync.model_entries(catalog)
        decisions = sync.load_json(CATALOG_DECISIONS_PATH)
        generated = sync.build_catalog_authority(prices, decisions)
        embedded = sync.load_json(CATALOG_AUTHORITY_PATH)

        self.assertEqual(generated, embedded)
        self.assertEqual(
            {(entry["provider"], entry["pricing_key"]) for entry in embedded["entries"]},
            {(row["litellm_provider"], key) for key, row in prices.items()},
        )
        metadata = catalog["_metadata"]
        authority_metadata = embedded["_metadata"]
        self.assertEqual(
            metadata["catalog_authority_sha256"],
            authority_metadata["classification_sha256"],
        )
        self.assertEqual(
            metadata["catalog_decision_source_sha256"],
            authority_metadata["decision_source_sha256"],
        )
        self.assertEqual(
            metadata["catalog_authority_entry_count"],
            authority_metadata["total_entry_count"],
        )

    def test_pull_request_ci_runs_catalog_authority_gates(self) -> None:
        workflow = CI_WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertIn(
            "python3 -m unittest discover -s scripts/test -p 'test_sync_litellm_pricing.py'",
            workflow,
        )
        self.assertIn("python3 scripts/sync_litellm_pricing.py --check", workflow)
        self.assertIn("ref: ${{ github.event.pull_request.head.sha }}", workflow)
        self.assertIn(
            'test "$(git rev-parse HEAD)" = "${EXPECTED_HEAD}"', workflow
        )


class OfficialPricingRegressionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.catalog = sync.load_json(CATALOG_PATH)

    def assert_fields(self, model: str, expected: dict[str, object]) -> None:
        row = self.catalog.get(model)
        self.assertIsInstance(row, dict, f"missing exact model ID: {model}")
        for field, value in expected.items():
            self.assertIn(field, row, f"{model} missing {field}")
            self.assertEqual(row[field], value, f"{model}.{field}")

    def test_issue_1211_claude_current_exact_ids(self) -> None:
        self.assert_fields(
            "claude-fable-5",
            {
                "input_cost_per_token": 0.000010,
                "output_cost_per_token": 0.000050,
                "cache_read_input_token_cost": 0.000001,
                "cache_creation_input_token_cost": 0.0000125,
                "cache_creation_input_token_cost_above_1hr": 0.000020,
            },
        )
        self.assert_fields(
            "claude-opus-5",
            {
                "input_cost_per_token": 0.000005,
                "output_cost_per_token": 0.000025,
                "cache_read_input_token_cost": 0.0000005,
                "cache_creation_input_token_cost": 0.00000625,
                "cache_creation_input_token_cost_above_1hr": 0.000010,
            },
        )
        self.assert_fields(
            "claude-sonnet-5",
            {
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000010,
                "cache_read_input_token_cost": 0.0000002,
                "cache_creation_input_token_cost": 0.0000025,
                "cache_creation_input_token_cost_above_1hr": 0.000004,
            },
        )

    def test_issue_1212_gemini_promo_exact_ids(self) -> None:
        expected = {
            "input_cost_per_token": 0.00000075,
            "output_cost_per_token": 0.00000375,
            "cache_read_input_token_cost": 0.000000075,
            "pricing_valid_through": "2026-12-31",
        }
        self.assert_fields("gemini/gemini-3.6-flash", expected | {"litellm_provider": "gemini"})
        self.assert_fields("gemini/gemini-3.7-flash", expected | {"litellm_provider": "gemini"})
        self.assert_fields("gemini-3.6-flash", expected)
        self.assert_fields("gemini-3.7-flash", expected)
        for model in (
            "gemini-3.6-flash",
            "gemini-3.7-flash",
            "gemini/gemini-3.6-flash",
            "gemini/gemini-3.7-flash",
        ):
            self.assertNotIn("input_cost_per_token_batches", self.catalog[model])
            self.assertNotIn("output_cost_per_token_batches", self.catalog[model])
            self.assertNotIn("cache_read_input_token_cost_batches", self.catalog[model])

    def test_issue_1213_xai_tiered_exact_ids(self) -> None:
        self.assert_fields(
            "xai/grok-4.5",
            {
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000006,
                "cache_read_input_token_cost": 0.0000003,
                "input_cost_per_token_above_200k_tokens": 0.000004,
                "output_cost_per_token_above_200k_tokens": 0.000012,
                "cache_read_input_token_cost_above_200k_tokens": 0.0000006,
                "input_cost_per_token_above_199999_tokens": 0.000004,
                "output_cost_per_token_above_199999_tokens": 0.000012,
                "cache_read_input_token_cost_above_199999_tokens": 0.0000006,
            },
        )
        self.assert_fields(
            "xai/grok-4.6",
            {
                "input_cost_per_token": 0.000002,
                "output_cost_per_token": 0.000006,
                "cache_read_input_token_cost": 0.0000005,
                "input_cost_per_token_above_200k_tokens": 0.000004,
                "output_cost_per_token_above_200k_tokens": 0.000012,
                "cache_read_input_token_cost_above_200k_tokens": 0.000001,
                "input_cost_per_token_above_199999_tokens": 0.000004,
                "output_cost_per_token_above_199999_tokens": 0.000012,
                "cache_read_input_token_cost_above_199999_tokens": 0.000001,
            },
        )

    def test_issue_1214_mistral_small_4_exact_ids(self) -> None:
        expected = {
            "input_cost_per_token": 0.00000015,
            "output_cost_per_token": 0.00000060,
            "cache_read_input_token_cost": 0.000000015,
        }
        self.assert_fields("mistral-small-2603", expected)
        self.assert_fields("mistral-small-4", expected)
        self.assert_fields("mistral-small-latest", expected)
        self.assert_fields(
            "mistral/mistral-small-2603",
            expected,
        )
        self.assert_fields("mistral/mistral-small-latest", expected)

    def test_issue_1215_cohere_exact_ids(self) -> None:
        self.assert_fields(
            "command-r-08-2024",
            {"input_cost_per_token": 0.00000015, "output_cost_per_token": 0.00000060},
        )
        self.assert_fields(
            "command-r-plus-08-2024",
            {"input_cost_per_token": 0.0000025, "output_cost_per_token": 0.000010},
        )

    def test_gpt_5_5_pro_has_no_cache_discount_or_unverified_tier(self) -> None:
        expected = {
            "input_cost_per_token": 0.000030,
            "output_cost_per_token": 0.000180,
            "cache_read_input_token_cost": 0.000030,
        }
        tier_fields = (
            "input_cost_per_token_above_272k_tokens",
            "output_cost_per_token_above_272k_tokens",
            "cache_read_input_token_cost_above_272k_tokens",
        )
        for model in ("gpt-5.5-pro", "gpt-5.5-pro-2026-04-23"):
            self.assert_fields(model, expected)
            for field in tier_fields:
                self.assertNotIn(field, self.catalog[model])

    def test_deepseek_time_of_use_is_preserved(self) -> None:
        for model in ("deepseek-v4-flash", "deepseek/deepseek-v4-flash"):
            with self.subTest(model=model):
                row = self.catalog[model]
                self.assertEqual(row["input_cost_per_token"], 0.00000022)
                self.assertEqual(row["output_cost_per_token"], 0.00000066)
                self.assertEqual(row["cache_read_input_token_cost"], 0.000000007)
                tou = row["time_of_use_pricing"]
                self.assertEqual(tou["timezone"], "UTC")
                self.assertEqual(tou["peak_rates"]["input_cost_per_token"], 0.00000044)
                self.assertEqual(tou["peak_rates"]["output_cost_per_token"], 0.00000132)
                self.assertEqual(tou["peak_rates"]["cache_read_input_token_cost"], 0.000000014)


if __name__ == "__main__":
    unittest.main()
