# 2026-10-03 issues / PR 本地验证记录

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

## 已取消的完整测试尝试（不计通过）

Realtime `2ee7911b` 首次特性完整测试在库测试通过后，挂起于既有 Responses 集成测试，停止进程后退出码 143。完整原始日志及 SHA-256 在归档 cancelled/ 中。该目标独立运行的 8 项测试通过；相同提交完整两线程重跑通过，见上方 realtime-admission 记录。挂起根因尚未证明，不声称已修复，不增加任意 timeout 掩盖问题。

## 远端验证入口

- Responses 历史 `0050a21b`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37120379335)、[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37120381165) 已成功，只适用于该旧提交。
- F07 最新 `c44b48d4`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126573698)。
- F09 最新 `48f1b2be`：[Main Full](https://github.com/majiayu000/litellm-rs/actions/runs/37126576346)、[Cross Platform](https://github.com/majiayu000/litellm-rs/actions/runs/37126578702)。

新提交的本地定向检查不等同最新完整 CI 通过。最终 GitHub 快照与合并结果见 [台账](litellm-parity-tracker.md)。没有实际供应商付费调用；发行准备编译不等同最终候选产物已发布。

归档 github/ 同时保留本轮 14 个实际合并 head 的 15 项成功检查回执，以及当前 Responses 手动 CI 的精确 head/status/conclusion 快照。它们是时间点证据，不预测之后的新提交。
