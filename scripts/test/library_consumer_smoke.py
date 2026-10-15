#!/usr/bin/env python3
"""Run the documented hello source as an external crate against local HTTP."""
import json
import os
from pathlib import Path
import subprocess
import threading
import tempfile
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = Path(__file__).resolve().parents[2]


def main():
    seen = []

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            seen.append((self.path, body, self.headers.get("Authorization")))
            response = json.dumps({
                "id": "consumer-smoke", "object": "chat.completion", "created": 1,
                "model": body["model"],
                "choices": [{"index": 0, "message": {
                    "role": "assistant", "content": "local consumer passed"
                }, "finish_reason": "stop"}],
                "usage": {"prompt_tokens": 2, "completion_tokens": 3, "total_tokens": 5},
            }).encode()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(response)))
            self.end_headers()
            self.wfile.write(response)

        def log_message(self, *_args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        env = dict(os.environ, CARGO_BUILD_JOBS="2")
        (ROOT / "target").mkdir(exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="library-consumer-", dir=ROOT / "target") as temp:
            consumer = Path(temp)
            manifest = (ROOT / "examples/Cargo.toml").read_text().replace(
                'path = ".."', f'path = {json.dumps(str(ROOT))}'
            )
            (consumer / "Cargo.toml").write_text(manifest)
            (consumer / "hello.rs").write_bytes((ROOT / "examples/hello.rs").read_bytes())
            result = subprocess.run([
                "cargo", "run", "--manifest-path", str(consumer / "Cargo.toml"),
                "--target-dir", str(ROOT / "target"), "--bin", "hello", "--",
                f"http://localhost:{server.server_port}/v1", "smoke-model",
            ], cwd=ROOT, env=env, check=True, timeout=1800, stdout=subprocess.PIPE, text=True)
        assert result.stdout.strip() == "local consumer passed", result.stdout
        assert len(seen) == 1, seen
        path, body, auth = seen[0]
        assert path == "/v1/chat/completions", path
        assert body["model"] == "smoke-model", body
        assert body["messages"] == [{"role": "user", "content": "Hello!"}], body
        assert auth is None, "local example unexpectedly sent credentials"
        print("PASS: external crate, documented source, local HTTP request and response")
    finally:
        server.shutdown()
        server.server_close()
        thread.join()


if __name__ == "__main__":
    main()
