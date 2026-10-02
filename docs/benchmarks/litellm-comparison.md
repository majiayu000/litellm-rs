# Same-host LiteLLM comparison

This measures one boundary: non-streaming chat through an unauthenticated gateway to the same deterministic local OpenAI-compatible server. It does not establish provider coverage, streaming performance, production security overhead or general superiority.

Use an otherwise idle macOS/Linux host. Create an isolated Python environment and pinned load generator, then run from a clean checkout:

```sh
uv venv --python 3.12 /tmp/litellm-benchmark-venv
uv pip install --python /tmp/litellm-benchmark-venv/bin/python \
  'litellm[proxy]==1.103.2' 'psutil==7.2.2' 'PyYAML==6.0.3'
cargo install oha --version 1.16.0 --locked --root target/bench-tools
/tmp/litellm-benchmark-venv/bin/python scripts/bench/compare_litellm.py \
  /tmp/litellm-comparison-results --oha "$PWD/target/bench-tools/bin/oha"
```

The output directory must not exist and must be outside the source tree. The runner builds the recorded Rust commit in a fresh release target directory with locked dependencies. It verifies pinned tool/package versions, saves every installed Python dependency in `python-freeze.txt`, and records the binary hash, build command, toolchains, host, exact configs, launch commands, workload and raw results. Reproduce the Python environment with the saved freeze file when comparing later runs, because transitive dependencies may change.

Both gateways use four workers with storage, auth, caching, rate limits, guardrails and spend logging disabled. The Rust side reuses `gateway-overhead.yaml`; the Python side follows LiteLLM's documented OpenAI-compatible model routing. Only local mock credentials are present. Runtime provider credentials, database URLs and proxy environment variables are not inherited. The cost map is local and telemetry is disabled. No paid provider is contacted.

Each of three rounds measures the direct mock baseline and both gateways, with a ten-second warmup and sixty-second sample at concurrency 64. Rust/Python order alternates. Ports are selected from 5567 upward, avoiding occupied listeners. Every gateway is freshly started for each sample and stopped afterwards. `--seconds`, `--warmup`, `--rounds` and `--concurrency` are available for diagnostic runs; their actual values are recorded and different workloads must not be compared as equivalent.

`comparison.json` links all raw oha and memory-sample files. It preserves non-200 responses and transport errors rather than dropping failed samples. `complete` means the run finished; `all_requests_succeeded` separately verifies warmup and measured HTTP success. An interrupted or failed run keeps logs and partial evidence with `complete: false`. Do not compare only successful subsets or select the best round.

Memory is the sampled sum of RSS for the gateway parent and descendants at 100 ms intervals. It may double-count shared pages and miss shorter peaks; it is not unique memory. For the direct baseline the measured process is the mock server. Host CPU utilization and load are saved alongside memory. The mock, load generator and gateway share CPU, so a bottleneck in the baseline or other workstation activity limits interpretation. Startup/import time is excluded from throughput and latency, while loaded process memory is included.

Read p50/p95/p99, throughput, errors and memory together. Report all rounds and the exact configuration before drawing conclusions. Raw artifacts can be large; keep them as run artifacts and link a measured report here only after verification.

Validation: `python -m unittest discover -s scripts/test -p test_litellm_comparison.py -v` in the pinned environment exercises error preservation, memory units, occupied ports, partial-evidence retention and child cleanup. Existing benchmark contract tests remain in `scripts/test/test_gateway_overhead_benchmark.py`.

References: [LiteLLM configuration](https://docs.litellm.ai/docs/proxy/configs), [LiteLLM CLI](https://docs.litellm.ai/docs/proxy/cli), [oha 1.16.0](https://github.com/hatoo/oha/releases/tag/v1.16.0).
