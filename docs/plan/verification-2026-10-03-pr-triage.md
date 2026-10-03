# 2026-10-03–04 issues / PR 本地验证记录

这些记录来自本轮独立 worktree 的实际命令输出，保留命令、源码提交、退出码和结果片段。结果仅适用于列出的提交及特性；远端 CI、实际供应商调用和发布单独验收。

[完整命令日志及结果清单](verification-2026-10-03-pr-triage.tar.gz)随本记录入库；下方保留可直接阅读的结果片段与原始日志 SHA-256。归档内 results.json 的日志路径可直接在解压目录使用。

## Perplexity / PR #1414

默认源码提交：`1732c83e`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,providers-extended complete_construction_credential_precedence_matrix`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：177.8 秒。
原始日志 SHA-256：`6a12e8674f536a7cf9048b81b8d3e50af7e79baa11791a1385da41ba9f4ceb99`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 55s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-bf4166c02ae2401a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10176 filtered out; finished in 0.29s
```

### `cargo test --lib --features gateway,sqlite,providers-extended retired_perplexity_chat_selectors_fail_before_transport`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：34.5 秒。
原始日志 SHA-256：`02ecb740eb563e9fa9e148e0b9d0d6024f894ee44d150a9ada4e49abf130cd0e`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.24s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-bf4166c02ae2401a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10176 filtered out; finished in 0.01s
```

### `cargo check`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：46.2 秒。
原始日志 SHA-256：`3a00f21369786b2e8af7253ac435bf7beffc984389cb36f5ddd21bc962fb2acd`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 46.10s
```

### `cargo test`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：205.5 秒。
原始日志 SHA-256：`e329e281d3c8eb770807662d44f80f15d0b7f58f43c865eabb42baa584375687`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 21s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7268 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 98.52s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.52s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`1732c83e507d7968525ec47c0342e8355a914c82`；退出码：`0`；耗时：47.9 秒。
原始日志 SHA-256：`c9bf6b79a9ac0614429d4b3321a271f815df34fd236eb3260a4b608e89e2d9da`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 47.81s
```

## Responses 和 Messages 首轮整合

默认源码提交：`5081ca2b / bb6ecc10`（更精确的每条命令提交见下方）。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes`

源码提交：`5081ca2b5e3e2c04a09b29bb17532d6d8113f59d`；退出码：`0`；耗时：146.6 秒。
原始日志 SHA-256：`0f24e004c951626e9dbe4acb611b9ef24589879499cc6ae08140f1a42d12ff27`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 35s
     Running tests/gemini_sdk_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.15s
     Running tests/native_responses_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.88s
     Running tests/responses_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.47s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`5081ca2b5e3e2c04a09b29bb17532d6d8113f59d`；退出码：`0`；耗时：111.7 秒。
原始日志 SHA-256：`d2d82fc8e19f8d293a74b3c7c97de78c2d17b0ff45d725b09120bfa452a32e7a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 51s
```

### `cargo test --features gateway,sqlite,mcp --test native_messages_routes`

源码提交：`bb6ecc104d419909b11112983e7278abdaef76ed`；退出码：`0`；耗时：82.8 秒。
原始日志 SHA-256：`e60d193dd0c7159f4e164c234117233ed1a1788094f32e577b67f38c17fa56a0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 14s
     Running tests/native_messages_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/native_messages_routes-80799fc45f172daa)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.21s
```

## Realtime 前轮 acknowledgment/settlement 边界 / PR #1405

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：2.9 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests::`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：82.6 秒。
原始日志 SHA-256：`50faf76386f011cc60d277b70c64e7177d62ce09e195faedf3c57d272c84d95e`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 9884 filtered out; finished in 18.34s
```

### `cargo check`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：6.0 秒。
原始日志 SHA-256：`2a0a67ae9d4b12b58d9abb58b185335457502e32a80b385bf06e3b136e794935`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.96s
```

### `cargo test`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：138.0 秒。
原始日志 SHA-256：`63949d5dd637bbce34b7360acf262d5e169b5ab3813293baf148d4e84dc733a4`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 12.54s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 94.01s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.83s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：21.3 秒。
原始日志 SHA-256：`3d7770545b2f2f9084105d0d12cbb54b43c92d5e734bd9e5493fe773924faaeb`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.17s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：387.2 秒。
原始日志 SHA-256：`fe7d618216b1b7c1f53ae1a3d54c447294b847406e264dd925a96b9923b19276`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 51.20s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9910 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 106.69s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.80s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.05s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.16s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.64s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.27s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.63s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.60s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.04s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.28s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.36s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.41s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 35.07s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.43s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.30s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.60s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.64s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.46s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`a23f37e4ecb51d923a9d5e51a13f7d83210a66c2`；退出码：`0`；耗时：42.1 秒。
原始日志 SHA-256：`3958a993b693fdef3667f04a9ba8573e40620483c1f15fe0d6a6a5677440fcaa`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.92s
```

## Realtime 前轮 JWT/budget 修复 / PR #1405

默认源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests::`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：56.6 秒。
原始日志 SHA-256：`a4e6514443a1ac7383b8d0edbaa7fd7c0e1033311c89cf61568f1ad7b37ef383`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 35.66s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 9884 filtered out; finished in 18.21s
```

### `cargo check`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：41.4 秒。
原始日志 SHA-256：`c2aa80c7dc038ee41c50f79482cf5003d83338775b221167c7ee8bbf13560620`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.38s
```

### `cargo test`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：205.8 秒。
原始日志 SHA-256：`6caba784243d0143770bb318a532cb585ba2a18344205fe95558e581689912dd`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 22s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 94.15s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.69s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：48.9 秒。
原始日志 SHA-256：`3a72a115fcd19fecfe950591b070f004767ba4dbf636b0a435687e517381a0a9`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 48.77s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：456.0 秒。
原始日志 SHA-256：`6d21297a697fd3700984a3824605c312a04248f9dd4e63da45d8ed2f753202b8`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 41s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9908 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.68s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.72s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.05s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.12s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.65s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.19s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.07s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.92s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.68s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.54s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.62s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.58s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.01s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 33.60s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.50s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.65s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.53s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.45s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.49s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`8d0ba253cf4e8e98f54c732e8a94dd0916abd38d`；退出码：`0`；耗时：93.0 秒。
原始日志 SHA-256：`567b7d90110e47b7c7eb1bb30faf03d1f5b904c9cf81d65f349abc6d2b210429`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 32s
```

## F06 和 Azure/Voyage 目录整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`e1623efe5f2156b338ce564853929e37d8a631df`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`e1623efe5f2156b338ce564853929e37d8a631df`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`178e8c232717860bb0a00ab195a35f0a69654f63d627e51cf4372f06024e1c90`。

```text
validated 4570 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 52 tests in 0.821s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`e1623efe5f2156b338ce564853929e37d8a631df`；退出码：`0`；耗时：1.6 秒。
原始日志 SHA-256：`3888865fd76862a6115162011965cf9ebbf9dfed12f0b82a9180f0bec1bbe1e7`。

```text
validated 4451 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test catalog_nonchat_routes`

源码提交：`e1623efe5f2156b338ce564853929e37d8a631df`；退出码：`0`；耗时：108.7 秒。
原始日志 SHA-256：`36572f1fea2dd1648998fbfe44f38222a5bcd9f491be6c7a3a75af9efe22e08b`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 15s
     Running tests/catalog_nonchat_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
     Running tests/native_responses_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.94s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`e1623efe5f2156b338ce564853929e37d8a631df`；退出码：`0`；耗时：85.1 秒。
原始日志 SHA-256：`6a7a2e55e15f7cd6e72d01bd845f3484e4e371683a67cd58ddebbaa4abda9020`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 24s
```

### `cargo fmt --check`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：2.8 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`4c0165729c7cb0ca0a500d7ee87e6af0c67552efbe938e337a3f3ef9025e3876`。

```text
validated 4570 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 52 tests in 0.828s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：1.4 秒。
原始日志 SHA-256：`3888865fd76862a6115162011965cf9ebbf9dfed12f0b82a9180f0bec1bbe1e7`。

```text
validated 4451 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a azure_ai`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：129.3 秒。
原始日志 SHA-256：`7b45b668e77be1d94fe878050ea5a4a54db3388eca22e957ff5da2235b1bad62`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 05s
     Running unittests src/lib.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 10409 filtered out; finished in 0.01s
```

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a providers::voyage::`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：50.3 秒。
原始日志 SHA-256：`125dd63eb7b75f131d672edd5911492c5922fb930b20764f8af05d9b5bd45d1f`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 47.71s
     Running unittests src/lib.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 10405 filtered out; finished in 0.01s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：102.3 秒。
原始日志 SHA-256：`f61453cbcd090e0b5c645ec7677f5308da1664e0e0432d0cc7d2c482e892ff88`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 42s
```

## 云兼容和 Responses 前轮整链

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`db89574fd762d346378fcb327142649ff75498ce`；退出码：`0`；耗时：3.0 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes`

源码提交：`db89574fd762d346378fcb327142649ff75498ce`；退出码：`0`；耗时：81.9 秒。
原始日志 SHA-256：`8e35114fdcb78346e1bda47cc0d875f5360a41c0c8240709677d7654d8e10955`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 19s
     Running tests/catalog_nonchat_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`db89574fd762d346378fcb327142649ff75498ce`；退出码：`0`；耗时：100.1 秒。
原始日志 SHA-256：`49291c8865b3296ab9ea5a6b3996a3095dd1c1befaee78285a12defdd4523153`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 39s
```

### `cargo fmt --check`

源码提交：`0050a21bb521fa7723ea9dae139a26754d3932c9`；退出码：`0`；耗时：3.0 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`0050a21bb521fa7723ea9dae139a26754d3932c9`；退出码：`0`；耗时：2.1 秒。
原始日志 SHA-256：`3888865fd76862a6115162011965cf9ebbf9dfed12f0b82a9180f0bec1bbe1e7`。

```text
validated 4451 upstream LiteLLM pricing entries and 207 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test catalog_nonchat_routes`

源码提交：`0050a21bb521fa7723ea9dae139a26754d3932c9`；退出码：`0`；耗时：143.5 秒。
原始日志 SHA-256：`ea3eb8ebc0b07669d6543b3c4bdef14511d584267b5269c19eb904cc653cc5cf`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 26s
     Running tests/catalog_nonchat_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s
     Running tests/gemini_sdk_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.41s
     Running tests/native_responses_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 33.48s
     Running tests/responses_routes.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.55s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`0050a21bb521fa7723ea9dae139a26754d3932c9`；退出码：`0`；耗时：85.3 秒。
