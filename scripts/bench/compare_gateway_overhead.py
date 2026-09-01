#!/usr/bin/env python3
"""Compare two gateway-overhead artifacts without hiding invalid evidence."""

from __future__ import annotations

import json
import math
import re
import sys
from datetime import datetime
from pathlib import Path
from typing import Any


THROUGHPUT_DROP_LIMIT = 0.10
LATENCY_INCREASE_LIMIT = 0.15
REGRESSION_EXIT = 10
ERROR_EXIT = 2
SHA_PATTERN = re.compile(r"[0-9a-f]{40}")


class ComparisonError(Exception):
    """The benchmark evidence cannot be compared safely."""


def load_artifact(path: Path) -> dict[str, Any]:
    try:
        artifact = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ComparisonError(f"cannot read benchmark artifact {path}: {error}") from error
    if not isinstance(artifact, dict):
        raise ComparisonError(f"benchmark artifact {path} must be a JSON object")
    return artifact


def required_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ComparisonError(f"{label} must be a JSON object")
    return value


def required_number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ComparisonError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number <= 0:
        raise ComparisonError(f"{label} must be finite and greater than zero")
    return number


def validate_artifact(artifact: dict[str, Any], label: str) -> None:
    if artifact.get("schema_version") != 1:
        raise ComparisonError(f"{label} has an unsupported schema_version")

    captured_at = artifact.get("captured_at")
    if not isinstance(captured_at, str):
        raise ComparisonError(f"{label}.captured_at must be a UTC timestamp")
    try:
        datetime.strptime(captured_at, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError as error:
        raise ComparisonError(f"{label}.captured_at must be a UTC timestamp") from error

    source = required_mapping(artifact.get("source"), f"{label}.source")
    git_sha = source.get("git_sha")
    if not isinstance(git_sha, str) or SHA_PATTERN.fullmatch(git_sha) is None:
        raise ComparisonError(f"{label}.source.git_sha must be a full lowercase Git SHA")
    if source.get("git_dirty") is not False:
        raise ComparisonError(f"{label} must come from a clean Git worktree")
    if source.get("build_flags") != ["--release", "--bin", "gateway"]:
        raise ComparisonError(f"{label}.source.build_flags do not match the benchmark contract")

    required_mapping(artifact.get("environment"), f"{label}.environment")
    required_mapping(artifact.get("workload"), f"{label}.workload")
    results = required_mapping(artifact.get("results"), f"{label}.results")
    required_number(results.get("requests_per_second"), f"{label}.results.requests_per_second")
    latency = required_mapping(results.get("latency_ms"), f"{label}.results.latency_ms")
    for percentile in ("p50", "p95", "p99"):
        required_number(latency.get(percentile), f"{label}.results.latency_ms.{percentile}")
    error_rate = results.get("error_rate")
    if isinstance(error_rate, bool) or not isinstance(error_rate, (int, float)):
        raise ComparisonError(f"{label}.results.error_rate must be numeric")
    if not math.isfinite(float(error_rate)) or float(error_rate) != 0:
        raise ComparisonError(f"{label}.results.error_rate must be zero")

    oha_raw = required_mapping(artifact.get("oha_raw"), f"{label}.oha_raw")
    summary = required_mapping(oha_raw.get("summary"), f"{label}.oha_raw.summary")
    success_rate = summary.get("successRate")
    if isinstance(success_rate, bool) or not isinstance(success_rate, (int, float)):
        raise ComparisonError(f"{label}.oha_raw.summary.successRate must be numeric")
    if not math.isfinite(float(success_rate)) or float(success_rate) != 1:
        raise ComparisonError(f"{label}.oha_raw.summary.successRate must be one")
    error_distribution = required_mapping(
        oha_raw.get("errorDistribution"),
        f"{label}.oha_raw.errorDistribution",
    )
    if error_distribution:
        raise ComparisonError(f"{label}.oha_raw.errorDistribution must be empty")
    status_distribution = required_mapping(
        oha_raw.get("statusCodeDistribution"),
        f"{label}.oha_raw.statusCodeDistribution",
    )
    if set(status_distribution) != {"200"}:
        raise ComparisonError(
            f'{label}.oha_raw.statusCodeDistribution must contain only "200"'
        )


def change_fraction(baseline: float, candidate: float) -> float:
    return round((candidate - baseline) / baseline, 6)


def metric_result(
    baseline: float,
    candidate: float,
    *,
    regression_when_below: float | None = None,
    regression_when_above: float | None = None,
) -> dict[str, Any]:
    change = change_fraction(baseline, candidate)
    regressed = (
        regression_when_below is not None and change < regression_when_below
    ) or (
        regression_when_above is not None and change > regression_when_above
    )
    return {
        "baseline": baseline,
        "candidate": candidate,
        "change_fraction": change,
        "status": "regression" if regressed else "pass",
    }


def compare(baseline: dict[str, Any], candidate: dict[str, Any]) -> dict[str, Any]:
    validate_artifact(baseline, "baseline")
    validate_artifact(candidate, "candidate")

    if baseline["environment"] != candidate["environment"]:
        raise ComparisonError("environment mismatch between baseline and candidate")
    if baseline["workload"] != candidate["workload"]:
        raise ComparisonError("workload mismatch between baseline and candidate")

    baseline_results = baseline["results"]
    candidate_results = candidate["results"]
    metrics = {
        "requests_per_second": metric_result(
            float(baseline_results["requests_per_second"]),
            float(candidate_results["requests_per_second"]),
            regression_when_below=-THROUGHPUT_DROP_LIMIT,
        )
    }
    for percentile in ("p50", "p95", "p99"):
        metrics[f"latency_ms.{percentile}"] = metric_result(
            float(baseline_results["latency_ms"][percentile]),
            float(candidate_results["latency_ms"][percentile]),
            regression_when_above=LATENCY_INCREASE_LIMIT,
        )

    verdict = (
        "regression"
        if any(metric["status"] == "regression" for metric in metrics.values())
        else "pass"
    )
    return {
        "schema_version": 1,
        "verdict": verdict,
        "baseline": {
            "git_sha": baseline["source"]["git_sha"],
            "captured_at": baseline["captured_at"],
        },
        "candidate": {
            "git_sha": candidate["source"]["git_sha"],
            "captured_at": candidate["captured_at"],
        },
        "policy": {
            "mode": "report_only",
            "throughput_drop_limit_fraction": THROUGHPUT_DROP_LIMIT,
            "latency_increase_limit_fraction": LATENCY_INCREASE_LIMIT,
        },
        "metrics": metrics,
    }


def main(argv: list[str]) -> int:
    if len(argv) != 4:
        print(
            "usage: compare_gateway_overhead.py <baseline.json> <candidate.json> <report.json>",
            file=sys.stderr,
        )
        return ERROR_EXIT

    baseline_path, candidate_path, report_path = map(Path, argv[1:])
    try:
        report = compare(load_artifact(baseline_path), load_artifact(candidate_path))
        report_path.write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )
    except (ComparisonError, OSError) as error:
        print(f"benchmark comparison error: {error}", file=sys.stderr)
        return ERROR_EXIT

    print(json.dumps(report, indent=2, sort_keys=True))
    return REGRESSION_EXIT if report["verdict"] == "regression" else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
