#!/usr/bin/env python3
"""Contract tests for the reproducible gateway-overhead benchmark."""

from __future__ import annotations

import http.client
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import textwrap
import threading
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
MOCK_PATH = REPO_ROOT / "scripts" / "bench" / "mock_openai.py"
RUNNER_PATH = REPO_ROOT / "scripts" / "bench" / "run_gateway_overhead.sh"
CONFIG_PATH = REPO_ROOT / "scripts" / "bench" / "gateway-overhead.yaml"
METHODOLOGY_PATH = REPO_ROOT / "docs" / "benchmarks" / "gateway-overhead.md"
COMPARATOR_PATH = REPO_ROOT / "scripts" / "bench" / "compare_gateway_overhead.py"
WORKFLOW_PATH = REPO_ROOT / ".github" / "workflows" / "gateway-overhead-benchmark.yml"


def load_mock_module():
    spec = importlib.util.spec_from_file_location("mock_openai", MOCK_PATH)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {MOCK_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class GatewayOverheadBenchmarkContractTests(unittest.TestCase):
    def write_artifact(
        self,
        path: Path,
        *,
        git_sha: str,
        requests_per_second: float = 1_000.0,
        p50: float = 1.0,
        p95: float = 2.0,
        p99: float = 3.0,
        concurrency: int = 64,
    ) -> None:
        path.write_text(
            json.dumps(
                {
                    "schema_version": 1,
                    "captured_at": "2026-09-01T00:00:00Z",
                    "source": {
                        "git_sha": git_sha,
                        "git_dirty": False,
                        "build_flags": ["--release", "--bin", "gateway"],
                    },
                    "environment": {
                        "hardware": {
                            "cpu_model": "test-cpu",
                            "logical_cpus": 4,
                            "memory_bytes": 8_000_000_000,
                        },
                        "os": {
                            "name": "Linux",
                            "release": "test-release",
                            "architecture": "x86_64",
                        },
                        "rust": "rustc 1.96.1 test",
                        "cargo": "cargo 1.96.1 test",
                        "python": "Python 3.12 test",
                        "oha": "oha 1.16.0",
                    },
                    "workload": {
                        "concurrency": concurrency,
                        "warmup_seconds": 10,
                        "duration_seconds": 60,
                        "request_bytes": 88,
                        "response_bytes": 389,
                        "protocol": "HTTP/1.1 keep-alive",
                        "route": "POST /v1/chat/completions",
                        "upstream": "deterministic local mock, fixed response, zero injected delay",
                    },
                    "results": {
                        "requests_per_second": requests_per_second,
                        "latency_ms": {"p50": p50, "p95": p95, "p99": p99},
                        "error_rate": 0.0,
                    },
                    "oha_raw": {
                        "summary": {"successRate": 1.0},
                        "errorDistribution": {},
                        "statusCodeDistribution": {"200": 1_000},
                    },
                }
            ),
            encoding="utf-8",
        )

    def run_comparison(
        self,
        baseline: Path,
        candidate: Path,
        report: Path,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                "python3",
                str(COMPARATOR_PATH),
                str(baseline),
                str(candidate),
                str(report),
            ],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )

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
        self.assertIn("CARGO_BUILD_RUSTFLAGS", runner)
        self.assertIn('CARGO_TARGET_DIR="$tmp_dir/cargo-target"', runner)
        self.assertIn('artifact_tmp="$tmp_dir/artifact.json"', runner)
        self.assertIn('.statusCodeDistribution | keys == ["200"]', runner)
        self.assertIn("--max-time", runner)
        self.assertIn("/proc/cpuinfo", runner)
        self.assertIn("%Y-%m-%dT%H%M%SZ", methodology)
        self.assertNotIn("\ntarget/release/gateway", runner)
        self.assertNotIn(" target/release/gateway", runner)

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

        result = self.run_preflight(
            extra_env={"CARGO_BUILD_RUSTFLAGS": "-C target-cpu=native"}
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "build override is not allowed: CARGO_BUILD_RUSTFLAGS",
            result.stderr,
        )

        result = self.run_preflight(external_cargo_config=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("external Cargo configuration is not allowed", result.stderr)

    def test_runner_builds_into_an_isolated_target_directory(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            probe_repo = temp_dir / "probe-repo"
            runner_dir = probe_repo / "scripts" / "bench"
            runner_dir.mkdir(parents=True)
            shutil.copy(RUNNER_PATH, runner_dir / "run_gateway_overhead.sh")
            (probe_repo / ".gitignore").write_text("/target/\n", encoding="utf-8")
            cargo_home = temp_dir / "cargo-home"
            cargo_home.mkdir()
            subprocess.run(
                ["git", "init", "-b", "main"],
                cwd=probe_repo,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "add", "."],
                cwd=probe_repo,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.email=probe@example.com",
                    "-c",
                    "user.name=probe",
                    "commit",
                    "-m",
                    "probe",
                ],
                cwd=probe_repo,
                check=True,
                capture_output=True,
                text=True,
            )

            bin_dir = temp_dir / "bin"
            bin_dir.mkdir()
            probe_log = temp_dir / "probe-target-dir"
            decoy_log = temp_dir / "decoy.log"
            isolated_log = temp_dir / "isolated.log"
            fake_oha = bin_dir / "oha"
            fake_oha.write_text("#!/bin/sh\nprintf '%s\\n' 'oha 1.16.0'\n", encoding="utf-8")
            fake_oha.chmod(0o755)
            fake_cargo = bin_dir / "cargo"
            fake_cargo.write_text(
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env python3
                    import os
                    import pathlib
                    import sys

                    target = os.environ.get("CARGO_TARGET_DIR", "")
                    pathlib.Path({str(probe_log)!r}).write_text(target)
                    if len(sys.argv) >= 2 and sys.argv[1] in ("-V", "--version"):
                        print("cargo 1.96.1 (test)")
                        raise SystemExit(0)
                    if not target:
                        sys.stderr.write("CARGO_TARGET_DIR was not set\\n")
                        raise SystemExit(2)
                    binary = pathlib.Path(target) / "release" / "gateway"
                    binary.parent.mkdir(parents=True, exist_ok=True)
                    binary.write_text(
                        "#!/bin/sh\\n"
                        f"printf isolated >> '{isolated_log}'\\n"
                        "exit 1\\n"
                    )
                    binary.chmod(0o755)
                    raise SystemExit(0)
                    """
                ),
                encoding="utf-8",
            )
            fake_cargo.chmod(0o755)

            decoy_path = probe_repo / "target" / "release" / "gateway"
            decoy_path.parent.mkdir(parents=True, exist_ok=True)
            decoy_path.write_text(
                "#!/bin/sh\n"
                f"printf decoy >> '{decoy_log}'\n"
                "exit 0\n",
                encoding="utf-8",
            )
            decoy_path.chmod(0o755)

            environment = os.environ.copy()
            environment["PATH"] = f"{bin_dir}{os.pathsep}{environment['PATH']}"
            environment["CARGO_HOME"] = str(cargo_home)
            output = temp_dir / "artifact.json"
            result = subprocess.run(
                ["bash", str(runner_dir / "run_gateway_overhead.sh"), str(output)],
                cwd=probe_repo,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )

            self.assertNotEqual(result.returncode, 0, result.stderr)
            self.assertTrue(probe_log.exists(), result.stderr)
            target_dir = Path(probe_log.read_text(encoding="utf-8"))
            self.assertTrue(str(target_dir), result.stderr)
            self.assertNotEqual(target_dir.resolve(), (probe_repo / "target").resolve())
            self.assertEqual(target_dir.name, "cargo-target")
            self.assertTrue(isolated_log.exists(), result.stderr)
            self.assertEqual(isolated_log.read_text(encoding="utf-8"), "isolated")
            self.assertFalse(decoy_log.exists())

    def test_runner_refuses_an_existing_artifact(self) -> None:
        result = self.run_preflight(existing_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("benchmark artifact already exists", result.stderr)

    def test_readme_does_not_make_unpublished_gateway_performance_claims(self) -> None:
        readme = (REPO_ROOT / "README.md").read_text(encoding="utf-8")
        self.assertNotIn("10,000+ requests/second", readme)
        self.assertNotIn("<10ms routing overhead", readme)
        self.assertIn("docs/benchmarks/gateway-overhead.md", readme)

    def test_comparator_writes_commit_bound_machine_readable_pass(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            baseline = temp_dir / "baseline.json"
            candidate = temp_dir / "candidate.json"
            report = temp_dir / "comparison.json"
            self.write_artifact(baseline, git_sha="a" * 40)
            self.write_artifact(candidate, git_sha="b" * 40, requests_per_second=950.0)

            result = self.run_comparison(baseline, candidate, report)

            self.assertEqual(result.returncode, 0, result.stderr)
            comparison = json.loads(report.read_text(encoding="utf-8"))
            self.assertEqual(comparison["verdict"], "pass")
            self.assertEqual(comparison["baseline"]["git_sha"], "a" * 40)
            self.assertEqual(comparison["candidate"]["git_sha"], "b" * 40)
            self.assertEqual(
                comparison["metrics"]["requests_per_second"]["change_fraction"],
                -0.05,
            )

    def test_comparator_distinguishes_regression_from_invalid_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            baseline = temp_dir / "baseline.json"
            candidate = temp_dir / "candidate.json"
            report = temp_dir / "comparison.json"
            self.write_artifact(baseline, git_sha="a" * 40)
            self.write_artifact(candidate, git_sha="b" * 40, p95=2.5)

            regression = self.run_comparison(baseline, candidate, report)

            self.assertEqual(regression.returncode, 10, regression.stderr)
            self.assertEqual(
                json.loads(report.read_text(encoding="utf-8"))["verdict"],
                "regression",
            )

            self.write_artifact(candidate, git_sha="b" * 40, concurrency=32)
            invalid = self.run_comparison(baseline, candidate, report)
            self.assertEqual(invalid.returncode, 2)
            self.assertIn("workload mismatch", invalid.stderr)

    def test_comparator_requires_auditable_raw_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            baseline = temp_dir / "baseline.json"
            candidate = temp_dir / "candidate.json"
            report = temp_dir / "comparison.json"
            self.write_artifact(baseline, git_sha="a" * 40)

            for missing_field in ("captured_at", "oha_raw"):
                with self.subTest(missing_field=missing_field):
                    self.write_artifact(candidate, git_sha="b" * 40)
                    artifact = json.loads(candidate.read_text(encoding="utf-8"))
                    del artifact[missing_field]
                    candidate.write_text(json.dumps(artifact), encoding="utf-8")

                    invalid = self.run_comparison(baseline, candidate, report)

                    self.assertEqual(invalid.returncode, 2)
                    self.assertIn(missing_field, invalid.stderr)

    def test_comparator_rejects_nonzero_error_rate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            baseline = temp_dir / "baseline.json"
            candidate = temp_dir / "candidate.json"
            report = temp_dir / "comparison.json"
            self.write_artifact(baseline, git_sha="a" * 40)
            self.write_artifact(candidate, git_sha="b" * 40)
            artifact = json.loads(candidate.read_text(encoding="utf-8"))
            artifact["results"]["error_rate"] = 0.01
            artifact["oha_raw"]["summary"]["successRate"] = 0.99
            artifact["oha_raw"]["statusCodeDistribution"] = {"200": 99, "500": 1}
            candidate.write_text(json.dumps(artifact), encoding="utf-8")

            invalid = self.run_comparison(baseline, candidate, report)

            self.assertEqual(invalid.returncode, 2)
            self.assertIn("error_rate must be zero", invalid.stderr)

    def test_comparator_rejects_errors_present_only_in_raw_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            temp_dir = Path(directory)
            baseline = temp_dir / "baseline.json"
            candidate = temp_dir / "candidate.json"
            report = temp_dir / "comparison.json"
            self.write_artifact(baseline, git_sha="a" * 40)
            self.write_artifact(candidate, git_sha="b" * 40)
            artifact = json.loads(candidate.read_text(encoding="utf-8"))
            artifact["oha_raw"]["errorDistribution"] = {"connection": 1}
            candidate.write_text(json.dumps(artifact), encoding="utf-8")

            transport_error = self.run_comparison(baseline, candidate, report)

            self.assertEqual(transport_error.returncode, 2)
            self.assertIn("errorDistribution must be empty", transport_error.stderr)

            self.write_artifact(candidate, git_sha="b" * 40)
            artifact = json.loads(candidate.read_text(encoding="utf-8"))
            artifact["oha_raw"]["statusCodeDistribution"] = {"200": 999, "500": 1}
            candidate.write_text(json.dumps(artifact), encoding="utf-8")

            status_error = self.run_comparison(baseline, candidate, report)

            self.assertEqual(status_error.returncode, 2)
            self.assertIn("must contain only", status_error.stderr)

    def test_workflow_file_is_present(self) -> None:
        self.assertTrue(WORKFLOW_PATH.is_file())
        workflow = WORKFLOW_PATH.read_text(encoding="utf-8")
        self.assertGreater(len(workflow), 0)
        self.assertIn("id: harness", workflow)
        self.assertIn("steps.harness.outputs.identical == 'true'", workflow)


if __name__ == "__main__":
    unittest.main()