原始日志 SHA-256：`9266eaf3dd49c61bcd828de95637d26e2c642c50fe0f66983c760cf95aef2e2c`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 25s
```

## Azure AI providers-extra 实际模块

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo test --lib --features gateway,sqlite,providers-extended,providers-extra,mcp,a2a core::providers::azure_ai::`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：141.2 秒。
原始日志 SHA-256：`af5eb34cab14a734a3b50f7de48bfae22c1cf2cb414f03e907c676f4e1ea53ba`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 17s
     Running unittests src/lib.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/litellm_rs-def4a316c66bf5cd)
test result: ok. 107 passed; 0 failed; 0 ignored; 0 measured; 11124 filtered out; finished in 0.79s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,providers-extra,mcp,a2a -- -D warnings`

源码提交：`d0eb912d579e66693ffa91d8e9e4225ab19bceb5`；退出码：`0`；耗时：117.4 秒。
原始日志 SHA-256：`a613041fa5fc62281aaab17547774657b3d6bf47abc6d049b78303489b99c989`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 57s
```

## Replicate 与 fal 能力整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a providers::replicate::`

源码提交：`683bc29da255e2c65991ce1292f875dfea28d6db`；退出码：`0`；耗时：147.3 秒。
原始日志 SHA-256：`2a6f7205289a44103d837d4814cdcb87c00308ca25f1a8ddf0511db718b586ee`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 21s
     Running unittests src/lib.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 107 passed; 0 failed; 0 ignored; 0 measured; 10228 filtered out; finished in 3.03s
```

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a providers::fal_ai::`

源码提交：`683bc29da255e2c65991ce1292f875dfea28d6db`；退出码：`0`；耗时：33.6 秒。
原始日志 SHA-256：`cd957027322c4dffdaa4cc1c20c861105282975aaa014e9531ab27d5272d2dab`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.09s
     Running unittests src/lib.rs (/Users/apple/.codex/worktrees/litellm-pr-triage-20261003/target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 10286 filtered out; finished in 0.02s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`683bc29da255e2c65991ce1292f875dfea28d6db`；退出码：`0`；耗时：83.9 秒。
原始日志 SHA-256：`e8bfbb1b6c24e45207281b9ce4464f04a13c14ca9ad9ee40e0246426bcb31c85`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 23s
```

## 云兼容 credential/计费边界修复 / PR #1418

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：2.8 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：21.7 秒。
原始日志 SHA-256：`b662086315c42e514d0dfd3851336f39b8af48443131a973eeafb38c951fc4a1`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 19.41s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
```

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a complete_construction_credential_precedence_matrix`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：131.7 秒。
原始日志 SHA-256：`d50c877c890a5b39c6b2da175395f856c4226b65d99190aa163d28172e8f7692`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 08s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10414 filtered out; finished in 0.32s
```

### `cargo check`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：37.8 秒。
原始日志 SHA-256：`51426df0f9d0adf39339b239571e1695c044ac4a6226e4cfbdb7f6a6146a5a32`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 37.69s
```

### `cargo test`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：215.9 秒。
原始日志 SHA-256：`d7ec51877caf16b9cb90486196d8bc2f3e98915d3fb633f4a09a014589364d69`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 15s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7141 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 92.23s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.63s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.32s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：49.5 秒。
原始日志 SHA-256：`033496b487e22b260649a5e0d5134b3a5e5559672448cd70a9da16c8cfcd998d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 49.33s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：446.1 秒。
原始日志 SHA-256：`9cfdff8f7288ecea6a6cf666b6c72070f4709d43ae3afd67d42f931d3b8424f2`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 49.19s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 10414 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 115.05s
     Running unittests src/main.rs (target/debug/deps/gateway-a5d38d567cbc3db7)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-21a9c004f1e6241a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-e2bfcd56c2e241f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.75s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-0656e58a4f22215c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 12.08s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-b14436f653b04002)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 27.35s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-74adf5fb87f60f97)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.75s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-4eb58dd4b33c3b91)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.44s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-9cc2e14d986a5227)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-72ef1c7a753ea471)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-a7be35e9519931ee)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.42s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-a628a08640a662c5)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.62s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.61s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.92s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-fb431b9ae22c9782)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.59s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-d79a9518ac2659a6)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.44s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-59d5bed5037d9b7e)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.58s
     Running tests/lib.rs (target/debug/deps/lib-e6bb028cb02624e8)
test result: ok. 255 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 34.26s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-032b5c59b58b5a6e)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-a442b1d8d56477e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-b6769d5ef975fe19)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 48 filtered out; finished in 0.00s
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.02s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-f5cfa1ee9b707c58)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.23s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-51ae7c1f27ea601b)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.41s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.05s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-dbd06c1f7444ae0b)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.62s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-bfc2853ebee4c9f9)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-075696889d77e547)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.34s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.69s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-e5300c85d027a431)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-848e05df4867fcd9)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.02s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.02s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`941357cbb4476edc2983eddfb3e5179504844751`；退出码：`0`；耗时：107.3 秒。
原始日志 SHA-256：`1c449c6f605e01d3f31d8f89dd26f98613b096477e795a63d021a4bb5bfb6752`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 47s
```

## 中文供应商 embeddings usage / PR #1421

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：2.8 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：158.1 秒。
原始日志 SHA-256：`c1ee91aca38e6f8c410f58ced24fa2b7d955d3499bd0d3b10da61360d7ecb13a`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 35s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s
```

### `cargo check`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：36.1 秒。
原始日志 SHA-256：`f6c1d171782cc28013717b6e36d603acdf99cf3f059eaf763e6f5c1421c6e498`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 36.03s
```

### `cargo test`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：178.8 秒。
原始日志 SHA-256：`41438116c60e0befae45019942694519cdb1ddbdd7df5d4b4817dbd6f79c0bfd`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 02s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7137 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 86.10s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.56s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：44.3 秒。
原始日志 SHA-256：`1e79f0a5127a85a5ffd4a1a723b758738eac1fac46e6fb0e9574be0e962222d5`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 44.23s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：473.8 秒。
原始日志 SHA-256：`972381732405d809621a03e5b0938f3fc314384fb40fdf864900fd7233756054`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 25s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 10410 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 95.87s
     Running unittests src/main.rs (target/debug/deps/gateway-a5d38d567cbc3db7)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-21a9c004f1e6241a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-e2bfcd56c2e241f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.96s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-0656e58a4f22215c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.05s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-b14436f653b04002)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 18.43s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-74adf5fb87f60f97)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.57s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-4eb58dd4b33c3b91)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-9cc2e14d986a5227)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-72ef1c7a753ea471)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-a7be35e9519931ee)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-a628a08640a662c5)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.56s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.63s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.61s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-fb431b9ae22c9782)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.05s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-d79a9518ac2659a6)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.32s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-59d5bed5037d9b7e)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.45s
     Running tests/lib.rs (target/debug/deps/lib-e6bb028cb02624e8)
test result: ok. 255 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 33.55s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-032b5c59b58b5a6e)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-a442b1d8d56477e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-b6769d5ef975fe19)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 48 filtered out; finished in 0.00s
test result: ok. 49 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.02s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-f5cfa1ee9b707c58)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.41s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.98s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-dbd06c1f7444ae0b)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.36s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-bfc2853ebee4c9f9)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-075696889d77e547)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 4.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.06s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-e5300c85d027a431)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-848e05df4867fcd9)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.80s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`d834675933cab42125e09a474c267acc12cffb53`；退出码：`0`；耗时：105.5 秒。
原始日志 SHA-256：`c8ff71cc26cde73782730c48de2ac5e6249d9ff5d84f65da4beba01c46344c45`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 45s
```

## Azure/Voyage 当前整合 / PR #1416

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`2f06e5102b3589750c5fb2ab79fe1c2399763a1cdaddb9ccae16108e78938145`。

```text
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 53 tests in 0.855s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：1.8 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --lib --features gateway,sqlite,providers-extra,providers-extended,mcp,a2a azure_ai`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：185.1 秒。
原始日志 SHA-256：`634669a84287f2c3b326941afa81757082bca8dfd3cdc633971fb6747f612e04`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3m 01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-def4a316c66bf5cd)
test result: ok. 116 passed; 0 failed; 0 ignored; 0 measured; 10902 filtered out; finished in 1.05s
```

### `cargo test --lib --features gateway,sqlite,providers-extra,providers-extended,mcp,a2a providers::voyage::`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：68.1 秒。
原始日志 SHA-256：`7622d6091a5ba3b2d92040349ebb8606b0b13fffb3225ba73481c98e39585535`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 05s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-def4a316c66bf5cd)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 11012 filtered out; finished in 0.01s
```

### `cargo test --features gateway,sqlite,providers-extra,providers-extended,mcp,a2a --test native_responses_routes`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：118.7 秒。
原始日志 SHA-256：`69fb7bcecd30aaee184ba984cf5eda2f2a955534ad4415076f56596d7e207e86`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 26s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-17b862efcbd82a47)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 30.20s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extra,providers-extended,mcp,a2a -- -D warnings`

源码提交：`19df4e8f9f916564525a4448757b19d3eb6f38c8`；退出码：`0`；耗时：115.3 秒。
原始日志 SHA-256：`29537af2be7b47ef5f4918d2e1db282feac24b5cf896d779c612ca773c98e215`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 55s
```

## 云兼容主线整合 / PR #1418

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`95ea52e71e846b82dc809cdeed1e394675da50d6`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes`

源码提交：`95ea52e71e846b82dc809cdeed1e394675da50d6`；退出码：`0`；耗时：69.9 秒。
原始日志 SHA-256：`6b4835f4913897ac76d3e3632fadc08bd30bc68401a1f710c33ff622d8023ba8`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 07s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`95ea52e71e846b82dc809cdeed1e394675da50d6`；退出码：`0`；耗时：74.0 秒。
原始日志 SHA-256：`2bba533b66299d73b3a424326ab1406f867effbde8f2d6e72084b1667ca9d89e`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 13s
```

## 中文兼容主线整合 / PR #1421

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`cfbf58bf27460edb5e581903e64ae0ddb1f04ee9`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes`

源码提交：`cfbf58bf27460edb5e581903e64ae0ddb1f04ee9`；退出码：`0`；耗时：72.0 秒。
原始日志 SHA-256：`d0e0564b3b37ceb82cdd22632a871509df3913b1ad8814bab6495beff946ffca`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 07s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.16s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`cfbf58bf27460edb5e581903e64ae0ddb1f04ee9`；退出码：`0`；耗时：74.4 秒。
原始日志 SHA-256：`c32f0244b926af4dac49fb34ee2009b66611ce9590f5a0ff7c9d48869f2396ec`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 14s
```

