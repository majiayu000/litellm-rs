#!/usr/bin/env python3
"""Measure real gateway processes with JWT, Redis budgets and a SQLite ledger.

Adapts gateway-overhead.yaml and the existing Redis RESP client. The upstream is
synthetic, deterministic and local; this does not measure real LLM performance.
The caller builds the binary. This runner never builds Rust or changes services.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor
import copy
from datetime import datetime, timezone
import hashlib
import http.client
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import secrets
import socket
import sqlite3
import subprocess
import tempfile
import threading
import time
import uuid

import yaml
from redis_budget_history import Redis, distribution

ROOT = Path(__file__).resolve().parents[2]


def command(*args):
    return subprocess.check_output(args, text=True).strip()


def source_evidence():
    diff = subprocess.check_output(["git", "-C", str(ROOT), "diff", "HEAD", "--binary"])
    paths = subprocess.check_output(["git", "-C", str(ROOT), "ls-files", "-co", "--exclude-standard", "-z"]).split(b"\0")
    digest = hashlib.sha256()
    for name in sorted(set(paths)):
        if not name:
            continue
        path = ROOT / os.fsdecode(name)
        digest.update(name + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest() if path.is_file() else b"<missing>")
    return {"git_sha": command("git", "-C", str(ROOT), "rev-parse", "HEAD"),
        "git_dirty": bool(command("git", "-C", str(ROOT), "status", "--porcelain")),
        "git_diff_sha256": hashlib.sha256(diff).hexdigest(), "source_files_sha256": digest.hexdigest()}


def hardware_evidence():
    if platform.system() == "Darwin":
        return {"cpu_model": command("sysctl", "-n", "machdep.cpu.brand_string"),
                "memory_bytes": int(command("sysctl", "-n", "hw.memsize"))}
    cpu = next((line.split(":", 1)[1].strip() for line in Path("/proc/cpuinfo").read_text().splitlines()
                if line.startswith("model name")), platform.processor())
    memory = next(line.split()[1] for line in Path("/proc/meminfo").read_text().splitlines() if line.startswith("MemTotal:"))
    return {"cpu_model": cpu, "memory_bytes": int(memory) * 1024}


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def wait_for(predicate, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.025)
    raise TimeoutError("condition exceeded its recorded deadline")


class Upstream(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 128

    def __init__(self, duration, frame_bytes, interval):
        self.duration, self.frame_bytes, self.interval = duration, frame_bytes, interval
        self.events = {}
        self.lock = threading.Lock()
        super().__init__(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.serve_forever, daemon=True).start()


class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_args):
        pass

    def do_POST(self):
        self.close_connection = True
        if self.path != "/v1/chat/completions":
            self.send_error(404)
            return
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        identity = request["messages"][0]["content"]
        with self.server.lock:
            self.server.events[identity] = {"entered": time.monotonic()}
        if not request.get("stream", False):
            body = json.dumps({"id": "deployment-benchmark", "object": "chat.completion",
                "created": 1700000000, "model": request["model"], "choices": [{"index": 0,
                "message": {"role": "assistant", "content": "pong"}, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}}).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Transfer-Encoding", "chunked")
        self.end_headers()

        def send(payload):
            frame = b"data: " + payload + b"\n\n"
            self.wfile.write(f"{len(frame):x}\r\n".encode() + frame + b"\r\n")
            self.wfile.flush()

        start = time.monotonic()
        frames = 0
        try:
            while time.monotonic() - start < self.server.duration:
                payload = {"id": "deployment-benchmark", "object": "chat.completion.chunk",
                    "created": 1700000000, "model": request["model"], "choices": [{"index": 0,
                    "delta": {"content": "x" * self.server.frame_bytes}, "finish_reason": None}]}
                send(json.dumps(payload, separators=(",", ":")).encode())
                frames += 1
                time.sleep(self.server.interval)
            send(json.dumps({"id": "deployment-benchmark", "object": "chat.completion.chunk",
                "created": 1700000000, "model": request["model"], "choices": [{"index": 0,
                "delta": {}, "finish_reason": "stop"}], "usage": {"prompt_tokens": 1,
                "completion_tokens": frames, "total_tokens": frames + 1}}).encode())
            send(b"[DONE]")
            self.wfile.write(b"0\r\n\r\n")
            self.wfile.flush()
            outcome = "completed"
        except (BrokenPipeError, ConnectionResetError):
            outcome = "disconnected"
        finally:
            with self.server.lock:
                self.server.events[identity].update(finished=time.monotonic(), outcome=outcome, frames=frames)


class Gateway:
    def __init__(self, binary, config, directory, index):
        self.port = free_port()
        config = copy.deepcopy(config)
        config["server"]["port"] = self.port
        self.path = directory / f"gateway-{index}.yaml"
        self.path.write_text(yaml.safe_dump(config))
        self.path.chmod(0o600)
        self.log_path = directory / f"gateway-{index}.log"
        self.log = self.log_path.open("w")
        self.token = None
        self.process = subprocess.Popen([str(binary), "--config", str(self.path), "--log-level", "warn"], cwd=ROOT,
            stdout=self.log, stderr=subprocess.STDOUT, env={**os.environ, "RUST_LOG": "warn"})
        def ready():
            if self.process.poll() is not None:
                raise RuntimeError(f"gateway {index} exited; inspect retained log")
            try:
                return self.request("/health")[0] == 200
            except OSError:
                return False
        try:
            wait_for(ready)
        except BaseException:
            self.stop()
            raise

    def request(self, path, body=None, method=None, authenticated=True):
        conn = http.client.HTTPConnection("127.0.0.1", self.port, timeout=30)
        try:
            conn.request(method or ("GET" if body is None else "POST"), path,
                body=None if body is None else json.dumps(body), headers={"Content-Type": "application/json",
                **({"Authorization": "Bearer " + self.token} if self.token and authenticated else {})})
            response = conn.getresponse()
            return response.status, json.loads(response.read())
        finally:
            conn.close()

    def stop(self):
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self.log.close()


def sample(node, model, *, stream=False, slow_delay=0, cancel_after=None):
    identity = "bench-" + str(uuid.uuid4())
    result = {"request_id": identity, "node_port": node.port, "status": None, "error": None,
              "bytes": 0, "done": False}
    conn = http.client.HTTPConnection("127.0.0.1", node.port, timeout=180)
    start = time.monotonic()
    try:
        conn.request("POST", "/v1/chat/completions", json.dumps({"model": model,
            "messages": [{"role": "user", "content": identity}], "max_tokens": 8192,
            "stream": stream, **({"stream_options": {"include_usage": True}} if stream else {})}),
            headers={"Content-Type": "application/json", "Authorization": "Bearer " + node.token,
                     "x-request-id": identity})
        response = conn.getresponse()
        result["status"] = response.status
        if stream and response.status == 200:
            first = response.read(1)
            result["ttfb_ms"] = (time.monotonic() - start) * 1000
            result["bytes"] = len(first)
            suffix = first
            while True:
                if cancel_after is not None and time.monotonic() - start >= cancel_after:
                    # Shutdown the actual socket; HTTPResponse otherwise retains a reference.
                    response.fp.raw._sock.shutdown(socket.SHUT_RDWR)
                    result["cancelled_at"] = time.monotonic()
                    response.close()
                    break
                chunk = response.read1(4096)
                if not chunk:
                    break
                result["bytes"] += len(chunk)
                suffix = (suffix + chunk)[-8192:]
                if b"data: [DONE]" in suffix:
                    result["done"] = True
                if slow_delay:
                    time.sleep(slow_delay)
        else:
            body = response.read()
            result["bytes"] = len(body)
            if response.status == 200:
                parsed = json.loads(body)
                result["done"] = parsed["choices"][0]["finish_reason"] == "stop"
    except (OSError, http.client.HTTPException, ValueError, KeyError) as error:
        result["error"] = type(error).__name__
    finally:
        conn.close()
    result["latency_ms"] = (time.monotonic() - start) * 1000
    return result


def memory_snapshot(nodes):
    # ps reports resident KiB on both macOS and Linux. Redis/driver are excluded.
    return [int(command("ps", "-o", "rss=", "-p", str(node.process.pid))) * 1024 for node in nodes]


def measured(nodes, task):
    readings, errors = [], []
    stopped = threading.Event()
    def monitor():
        while not stopped.is_set():
            try:
                readings.append({"at_monotonic": time.monotonic(), "rss_bytes": memory_snapshot(nodes),
                    "host_load_average": list(os.getloadavg()),
                    "host_process_cpu_percent_sum": sum(float(value) for value in command("ps", "-A", "-o", "pcpu=").split())})
            except (subprocess.CalledProcessError, ValueError) as error:
                errors.append(type(error).__name__)
                return
            stopped.wait(0.1)
    thread = threading.Thread(target=monitor, daemon=True)
    thread.start()
    start = time.monotonic()
    try:
        samples = task()
    finally:
        stopped.set()
        thread.join(timeout=5)
    if errors or not readings:
        raise RuntimeError("gateway RSS measurement failed")
    elapsed = time.monotonic() - start
    failures = sum(r["status"] != 200 or r["error"] is not None or
                   (not r["done"] and "cancelled_at" not in r) for r in samples)
    return {"duration_seconds": elapsed, "requests": len(samples), "requests_per_second": len(samples) / elapsed,
        "latency_ms": distribution([r["latency_ms"] for r in samples]),
        "ttfb_ms": distribution([r["ttfb_ms"] for r in samples if "ttfb_ms" in r]) if any("ttfb_ms" in r for r in samples) else None,
        "errors": failures, "error_rate": failures / len(samples),
        "rss_peak_bytes_per_gateway": [max(row["rss_bytes"][i] for row in readings) for i in range(len(nodes))],
        "rss_peak_bytes_combined": max(sum(row["rss_bytes"]) for row in readings),
        "rss_samples": readings, "samples": samples}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--build-description", required=True, help="Exact build command/profile used by caller")
    parser.add_argument("--output", required=True, type=Path, help="New evidence directory")
    parser.add_argument("--redis-image", default="redis:7-alpine")
    parser.add_argument("--duration", type=float, default=60)
    parser.add_argument("--concurrency", type=int, default=8)
    parser.add_argument("--stream-seconds", type=float, default=60)
    parser.add_argument("--stream-concurrency", type=int, default=4)
    parser.add_argument("--read-delay", type=float, default=0.1)
    parser.add_argument("--frame-bytes", type=int, default=4096)
    parser.add_argument("--frame-interval", type=float, default=0.05)
    parser.add_argument("--cancel-after", type=float, default=2)
    parser.add_argument("--recovery-timeout", type=float, default=30)
    args = parser.parse_args()
    if min(args.duration, args.stream_seconds, args.concurrency, args.stream_concurrency,
           args.frame_bytes, args.frame_interval, args.cancel_after, args.recovery_timeout) <= 0 or args.read_delay < 0:
        parser.error("durations, sizes and concurrency must be positive; read-delay nonnegative")
    binary = args.binary.resolve(strict=True)
    args.output.mkdir(parents=True, exist_ok=False)
    nodes, backend, upstream, container = [], None, None, None
    artifact = {"schema_version": 1, "captured_at": datetime.now(timezone.utc).isoformat(), "completed": False,
        "source": {**source_evidence(),
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "build_description": args.build_description,
            "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()},
        "environment": {"os": platform.platform(), "architecture": platform.machine(), "logical_cpus": os.cpu_count(),
            "hardware": hardware_evidence(), "python": platform.python_version(), "pyyaml": yaml.__version__, "docker": command("docker", "version", "--format", "{{.Server.Version}}")},
        "workload": {**{k: v for k, v in vars(args).items() if k not in ("binary", "output", "build_description")},
            "protocol": "HTTP/1.1; fresh connection per request; closed-loop clients", "max_tokens": 8192, "gateway_log_level": "warn",
            "stream_usage": "synthetic final usage included", "rss_sampling_seconds": 0.1},
        "measurement_context": "Shared active host; other repository work/builds may overlap. Exploratory deployment-profile measurements, not an idle-host ceiling or before/after speedup",
        "cpu_sampling_method": "Sum ps -A -o pcpu= across host processes; OS-reported recent process CPU percentages, may exceed 100 across cores, sampled with load average and RSS at ~100 ms",
        "boundary": "Two real gateway processes, JWT/session auth, shared Redis provider/model budgets, shared file-backed SQLite terminal ledger; local synthetic upstream, no real LLM or public network", "results": {}}
    temporary = tempfile.TemporaryDirectory(prefix="litellm-deployment-bench-")
    directory = Path(temporary.name)
    try:
        redis_port = free_port()
        container = command("docker", "run", "-d", "--rm", "-p", f"127.0.0.1:{redis_port}:6379",
            args.redis_image, "redis-server", "--save", "", "--appendonly", "no")
        artifact["environment"]["redis_image_id"] = command("docker", "inspect", "--format", "{{.Image}}", container)
        artifact["environment"]["redis_version"] = command("docker", "exec", container, "redis-server", "--version")
        def redis_ready():
            nonlocal backend
            try:
                backend = Redis(("127.0.0.1", redis_port))
                return backend.command("PING") == "PONG"
            except OSError:
                if backend:
                    backend.close()
                return False
        wait_for(redis_ready)
        upstream = Upstream(args.stream_seconds, args.frame_bytes, args.frame_interval)
        config = yaml.safe_load((ROOT / "scripts/bench/gateway-overhead.yaml").read_text())
        config["server"].update(workers=2)
        config["providers"][0].update(name="deployment-benchmark", provider_type="openai",
            api_key="sk-synthetic-local-mock-only-not-a-real-key", models=["gpt-4"],
            base_url=f"http://127.0.0.1:{upstream.server_port}/v1", timeout=180)
        database = directory / "ledger.sqlite"
        config["storage"]["database"].update(enabled=True, url=f"sqlite://{database}?mode=rwc",
            auto_migrate=True, max_connections=8, connection_timeout=10, fallback_to_sqlite=False)
        config["storage"]["redis"].update(enabled=True, url=f"redis://127.0.0.1:{redis_port}",
            max_connections=16, connection_timeout=5, allow_degraded=False)
        config["storage"]["files"] = {"storage_type": "local", "local_path": str(directory / "files")}
        config["storage"]["request_ledger"] = {"enabled": True, "write_failure": "fail", "retention_days": 1}
        config["auth"].update(enable_jwt=True, enable_api_key=False, allow_anonymous=False,
            jwt_secret="Aa1!" + secrets.token_urlsafe(48))
        config["pricing"].update(unpriced_model_policy="reject", unpriced_fallback_cost_per_1k_tokens=None)
        artifact["configuration"] = copy.deepcopy(config)
        artifact["configuration"]["auth"]["jwt_secret"] = "<ephemeral-redacted>"
        artifact["configuration"]["storage"]["files"]["local_path"] = "<isolated-temp-directory>/files"
        artifact["configuration"]["storage"]["database"]["url"] = "sqlite://<isolated-temp-file>?mode=rwc"
        artifact["configuration"]["storage"]["redis"]["url"] = "redis://127.0.0.1:<owned-container-port>"
        for index in range(2):
            nodes.append(Gateway(binary, config, directory, index))
        username, password = "bench" + secrets.token_hex(8), "Aa1!" + secrets.token_urlsafe(32)
        status, body = nodes[0].request("/auth/register", {"username": username,
            "email": username + "@example.test", "password": password})
        if status != 201:
            raise RuntimeError(f"registration returned HTTP {status}")
        with sqlite3.connect(database) as db:
            db.execute("UPDATE users SET role='admin', status='active', email_verified=1 WHERE username=?", (username,))
        for node in nodes:
            status, body = node.request("/auth/login", {"username": username, "password": password})
            if status != 200:
                raise RuntimeError(f"login returned HTTP {status}")
            node.token = body["data"]["access_token"]
            if node.request("/v1/chat/completions", {"model": "gpt-4", "messages": []}, authenticated=False)[0] != 401:
                raise RuntimeError("unauthenticated request was not rejected")
            for scope, name in [("providers", "deployment-benchmark"), ("models", "gpt-4")]:
                status, _ = node.request("/v1/budget/" + scope, {scope[:-1]: name, "max_budget": 1_000_000})
                if status != 201:
                    raise RuntimeError(f"{scope} budget setup returned HTTP {status}")
        # Warmup outside measured window; ledger must already be real and queryable.
        for _ in range(10):
            warmup = sample(nodes[0], "gpt-4")
            if warmup["error"] or warmup["status"] != 200:
                raise RuntimeError("warmup did not succeed")
        for scope, name in [("provider", "deployment-benchmark"), ("model", "gpt-4")]:
            if backend.command("HLEN", f"litellm-rs:budget:v1:{scope}:{name}") == 0:
                raise RuntimeError(f"warmup did not exercise the Redis {scope} budget")
        if nodes[0].request("/admin/request-ledger?limit=1")[0] != 200:
            raise RuntimeError("authenticated ledger query failed")
        def unary():
            deadline = time.monotonic() + args.duration
            def worker(index):
                rows = []
                while time.monotonic() < deadline:
                    rows.append(sample(nodes[index % 2], "gpt-4"))
                return rows
            with ThreadPoolExecutor(max_workers=args.concurrency) as pool:
                return [row for rows in pool.map(worker, range(args.concurrency)) for row in rows]
        artifact["results"]["authenticated_shared_budget_ledger"] = measured(nodes, unary)
        def streams(cancel):
            with ThreadPoolExecutor(max_workers=args.stream_concurrency) as pool:
                return list(pool.map(lambda i: sample(nodes[i % 2], "gpt-4", stream=True,
                    slow_delay=args.read_delay, cancel_after=args.cancel_after if cancel else None), range(args.stream_concurrency)))
        artifact["results"]["long_stream_slow_consumer"] = measured(nodes, lambda: streams(False))
        cancellation = measured(nodes, lambda: streams(True))
        artifact["results"]["cancelled_slow_consumer"] = cancellation
        for row in cancellation["samples"]:
            if "cancelled_at" not in row:
                continue
            started = row["cancelled_at"]
            deadline = started + args.recovery_timeout
            upstream_finished = ledger_finished = None
            while time.monotonic() < deadline:
                now = time.monotonic()
                with upstream.lock:
                    event = upstream.events.get(row["request_id"], {}).copy()
                if event.get("outcome") == "disconnected":
                    upstream_finished = event["finished"]
                with sqlite3.connect(database) as db:
                    terminal = db.execute("SELECT terminal_status, cost FROM request_ledger WHERE request_id=?", (row["request_id"],)).fetchone()
                if terminal and ledger_finished is None:
                    ledger_finished = now
                    row["terminal_status"], row["ledger_cost"] = terminal
                if upstream_finished and ledger_finished:
                    break
                time.sleep(0.025)
            row["upstream_disconnect_ms"] = None if upstream_finished is None else (upstream_finished - started) * 1000
            row["ledger_terminal_observed_ms"] = None if ledger_finished is None else (ledger_finished - started) * 1000
            row["recovery_timeout_seconds"] = args.recovery_timeout
            row["upstream_outcome"] = upstream.events.get(row["request_id"], {}).get("outcome", "active")
        artifact["results"]["rss_after_recovery_bytes_per_gateway"] = memory_snapshot(nodes)
        artifact["results"]["redis_budget_state"] = {scope: backend.command("HGETALL", f"litellm-rs:budget:v1:{scope}:{name}")
            for scope, name in [("provider", "deployment-benchmark"), ("model", "gpt-4")]}
        measured_rows = [r for key in ("authenticated_shared_budget_ledger", "long_stream_slow_consumer", "cancelled_slow_consumer")
            for r in artifact["results"][key]["samples"]]
        with sqlite3.connect(database) as db:
            db.row_factory = sqlite3.Row
            ledger = []
            for row in measured_rows:
                found = db.execute("SELECT * FROM request_ledger WHERE request_id=?", (row["request_id"],)).fetchone()
                if found:
                    item = dict(found)
                    for column in ("billing", "reconciliation"):
                        if isinstance(item[column], str):
                            item[column] = json.loads(item[column])
                    ledger.append(item)
        artifact["results"]["measured_ledger_rows"] = ledger
        artifact["results"]["measured_ledger_rows_missing"] = len(measured_rows) - len(ledger)
        ledger_by_id = {item["request_id"]: item for item in ledger}
        billing_checks = []
        for row in cancellation["samples"]:
            if "cancelled_at" not in row:
                continue
            status, page = nodes[0].request("/admin/request-ledger?request_id=" + row["request_id"])
            api_item = page.get("items", [None])[0] if status == 200 and page.get("items") else None
            sql_item = ledger_by_id.get(row["request_id"], {})
            billing = sql_item.get("billing") or {}
            checks = {
                "cost_remains_unknown": sql_item.get("cost") is None and bool(sql_item),
                "unknown_reason_recorded": bool(billing.get("unknown_reason")),
                "waiting_state_recorded": bool(billing.get("awaiting_since")),
                "supplier_verification_absent": sql_item.get("reconciliation") is None and bool(sql_item),
                "admin_matches_sql_billing": api_item is not None and api_item.get("billing") == sql_item.get("billing"),
                "admin_wait_duration_visible": api_item is not None and api_item.get("awaiting_duration_ms") is not None,
            }
            for scope in ("provider", "model"):
                reserved = billing.get(scope + "_reserved_amount")
                charged = billing.get(scope + "_charge_amount")
                checks[scope + "_estimate_responsibility_acknowledged"] = (reserved is not None and reserved > 0
                    and charged == reserved and billing.get(scope + "_settlement") == "settled")
            billing_checks.append({"request_id": row["request_id"], "admin_status": status,
                "admin_item": api_item, "checks": checks})
        artifact["results"]["cancelled_billing_observations"] = billing_checks
        artifact["results"]["cancelled_billing_check_errors"] = sum(not all(item["checks"].values()) for item in billing_checks)
        artifact["source_after_run"] = source_evidence()
        artifact["completed"] = True
        # Keep measurements and logs, never JWT secrets, passwords, tokens or the auth database.
        for node in nodes:
            node.stop()
            (args.output / node.log_path.name).write_text(node.log_path.read_text())
        nodes.clear()
    except BaseException as error:
        artifact["failure"] = {"type": type(error).__name__, "message": str(error)}
        raise
    finally:
        for node in nodes:
            node.stop()
            if node.log_path.exists():
                (args.output / node.log_path.name).write_text(node.log_path.read_text())
        for log_path in directory.glob("gateway-*.log"):
            (args.output / log_path.name).write_text(log_path.read_text())
        if backend:
            backend.close()
        if upstream:
            upstream.shutdown()
            upstream.server_close()
        if container:
            subprocess.run(["docker", "rm", "-f", container], check=True, capture_output=True)
        temporary.cleanup()
        (args.output / "result.json").write_text(json.dumps(artifact, indent=2) + "\n")
    print(json.dumps({"completed": artifact["completed"], "artifact": str(args.output / "result.json"),
        "unary_errors": artifact["results"]["authenticated_shared_budget_ledger"]["errors"],
        "stream_errors": artifact["results"]["long_stream_slow_consumer"]["errors"],
        "ledger_rows_missing": artifact["results"]["measured_ledger_rows_missing"],
        "cancelled_billing_check_errors": artifact["results"]["cancelled_billing_check_errors"]}))
    if any(artifact["results"][key]["errors"] for key in ("authenticated_shared_budget_ledger", "long_stream_slow_consumer", "cancelled_slow_consumer")) or artifact["results"]["measured_ledger_rows_missing"] or artifact["results"]["cancelled_billing_check_errors"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
