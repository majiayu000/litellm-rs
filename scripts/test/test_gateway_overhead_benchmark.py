#!/usr/bin/env python3
"""Contract tests for the reproducible gateway-overhead benchmark."""

from __future__ import annotations

import http.client
import importlib.util
import json
import os
import subprocess
import tempfile
import threading
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MOCK_PATH = REPO_ROOT / "scripts" / "bench" / "mock_openai.py"
RUNNER_PATH = REPO_ROOT / "scripts" / "bench" / "run_gateway_overhead.sh"
CONFIG_PATH = REPO_ROOT / "scripts" / "bench" / "gateway-overhead.yaml"
METHODOLOGY_PATH = REPO_ROOT / "docs" / "benchmarks" / "gateway-overhead.md"


def load_mock_module():
    spec = importlib.util.spec_from_file_location("mock_openai", MOCK_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {MOCK_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class GatewayOverheadBenchmarkContractTests(unittest.TestCase):
    def run_preflight(
        self,
        *,
        oha_version: str = "oha 1.16.0",
        extra_env: dict[str, str] | None = None,
        existing_output: bool = False,
        external_cargo_config: bool = False,
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            bin_dir = temp_dir / "bin"
            bin_dir.mkdir()
            fake_oha = bin_dir / "oha"
            fake_oha.write_text(
                f"#!/bin/sh\nprintf '%s\\n' '{oha_version}'\n",
                encoding="utf-8",
            )
            fake_oha.chmod(0o755)
            output = temp_dir / "artifact.json"
            if existing_output:
                output.write_text("do not replace", encoding="utf-8")
            environment = os.environ.copy()
            environment["PATH"] = f"{bin_dir}{os.pathsep}{environment['PATH']}"
            if external_cargo_config:
                cargo_home = temp_dir / "cargo-home"
                cargo_home.mkdir()
                (cargo_home / "config.toml").write_text(
                    '[build]\nrustflags = ["-C", "target-cpu=native"]\n',
                    encoding="utf-8",
                )
                environment["CARGO_HOME"] = str(cargo_home)
            if extra_env:
                environment.update(extra_env)
            return subprocess.run(
                ["bash", str(RUNNER_PATH), str(output)],
                cwd=REPO_ROOT,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )

    def test_mock_upstream_returns_a_fixed_openai_response(self) -> None:
        module = load_mock_module()
        server = module.create_server("127.0.0.1", 0)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)

        connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
        request = json.dumps(
            {"model": "benchmark-model", "messages": [{"role": "user", "content": "ping"}]},
            separators=(",", ":"),
        )
        connection.request(
            "POST",
            "/v1/chat/completions",
            body=request,
            headers={"content-type": "application/json"},
        )
        response = connection.getresponse()
        first_body = response.read()
        connection.close()

        self.assertEqual(response.status, 200)
        self.assertEqual(json.loads(first_body), module.CHAT_RESPONSE)

        connection = http.client.HTTPConnection("127.0.0.1", server.server_port)
        connection.request(
            "POST",
            "/v1/chat/completions",
            body=request,
            headers={"content-type": "application/json"},
        )
        second_body = connection.getresponse().read()
        connection.close()
        self.assertEqual(first_body, second_body)

    def test_runner_and_methodology_publish_the_required_contract(self) -> None:
        subprocess.run(["bash", "-n", str(RUNNER_PATH)], check=True)
        runner = RUNNER_PATH.read_text(encoding="utf-8")
        methodology = METHODOLOGY_PATH.read_text(encoding="utf-8")
        config = CONFIG_PATH.read_text(encoding="utf-8")

        for field in (
            "hardware",
            "os",
            "rust",
            "build_flags",
            "git_sha",
            "concurrency",
            "request_bytes",
            "response_bytes",
            "duration_seconds",
            "warmup_seconds",
            "requests_per_second",
            "latency_ms",
            "p50",
            "p95",
            "p99",
            "error_rate",
            "oha_raw",
        ):
            self.assertIn(field, runner)
            self.assertIn(field, methodology)

        for supported_oha_flag in ("-w", "-m POST", "-H", "-d", "-c", "-z"):
            self.assertIn(supported_oha_flag, runner)
        for nonexistent_oha_flag in ("--body", "--concurrency", "--duration"):
            self.assertNotIn(nonexistent_oha_flag, runner)

        self.assertIn("127.0.0.1:18000", config)
        self.assertIn("endpoint_access: private_network", config)
        self.assertIn("cache:\n  enabled: false", config)
        self.assertIn("rate_limit:\n  enabled: false", config)
        self.assertIn("python", runner)
        self.assertIn("python", methodology)
        self.assertIn("python3 -VV", runner)
        self.assertIn("kill -0", runner)
        self.assertIn("source_git_sha", runner)
        self.assertIn("CARGO_PROFILE_RELEASE_", runner)
        self.assertIn('artifact_tmp="$tmp_dir/artifact.json"', runner)
        self.assertIn('.statusCodeDistribution | keys == ["200"]', runner)
        self.assertIn("--max-time", runner)
        self.assertIn("/proc/cpuinfo", runner)
        self.assertIn("%Y-%m-%dT%H%M%SZ", methodology)

    def test_runner_requires_the_exact_oha_release(self) -> None:
        result = self.run_preflight(oha_version="oha 1.16.0-dev")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact oha version", result.stderr)

    def test_runner_rejects_unrecorded_build_overrides(self) -> None:
        result = self.run_preflight(extra_env={"RUSTFLAGS": "-C target-cpu=native"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("build override is not allowed: RUSTFLAGS", result.stderr)

        result = self.run_preflight(extra_env={"CARGO_TARGET_DIR": "/tmp/other-target"})
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("build override is not allowed: CARGO_TARGET_DIR", result.stderr)

        result = self.run_preflight(external_cargo_config=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("external Cargo configuration is not allowed", result.stderr)

    def test_runner_refuses_an_existing_artifact(self) -> None:
        result = self.run_preflight(existing_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("benchmark artifact already exists", result.stderr)

    def test_readme_does_not_make_unpublished_gateway_performance_claims(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertNotIn("10,000+ requests/second", readme)
        self.assertNotIn("<10ms routing overhead", readme)
        self.assertIn("docs/benchmarks/gateway-overhead.md", readme)


if __name__ == "__main__":
    unittest.main()