## 发行准备精确特性编译 / Draft PR #1413

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`674f804f81c6f8fca48acadd1f1cbdd78d00a5e7`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a --bin gateway`

源码提交：`674f804f81c6f8fca48acadd1f1cbdd78d00a5e7`；退出码：`0`；耗时：104.4 秒。
原始日志 SHA-256：`79823d3bb05b9ed25500fa3d9d074d3c45bf7fde87d9a19aa723c4b9c40ec9a9`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 44s
```

## Realtime 五条 admission/session 审查完整验证 / PR #1405

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests::`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：72.9 秒。
原始日志 SHA-256：`7e3c61bba75dd5acf6f3df0bafae960999b304078e2d6ced809ee8dd07bf6925`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 51.53s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 9885 filtered out; finished in 18.81s
```

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp cancelled_realtime_admission_restores_shared_rpm`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：40.1 秒。
原始日志 SHA-256：`df869725dbafec5b7747758cab14d1133a30f9db15a7b5727311c72bc74ef16d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 37.04s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9913 filtered out; finished in 0.50s
```

执行环境：Isolated session Redis 7 container, 127.0.0.1:32775; ephemeral, no real credentials。

### `cargo check`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：5.5 秒。
原始日志 SHA-256：`5f8dca1c6e44271dbbc9cb311ba253fb106c538a7af79f9c40804a46e54a49f5`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.39s
```

### `cargo test`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：133.8 秒。
原始日志 SHA-256：`152cfaa2f92b390f890d9e9441dd3817ba090c64985a76607c904652ec46550d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 14.86s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 90.41s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.66s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：19.4 秒。
原始日志 SHA-256：`3c6b74839b3a4119e236fbf6b1b75649e5b20b1e04712602d1b5ddfbb7bab9af`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.31s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp -- --test-threads=2`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：641.2 秒。
原始日志 SHA-256：`c3facc6147b5042cfbd6f6a98531837a0f934472fd1241506fe21d13ff94432c`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 37.70s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9913 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 192.30s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.38s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.11s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.21s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.72s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.24s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.15s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.70s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.99s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.33s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.77s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.85s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.67s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 89.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.72s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 34.09s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.82s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.83s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.71s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32775。

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`2ee7911bc21b7479afcf9b8e7bb2634afb45bc2b`；退出码：`0`；耗时：33.9 秒。
原始日志 SHA-256：`d19f3309c38c12d05747df774b44c63e4aaf1524face2670ebb57a74734bd47f`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.78s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32775。

## Responses 当前整链 / Draft PR #1383

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`48f1b2beaf263bd8c8c07c7eda71d8e3943a5bc8`；退出码：`0`；耗时：2.8 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`48f1b2beaf263bd8c8c07c7eda71d8e3943a5bc8`；退出码：`0`；耗时：7.5 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`48f1b2beaf263bd8c8c07c7eda71d8e3943a5bc8`；退出码：`0`；耗时：250.9 秒。
原始日志 SHA-256：`ba1b26b38bb5e522e12aa0d82594a39a72c8c2726cf23eec6e29588dd792930c`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 10s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.46s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.33s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.52s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.81s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`48f1b2beaf263bd8c8c07c7eda71d8e3943a5bc8`；退出码：`0`；耗时：110.0 秒。
原始日志 SHA-256：`ede22930b892b63f3c23d1319336b1f52aa37cdcdff7e4e9a31fb83c88e66373`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 49s
```

## Realtime 最新主线及配置预算身份 / PR #1405

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：2.8 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：3.5 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests::`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：136.3 秒。
原始日志 SHA-256：`4fef2290b7477e974f5c2bc62e8468df7581567ebb99d953bd3df45a4d22f23d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 55s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 18.33s
```

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp cancelled_realtime_admission_restores_shared_rpm`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：35.2 秒。
原始日志 SHA-256：`f7610933ccaa5ab44b3917e5828b65298c8a985f3bb2eae99645206b412e31d8`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 32.04s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9833 filtered out; finished in 0.32s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32775。

### `cargo check`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：7.9 秒。
原始日志 SHA-256：`173ac01aa6ca1faab33c6fa45e31d5e9d5d81de50c09ba7db7cadeb7cdbf724a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.83s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：23.5 秒。
原始日志 SHA-256：`76fd572c4985db9499bb7ebf71d2b5e777db7d995b3052e6f085a9ec877326ec`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.40s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`969ba9ca3f87c71efde665b73ae3fc858239c9ea`；退出码：`0`；耗时：71.5 秒。
原始日志 SHA-256：`cc451e21005c9aa9d7b4334d6cd6af6698cb2b6260c4eff265713b11f49d5f72`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 11s
```

## 云兼容最新直接 factory credential 修复 / PR #1418

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a direct_factory_skips_blank_primary_and_alternate_credentials`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：121.1 秒。
原始日志 SHA-256：`50519a1e42f7d4e26a80be483ca63c3e7236a7bb52ea9800ee6fcbe9180b9ce1`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 58s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10252 filtered out; finished in 0.01s
```

### `cargo test --lib --features gateway,sqlite,providers-extended,mcp,a2a complete_construction_credential_precedence_matrix`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：32.9 秒。
原始日志 SHA-256：`f3e7d6d2a37dd754f312b3562586c07b632a5abce949895d99854d246e16ab91`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.09s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 10252 filtered out; finished in 0.35s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：35.8 秒。
原始日志 SHA-256：`486e95a3229feb28ee0675087227ddf4a9383ab7354328cd91275cafca26156d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 32.60s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.81s
```

### `cargo check`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：7.5 秒。
原始日志 SHA-256：`8cd74d0f58bae5d4701e43eea06398eec422e599ac174c6b7cda7a4d1484510d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.42s
```

### `cargo test -- --test-threads=2`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：177.6 秒。
原始日志 SHA-256：`4d8d278bbd309f945e4c38eacd5f33d2af0b4f92722dca52ba98a179dc239756`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.71s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 107.44s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.52s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.07s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：26.0 秒。
原始日志 SHA-256：`90e46902876bec382efa6614701927ccd8b99ec9d256b312991ea22f07008402`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.82s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a -- --test-threads=2`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：623.8 秒。
原始日志 SHA-256：`a9c4050d48661010d94e16723cb3a3245603968ebd13cddd38da7fb1fe8e7f6a`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 42.01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 10252 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 160.55s
     Running unittests src/main.rs (target/debug/deps/gateway-a5d38d567cbc3db7)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-21a9c004f1e6241a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-e2bfcd56c2e241f0)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 8.26s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-0656e58a4f22215c)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.01s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.12s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-b14436f653b04002)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.99s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-74adf5fb87f60f97)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.22s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.79s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-4eb58dd4b33c3b91)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-9cc2e14d986a5227)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-72ef1c7a753ea471)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-a7be35e9519931ee)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.95s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-a628a08640a662c5)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.60s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.97s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 33.37s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-fb431b9ae22c9782)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.49s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-d79a9518ac2659a6)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.92s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-59d5bed5037d9b7e)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.79s
     Running tests/lib.rs (target/debug/deps/lib-e6bb028cb02624e8)
test result: ok. 255 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 94.11s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-032b5c59b58b5a6e)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-a442b1d8d56477e4)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-b6769d5ef975fe19)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 49 filtered out; finished in 0.00s
test result: ok. 50 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.04s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-f5cfa1ee9b707c58)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.22s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-51ae7c1f27ea601b)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.24s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 34.03s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-dbd06c1f7444ae0b)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-bfc2853ebee4c9f9)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-075696889d77e547)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.68s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.81s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-e5300c85d027a431)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-848e05df4867fcd9)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.76s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`c354fd6fa39ef588a2654ea178b00fbfc0c72080`；退出码：`0`；耗时：34.2 秒。
原始日志 SHA-256：`7d2e732f63ec9b5ddea083176cdd975a5dff2c945e03cf086b04869b290d53d9`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.05s
```

## Realtime session acknowledgment/取消终态最新完整检查 / PR #1405

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests:: -- --test-threads=2`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：106.6 秒。
原始日志 SHA-256：`1e3d7e559bd5128721f153d976c0a2d8461f8ea38dfeaf8c57444793a14502e9`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 09s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 33.75s
```

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp cancelled_realtime_admission_restores_shared_rpm`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：39.3 秒。
原始日志 SHA-256：`49a55a29e7a07c041d54b8278f5a4d7868c4d2f4c1d8b62456e4d186433c8a52`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 36.07s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9836 filtered out; finished in 0.33s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32776。

### `cargo check`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：7.3 秒。
原始日志 SHA-256：`35dcd49cd135fec910f94aedd4be276aa3272d0e3ace480a3c7ed4c4deeaae71`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.26s
```

### `cargo test -- --test-threads=2`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：185.5 秒。
原始日志 SHA-256：`f4b1add1c1cac8fc9f37d6be80d76bf89be9cd9fabd2672da51ee030a8e1b613`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 34.96s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 110.38s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.61s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：21.0 秒。
原始日志 SHA-256：`af41db395978230bae171f859a0f757b49828f46238ea86d442e0cf2506e933e`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.90s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp -- --test-threads=2`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：704.4 秒。
原始日志 SHA-256：`bb3ca125799e208458c48b4c1cac3e8da50e261899666d11ea2ea0bfb496653c`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 28s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9836 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 198.29s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.34s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.13s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.77s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.33s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.44s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.21s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.47s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.02s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.65s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.17s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.68s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.49s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.37s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 93.76s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.53s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-735a74510a495074)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.09s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 33.79s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.66s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.78s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.70s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32776。

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`eef1c94970784f9d4fb4f70b890fec4ec37509af`；退出码：`0`；耗时：34.1 秒。
原始日志 SHA-256：`ce0653354def9b6ce81b2296d0456f73d34387c16b0185fc491825825eba119d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.99s
```

## 中文供应商取消 CI 的精确特性定向复核

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo test --test lib --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation test_responses_runtime_policy_errors_use_openai_shape -- --test-threads=1`

源码提交：`cfbf58bf27460edb5e581903e64ae0ddb1f04ee9`；退出码：`0`；耗时：80.4 秒。
原始日志 SHA-256：`0afc62536e48fb90174782b28279cd13767b822bdb891d84e019cf2792d7aa39`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 17s
     Running tests/lib.rs (target/debug/deps/lib-8ec59626415bf71f)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 269 filtered out; finished in 0.93s
```

执行环境：Local isolated worktree, exact cancelled-CI release feature flags, actual integration target lib; not a complete CI replacement。

## Realtime 00873e07 backend/cap 错误修复

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp server::routes::ai::realtime::tests:: -- --test-threads=2`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：97.2 秒。
原始日志 SHA-256：`c108a25e1483f72e7bcf1b2c08c306154a5eed063eac034d85505fee4ea21de9`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 32.32s
```

