#!/usr/bin/env python3
"""Exercise two real gateway processes against isolated PostgreSQL and Redis services."""
import argparse
import concurrent.futures
import copy
import http.client
import json
import os
from pathlib import Path
import secrets
import socket
import subprocess
import threading
import time
import urllib.error
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

import redis
import yaml


def free_port():
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def wait_until(predicate, timeout=10):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.05)
    raise AssertionError("condition did not converge within its deadline")


class Upstream(ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        self.mode = "ok"
        self.calls = 0
        self.lock = threading.Lock()
        self.entered = threading.Event()
        self.release = threading.Event()
        self.release.set()
        super().__init__(("127.0.0.1", 0), Handler)
        threading.Thread(target=self.serve_forever, daemon=True).start()

    def hold(self):
        self.entered.clear()
        self.release.clear()
        self.mode = "hold"


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_args):
        return  # HTTP access data is not needed; gateway logs are retained.

    def do_POST(self):
        request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        with self.server.lock:
            self.server.calls += 1
        mode = self.server.mode
        self.server.entered.set()
        if mode == "hold" and not self.server.release.wait(660):
            raise TimeoutError("test did not release held upstream request")
        status = 429 if mode == "fail" else 200
        body = {"error": {"message": "test upstream rate limit", "type": "rate_limit_error"}} if status == 429 else {
            "id": "ha-response", "object": "chat.completion", "created": 0,
            "model": request["model"], "choices": [{"index": 0, "message": {
                "role": "assistant", "content": "ok"}, "finish_reason": "stop"}],
            "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
        }
        data = json.dumps(body).encode()
        try:
            self.send_response(status)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except (BrokenPipeError, ConnectionResetError):
            # The owner-kill scenario deliberately removes this HTTP peer.
            print("Mock peer disconnected after gateway termination", flush=True)


