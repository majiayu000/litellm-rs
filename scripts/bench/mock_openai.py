#!/usr/bin/env python3
"""Deterministic OpenAI-compatible upstream for gateway overhead benchmarks."""

from __future__ import annotations

import argparse
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


CHAT_RESPONSE: dict[str, Any] = {
    "id": "chatcmpl-benchmark",
    "object": "chat.completion",
    "created": 1700000000,
    "model": "benchmark-model",
    "choices": [
        {
            "index": 0,
            "message": {"role": "assistant", "content": "pong"},
            "finish_reason": "stop",
        }
    ],
    "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2},
}
CHAT_RESPONSE_BYTES = json.dumps(
    CHAT_RESPONSE, separators=(",", ":"), sort_keys=True
).encode("utf-8")
HEALTH_RESPONSE_BYTES = b'{"status":"ok"}'


class MockOpenAIHandler(BaseHTTPRequestHandler):
    """Serve one fixed chat response without timestamps, randomness, or delay."""

    protocol_version = "HTTP/1.1"

    def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path != "/health":
            self._send_json(404, b'{"error":"not_found"}')
            return
        self._send_json(200, HEALTH_RESPONSE_BYTES)

    def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler API
        if self.path != "/v1/chat/completions":
            self._send_json(404, b'{"error":"not_found"}')
            return

        content_length = self.headers.get("content-length")
        if content_length is None:
            self._send_json(411, b'{"error":"content_length_required"}')
            return
        try:
            payload = json.loads(self.rfile.read(int(content_length)))
        except (ValueError, json.JSONDecodeError):
            self._send_json(400, b'{"error":"invalid_json"}')
            return
        if not isinstance(payload, dict):
            self._send_json(400, b'{"error":"invalid_request"}')
            return

        self._send_json(200, CHAT_RESPONSE_BYTES)

    def _send_json(self, status: int, body: bytes) -> None:
        self.send_response(status)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format: str, *args: object) -> None:
        del format, args


class MockOpenAIServer(ThreadingHTTPServer):
    daemon_threads = True
    allow_reuse_address = True


def create_server(host: str, port: int) -> MockOpenAIServer:
    return MockOpenAIServer((host, port), MockOpenAIHandler)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=18000)
    args = parser.parse_args()
    server = create_server(args.host, args.port)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        raise SystemExit(0) from None
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
