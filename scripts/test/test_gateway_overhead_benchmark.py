#!/usr/bin/env python3
"""Contract tests for the reproducible gateway-overhead benchmark."""

from __future__ import annotations

import http.client
import importlib.util
import json
import subprocess
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

    def test_readme_does_not_make_unpublished_gateway_performance_claims(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertNotIn("10,000+ requests/second", readme)
        self.assertNotIn("<10ms routing overhead", readme)
        self.assertIn("docs/benchmarks/gateway-overhead.md", readme)


if __name__ == "__main__":
    unittest.main()
