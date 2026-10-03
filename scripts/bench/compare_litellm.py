#!/usr/bin/env python3
"""Same-host non-streaming chat comparison. Run with the pinned benchmark venv."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import platform
import signal
import socket
import statistics
import subprocess
import sys
import time
import urllib.error
import urllib.request

import psutil
import yaml

ROOT = Path(__file__).resolve().parents[2]
BUILD = ["cargo", "build", "--locked", "--release", "--bin", "gateway", "--no-default-features", "--features", "sqlite,redis,metrics,tracing"]
REQUEST = json.dumps({"model": "gpt-4", "messages": [{"role": "user", "content": "ping"}], "stream": False}, separators=(",", ":"))
VERSIONS = {"litellm": "1.103.2", "psutil": "7.2.2", "PyYAML": "6.0.3"}


def command(args: list[str], **kwargs) -> str:
    return subprocess.check_output(args, text=True, cwd=ROOT, **kwargs).strip()


def source_identity() -> str:
    if command(["git", "status", "--porcelain"]):
        raise RuntimeError("benchmark requires a clean source checkout")
    return command(["git", "rev-parse", "HEAD"])


def free_ports(first: int) -> list[int]:
    # Keep reservations together while choosing ports; service startup also fails
    # closed if another process wins the remaining bind race.
    sockets = []
    ports = []
    try:
        for port in range(first, first + 100):
            sock = socket.socket()
            try:
                sock.bind(("127.0.0.1", port))
            except OSError:
                sock.close()
                continue
            sockets.append(sock)
            ports.append(port)
            if len(ports) == 3:
                return ports
        raise RuntimeError("no three free benchmark ports")
    finally:
        for sock in sockets:
            sock.close()


@contextmanager
def service(args: list[str], log: Path, env: dict[str, str]):
    with log.open("w") as output:
        process = subprocess.Popen(args, stdout=output, stderr=subprocess.STDOUT, cwd=log.parent, env=env, start_new_session=True)
        try:
            yield process
        finally:
            # A dead launcher may still have live worker descendants in its session.
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                pass
            finally:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)


def ready(process: subprocess.Popen, url: str) -> None:
    deadline = time.monotonic() + 90
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise RuntimeError(f"service exited with {process.returncode}; inspect saved log")
        try:
            with urllib.request.urlopen(url, timeout=1) as response:
                if response.status == 200:
                    return
        except (OSError, urllib.error.URLError):
            pass
        time.sleep(0.1)
    raise RuntimeError(f"service readiness timeout: {url}")


def probe(url: str) -> dict:
    request = urllib.request.Request(url, data=REQUEST.encode(), headers={"content-type": "application/json"})
    with urllib.request.urlopen(request, timeout=15) as response:
        data = response.read()
        value = json.loads(data)
        if value["choices"][0]["message"]["content"] != "pong" or value["usage"]["total_tokens"] != 2:
            raise RuntimeError("unexpected response semantics")
        return {"response_bytes": len(data), "body": value}


def memory_kib(process: subprocess.Popen) -> int:
    parent = psutil.Process(process.pid)
    total = 0
    for child in [parent, *parent.children(recursive=True)]:
        try:
            total += child.memory_info().rss
        except psutil.NoSuchProcess:
            pass
    return total // 1024


def summarize(raw: dict, samples: list[dict]) -> dict:
    return {
        "requests_per_second": raw["summary"]["requestsPerSec"],
        "latency_ms": {key: raw["latencyPercentiles"][key] * 1000 for key in ("p50", "p95", "p99")},
        "error_rate": 1 - raw["summary"]["successRate"],
        "status_codes": raw["statusCodeDistribution"],
        "transport_errors": raw["errorDistribution"],
        "process_tree_rss_kib": {
            "peak": max(sample["rss_kib"] for sample in samples),
            "median": statistics.median(sample["rss_kib"] for sample in samples),
        },
    }


def measure(oha: str, process: subprocess.Popen, url: str, seconds: int, concurrency: int, output: Path, env: dict) -> dict:
    args = [oha, "--no-tui", "-w", "--output-format", "json", "-m", "POST", "-H", "content-type: application/json", "-d", REQUEST, "-c", str(concurrency), "-z", f"{seconds}s", url]
    samples = []
    started = time.monotonic()
    with output.open("w") as stream, output.with_suffix(".stderr.log").open("w") as errors:
        load = subprocess.Popen(args, stdout=stream, stderr=errors, env=env)
        try:
            while load.poll() is None:
                if process.poll() is not None:
                    raise RuntimeError("gateway exited during measurement")
                samples.append({"elapsed_seconds": time.monotonic() - started, "rss_kib": memory_kib(process), "host_cpu_percent": psutil.cpu_percent(), "load_average": os.getloadavg()})
                if time.monotonic() - started > seconds + 30:
                    raise RuntimeError("load generator exceeded deadline")
                time.sleep(0.1)
            if load.returncode:
                raise RuntimeError(f"load generator exited with {load.returncode}")
        finally:
            if load.poll() is None:
                load.terminate()
                try:
                    load.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    load.kill()
                    load.wait()
            output.with_suffix(".memory.json").write_text(json.dumps(samples, indent=2) + "\n")
    raw = json.loads(output.read_text())
    return {"command": args, "raw": output.name, "memory": output.with_suffix(".memory.json").name, **summarize(raw, samples)}


def reject_external_cargo_config(env: dict[str, str]) -> None:
    directories = [parent / ".cargo" for parent in ROOT.parents]
    directories.append(Path(env.get("CARGO_HOME", str(Path(env["HOME"]) / ".cargo"))).expanduser())
    for directory in dict.fromkeys(directories):
        for name in ("config", "config.toml"):
            if (directory / name).exists():
                raise RuntimeError(f"unrecorded external Cargo configuration: {directory / name}")


def cpu_model() -> str:
    if platform.system() == "Darwin":
        return command(["sysctl", "-n", "machdep.cpu.brand_string"])
    if platform.system() == "Linux":
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith(("model name", "Hardware")):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unavailable"


def record_sample(report, save, args, process, port, output, prefix, round_index, name, env):
    def references(stem):
        return {"raw": f"{stem}.json", "memory": f"{stem}.memory.json", "stderr": f"{stem}.stderr.log"}
    sample = {"round": round_index + 1, "implementation": name, "complete": False,
              "phase": "probe", "warmup": references(f"{prefix}-warmup"), **references(prefix)}
    report["samples"].append(sample)
    save()
    url = f"http://127.0.0.1:{port}/v1/chat/completions"
    try:
        sample["probe"] = probe(url)
        sample["phase"] = "warmup"
        save()
        sample["warmup"].update(measure(args.oha, process, url, args.warmup, args.concurrency, output / f"{prefix}-warmup.json", env))
        sample["phase"] = "measurement"
        save()
        sample.update(measure(args.oha, process, url, args.seconds, args.concurrency, output / f"{prefix}.json", env))
        sample.update(complete=True, phase="complete")
    except BaseException as error:
        sample["failure"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        save()


def run(args: argparse.Namespace) -> None:
    git_sha = source_identity()
    for package, version in VERSIONS.items():
        if importlib.metadata.version(package) != version:
            raise RuntimeError(f"requires {package}=={version}")
    if command([args.oha, "--version"]) != "oha 1.16.0":
        raise RuntimeError("requires oha 1.16.0")
    output = args.output.resolve()
    if output == ROOT or ROOT in output.parents:
        raise RuntimeError("write artifacts outside the source checkout")
    output.mkdir(parents=True, exist_ok=False)
    # Do not inherit provider credentials, routing overrides, proxies, or DB URLs.
    env = {key: os.environ[key] for key in ("PATH", "HOME", "LANG", "TMPDIR", "RUSTUP_HOME", "CARGO_HOME", "RUSTUP_TOOLCHAIN") if key in os.environ}
    env.update({"LITELLM_LOCAL_MODEL_COST_MAP": "True", "LITELLM_TELEMETRY": "False", "DO_NOT_TRACK": "1", "LITELLM_LOG": "ERROR"})
    mock_port, rust_port, python_port = free_ports(args.first_port)
    rust_config = yaml.safe_load((ROOT / "scripts/bench/gateway-overhead.yaml").read_text())
    rust_config["server"].update(port=rust_port, workers=4)
    rust_config["providers"][0].update(provider_type="openai", name="benchmark-openai", models=["gpt-4"], api_key="sk-local-mock-only-not-a-real-key")
    rust_config["providers"][0]["base_url"] = f"http://127.0.0.1:{mock_port}/v1"
    (output / "rust.yaml").write_text(yaml.safe_dump(rust_config))
    python_config = {
        "model_list": [{"model_name": "gpt-4", "litellm_params": {"model": "openai/gpt-4", "api_base": f"http://127.0.0.1:{mock_port}/v1", "api_key": "local-mock-only", "max_retries": 0, "timeout": 10}}],
        "litellm_settings": {"cache": False, "num_retries": 0, "telemetry": False},
        "general_settings": {"disable_spend_logs": True, "background_health_checks": False},
        "router_settings": {"num_retries": 0},
    }
    (output / "python.yaml").write_text(yaml.safe_dump(python_config))
    freeze = sorted(f"{dist.metadata['Name']}=={dist.version}" for dist in importlib.metadata.distributions())
    (output / "python-freeze.txt").write_text("\n".join(freeze) + "\n")
    report = {
        "schema_version": 1, "captured_at": datetime.now(timezone.utc).isoformat(),
        "source": {"git_sha": git_sha, "build_command": BUILD},
        "environment": {"platform": platform.platform(), "machine": platform.machine(), "cpu_model": cpu_model(), "cpus": psutil.cpu_count(), "memory_bytes": psutil.virtual_memory().total, "python": sys.version, "rust": command(["rustc", "-Vv"], env=env), "cargo": command(["cargo", "-V"], env=env), "oha": command([args.oha, "--version"]), "packages": VERSIONS},
        "workload": {"concurrency": args.concurrency, "duration_seconds": args.seconds, "warmup_seconds": args.warmup, "rounds": args.rounds, "workers_each": 4, "request": json.loads(REQUEST), "memory_sample_seconds": 0.1},
        "samples": [], "complete": False,
        "limits": ["Plain non-streaming chat; auth, storage, cache, guardrails and spend logging disabled.", "Local mock and load generator share host CPU with gateway.", "Sampled sum of process-tree RSS can double-count shared pages; not unique memory or a precise allocation peak.", "Results apply only to recorded configurations and host load, not feature parity or paid provider latency."],
    }
    def save():
        (output / "comparison.json").write_text(json.dumps(report, indent=2) + "\n")
    save()
    try:
        reject_external_cargo_config(env)
        report["source"]["external_cargo_config"] = "none; checked before build"
        build_env = dict(env, CARGO_TARGET_DIR=str(output / "cargo-target"))
        with (output / "build.log").open("w") as log:
            subprocess.run(BUILD, cwd=ROOT, env=build_env, stdout=log, stderr=subprocess.STDOUT, check=True)
        binary = output / "cargo-target/release/gateway"
        report["source"]["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
        subprocess.run([str(binary), "--config", str(output / "rust.yaml"), "validate-config"], env=env, cwd=ROOT, check=True, stdout=subprocess.DEVNULL)
        variants = {
            "rust": ([str(binary), "--config", str(output / "rust.yaml"), "--log-level", "error", "serve"], rust_port, "/health"),
            "litellm": ([str(Path(sys.executable).parent / "litellm"), "--config", str(output / "python.yaml"), "--host", "127.0.0.1", "--port", str(python_port), "--num_workers", "4", "--telemetry", "False"], python_port, "/health/liveliness"),
        }
        mock_command = [sys.executable, str(ROOT / "scripts/bench/mock_openai.py"), "--port", str(mock_port)]
        report["launch_commands"] = {"upstream": mock_command, **{name: launch for name, (launch, _, _) in variants.items()}}
        save()
        with service(mock_command, output / "mock.log", env) as mock:
            ready(mock, f"http://127.0.0.1:{mock_port}/health")
            for round_index in range(args.rounds):
                # Alternate order so one implementation is not always measured cold/first.
                order = ["rust", "litellm"] if round_index % 2 == 0 else ["litellm", "rust"]
                for name in ["upstream", *order]:
                    prefix = f"round-{round_index + 1}-{name}"
                    def sample(process, port):
                        record_sample(report, save, args, process, port, output, prefix, round_index, name, env)
                    if name == "upstream":
                        sample(mock, mock_port)
                    else:
                        launch, port, health = variants[name]
                        with service(launch, output / f"{prefix}.log", env) as process:
                            ready(process, f"http://127.0.0.1:{port}{health}")
                            sample(process, port)
        if source_identity() != git_sha:
            raise RuntimeError("source changed during benchmark")
        report["complete"] = True
        report["all_requests_succeeded"] = all(
            result["error_rate"] == 0 and not result["transport_errors"]
            and set(result["status_codes"]) == {"200"}
            for sample in report["samples"] for result in (sample, sample["warmup"])
        )
    except BaseException as error:
        report["failure"] = f"{type(error).__name__}: {error}"
        raise
    finally:
        save()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("--oha", required=True)
    parser.add_argument("--first-port", type=int, default=5567)
    parser.add_argument("--seconds", type=int, default=60)
    parser.add_argument("--warmup", type=int, default=10)
    parser.add_argument("--concurrency", type=int, default=64)
    parser.add_argument("--rounds", type=int, default=3)
    options = parser.parse_args()
    if min(options.seconds, options.warmup, options.concurrency, options.rounds) <= 0:
        parser.error("duration, warmup, concurrency and rounds must be positive")
    run(options)
