"""Behavioral checks for benchmark evidence and child-process cleanup."""
import importlib.util
import json
import os
from pathlib import Path
import socket
import sys
import tempfile
import unittest

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


if __name__ == "__main__":
    unittest.main()
