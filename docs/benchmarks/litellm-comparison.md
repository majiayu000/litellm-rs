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

Both gateways use four workers with storage, auth, caching, rate limits, guardrails and spend logging disabled. The Rust side starts from `gateway-overhead.yaml` and selects the OpenAI adapter with the priced `gpt-4` ID; the Python side uses the same ID through LiteLLM's documented OpenAI-compatible routing. Both base URLs point only to the mock. This avoids the per-request unpriced-model error logging of the original vLLM fixture. The mock has a 256-connection accept backlog for the 64-client burst. Only local mock credentials are present. Runtime provider credentials, database URLs and proxy environment variables are not inherited. The cost map is local and telemetry is disabled. No paid provider is contacted.

Each of three rounds measures the direct mock baseline and both gateways, with a ten-second warmup and sixty-second sample at concurrency 64. Rust/Python order alternates. Ports are selected from 5567 upward, avoiding occupied listeners. Every gateway is freshly started for each sample and stopped afterwards. `--seconds`, `--warmup`, `--rounds` and `--concurrency` are available for diagnostic runs; their actual values are recorded and different workloads must not be compared as equivalent.

`comparison.json` links all raw oha and memory-sample files. It preserves non-200 responses and transport errors rather than dropping failed samples. `complete` means the run finished; `all_requests_succeeded` separately verifies warmup and measured HTTP success. An interrupted or failed run keeps logs and partial evidence with `complete: false`. Do not compare only successful subsets or select the best round.

Memory is the sampled sum of RSS for the gateway parent and descendants at 100 ms intervals. It may double-count shared pages and miss shorter peaks; it is not unique memory. For the direct baseline the measured process is the mock server. Host CPU utilization and load are saved alongside memory. The mock, load generator and gateway share CPU, so a bottleneck in the baseline or other workstation activity limits interpretation. Startup/import time is excluded from throughput and latency, while loaded process memory is included.

Read p50/p95/p99, throughput, errors and memory together. Report all rounds and the exact configuration before drawing conclusions. Raw artifacts can be large; keep them as run artifacts and link a measured report here only after verification.

Validation: `python -m unittest discover -s scripts/test -p test_litellm_comparison.py -v` in the pinned environment exercises error preservation, memory units, occupied ports, partial-evidence retention and child cleanup. Existing benchmark contract tests remain in `scripts/test/test_gateway_overhead_benchmark.py`.

References: [LiteLLM configuration](https://docs.litellm.ai/docs/proxy/configs), [LiteLLM CLI](https://docs.litellm.ai/docs/proxy/cli), [oha 1.16.0](https://github.com/hatoo/oha/releases/tag/v1.16.0).

The first diagnostic run on 2026-10-03 (source `4682f4f8`) was rejected as comparison evidence: pausing the sampler left only ten seconds of RSS observations in the first 60-second baseline, and the unpriced Rust fixture wrote errors per request. Its raw artifacts remain at `/tmp/litellm-comparison-20261003`; the corrected run below used a new directory and completed without any sampling interruption.

## Recorded run: 2026-10-03

The corrected run completed all three rounds (including warmups) with only HTTP 200 responses and no transport errors. The source commit is `93f4e09049aeb19c3f0a931e0a7403937e4d2323`; the clean release binary SHA-256 is `7c9674e3f48c7dc61297fb19854b5868a7c63df835575c5aa3553af30676fd3f`. Host: macOS 26.5.1, arm64, 16 logical CPUs and 128 GiB RAM. Both gateways use the `gpt-4` model ID **against the same localhost mock**, not a real model API.

| Round | Path | Requests/s | p50 ms | p95 ms | p99 ms | Peak process-tree RSS MiB | Mean host CPU |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | upstream | 20580 | 2.38 | 8.02 | 11.57 | 25.3 | 18.3% |
| 1 | rust | 18272 | 2.63 | 8.71 | 12.23 | 95.1 | 20.2% |
| 1 | litellm | 1069 | 57.52 | 69.78 | 147.24 | 1716.2 | 21.6% |
| 2 | upstream | 20141 | 2.31 | 8.32 | 11.97 | 26.6 | 14.7% |
| 2 | litellm | 883 | 69.49 | 87.89 | 171.51 | 1719.0 | 30.3% |
| 2 | rust | 18152 | 2.70 | 8.72 | 12.41 | 97.0 | 21.6% |
| 3 | upstream | 20297 | 2.37 | 8.14 | 11.71 | 26.7 | 17.8% |
| 3 | rust | 18159 | 2.68 | 8.73 | 12.35 | 96.9 | 22.7% |
| 3 | litellm | 1110 | 56.51 | 64.08 | 82.06 | 1721.2 | 21.7% |

These are observed values for this workload, not a general speedup claim. Rust runs close to the direct-upstream ceiling here, so the mock/load-generator capacity also constrains the result. The host was a shared workstation, not an isolated benchmark machine; other processes were present and the second Python sample had higher whole-host CPU utilization. All rounds are retained. This task ran no local builds or tests during measurement. The runner was held only after starting the release build and resumed before any sample; none of the corrected samples were paused.

The nominal 100 ms RSS interval yielded 432–466 samples over each 60-second measurement due to process-tree inspection and scheduling. RSS sums are not unique memory. Gateway error logs were empty for all three Rust samples; Python startup logs and every load-generator result are included in the evidence. Startup time, TLS, authentication, streaming, storage and content checks were not benchmarked.

[Download the compressed evidence bundle](litellm-comparison-20261003.json.gz). It contains the full comparison, raw oha JSON, all warmup/measurement RSS and host-load samples, launch/build commands, configs, Python dependency freeze and service/build logs. It also records the rejected first diagnostic run and its truncated baseline RSS sample. Bundle SHA-256: `f282e7b05d80d1aa6ada237e43b79ca0050be10a91c985c105adc8a66a1637f6`. Decompress as UTF-8 JSON; raw file references in `comparison` resolve against `json_files` and `text_files` in the bundle.

### Review-time metadata and runner corrections

The host CPU was identified as **Apple M3 Max** during review on the same workstation. The archived run predates automatic CPU-model capture and the external Cargo-config preflight: the home Cargo directory and checkout ancestors were checked during review and contained no Cargo config, but this was not captured at measurement time. The historical evidence bundle is preserved unchanged; do not interpret this later check as contemporaneous evidence.

The archived mock command can be reconstructed from its pinned runner source and saved mock port as `/private/tmp/litellm-parity-bench-venv/bin/python /Users/lifcc/.codex/worktrees/litellm-bedrock-catalog/litellm-rs/scripts/bench/mock_openai.py --port 5567`; it was omitted from that run's `launch_commands`. New runs record the actual mock invocation together with both gateway invocations, capture the CPU model, and reject external Cargo configuration before building. Failed samples now enter the report before measurement with raw, stderr and RSS references; cleanup also terminates worker groups whose launcher has already exited. Nine targeted runner tests pass, including Linux ARM implementer/part/revision capture for heterogeneous processors. These changes do not alter the historical numbers or claim a new benchmark run.