### `cargo test --lib --features gateway,sqlite,websockets,a2a,mcp cancelled_realtime_admission_restores_shared_rpm`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：33.5 秒。
原始日志 SHA-256：`d79976b71e6a9db7d5cd506b82d405a952ac844505630740002b8804b48e0b95`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 30.35s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 9838 filtered out; finished in 0.32s
```

执行环境：Isolated session Redis 7, 127.0.0.1:32777。

### `cargo check`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：5.3 秒。
原始日志 SHA-256：`84abef7b414d5f10e92eff685576668c5bcf74ca6a17fa52c56a5eeaea4fe26a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.17s
```

### `cargo test -- --test-threads=2`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：150.9 秒。
原始日志 SHA-256：`77101fc62b529c50dba48d9da44249fa00cd2f91f2ff575786258e51d4663fa1`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 16.43s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 103.54s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.57s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.03s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：17.0 秒。
原始日志 SHA-256：`6b99260b6ebbd2d9fbe3514340dcffb6bd7b138920f2f9d5eb02123fbfa3ebd0`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.84s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`00873e078f996aaa074413ee5e254bcc7ebaa3c1`；退出码：`0`；耗时：32.1 秒。
原始日志 SHA-256：`8ebb341ef3e2ebbcdd8d75ffaa700e8409edc8319bf9c7bbe4253a2a38513230`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.98s
```

## 中文/云兼容6aa3a043整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`6aa3a043dfc7e188e4407e2c6b00c1a7ac2f912e`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`6aa3a043dfc7e188e4407e2c6b00c1a7ac2f912e`；退出码：`0`；耗时：3.2 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`6aa3a043dfc7e188e4407e2c6b00c1a7ac2f912e`；退出码：`0`；耗时：34.4 秒。
原始日志 SHA-256：`c58ee446404418241945752118760ca1fcdbad6c9268610106dd2c4fa20b26f1`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.54s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.18s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`6aa3a043dfc7e188e4407e2c6b00c1a7ac2f912e`；退出码：`0`；耗时：32.1 秒。
原始日志 SHA-256：`8123fa9cdeb4a6f8b4a77787625a57074ff008d62fed577b72e1960ed20a586a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.02s
```

## 百川两个原生协议审查最新验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：3.0 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：192.5 秒。
原始日志 SHA-256：`e1444eac52a310d8bfbcb36bfd87430093c2ad3450c2d5a0ef13aecd07e38018`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 34.48s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.34s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：29.3 秒。
原始日志 SHA-256：`367aad1a6799c020ec6b249d9e171098abcf251aba61c491ad42bb282b025222`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.19s
```

### `cargo check`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：5.2 秒。
原始日志 SHA-256：`529722695a5a78dc54c51e02f6b6bbb984d58156b4cd40721aba1fc55efed52c`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.15s
```

### `cargo test -- --test-threads=2`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：174.0 秒。
原始日志 SHA-256：`0bd5b25ab9ae48e7f3caafdc0fb29a2558bd19dbd9c8d44cfc95874085faaca0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 21.27s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 118.41s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.51s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.55s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：19.9 秒。
原始日志 SHA-256：`c7afdcaccfd791f2b4274f269c75330b8be113bc340772deffad210f925bcae7`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 19.74s
```

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`823b60abf0b796efb34b2b8e9c7af1cfc32f6a830ca1e66a4376a14d109970a8`。

```text
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 53 tests in 0.824s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`bb85cf446f197838c23d74ba101e1f6ee94b0159`；退出码：`0`；耗时：2.2 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Ark未知价格最新验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：577.2 秒。
原始日志 SHA-256：`c618f899cd4a3b3c170d52b9e58291a57ec7382d0c0a08d0285c873896d81d68`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.92s
```

### `cargo test -- --test-threads=2`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：345.2 秒。
原始日志 SHA-256：`7db3f15aa8263aa596055a8e4b057adb92f0829197f832aabdc12ef8b4624f09`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 15.29s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 106.31s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.57s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：18.0 秒。
原始日志 SHA-256：`3a4cb29b4a5749e1a7ae616595a11f71a4a79f8cb5d57c0fd74477405cffb1cb`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.87s
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：357.8 秒。
原始日志 SHA-256：`1d3c4f5c6a4e449047b8f3943c41cffb27ba14077f727336b63431e6dd9549da`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 22.30s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.78s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：41.3 秒。
原始日志 SHA-256：`f9cd6973deffb9bf64edde6ee014e694a868f2909b45a45d81a2aa709b11942a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.18s
```

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：1.3 秒。
原始日志 SHA-256：`2ba965cc1192a9d3248ac3535e8be2040d6856ad1cb45dd5216ac90e90dbfc4b`。

```text
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 54 tests in 1.072s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`f7870fb26b6430fb476f705f6d3e4583a670ece1`；退出码：`0`；耗时：5.8 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## AIML/Comet/云兼容主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`1d8ea74a3e78922258aca6b0a0b3a40cf3977f0c`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`1d8ea74a3e78922258aca6b0a0b3a40cf3977f0c`；退出码：`0`；耗时：3.4 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`1d8ea74a3e78922258aca6b0a0b3a40cf3977f0c`；退出码：`0`；耗时：119.2 秒。
原始日志 SHA-256：`071d31f234ea4af0b991293950d9bb8918f37cb8e7b5ce526dbc86a957db54d3`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 53s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 29 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.80s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`1d8ea74a3e78922258aca6b0a0b3a40cf3977f0c`；退出码：`0`；耗时：96.6 秒。
原始日志 SHA-256：`9d826b2e370254697aff3b2f458f90a87c67fb9557eb2854332c30d1259ce10c`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 36s
```

## 中文/W&B最新整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`360bf9ce3242d7b03894a7c957035acebcd2977f`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`360bf9ce3242d7b03894a7c957035acebcd2977f`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`360bf9ce3242d7b03894a7c957035acebcd2977f`；退出码：`0`；耗时：35.0 秒。
原始日志 SHA-256：`457ca9188e488a682bb3b233b0b31f1e75c70834b9cb167afeee5110a327f65d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 27.62s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.88s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`360bf9ce3242d7b03894a7c957035acebcd2977f`；退出码：`0`；耗时：46.2 秒。
原始日志 SHA-256：`19234107b02afb10bfd21606eeb4c795b7099a00986b839a7c0e9c142b0c0355`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 45.79s
```

## 多模态聚合/W&B最新整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`8a93bf2ee2c97505affed22c40e8a37f6cae1f77`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`8a93bf2ee2c97505affed22c40e8a37f6cae1f77`；退出码：`0`；耗时：3.0 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`8a93bf2ee2c97505affed22c40e8a37f6cae1f77`；退出码：`0`；耗时：31.8 秒。
原始日志 SHA-256：`6ba3a375796e52feca51666f5dc93d8040f6d4519d357226a716d4e581e69a28`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 24.25s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.86s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`8a93bf2ee2c97505affed22c40e8a37f6cae1f77`；退出码：`0`；耗时：38.0 秒。
原始日志 SHA-256：`d6530088970621409fb33a1e9c7f59afe57a533d5016293a870b437682d233ad`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 37.79s
```

## Realtime六条审查及当前main完整验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：3.2 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime::tests -- --test-threads=2`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：117.3 秒。
原始日志 SHA-256：`4f133bc185cab3f09e66129b4a89af0a27ae9e44176864f497812b8d74276ac0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 14s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 39.66s
```

### `cargo check`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：5.9 秒。
原始日志 SHA-256：`83a8619961bc6197026c6a18f01b0782e1da869bdb00a7f9011a655e1099965f`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.83s
```

### `cargo test -- --test-threads=2`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：162.0 秒。
原始日志 SHA-256：`7fce70a834b2daa195723dc636d8830370b15d66dcfca696c672f3ecff17d97e`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 24.70s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.96s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.41s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：17.5 秒。
原始日志 SHA-256：`6d3cd60482be4442bc904602e0c2d2a3d932cffb88b6b16b8a8e11bfb0beefb8`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.45s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：31.3 秒。
原始日志 SHA-256：`69c9fbd7e7467324ec6b9673e5f85f8961e4c89737c74d09549437d037696d6c`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.19s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp -- --test-threads=2`

源码提交：`bdb466d6539c7eac388e42d44a02ab3549c18a54`；退出码：`0`；耗时：644.4 秒。
原始日志 SHA-256：`f24ab0ee852fef01befcc2b197324ea4ecb3cbf6c71128ff2800978bcad36eef`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 45.45s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9841 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 196.41s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.27s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.15s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.10s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.27s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.48s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.17s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.96s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.62s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.99s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.31s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.41s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.64s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.57s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 88.73s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.30s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-735a74510a495074)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.28s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 33.96s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.31s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.72s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.81s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.69s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

## F07首次取消CI的精确目标复核

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo test --workspace --locked --features postgres,sqlite,redis,s3,metrics,tracing,websockets --test audio_native_providers tests::elevenlabs_rejects_standard_tts_speed_before_upstream_io -- --exact --nocapture`

源码提交：`c44b48d426bc59691c216fe91731d00b6c0873b6`；退出码：`0`；耗时：139.0 秒。
原始日志 SHA-256：`e16eeaa471576cc910be9e93ef1368728288900e48787edf79d91a54f82e1aa8`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 16s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-4c061e1f76fb89d3)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.15s
```

## 中文供应商最新native/media主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`edda3bbe77c011ffc13881dcf18b550f9a75341c`；退出码：`0`；耗时：3.1 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`edda3bbe77c011ffc13881dcf18b550f9a75341c`；退出码：`0`；耗时：1.9 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`edda3bbe77c011ffc13881dcf18b550f9a75341c`；退出码：`0`；耗时：59.0 秒。
原始日志 SHA-256：`5f5b0426751de58e1014315f73387fd7db480438ed931a37887de60ac2643eed`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 51.02s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.43s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`edda3bbe77c011ffc13881dcf18b550f9a75341c`；退出码：`0`；耗时：71.4 秒。
原始日志 SHA-256：`2fc3327b79f683796036147c0664d69c8b1ad949c5a50b762acf51bf4e89bd8d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 10s
```

