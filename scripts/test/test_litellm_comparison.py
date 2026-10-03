"""Behavioral checks for benchmark evidence and child-process cleanup."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import time

SCRIPT = Path(__file__).resolve().parents[1] / "bench/compare_litellm.py"
spec = importlib.util.spec_from_file_location("compare_litellm", SCRIPT)
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class ComparisonTests(unittest.TestCase):
    def test_summary_preserves_errors_and_converts_latency(self):
        raw = {"summary": {"requestsPerSec": 12, "successRate": 0.75}, "latencyPercentiles": {"p50": .01, "p95": .02, "p99": .03}, "statusCodeDistribution": {"200": 3, "429": 1}, "errorDistribution": {"timeout": 1}}
        result = bench.summarize(raw, [{"rss_kib": 100}, {"rss_kib": 200}])
        self.assertEqual(result["error_rate"], .25)
        self.assertEqual(result["latency_ms"]["p99"], 30)
        self.assertEqual(result["status_codes"]["429"], 1)
        self.assertEqual(result["transport_errors"], {"timeout": 1})
        self.assertEqual(result["process_tree_rss_kib"], {"peak": 200, "median": 150})

    def test_port_selection_skips_owned_listener(self):
        with socket.socket() as owned:
            owned.bind(("127.0.0.1", 0))
            first = owned.getsockname()[1]
            if first > 65430:
                self.skipTest("ephemeral port too close to range end")
            ports = bench.free_ports(first)
            self.assertEqual(len(set(ports)), 3)
            self.assertNotIn(first, ports)
            for port in ports:
                with socket.socket() as probe:
                    probe.bind(("127.0.0.1", port))

    def test_failed_measurement_keeps_memory_and_raw_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fake = root / "oha"
            fake.write_text(f"#!{sys.executable}\nimport time\nprint('{{}}', flush=True)\ntime.sleep(.25)\nraise SystemExit(7)\n")
            fake.chmod(0o755)
            with bench.service([sys.executable, "-c", "import time; time.sleep(30)"], root / "service.log", dict(os.environ)) as process:
                with self.assertRaisesRegex(RuntimeError, "exited with 7"):
                    bench.measure(str(fake), process, "http://127.0.0.1:1", 1, 1, root / "run.json", dict(os.environ))
                self.assertTrue(json.loads((root / "run.memory.json").read_text()))
                self.assertEqual(json.loads((root / "run.json").read_text()), {})
            self.assertIsNotNone(process.poll())

    def test_service_is_stopped_when_caller_raises(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(RuntimeError, "abort"):
                with bench.service([sys.executable, "-c", "import time; time.sleep(30)"], Path(directory) / "service.log", dict(os.environ)) as process:
                    raise RuntimeError("abort")
            self.assertIsNotNone(process.poll())

    def test_partial_sample_links_evidence_when_warmup_fails(self):
        report = {"samples": []}
        saves = []
        args = SimpleNamespace(oha="oha", warmup=1, seconds=1, concurrency=1)
        with patch.object(bench, "probe", return_value={}), patch.object(bench, "measure", side_effect=RuntimeError("failed warmup")):
            with self.assertRaisesRegex(RuntimeError, "failed warmup"):
                bench.record_sample(report, lambda: saves.append(json.loads(json.dumps(report))), args, None, 1234, Path("/tmp"), "round-1-rust", 0, "rust", {})
        sample = report["samples"][0]
        self.assertFalse(sample["complete"])
        self.assertEqual(sample["phase"], "warmup")
        self.assertEqual(sample["warmup"]["memory"], "round-1-rust-warmup.memory.json")
        self.assertIn("failure", saves[-1]["samples"][0])
        self.assertNotIn("raw", saves[0]["samples"][0])
        self.assertNotIn("raw", sample)

    def test_failed_probe_does_not_reference_unstarted_measurements(self):
        report = {"samples": []}
        args = SimpleNamespace(oha="oha", warmup=1, seconds=1, concurrency=1)
        with patch.object(bench, "probe", side_effect=ValueError("invalid response")), patch.object(bench, "measure") as measure:
            with self.assertRaisesRegex(ValueError, "invalid response"):
                bench.record_sample(report, lambda: None, args, None, 1234, Path("/tmp"), "round-1-rust", 0, "rust", {})
            measure.assert_not_called()
        sample = report["samples"][0]
        self.assertEqual(sample["phase"], "probe")
        self.assertIn("failure", sample)
        for key in ["warmup", "raw", "memory", "stderr"]:
            self.assertNotIn(key, sample)

    def test_external_cargo_configuration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkout = root / "repo"
            checkout.mkdir()
            cargo = root / "cargo-home"
            cargo.mkdir()
            with patch.object(bench, "ROOT", checkout):
                env = {"HOME": str(root), "CARGO_HOME": str(cargo)}
                bench.reject_external_cargo_config(env)
                (cargo / "config.toml").write_text('[build]\nrustflags = ["-Ctarget-cpu=native"]\n')
                with self.assertRaisesRegex(RuntimeError, "unrecorded external Cargo"):
                    bench.reject_external_cargo_config(env)
                (cargo / "config.toml").unlink()
                (root / ".cargo").mkdir()
                (root / ".cargo/config").write_text("[build]\n")
                with self.assertRaisesRegex(RuntimeError, "unrecorded external Cargo"):
                    bench.reject_external_cargo_config(env)

    def test_checkout_root_cargo_configuration_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checkout = root / "repo"
            (checkout / ".cargo").mkdir(parents=True)
            (checkout / ".cargo/config.toml").write_text("[build]\n")
            with patch.object(bench, "ROOT", checkout):
                with self.assertRaisesRegex(RuntimeError, "unrecorded external Cargo"):
                    bench.reject_external_cargo_config({"HOME": str(root), "CARGO_HOME": str(root / "cargo-home")})

    def test_local_probes_ignore_ambient_proxies(self):
        from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
        import threading
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                self.send_response(200)
                self.end_headers()
            def do_POST(self):
                self.rfile.read(int(self.headers.get("content-length", 0)))
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"choices":[{"message":{"content":"pong"}}],"usage":{"total_tokens":2}}')
            def log_message(self, *args):
                pass
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever)
        thread.start()
        try:
            env = {"HTTP_PROXY":"http://127.0.0.1:1", "http_proxy":"http://127.0.0.1:1", "NO_PROXY":"", "no_proxy":""}
            with patch.dict(os.environ, env), patch.object(bench.urllib.request, "getproxies", return_value={"http":"http://127.0.0.1:1"}):
                url = f"http://127.0.0.1:{server.server_port}"
                bench.ready(SimpleNamespace(poll=lambda: None), url)
                self.assertEqual(bench.probe(url)["body"]["usage"]["total_tokens"], 2)
        finally:
            server.shutdown()
            thread.join()
            server.server_close()

    def test_orphan_workers_are_stopped_after_launcher_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            pidfile = root / "child.pid"
            source = "import subprocess,sys; from pathlib import Path; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); Path(sys.argv[1]).write_text(str(p.pid))"
            with bench.service([sys.executable, "-c", source, str(pidfile)], root / "launcher.log", dict(os.environ)) as launcher:
                launcher.wait(timeout=5)
                child = bench.psutil.Process(int(pidfile.read_text()))
                self.assertTrue(child.is_running())
            deadline = time.monotonic() + 3
            while time.monotonic() < deadline and child.is_running() and child.status() != bench.psutil.STATUS_ZOMBIE:
                time.sleep(.02)
            self.assertTrue(not child.is_running() or child.status() == bench.psutil.STATUS_ZOMBIE)

    def test_cpu_model_records_host_processor(self):
        self.assertTrue(bench.cpu_model())

    def test_linux_arm_cpu_identity_preserves_heterogeneous_parts(self):
        cpuinfo = "processor : 0\nCPU implementer : 0x41\nCPU part : 0xd40\nCPU revision : 1\n\nprocessor : 1\nCPU implementer : 0x41\nCPU part : 0xd41\nCPU revision : 2\n"
        with patch.object(bench.platform, "system", return_value="Linux"), patch.object(Path, "read_text", return_value=cpuinfo), patch.object(bench.platform, "processor", return_value=""):
            identities = json.loads(bench.cpu_model())
        self.assertEqual([cpu["CPU part"] for cpu in identities], ["0xd40", "0xd41"])
        self.assertTrue(all(cpu["CPU implementer"] == "0x41" for cpu in identities))


if __name__ == "__main__":
    unittest.main()