class Gateway:
    def __init__(self, binary, config, key, output, name):
        self.last_revision = None
        self.port = free_port()
        config = copy.deepcopy(config)
        config["server"]["port"] = self.port
        self.config_path = output / f"{name}.yaml"
        self.config_path.write_text(yaml.safe_dump(config))
        self.log = (output / f"{name}.log").open("w")
        env = os.environ.copy()
        env["LITELLM_HA_CONFIG_KEY"] = key
        self.process = subprocess.Popen([str(binary), "--config", str(self.config_path)],
                                        env=env, stdout=self.log, stderr=subprocess.STDOUT)
        def live():
            if self.process.poll() is not None:
                raise AssertionError(f"{name} exited during startup; inspect its gateway log")
            try:
                return self.request("/health")[0] == 200
            except (OSError, urllib.error.URLError):
                return False
        try:
            wait_until(live, 30)
        except BaseException:
            self.stop()
            self.config_path.unlink(missing_ok=True)
            raise

    def request(self, path, body=None, method=None):
        request = urllib.request.Request(f"http://127.0.0.1:{self.port}{path}",
            data=None if body is None else json.dumps(body).encode(), method=method,
            headers={"Content-Type": "application/json"})
        try:
            response = urllib.request.urlopen(request, timeout=25)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            content = response.read().decode("utf-8", errors="replace")
            try:
                return response.status, json.loads(content)
            except json.JSONDecodeError as error:
                raise AssertionError(
                    f"{path}: HTTP {response.status} returned non-JSON: {content[:500]!r}"
                ) from error

    def chat(self, model):
        return self.request("/v1/chat/completions", {"model": model,
            "messages": [{"role": "user", "content": "hello"}], "max_tokens": 100})

    def revision(self):
        status, body = self.request("/admin/routing/revision")
        assert status == 200, (status, body)
        self.last_revision = body
        return body

    def stop(self, kill=False):
        if self.process.poll() is None:
            self.process.kill() if kill else self.process.terminate()
        try:
            self.process.wait(timeout=15)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=5)
        self.log.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    database_url = os.environ["HA_DATABASE_URL"]
    redis_url = os.environ["REDIS_URL"]
    backend = redis.Redis.from_url(redis_url, decode_responses=True, socket_timeout=5)
    backend.ping()
    root = Path(__file__).resolve().parents[2]
    config = yaml.safe_load((root / "scripts/bench/gateway-overhead.yaml").read_text())
    prefix = "ha-" + secrets.token_hex(6)
    model = prefix + "-budget"
    upstream = Upstream()
    config["providers"][0].update(name=prefix, models=[model], timeout=300,
        base_url=f"http://127.0.0.1:{upstream.server_port}/v1")
    config["storage"]["database"].update(enabled=True, url=database_url, auto_migrate=True)
    config["storage"]["redis"].update(enabled=True, url=redis_url, allow_degraded=False)
    config["storage"]["config_sync_key_env"] = "LITELLM_HA_CONFIG_KEY"
    config["pricing"]["unpriced_fallback_cost_per_1k_tokens"] = 1000.0
    key = secrets.token_hex(32)
    nodes = []
    results = {}
    capture = backend.pubsub()
    capture.subscribe("litellm-rs:config:revisions")
    try:
        a = Gateway(args.binary.resolve(), config, key, args.output, "node-a")
        nodes.append(a)
        # Both nodes start before any authoritative snapshot; B initially has a wrong key.
        b = Gateway(args.binary.resolve(), config, secrets.token_hex(32), args.output, "node-b-wrong-key")
        nodes.append(b)
        status, response = a.request(f"/admin/providers/{prefix}", {"weight": 2}, "PATCH")
        assert status == 200, (status, response)
        revision = response["generation"]
        wait_until(lambda: b.revision()["last_apply_error"] is not None)
        assert b.revision()["active_revision"] == 0
        assert a.revision()["active_revision"] == revision
        assert a.chat(model)[0] == 200
        results["failed_node_preserves_old_revision"] = True
        b.stop()
        b = Gateway(args.binary.resolve(), config, key, args.output, "node-b-recovered")
        nodes.append(b)
        assert b.revision()["active_revision"] == revision
        before = b.revision()
        backend.publish("litellm-rs:config:revisions", str(revision))
        backend.publish("litellm-rs:config:revisions", "0")
        time.sleep(1.1)
        assert b.revision() == before
        # Drop Pub/Sub connections; both nodes must reconnect and fetch the authority.
        subscribers = [client for client in backend.client_list()
                       if int(client.get("sub", 0)) > 0 or int(client.get("psub", 0)) > 0]
        assert len(subscribers) >= 3, "both gateways and the capture must be subscribed"
        for client in subscribers:
            backend.client_kill_filter(_id=client["id"])
        status, response = a.request(f"/admin/providers/{prefix}", {"weight": 3}, "PATCH")
        assert status == 200, (status, response)
        revision = response["generation"]
        wait_until(lambda: b.revision()["active_revision"] == revision)
        old_subscriber_ids = {client["id"] for client in subscribers}
        wait_until(lambda: len([client for client in backend.client_list()
            if int(client.get("sub", 0)) > 0 and client["id"] not in old_subscriber_ids]) >= 2)
        results["revision_reconnect_converges"] = True
        # Capture a fresh notification explicitly after reconnection.
        capture.close()
        capture = backend.pubsub()
        capture.subscribe("litellm-rs:config:revisions")
        wait_until(lambda: capture.get_message(timeout=1) is not None)
        status, response = a.request(f"/admin/providers/{prefix}", {"weight": 4}, "PATCH")
        assert status == 200, (status, response)
        revision = response["generation"]
        message = capture.get_message(ignore_subscribe_messages=True, timeout=5)
        assert message is not None and message["data"] == str(revision), message
        wait_until(lambda: b.revision()["active_revision"] == revision)
        results["notification_contains_only_revision_id"] = True

        # Two requests each reserve ~100 output tokens; the 150 budget permits only one.
        for node in [a, b]:
            status, body = node.request("/v1/budget/models", {"model": model, "max_budget": 150})
            assert status == 201, (status, body)
        upstream.hold()
        with concurrent.futures.ThreadPoolExecutor() as executor:
            first = executor.submit(a.chat, model)
            assert upstream.entered.wait(10)
            calls = upstream.calls
            assert b.chat(model)[0] == 402
            assert upstream.calls == calls
            upstream.release.set()
            assert first.result(timeout=10)[0] == 200
        assert b.chat(model)[0] == 200
        results["budget_cannot_double_spend_and_settlement_releases_hold"] = True

        def add_provider(suffix):
            provider = copy.deepcopy(config["providers"][0])
            provider.update(name=prefix + "-" + suffix, models=[prefix + "-" + suffix],
                            max_concurrent_requests=1)
            status, body = a.request("/admin/providers", provider)
            assert status == 200, (status, body)
            wait_until(lambda: b.revision()["active_revision"] == body["generation"])
            return provider["models"][0]

        limited = add_provider("admission")
        upstream.hold()
        with concurrent.futures.ThreadPoolExecutor() as executor:
            first = executor.submit(a.chat, limited)
            assert upstream.entered.wait(10)
            calls = upstream.calls
            assert b.chat(limited)[0] == 503
            assert upstream.calls == calls
            upstream.release.set()
            assert first.result(timeout=10)[0] == 200
        assert b.chat(limited)[0] == 200
        results["replicas_do_not_multiply_admission"] = True

        circuit = add_provider("circuit")
        status, body = a.request("/admin/routing/policy", {"circuit_breaker": {
            "failure_threshold": 1, "recovery_timeout": 30, "min_requests": 1,
            "success_threshold": 1}}, "PUT")
        assert status == 200, (status, body)
        wait_until(lambda: b.revision()["active_revision"] == body["generation"])
        upstream.mode = "fail"
        assert a.chat(circuit)[0] != 200
        time.sleep(0.1)
        calls = upstream.calls
        assert b.chat(circuit)[0] == 503
        assert upstream.calls == calls
        upstream.mode = "ok"
        time.sleep(30.1)
        assert b.chat(circuit)[0] == 200
        results["cooldown_converges_and_recovers"] = True

        owner_model = add_provider("owner")
        for node in [a, b]:
            status, body = node.request("/v1/budget/models", {"model": owner_model, "max_budget": 150})
            assert status == 201, (status, body)
        upstream.hold()
        with concurrent.futures.ThreadPoolExecutor() as executor:
            first = executor.submit(a.chat, owner_model)
            assert upstream.entered.wait(10)
            acquired_at = time.monotonic()
            assert b.chat(owner_model)[0] == 503
            a.revision()
            a.stop(kill=True)
            try:
                first.result(timeout=10)
                raise AssertionError("killed request unexpectedly completed")
            except (http.client.RemoteDisconnected, urllib.error.URLError, ConnectionError):
                pass  # Expected outcome of killing the owner process.
            assert b.chat(owner_model)[0] == 503
            print("Owner killed; waiting for the real 600-second lease expiry", flush=True)
            time.sleep(max(0, acquired_at + 602 - time.monotonic()))
            upstream.release.set()
            assert b.chat(owner_model)[0] == 200
        results["killed_owner_lease_recovered_within_620_seconds"] = time.monotonic() - acquired_at < 620
        assert results["killed_owner_lease_recovered_within_620_seconds"]
        (args.output / "results.json").write_text(json.dumps(results, indent=2))
        print(json.dumps(results, indent=2), flush=True)
    finally:
        try:
            # Keep failure evidence even when a request or assertion fails.
            diagnostics = {"results": results, "nodes": []}
            for node in nodes:
                entry = {"pid": node.process.pid, "exit_code": node.process.poll(), "revision": node.last_revision}
                if node.process.poll() is None:
                    try:
                        entry["revision"] = node.revision()
                        entry["routing"] = node.request("/admin/routing/inventory")[1]
                    except (OSError, urllib.error.URLError, ValueError, AssertionError) as error:
                        entry["diagnostic_error"] = type(error).__name__
                diagnostics["nodes"].append(entry)
            try:
                diagnostics["redis"] = {key: backend.hgetall(key) for key in backend.scan_iter(match=f"*{prefix}*")
                                        if backend.type(key) == "hash"}
            except redis.RedisError as error:
                diagnostics["redis_error"] = type(error).__name__
            (args.output / "diagnostics.json").write_text(json.dumps(diagnostics, indent=2))
        finally:
            upstream.release.set()
            for node in nodes:
                node.config_path.unlink(missing_ok=True)  # Config may contain service credentials.
            for node in nodes:
                node.stop()
            capture.close()
            upstream.shutdown()
            upstream.server_close()



if __name__ == "__main__":
    main()