## AIML最新native/media主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`97d3e5f6d917755234f793ea1cc8fd700dac8b1a`；退出码：`0`；耗时：3.1 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`97d3e5f6d917755234f793ea1cc8fd700dac8b1a`；退出码：`0`；耗时：2.0 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`97d3e5f6d917755234f793ea1cc8fd700dac8b1a`；退出码：`0`；耗时：120.3 秒。
原始日志 SHA-256：`cd13598e8b7c2928aaddcee7ec15a7c44dfe13c4b9f497b949b585dbd665c49a`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 48s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.22s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`97d3e5f6d917755234f793ea1cc8fd700dac8b1a`；退出码：`0`；耗时：49.4 秒。
原始日志 SHA-256：`27e1439a9e2f111e1e06232d4bdaa4cf01289a56199610a049f57876bb7e8699`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 49.14s
```

## CompactifAI最新native/media主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：3.1 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：1.9 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：54.6 秒。
原始日志 SHA-256：`a7c7141f7b8b2689b4273a626418d52c09503ff6b26db13d9d20bbe24d6ac7f0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 46.72s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.08s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：60.7 秒。
原始日志 SHA-256：`5305656582122328e6e955dacb774de5117fd326226775c4c31aa9c2316475d6`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 00s
```

## CompactifAI超预算实际费用审查完整验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite --test audio_routes -- --test-threads=2`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：148.0 秒。
原始日志 SHA-256：`622af6807f0cf36c73db99557cb436f821d404ffde22099a9c8671de289cde48`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 47s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.71s
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：25.1 秒。
原始日志 SHA-256：`2b24a4450a3418073e8d8d70a4fbc937f2223a17519e7167da063d45840f70e5`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 17.93s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 33 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.78s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：98.0 秒。
原始日志 SHA-256：`8ef1bddfdd6c16067bb762a97ac0bbe004454353ed7a7dde37e001c7f331568d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 37s
```

### `cargo check`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：23.7 秒。
原始日志 SHA-256：`43632fce273dc26abc80ab1598900b0f1ea79d60c12803456772eb80fab10ddd`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.64s
```

### `cargo test -- --test-threads=2`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：204.3 秒。
原始日志 SHA-256：`cb284a822334675ea6bbfc09619f39f20239e55ab10d63a47bb84a96532689d0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 08s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 105.02s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.42s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：43.6 秒。
原始日志 SHA-256：`8e59592a0cabe7be59746503aa013a911d940c4506855cf9a9f5b6a7382d4c32`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 43.46s
```

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`24af6e41d967dcb6a5cdd3b1f4cd50d882dbc4025dc4d1cb28abfd8664a2d22a`。

```text
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 53 tests in 0.840s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`af0ccf56054055ac270a6b57e6c884d0b2aedf9e`；退出码：`0`；耗时：1.8 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Ark移除价格仍刷新非价格metadata

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`34af8d2c02de227a8ec75f1db49e5267de255239`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`7ff9e732cc7dc50f6fb58ea69e2d7270fdee1c35d6528ed082a1a6951e2ab6b5`。

```text
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 54 tests in 0.835s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`34af8d2c02de227a8ec75f1db49e5267de255239`；退出码：`0`；耗时：2.3 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Ark当前整合head的Python/sync

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`265f825ade244e46296ed0b76fc3bf896e5a5fedd473986f8e55d95848e734d6`。

```text
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 54 tests in 0.843s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：2.3 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## 中文供应商/百川主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`5afc65361099b89ceba520e66de4ec60c20d78ec`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`5afc65361099b89ceba520e66de4ec60c20d78ec`；退出码：`0`；耗时：1.8 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`5afc65361099b89ceba520e66de4ec60c20d78ec`；退出码：`0`；耗时：46.0 秒。
原始日志 SHA-256：`5c49872a63b09fd673ad879a361f257bebb3fcded2370a05306b551fc323bbfb`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 38.80s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 37 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.84s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`5afc65361099b89ceba520e66de4ec60c20d78ec`；退出码：`0`；耗时：33.5 秒。
原始日志 SHA-256：`b54f0415f0abe5a0a11d8af77ebe4e69b2a83ff77ed297d5f2aaa7c7c8671afa`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.41s
```

## Ark/百川主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：1.7 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：42.4 秒。
原始日志 SHA-256：`ea0fa9405aae8ce611be81a1961acef0c71aa45e586192329c2f86d519ca3247`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 35.09s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.82s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`c8c78d986606297f27d68a21725e8da2f8c66559`；退出码：`0`；耗时：29.8 秒。
原始日志 SHA-256：`0de90a0e651e934f23c5efa8d5210eab8e5e9169237b9962b770467d92531a26`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.70s
```

## AIML缺失/无效usage拒绝完整验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：36.8 秒。
原始日志 SHA-256：`90f34c0eb8138774bf5c52310eb21b3f1d9ab74426b2260c22333d02cf375639`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.52s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.80s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：28.5 秒。
原始日志 SHA-256：`c99ac060616d4499b877b0547e0aee4d8be5e2dde25368dda534f01e260aa60a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.33s
```

### `cargo check`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：23.2 秒。
原始日志 SHA-256：`5dadf2aaf39def392745fb554eb3db30012b117056c5a51f470e0b41204073d2`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 23.20s
```

### `cargo test -- --test-threads=2`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：203.2 秒。
原始日志 SHA-256：`b8b5468e60a2764ff62843ed5c190675fb5ac3188deef84a3b4b5c78af059124`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 09s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 103.92s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.42s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.38s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：45.0 秒。
原始日志 SHA-256：`dd4d9ed4fd52e2fd5887c1268125dfaae979509a326b700890d065d2539f56b2`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 44.95s
```

### `python3 -m unittest discover -s scripts/test -p test_sync_litellm_pricing.py`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`7f54f980beef0da76edbf5b75802d1b73690aa917dfcb55b1a56cac892bf6b73`。

```text
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
validated 4570 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/dddddddddddddddddddddddddddddddddddddddd/model_prices_and_context_window.json; classified 4570 exact pricing rows
Ran 53 tests in 0.831s
OK
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`5d6a9937e9fcf0a703b50fd3bc1a8b4373862933`；退出码：`0`；耗时：2.2 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## AIML/百川主线整合

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`fc6d9282e741a733351f3c2090307a9e3c7f0930`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`fc6d9282e741a733351f3c2090307a9e3c7f0930`；退出码：`0`；耗时：1.8 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`fc6d9282e741a733351f3c2090307a9e3c7f0930`；退出码：`0`；耗时：38.9 秒。
原始日志 SHA-256：`f7087734d60b101212c85d0b89ecef34aa2ddb81892aa8a4a1781d8666f6e0a4`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.72s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.81s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`fc6d9282e741a733351f3c2090307a9e3c7f0930`；退出码：`0`；耗时：30.6 秒。
原始日志 SHA-256：`50466dff95ea51f316c385e5adb493ebb70376c968f3ffb928ba108fef281820`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.47s
```

## Realtime live-policy初轮已完成7项（完整扩展主动停止另列）

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：1.7 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime::tests -- --test-threads=2`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：115.1 秒。
原始日志 SHA-256：`5d5be54115aad0e6679071438adb88e716ace187d977b01bd2e73dc2dd44a07f`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 12s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 39.04s
```

### `cargo check`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：8.2 秒。
原始日志 SHA-256：`ad4dc75d7cecd827186b9e0edd03867769eee558fa80862d94df366c75182b4b`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.13s
```

### `cargo test -- --test-threads=2`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：163.1 秒。
原始日志 SHA-256：`047cf9cd3fa90cd4a08ef71ec79d0b00c912059db2b91ebf7cd541c0abf4faa1`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 24.17s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.53s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.43s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.44s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：18.8 秒。
原始日志 SHA-256：`c9996bf37fcadc7971f47564b9689813535c1b5b2ed9d92a1ef277524a15f27b`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.70s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`cb14af2f9d1413c76d86ce9733e2b33cb3e872a7`；退出码：`0`；耗时：35.9 秒。
原始日志 SHA-256：`76b450f0569ffce5d23e7c5a5917d40280917facd4b801678d93298d68817042`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.76s
```

## Realtime live-policy/inf最终完整验证

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：2.3 秒。
原始日志 SHA-256：`5d7ab0771fbd1f91baf7d15ba487ec1f57d655b15b71a2f68f8fb467a0f6f15b`。

```text
validated 4451 upstream LiteLLM pricing entries and 215 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime::tests -- --test-threads=2`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：102.6 秒。
原始日志 SHA-256：`ee26f09975ac9b951ed49ac419aa2d90c16d89aa83711a440359122ae73eee33`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 00s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 9804 filtered out; finished in 39.21s
```

### `cargo check`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：5.1 秒。
原始日志 SHA-256：`bcb116d5121a610720b102ce70f8615570612307ac3cc466b601adc076c26fac`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.08s
```

### `cargo test -- --test-threads=2`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：154.0 秒。
原始日志 SHA-256：`3502ccf8ea1fee801d2ee91814deb9b2243b4c72fed3c2cb8f70c9a5bc9fb160`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 12.84s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 105.85s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.57s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：18.5 秒。
原始日志 SHA-256：`e21ee987de74138a70914a42b55767765d060638dcbc94d4b016688f0f2d10ec`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.35s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：32.6 秒。
原始日志 SHA-256：`b10b2ab3eca0b2cc2ba215dd6743c1737e8e8c2911f481b751edd139c1ca73aa`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.40s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp -- --test-threads=2`

源码提交：`2c8b3c06d61d0f858a5e3a81360e28af955bbd9c`；退出码：`0`；耗时：682.0 秒。
原始日志 SHA-256：`6bce67fe4a58b72483d9db33ec5519a84dfa8febeb6dbe80907314e3e3378ff4`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 45.92s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 9845 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 209.66s
     Running unittests src/main.rs (target/debug/deps/gateway-3ccd1c60996b7023)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-6d9e2c0c20ac2a18)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-6f4989045fcffa64)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.32s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-925ddb304d69a0bb)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.09s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-aa6c4994f4eb7ce5)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.15s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-db19240643568ad3)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.22s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-e27ed0cccc978ace)
test result: ok. 32 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.90s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-6494bf0b8f5d0424)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.22s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-457ccee089d7e935)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.48s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-c17f21fd78a78024)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-b886a397056b2606)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-af2cd426dabc2a86)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.74s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-8da60ec68f918266)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.02s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-ae03036d7cdd3d70)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.07s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-8002aa1d8b952e17)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.46s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-af0b829d4d4424ee)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.62s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-0c5e1ed93acfbf66)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.57s
     Running tests/lib.rs (target/debug/deps/lib-9987c2ce9120c0b9)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 88.57s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-ec1e9bc8ed28a584)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-13d014b274326a3d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a1809d793decc1f6)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-c8be3720abf10b94)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.26s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-735a74510a495074)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.34s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-790f150b44d8e364)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 33.93s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-90f0845d1e22c8f8)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-ea2f799e88b6e2ba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-b7ffdf5b322ecb2f)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.75s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-716e2fc1be8fe553)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.84s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-941ab22d516f2963)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-b3e5e2b4fc2046f2)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.03s
   Doc-tests litellm_rs
test result: ok. 33 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.18s
```

