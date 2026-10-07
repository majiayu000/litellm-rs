#!/usr/bin/env python3
"""Measure the committed budget Lua against an isolated, owned Redis process.

Uses only Python's standard library. No shared Redis endpoint is accepted and no
application credentials or production keys are read. This is a script-latency
measurement, not gateway throughput or a release performance gate.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import socket
import subprocess
import tempfile
import threading
import time


class Redis:
    def __init__(self, address: tuple[str, int]):
        self.socket = socket.create_connection(address, timeout=15)
        self.socket.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        self.reader = self.socket.makefile("rb")

    def close(self):
        self.reader.close()
        self.socket.close()

    def read(self):
        line = self.reader.readline()
        if not line.endswith(b"\r\n"):
            raise RuntimeError("Redis closed an incomplete response")
        kind, value = line[:1], line[1:-2]
        if kind == b"-":
            raise RuntimeError(value.decode())
        if kind == b":":
            return int(value)
        if kind == b"+":
            return value.decode()
        if kind == b"$":
            length = int(value)
            if length == -1:
                return None
            data = self.reader.read(length + 2)
            if len(data) != length + 2 or not data.endswith(b"\r\n"):
                raise RuntimeError("incomplete Redis bulk response")
            return data[:-2].decode()
        if kind == b"*":
            return [self.read() for _ in range(int(value))]
        raise RuntimeError(f"unexpected Redis response type {kind!r}")

    def command(self, *args):
        encoded = [str(arg).encode() for arg in args]
        data = [f"*{len(encoded)}\r\n".encode()]
        for arg in encoded:
            data.extend([f"${len(arg)}\r\n".encode(), arg, b"\r\n"])
        self.socket.sendall(b"".join(data))
        return self.read()


def distribution(values):
    if not values:
        raise RuntimeError("measurement produced no samples")
    ordered = sorted(values)
    result = {"count": len(values), "mean_ms": sum(values) / len(values)}
    for label, percentile in [("p50_ms", 0.5), ("p95_ms", 0.95), ("p99_ms", 0.99)]:
        result[label] = ordered[max(0, math.ceil(len(values) * percentile) - 1)]
    result["max_ms"] = ordered[-1]
    return result


def elapsed_ms(start):
    return (time.perf_counter_ns() - start) / 1_000_000


def seed(client, key, leases, receipts):
    # Half the unfinished identities are live; half have expired into pending.
    live = (leases + 1) // 2
    client.command("HSET", key, "c", receipts * 7, "o", live * 10, "e", 1)
    fields = []
    for index in range(leases):
        fields.extend([f"{'l' if index % 2 == 0 else 'p'}:history-{index}", "10:1700000600000:1"])
        if len(fields) >= 2000:
            client.command("HSET", key, *fields)
            fields = []
    for index in range(receipts):
        fields.extend([f"r:receipt-{index}", "7"])
        if len(fields) >= 2000:
            client.command("HSET", key, *fields)
            fields = []
    if fields:
        client.command("HSET", key, *fields)
    assert client.command("HLEN", key) == leases + receipts + 3
    return receipts * 7, live * 10


def operation(client, script, key, op, identity, *, amount=10, value=7, now=1700000000000):
    return client.command("EVALSHA", script, 1, key, op, now, 1, amount, value, 0, identity, 600000, 65536)


def verify_terminal_contract(client, script):
    key = "budget-history-contract"
    assert operation(client, script, key, "reserve", "late", value=100) == [1, 0, 10]
    later = 1700000000000 + 600001
    assert operation(client, script, key, "settle", "late", now=later) == [1, 7, 0]
    assert operation(client, script, key, "settle", "late", now=later) == [1, 7, 0]
    assert operation(client, script, key, "settle_response", "receipt", now=later) == [1, 14, 0]
    assert operation(client, script, key, "settle_response", "receipt", now=later) == [0, 14, 0]
    client.command("DEL", key)
    seed(client, key, 65536, 1000)
    assert operation(client, script, key, "reserve", "full", value=10**12)[0] == -2
    assert client.command("HEXISTS", key, "l:full") == 0
    operation(client, script, key, "settle", "history-1", now=later)
    assert operation(client, script, key, "reserve", "after-release", value=10**12, now=later)[0] == 1
    client.command("DEL", key)


def measure(client, address, script, leases, receipts, samples, warmup):
    key = f"budget-history-{leases}-{receipts}"
    committed, outstanding = seed(client, key, leases, receipts)
    stop = threading.Event()
    ping_values, ping_errors = [], []
    ready = threading.Event()

    def heartbeat():
        monitor = None
        try:
            monitor = Redis(address)
            ready.set()
            while not stop.is_set():
                start = time.perf_counter_ns()
                if monitor.command("PING") != "PONG":
                    raise RuntimeError("unexpected heartbeat response")
                ping_values.append(elapsed_ms(start))
                stop.wait(0.001)
        except Exception as error:
            ping_errors.append(str(error))
            ready.set()
        finally:
            if monitor:
                monitor.close()

    thread = threading.Thread(target=heartbeat, daemon=True)
    reserve_values, settle_values = [], []
    # Warmup is outside the observed heartbeat window and reported sample set.
    for index in range(warmup):
        identity = f"warmup-{index}"
        assert operation(client, script, key, "reserve", identity, value=10**12) == [1, committed, outstanding + 10]
        committed += 7
        assert operation(client, script, key, "settle", identity) == [1, committed, outstanding]
    thread.start()
    if not ready.wait(5):
        raise RuntimeError("heartbeat failed to start")
    try:
        for index in range(samples):
            identity = f"sample-{index}"
            start = time.perf_counter_ns()
            result = operation(client, script, key, "reserve", identity, value=10**12)
            reserve_values.append(elapsed_ms(start))
            assert result == [1, committed, outstanding + 10], result
            start = time.perf_counter_ns()
            result = operation(client, script, key, "settle", identity)
            settle_values.append(elapsed_ms(start))
            committed += 7
            assert result == [1, committed, outstanding], result
    finally:
        stop.set()
        thread.join(timeout=20)
    if thread.is_alive() or ping_errors:
        raise RuntimeError(f"heartbeat failed: {ping_errors}")
    assert client.command("HLEN", key) == leases + receipts + 3
    memory_bytes = client.command("MEMORY", "USAGE", key)
    client.command("DEL", key)
    return {
        "unfinished": leases, "live": (leases + 1) // 2, "pending": leases // 2,
        "receipts": receipts, "hash_memory_bytes": memory_bytes,
        "reserve": distribution(reserve_values), "settle": distribution(settle_values),
        "concurrent_ping": distribution(ping_values),
        "samples_ms": {"reserve": reserve_values, "settle": settle_values, "ping": ping_values},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--redis-server", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--samples", type=int, default=200)
    parser.add_argument("--warmup", type=int, default=10)
    args = parser.parse_args()
    if args.samples < 100 or args.warmup < 1:
        parser.error("use at least 100 measured samples and one warmup")
    if args.output.exists():
        parser.error("output already exists; choose a new evidence path")
    binary = args.redis_server.resolve(strict=True)
    root = Path(__file__).resolve().parents[2]
    source_path = root / "src/storage/redis/budget.rs"
    source = source_path.read_bytes()
    subprocess.run(["git", "diff", "--quiet", "HEAD", "--", str(source_path)], cwd=root, check=True)
    source_commit = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    match = re.search(rb'const BUDGET_LEASE_SCRIPT: &str = r#"(.*?)"#;', source, re.S)
    if not match:
        raise RuntimeError("cannot locate the exact production budget Lua")
    lua = match[1].decode()
    result = {
        "schema_version": 1, "source_commit": source_commit,
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "lua_sha256": hashlib.sha256(match[1]).hexdigest(),
        "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "redis_version": subprocess.check_output([str(binary), "--version"], text=True).strip(),
        "redis_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
        "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "platform": platform.platform(), "logical_cpus": os.cpu_count(),
        "samples_per_operation": args.samples, "warmup_pairs": args.warmup,
        "scope": "production Lua; isolated loopback Redis; sequential reserve/settle with concurrent 1ms PING; no gateway/network throughput claim",
        "rows": [],
    }
    with tempfile.TemporaryDirectory(prefix="lit-budget-") as directory:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as port_probe:
            port_probe.bind(("127.0.0.1", 0))
            address = port_probe.getsockname()
        with (Path(directory) / "redis.log").open("w+") as log:
            server = subprocess.Popen([
                str(binary), "--bind", "127.0.0.1", "--port", str(address[1]), "--protected-mode", "yes",
                "--save", "", "--appendonly", "no", "--dir", directory,
            ], stdout=log, stderr=subprocess.STDOUT)
            client = None
            try:
                deadline = time.monotonic() + 10
                while client is None and time.monotonic() < deadline:
                    if server.poll() is not None:
                        raise RuntimeError("owned Redis process exited during startup")
                    try:
                        client = Redis(address)
                    except (FileNotFoundError, ConnectionRefusedError):
                        time.sleep(0.02)
                if client is None:
                    raise RuntimeError("owned Redis process failed to become ready")
                server_info = dict(line.split(":", 1) for line in client.command("INFO", "server").splitlines() if ":" in line)
                if int(server_info["process_id"]) != server.pid:
                    raise RuntimeError("listener is not this benchmark's owned Redis process")
                script = client.command("SCRIPT", "LOAD", lua)
                verify_terminal_contract(client, script)
                result["terminal_contract"] = "late settlement, duplicate settlement, durable replay, capacity rejection and terminal release passed"
                for leases in (1000, 10000, 65000):
                    for receipts in (0, 10000, 65000):
                        row = measure(client, address, script, leases, receipts, args.samples, args.warmup)
                        result["rows"].append(row)
                        print(json.dumps({key: value for key, value in row.items() if key != "samples_ms"}), flush=True)
            finally:
                if client:
                    client.close()
                server.terminate()
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait(timeout=5)
    if source_path.read_bytes() != source:
        raise RuntimeError("budget source changed during measurement")
    result["completed_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        json.dump(result, output, indent=2)
        output.write("\n")


if __name__ == "__main__":
    main()
