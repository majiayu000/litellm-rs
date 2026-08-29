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
from datetime import date


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT_PATH = REPO_ROOT / "scripts" / "sync_litellm_pricing.py"
CATALOG_PATH = REPO_ROOT / "config" / "model_prices_extended.json"

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

    def test_gemini_promo_override_expires_without_overwriting_corrections(self) -> None:
        source = {
            model: {"litellm_provider": "test"}
            for model in sync.OFFICIAL_OVERRIDE_PATCHES
        }
        source["gemini-3.6-flash"] = {
            "litellm_provider": "gemini",
            "input_cost_per_token": 0.0000015,
            "output_cost_per_token": 0.0000075,
            "cache_read_input_token_cost": 0.00000015,
        }
        promotional = {
            "gemini-3.6-flash": {
                "litellm_provider": "gemini",
                **sync.OFFICIAL_OVERRIDE_PATCHES["gemini-3.6-flash"],
            }
        }
        corrected = {
            "gemini-3.6-flash": {
                **promotional["gemini-3.6-flash"],
                "input_cost_per_token": 0.00000125,
            }
        }

        active = sync.apply_official_overrides(
            source, promotional, as_of_date=date(2026, 12, 31)
        )
        expired = sync.apply_official_overrides(
            source, promotional, as_of_date=date(2027, 1, 1)
        )
        preserved = sync.apply_official_overrides(
            source, corrected, as_of_date=date(2027, 1, 1)
        )

        self.assertEqual(active["gemini-3.6-flash"], promotional["gemini-3.6-flash"])
        self.assertEqual(expired["gemini-3.6-flash"], source["gemini-3.6-flash"])
        self.assertEqual(preserved["gemini-3.6-flash"], corrected["gemini-3.6-flash"])


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

    def test_rejects_time_of_use_timezone_unsupported_by_runtime(self) -> None:
        row = {
            "litellm_provider": "test",
            "input_cost_per_token": 0.000001,
            "output_cost_per_token": 0.000002,
            "time_of_use_pricing": {
                "timezone": "Asia/Shanghai",
                "peak_windows": [
                    {"weekdays": [1], "start_hour": 1, "end_hour": 4}
                ],
                "peak_rates": {
                    "input_cost_per_token": 0.000002,
                    "output_cost_per_token": 0.000004,
                },
            },
        }

        with self.assertRaisesRegex(SystemExit, "timezone must be 'UTC'"):
            sync.validate_entries({"future-model": row}, min_models=1)

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