## CompactifAI accepted head ede71579

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：3.1 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：1.9 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：54.6 秒。
原始日志 SHA-256：`a7c7141f7b8b2689b4273a626418d52c09503ff6b26db13d9d20bbe24d6ac7f0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 46.72s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.08s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`ede71579fca3685e0d47653d325b8cd2c8645bfe`；退出码：`0`；耗时：60.7 秒。
原始日志 SHA-256：`5305656582122328e6e955dacb774de5117fd326226775c4c31aa9c2316475d6`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 00s
```

## Aggregators accepted head 0603aa0b

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`0603aa0b5dc0ab3cb2dc65a9231d54a986f4f59e`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`0603aa0b5dc0ab3cb2dc65a9231d54a986f4f59e`；退出码：`0`；耗时：1.5 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`0603aa0b5dc0ab3cb2dc65a9231d54a986f4f59e`；退出码：`0`；耗时：33.2 秒。
原始日志 SHA-256：`3db5a50b4b37f4983c47090cf76259b3541efe76deffc2eba0897bffb860a8e5`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 25.74s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.10s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`0603aa0b5dc0ab3cb2dc65a9231d54a986f4f59e`；退出码：`0`；耗时：28.2 秒。
原始日志 SHA-256：`651c0328c24e81cd99c88c2d0ab975f8ead8bcc6532dd400bc4131040eba7d81`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.05s
```

## SiliconFlow base64 rejection dfc484b4

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：7.6 秒。
原始日志 SHA-256：`0972518a71a7df8d691b58a975d368d0fb65780fd9176b83947f3377af82dc51`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.56s
```

### `cargo test -- --test-threads=2`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：159.6 秒。
原始日志 SHA-256：`d6cacf9d7f6066247929f3008f269559743627ea82a2ca86575e9a2f8aa3953d`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 28.75s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7132 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.59s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.41s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.50s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.30s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：21.3 秒。
原始日志 SHA-256：`e9c3f36fa9eeb7582a7618b81b08cdc796d92a75fce9050aaeb9451833f757d8`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 21.24s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：43.2 秒。
原始日志 SHA-256：`a358bb947d53e193a49a08028c3ebdda6ccdcf8a09460b7bf4f417434903a272`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 34.06s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.24s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：36.4 秒。
原始日志 SHA-256：`1a38f34ce0f162834761d16153ccbec1e493702d924ee9b5d8fda113d9c5e737`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 36.22s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`dfc484b40f5a8a8844d4383f5e56f55c9a78f834`；退出码：`0`；耗时：1.5 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Z.AI user_id mapping beb863af

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`beb863af6e4b39cbd2c8abf2f6f529ead1f3f84f`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`beb863af6e4b39cbd2c8abf2f6f529ead1f3f84f`；退出码：`0`；耗时：1.3 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`beb863af6e4b39cbd2c8abf2f6f529ead1f3f84f`；退出码：`0`；耗时：41.4 秒。
原始日志 SHA-256：`46706b42f52b13b4576c7457066a672f04cbc3e9ac9848439ba59d279d2beb90`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 33.71s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 42 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.25s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`beb863af6e4b39cbd2c8abf2f6f529ead1f3f84f`；退出码：`0`；耗时：34.4 秒。
原始日志 SHA-256：`4df5c33f19b8b1412700e99d89487c0783bf7e8062410217b702a7af648329a3`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.26s
```

## Chinese/aggregator fixture integration 64e2be88

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`64e2be88c1c034e6c232c8f95fa9f413a0014a88`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`64e2be88c1c034e6c232c8f95fa9f413a0014a88`；退出码：`0`；耗时：30.2 秒。
原始日志 SHA-256：`53ae74a5f5b437be57f180a3f9de93dcbaea51c136f74f72a2ea46293958e625`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 22.01s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 45 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.59s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`64e2be88c1c034e6c232c8f95fa9f413a0014a88`；退出码：`0`；耗时：46.7 秒。
原始日志 SHA-256：`ae7aacc5e0bcfff861a23eadb7818c421a3c67831e2943852bbc07767cb428a1`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 46.57s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`64e2be88c1c034e6c232c8f95fa9f413a0014a88`；退出码：`0`；耗时：1.4 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Chinese/xAI current integration

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`340723db1a207cd2461ec54cfe2befc800859c1e`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`340723db1a207cd2461ec54cfe2befc800859c1e`；退出码：`0`；耗时：47.5 秒。
原始日志 SHA-256：`ae095ee784fe88afb5ff41b9a980e4c0a8023d3ff944764da293aa0ce3c45902`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 39.55s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.56s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`340723db1a207cd2461ec54cfe2befc800859c1e`；退出码：`0`；耗时：41.9 秒。
原始日志 SHA-256：`9d8a1c4b9359ed4933e2be762b5c9b8860056907967e16f79d82fa818f91f023`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 41.73s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`340723db1a207cd2461ec54cfe2befc800859c1e`；退出码：`0`；耗时：1.3 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## xAI complete implementation validation 416f1429

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：28.6 秒。
原始日志 SHA-256：`7aec75838ad82046552618cf3f396c462159b9fb156cf50a716491b69b57cce8`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.59s
```

### `cargo test -- --test-threads=2`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：197.1 秒。
原始日志 SHA-256：`3701a6c237a17df44e6e59697e9b9fdeccb9e2a7302b6f0d144db7b0a413063c`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 108.06s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.45s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：39.2 秒。
原始日志 SHA-256：`3155924342b2acc9a002eeec5283b1023f7255cab27b05f776d9c563a7cbf8a8`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 39.14s
```

### `cargo test --features gateway,sqlite --test audio_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：118.1 秒。
原始日志 SHA-256：`517ae51e9db89e8afcd630d8c317181aa77c8cf77b292a589687b0c16fd7a3e7`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 08s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 39.99s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.78s
```

### `cargo test --features gateway,sqlite -- --test-threads=2`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：682.3 秒。
原始日志 SHA-256：`e6328680072fd1000394893e4aa8abc6eeb6a4a3934f4ca3562d7938f67240b5`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 05s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-65b0ef9a2e13d14c)
test result: ok. 9437 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 148.22s
     Running unittests src/main.rs (target/debug/deps/gateway-483566cd5f249439)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running unittests src/bin/pricing-tool.rs (target/debug/deps/pricing_tool-d0a19b224e9496a0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-4705fe001f3c3ef9)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.13s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-0a9faacc031f0801)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 0.00s
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 32.10s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.04s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-ea47471a5dbbe016)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 16.14s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 38 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.77s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-e91f4422f70ba56a)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.14s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-00ab693a85b6f4c0)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cfeedac72e91fe0)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-bb3cedcae13fbfe1)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-1457da6c6db74d19)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 11.48s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-db208652787adf43)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.90s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-8da43b5b2e94abb0)
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 31.78s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-284b05ef1ec8ac70)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.47s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-aa53cfee57d10c1b)
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.44s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-3f5c6ac3c01f26fb)
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.41s
     Running tests/lib.rs (target/debug/deps/lib-07eea0d4e92b5f9d)
test result: ok. 253 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 87.39s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-423fb182f8d3069b)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-eb09d842c44e25f1)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-a4bca7c8e6953b65)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-8b2da92115df6c2a)
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 28.87s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-ab47318e98b71eb4)
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.69s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-36cbff8d20e30c89)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 38.70s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-ab521677f92edefc)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-803dbec7e705ca3b)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-4f9104f2915c6436)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.48s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-a52cdca9147a5df2)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 7.06s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-19ee33dffc30bbd4)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-f67dc4a1de921aa8)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 9.52s
   Doc-tests litellm_rs
test result: ok. 31 passed; 0 failed; 31 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：77.8 秒。
原始日志 SHA-256：`aa43e15559c5e8350d944f5442a3fa5c5a3e864c0221ab63f85f6126cbf1e897`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 17s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`416f14293389258bfc83273871f2c46bdcd2d83e`；退出码：`0`；耗时：1.4 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## xAI accepted-main integration 5ca350aa

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`5ca350aae8228e1093e9d2b5c4af7d62c36a7bf9`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite --test audio_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`5ca350aae8228e1093e9d2b5c4af7d62c36a7bf9`；退出码：`0`；耗时：80.5 秒。
原始日志 SHA-256：`6846a4e09bafb948edce149c99557c80a8629b367d26b1c059314d433037eebd`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.53s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.11s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.11s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`5ca350aae8228e1093e9d2b5c4af7d62c36a7bf9`；退出码：`0`；耗时：30.9 秒。
原始日志 SHA-256：`3804e87afa10bcfb19486dd1b89dd872276690b4e1bfe9ab7894a72dfba9586f`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.71s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`5ca350aae8228e1093e9d2b5c4af7d62c36a7bf9`；退出码：`0`；耗时：1.5 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Responses accepted F07 integration c7984ab9

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`c7984ab98df41cc426cd48d1d05e8dcc917cf5a4`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`c7984ab98df41cc426cd48d1d05e8dcc917cf5a4`；退出码：`0`；耗时：1.1 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test gemini_router_fallback_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`c7984ab98df41cc426cd48d1d05e8dcc917cf5a4`；退出码：`0`；耗时：169.8 秒。
原始日志 SHA-256：`4967fab2b159ff3b5b0ed7841d91a3be785168611e5a38056180e7704a7d3ccb`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 33.39s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.85s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.94s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.17s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.27s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.86s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`c7984ab98df41cc426cd48d1d05e8dcc917cf5a4`；退出码：`0`；耗时：33.2 秒。
原始日志 SHA-256：`7c2f07198c9bf49f9c57603415ee10f6bdf59405a03391b1296d902e484d632d`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.06s
```

## Compaction routing/output/cache-write fixes c25a3188

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：26.0 秒。
原始日志 SHA-256：`1a8a8bd2948074213f17397ab8dc98b5ee24198d48c3d8b6864c225db78d353b`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.91s
```

### `cargo test -- --test-threads=2`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：212.5 秒。
原始日志 SHA-256：`63f0e0a784e4eb911125a4472520508e9a600fdd8ebce31e4522f9bcf746ef59`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 59.35s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 107.91s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.54s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.05s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：61.7 秒。
原始日志 SHA-256：`cc0e0c189d40f91f2adc49c02b83604168b7821021a271ec1b087539fb71ccdc`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test gemini_router_fallback_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：158.3 秒。
原始日志 SHA-256：`b2bdbbbd1ac4937a28e1abd6130d40c081cb987a699f67de50864dba4a2a69db`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 23.98s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.83s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.96s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.17s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.27s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.81s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：33.6 秒。
原始日志 SHA-256：`ab604478aaf965dfe1e2ba73a87a93d83bf3951da6850d93d5ce1d8f96794970`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.47s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`c25a3188a010de56b34e3334e7c3a0dcaa6fc3d5`；退出码：`0`；耗时：1.0 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Native cache-write OpenAPI 3ee6f7ef

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`3ee6f7ef340f5a498078d973da860adc83937871`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --lib openapi_contract_tests`

源码提交：`3ee6f7ef340f5a498078d973da860adc83937871`；退出码：`0`；耗时：110.4 秒。
原始日志 SHA-256：`9e5a6f472acf51969d6996f3e0c0f9bea08bcd3bf740dd5bbe1987c33ce3b666`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 48s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-8c847ea7a09bcebe)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 10259 filtered out; finished in 0.01s
```

## Realtime session cap/terminal errors e74ea291

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：7.4 秒。
原始日志 SHA-256：`7303ce34b32c5c0c0f041a4a7d577576201e8275d855608e6dc8de972852547f`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.34s
```

### `cargo test -- --test-threads=2`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：163.9 秒。
原始日志 SHA-256：`8a425a85e4fab6a1b9cd03600b7487624c5017d8fb50f497e136d71904843f99`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 18.52s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 109.08s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.54s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：19.1 秒。
原始日志 SHA-256：`3e9a4c52e3b38a6fb2ec07dc3ec2b3a451652c0a9fbdf4a8d6c61cf20ddebbba`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.96s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime -- --test-threads=2`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：154.6 秒。
原始日志 SHA-256：`23465e67db04eba1c6f4348192f2e62bfa12c244728deaba427daed9b3a3ce79`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 42s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 9815 filtered out; finished in 48.54s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：73.0 秒。
原始日志 SHA-256：`cf05dde428fb32c89eef83418282515ef5c6cbd4093b225c601b602d8851b3eb`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 12s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`e74ea2914eb86d6a1c72fb590a5d4d59554799f2`；退出码：`0`；耗时：1.1 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Realtime acknowledged scope/forwarded errors final validation

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：9.4 秒。
原始日志 SHA-256：`8879d5133d3bfe48f09c9ad4ea88ca93a541d9e0f0c49dfc20f0bfebaa101625`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.39s
```

### `cargo test -- --test-threads=2`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：154.9 秒。
原始日志 SHA-256：`2a7e6dfec7cd1ce7de0bec77e70090fdc24de039c466d8ff24458f528c8e0b3b`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 17.18s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 103.87s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.38s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.36s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.27s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.13s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：16.5 秒。
原始日志 SHA-256：`0e1a3a280007196d6e0c56270bbfcd83cb3d304f12875306dc56bc88c355c448`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 16.43s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime -- --test-threads=2`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：124.2 秒。
原始日志 SHA-256：`f716907f019891477ee9e8b507e7b72f58cc19114460fa1b458d4dae9cd4cc07`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 10s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 9815 filtered out; finished in 50.87s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：35.5 秒。
原始日志 SHA-256：`43e74efccdad96e589ddef4e063386a78a462432184c23600574a897917cd179`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 35.39s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`4bca53bfb4186b33c7300def62f5c8b947ecf0d3`；退出码：`0`；耗时：1.4 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Release preparation exact shipped profile e0d742a9

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a --bin gateway`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：47.8 秒。
原始日志 SHA-256：`4c992fc8f41351cf0b1eecec007adbf32648c2194c6a11241efad52446b3c614`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 47.70s
```

### `cargo test -- --test-threads=2`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：239.1 秒。
原始日志 SHA-256：`f964f2682edfe8916cbd6ebd797b81695c67bc1cc512edd627de38fff67ef9ec`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 42s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 110.37s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.46s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：51.0 秒。
原始日志 SHA-256：`087b803bca953f5fd73eb3cc2498ea0374083fc4371ffd45ef01a2cfee9384ae`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 50.90s
```

### `cargo clippy --all-targets --locked --no-default-features --features postgres,sqlite,redis,s3,metrics,tracing,websockets,providers-extra,providers-extended,mcp-validation,a2a -- -D warnings`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：101.0 秒。
原始日志 SHA-256：`1d20c6b8ada7db22857370e85af06f14296d07021e3d7b3e9a7166ef2c290ff1`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 40s
```

## Both Zhipu and Z.AI user_id contract bf42ea35

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`bf42ea35971df2b6cc473e8d02e1e603522b9fee`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`bf42ea35971df2b6cc473e8d02e1e603522b9fee`；退出码：`0`；耗时：37.6 秒。
原始日志 SHA-256：`d2a211b6dd53fb37ee0d190e16af7ea17acb37a9944eab0d78fdf36b61e21d0a`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 29.48s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.56s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`bf42ea35971df2b6cc473e8d02e1e603522b9fee`；退出码：`0`；耗时：31.6 秒。
原始日志 SHA-256：`45c425f76fb650f72a91d47276dcbf9e79fe95df798ba27a4b884bbf8a03f162`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.39s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`bf42ea35971df2b6cc473e8d02e1e603522b9fee`；退出码：`0`；耗时：1.2 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Chinese float-only admission final source

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`fa1c8eab02c29417e1fa729af1167acefb7cee43`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`fa1c8eab02c29417e1fa729af1167acefb7cee43`；退出码：`0`；耗时：47.4 秒。
原始日志 SHA-256：`a6ff53e24b950255619fe4154cb76af27a6f51e94eba600c8d99f04876a4246e`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 39.39s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.57s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`fa1c8eab02c29417e1fa729af1167acefb7cee43`；退出码：`0`；耗时：34.4 秒。
原始日志 SHA-256：`764ecb4663d4a9eae04a79a2d1832285b080783aa4ca1f971cc5e93090af91c4`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 34.22s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`fa1c8eab02c29417e1fa729af1167acefb7cee43`；退出码：`0`；耗时：1.2 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## OpenAI implicit cache-write scope final source

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：2.6 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：6.3 秒。
原始日志 SHA-256：`d953cecf462f2ab6e71cdfdd78eecd544b386bed8b7a0fefbafb981777280a7b`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.27s
```

### `cargo test -- --test-threads=2`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：149.5 秒。
原始日志 SHA-256：`12892a832469923da9321445088a98129af576f1227e29472e445c2b700d4e36`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 14.33s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 105.94s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.43s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.29s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.08s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：18.3 秒。
原始日志 SHA-256：`a7f42b86ee682198f8a2bdc9462b46b5f23d1adc520f48a6432b2ba7d17ed074`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.15s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test gemini_router_fallback_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：163.5 秒。
原始日志 SHA-256：`393f4962ad20a7fa334ac78cf89db99011e0092bc29541a077241ab6274e3210`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 28.18s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-ecdfd0d64b1b6abd)
test result: ok. 36 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.81s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-b808fc23751062ee)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.97s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-58abd9f97ef42733)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.19s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-c024eabe49d0a989)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.05s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-07cab494fd4693f7)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.81s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a -- -D warnings`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：32.5 秒。
原始日志 SHA-256：`0dfdb127ba0096ec3d8516724083ed41cb0a3d3d589a17c1b72861f77f8ad7d8`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 32.30s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`d61979f9d288be5d772326dfe539d0006d6bf435`；退出码：`0`；耗时：1.1 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Realtime unrepresentable caller cap final source

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：8.5 秒。
原始日志 SHA-256：`89d4f2ae0ed738fdab929bad9298b1ee5da8681b3fa49572c9c3bbba66cf5b9f`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.41s
```

### `cargo test -- --test-threads=2`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：157.9 秒。
原始日志 SHA-256：`5edfb319e88d9f9db5ecf2d6888507e572f79405b4d726e1a39b8e1485dc89a0`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 13.63s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 106.64s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.43s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.14s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：17.4 秒。
原始日志 SHA-256：`8fa62ac9c38b3872cb12ac86ee059bbe0826ce801b823fc8fff36bb1e9db8f33`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.25s
```

### `cargo test --features gateway,sqlite,websockets,a2a,mcp --lib server::routes::ai::realtime -- --test-threads=2`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：91.6 秒。
原始日志 SHA-256：`1938a21b8be351a1640354d254e9b41e8676ac5aab1234ab8955c6cb725a6656`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 35.55s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-ef74d497d8b5c13d)
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 9815 filtered out; finished in 52.70s
```

### `cargo clippy --all-targets --features gateway,sqlite,websockets,a2a,mcp -- -D warnings`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：33.8 秒。
原始日志 SHA-256：`6bcdc68d3db641b683edbfecefce389a5c65c4a49c2a68157f946f71c19aff94`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 33.66s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`5d691d1f61f2beef12dc6d7971818ddfadb32f6d`；退出码：`0`；耗时：1.2 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Release cancelled audio target exact features, not concurrency root cause proof

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo test --features postgres,sqlite,redis,s3,metrics,tracing,websockets --test audio_native_providers gateway_routes_select_native_deepgram_and_elevenlabs -- --nocapture`

源码提交：`e0d742a9de0101d21b6a57142245e4df040466c4`；退出码：`0`；耗时：107.0 秒。
原始日志 SHA-256：`de71592edf1724a435755aa3e567c5cb099eb3b59bad6f7eba5f279406651cc8`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 39s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-4c061e1f76fb89d3)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 18 filtered out; finished in 5.64s
```

## xAI post-merge language formatting follow-up #1439

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：2.4 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：6.3 秒。
原始日志 SHA-256：`b7774566f61c92a0b382f18bbc376ae2ce1e044bd54d86a3e1745980c98e625e`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.23s
```

### `cargo test -- --test-threads=2`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：154.8 秒。
原始日志 SHA-256：`65f973c1eb90349a812092a2f50cbb6d62041e50f0676b3f2f4ab94eaed921d3`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 23.35s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7133 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.45s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.51s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.28s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：18.2 秒。
原始日志 SHA-256：`9d44481ce2746c898132c71d8104c2e631fd46c0946e941e2112b30069343631`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.09s
```

### `cargo test --features gateway,sqlite --test audio_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：81.0 秒。
原始日志 SHA-256：`8b14c35d62a33da27c3392577348061780a6fe1ba4cdf2fa4ad12426afd7b698`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 32.11s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 40.06s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.09s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：29.3 秒。
原始日志 SHA-256：`136921b609242977b93c901b12ce3b7a650aa12e37db276293d96bf6190e982a`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.12s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`e4e4ee0da0e072ae88e9c724f908bcbb3b427b1d`；退出码：`0`；耗时：1.3 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## xAI actual gateway language-format integration final head

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`0f5ff3527b539e5a72abb721f5a98940153345d7`；退出码：`0`；耗时：2.5 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo test --features gateway,sqlite --test audio_routes --test catalog_nonchat_routes -- --test-threads=2`

源码提交：`0f5ff3527b539e5a72abb721f5a98940153345d7`；退出码：`0`；耗时：74.7 秒。
原始日志 SHA-256：`73c145739712fab45a2e640eba2be042fa3b8c68f9fcd0b0bf4c3053e58331ed`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 19.40s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-fd5378a956d1fb05)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.71s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-3f3f94a3a101728c)
test result: ok. 41 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.08s
```

### `cargo clippy --all-targets --features gateway,sqlite -- -D warnings`

源码提交：`0f5ff3527b539e5a72abb721f5a98940153345d7`；退出码：`0`；耗时：27.4 秒。
原始日志 SHA-256：`f1270e0928a6fb794c2b746b08fe29cffe2c5a149da75e8334d3cfb23b3f4ba6`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.30s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`0f5ff3527b539e5a72abb721f5a98940153345d7`；退出码：`0`；耗时：1.2 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## Final combined candidates: default/full and all affected profiles

默认源码提交：`按各命令记录`（更精确的每条命令提交见下方）。

### `cargo fmt --check`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：2.7 秒。
原始日志 SHA-256：`e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`。

命令成功完成，无结果正文。

### `cargo check`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：20.9 秒。
原始日志 SHA-256：`4b1bc472bb4ece1660b49ae414cb8093f0076b8de2db87b2d633b9796f7319e4`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.80s
```

### `cargo test -- --test-threads=2`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：162.4 秒。
原始日志 SHA-256：`557c44f26207675af193bc4caf5376298564f64b1c7ad7409f25933f670db16b`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 31.32s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-5f00bb94822156cc)
test result: ok. 7134 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 104.65s
     Running tests/api_key_budget_routes.rs (target/debug/deps/api_key_budget_routes-0c2486c0401784ce)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_native_providers.rs (target/debug/deps/audio_native_providers-47b7cad75e34e090)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-adb7c7d30f5ad92a)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/batches_routes.rs (target/debug/deps/batches_routes-968b1e045ee1c4b2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-7dff907de8d676f3)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/conformance_chat_entries.rs (target/debug/deps/conformance_chat_entries-f4514f58964b7568)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/cost_compatibility.rs (target/debug/deps/cost_compatibility-49f1ed00e26a1d19)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.39s
     Running tests/external_provider_registration.rs (target/debug/deps/external_provider_registration-0cb11b1a40549fa3)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s
     Running tests/files_routes.rs (target/debug/deps/files_routes-674db1bc3cbf3724)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/fine_tuning_routes.rs (target/debug/deps/fine_tuning_routes-23a0e71e49990457)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-d291096853550ff9)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-0d50d3f82f6607d8)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/gh965_d1ec_retry_helper_deprecation.rs (target/debug/deps/gh965_d1ec_retry_helper_deprecation-6d9a3f82743aa05a)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.82s
     Running tests/image_edit_variation_routes.rs (target/debug/deps/image_edit_variation_routes-f37efee5cd6b4f0c)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/image_router_fallback_routes.rs (target/debug/deps/image_router_fallback_routes-c362ad8cbf74602d)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/lib.rs (target/debug/deps/lib-d519f7ebc633b8bf)
test result: ok. 117 passed; 0 failed; 12 ignored; 0 measured; 0 filtered out; finished in 0.31s
     Running tests/live_bedrock.rs (target/debug/deps/live_bedrock-375a5d78a93cc723)
test result: ok. 0 passed; 0 failed; 3 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/live_gemini_catalog.rs (target/debug/deps/live_gemini_catalog-c177f58b525a78e0)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/media_native_providers.rs (target/debug/deps/media_native_providers-390cdb5bd708c67e)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/moderations_routes.rs (target/debug/deps/moderations_routes-e3efb2ddebf79672)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_messages_routes.rs (target/debug/deps/native_messages_routes-8a183910b16ccbd5)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-3eefe09c62a66c68)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/openai_legacy_function_forwarding.rs (target/debug/deps/openai_legacy_function_forwarding-e8b8c53473c312a4)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.26s
     Running tests/public_api_compat.rs (target/debug/deps/public_api_compat-402cd09306e5edba)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/rerank_routes.rs (target/debug/deps/rerank_routes-0c4b1896f6bc3035)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-eeb315bde38b5ed2)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/test_connection_pool.rs (target/debug/deps/test_connection_pool-ff5276b994b0840c)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
     Running tests/voyage_retrieval_routes.rs (target/debug/deps/voyage_retrieval_routes-59ffbccd00fad178)
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   Doc-tests litellm_rs
test result: ok. 30 passed; 0 failed; 27 ignored; 0 measured; 0 filtered out; finished in 0.04s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.07s
```

### `cargo clippy --all-targets -- -D warnings`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：24.3 秒。
原始日志 SHA-256：`da1d46d1bf2fd14a01c5ab819d1d442dbc7c9ddc1ff5d8b68de09f2aba32df58`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 24.19s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a,websockets --test native_responses_routes --test responses_routes --test gemini_sdk_routes --test gemini_router_fallback_routes --test catalog_nonchat_routes --test audio_routes -- --test-threads=2`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：265.7 秒。
原始日志 SHA-256：`68a4bfcb96376e19e04dc5a60821b170e30040226953e664305f3823c8be3ff3`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1m 23s
     Running tests/audio_routes.rs (target/debug/deps/audio_routes-bca3ecea8ecd48e5)
test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 45.87s
     Running tests/catalog_nonchat_routes.rs (target/debug/deps/catalog_nonchat_routes-6060ed2dd9fd6b19)
test result: ok. 47 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.55s
     Running tests/gemini_router_fallback_routes.rs (target/debug/deps/gemini_router_fallback_routes-31b12940b0c49848)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.97s
     Running tests/gemini_sdk_routes.rs (target/debug/deps/gemini_sdk_routes-2750e5d16bd5e4dd)
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 37.18s
     Running tests/native_responses_routes.rs (target/debug/deps/native_responses_routes-bf1f4cb264006c47)
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 62.09s
     Running tests/responses_routes.rs (target/debug/deps/responses_routes-fa43b1d3dc21f96c)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 10.82s
```

### `cargo test --features gateway,sqlite,providers-extended,mcp,a2a,websockets --lib server::routes::ai::realtime -- --test-threads=2`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：176.5 秒。
原始日志 SHA-256：`33149fb1fbf58b19f011facf3480d3397a86876d5fce86aec938db5abf3d0fe4`。

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 2m 01s
     Running unittests src/lib.rs (target/debug/deps/litellm_rs-42a9ef04f2b9fa14)
test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 10285 filtered out; finished in 51.67s
```

### `cargo clippy --all-targets --features gateway,sqlite,providers-extended,mcp,a2a,websockets -- -D warnings`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：83.2 秒。
原始日志 SHA-256：`946c442f71ad50112863a11c48993a3f9fdade357919acceef460ee8543b89c6`。

```text
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 23s
```

### `python3 scripts/sync_litellm_pricing.py --source-catalog config/model_prices_extended.json --check`

源码提交：`3a7b83413ceb26c452eb11a96f463b012d804d73`；退出码：`0`；耗时：1.4 秒。
原始日志 SHA-256：`71a8dbaeaded626c9ac4bd7ebed1bf7f480edbb56e7fc3d2aa4b8cc256f42fd9`。

```text
validated 4451 upstream LiteLLM pricing entries and 220 local compatibility entries from https://raw.githubusercontent.com/BerriAI/litellm/a5fef4b4e68963640c3062d464509129ec8863c6/model_prices_and_context_window.json; classified 4569 exact pricing rows
```

## 已取消的完整测试尝试（不计通过）

Realtime `2ee7911b` 首次特性完整测试在库测试通过后，挂起于既有 Responses 集成测试，停止进程后退出码 143。完整原始日志及 SHA-256 在归档 cancelled/ 中。该目标独立运行的 8 项测试通过；相同提交完整两线程重跑通过，见上方 realtime-admission 记录。挂起根因尚未证明，不声称已修复，不增加任意 timeout 掩盖问题。

## 远端验证入口

- Responses 历史 `0050a21b`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37120379335)、[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37120381165) 已成功，只适用于该旧提交。
- F07 已接受 `c44b48d4`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126573698)。
- F09 历史 `48f1b2be`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126576346)、[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37126578702)。

新提交的本地定向检查不等同最新完整 CI 通过。最终 GitHub 快照与合并结果见 [台账](litellm-parity-tracker.md)。没有实际供应商付费调用；发行准备编译不等同最终候选产物已发布。

## 中文供应商发行 CI 取消记录

`cfbf58bf` 的 [发行特性检查](https://github.com/majiayu000/litellm-rs/actions/runs/37125747886)首次 attempt 在 Test release features 阶段取消；日志显示既有 test_responses_runtime_policy_errors_use_openai_shape 超过 60 秒仍未完成。原始 job 111210574865 日志及 SHA-256 在 github/cn-release-cancelled 中保留。本轮只请求一次未成功job重跑，该次随后cfbf58bf的15项检查成功；此结果不证明更新head通过。定向复核使用完全相同的 release feature flags，若通过也不能证明并发挂起根因已修。

归档github/保留本轮27个实际合并head的成功CI回执及当前Responses手动CI精确head/status/conclusion。它们是时间点证据，不预测新提交。

## 后续未通过/空筛选记录

not-success/保存百川fixture/trait/锁scope错误、Realtime旧取消成功断言与空筛选0测试。它们全部不计实际测试通过；修正后的实际完整/定向检查在对应精确head组中。

F07 c44b48d4首次Main Full取消：原始job111212951066日志/元数据在github/，既有ElevenLabs目标的精确复核单列。一项定向测试成功不能证明并发挂起根因已修；只请求一次未成功job重跑，最终结论见手动run快照。

6aa3a043的CI Fast Test job111232671207首次取消，原始log/metadata在github/cn-6aa-unsuccessful中；它不证明后续360bf9ce检查成功，后续source/CI独立记录。

Realtime live-policy初轮完整扩展检查因审查发现活动响应lease必须保留而主动停止，退出143、不计通过；最终修正后的完整验证单独绑定最终source。

## 2026-10-04 未完成与修复过程

Realtime 4cc4e4fc 扩展全套在库测试完成后挂起于既有 moderation alias 集成测试，精确进程停止后 cargo 退出101；不计扩展全套通过，也不声称根因已修。中文供应商30c整合在clippy发现重复响应分支；Responses首次用了不存在的测试目标。后续修正检查单独绑定各最终提交。完整过程在 not-success/continuation-results.json，不能把前几个成功命令当作整组成功。

## 发行准备首次 CI 取消

e0d742a9 的 CI Fast 首次 Test 超时取消，日志最后挂起于既有 Deepgram/ElevenLabs gateway 音频目标。相同特性单独运行 1 项通过；这不证明并发挂起根因已修。保留原 job 日志与一次失败作业重跑的实际结论，只以最终全部成功的精确 head 回执验收。
