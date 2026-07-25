# Tech Spec

## Linked Issue

GH-1108 / #1108

## Product Spec

见 `specs/GH1108/product.md`。

## Implementation Gate

本实现依赖 GH1112 的 production neutral Google catalog API。当前
`origin/main@f09ddb7d4f871e735b9b132db58ae7e2300c7231` 尚无
`src/core/providers/google/**`，GH1112 又因 same-issue circuit breaker 被 `parked`。

因此本 packet 可以独立审查和合并，但 implementation lane 在以下条件全部满足前必须
保持 blocked：

1. GH1112 implementation 已合并到 `origin/main`；
2. merged head 提供 single neutral catalog、Developer availability overlay 和 shared
   request contract；
3. 本 spec 的 planned paths 与 merged API 重新核对；若 API/路径不同，先修订 tech/task，
   不得写回旧 `gemini/models/**` 建立第二套 authority；
4. fresh duplicate evidence 与 `implement` route gate 为 `allowed`。

## Codebase Context

以下锚点已在 `origin/main@f09ddb7d4f871e735b9b132db58ae7e2300c7231` 核验。

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Current Gemini registry | `src/core/providers/gemini/models/mod.rs:144-179` | 以 `HashMap` 存 17 个模型，duplicate insert 会覆盖，`list_models()` 直接返回 values。 | GH1112 将迁移为 neutral exact catalog；GH1108 只能在其 merged API 上刷新。 |
| Current 3.5 catalog | `src/core/providers/gemini/models/catalog/gemini35.rs:7-68` | 只登记 `gemini-3.5-flash`，无 3.6 Flash 或 3.5 Flash-Lite。 | 两个新 GA model records 的现状锚点。 |
| Provider list/preflight | `src/core/providers/gemini/provider.rs:51-56,74-112` | 模型列表来自 registry；validation 只有通用温度/top-p 数值范围。 | 需要按 exact model contract 拒绝 deprecated params 与 prefill。 |
| Supported params/mapping | `src/core/providers/gemini/provider.rs:169-212` | 所有 Gemini model 共用参数表；`temperature`/`top_p` 被映射，部分 unsupported 参数被 silent skip。 | B-005/B-007 要改为 model-specific fail-closed contract。 |
| Final Developer body | `src/core/providers/gemini/client.rs:252-320` | `transform_chat_request` 独立构造 contents 与 `generationConfig`，仍写入 temperature/topP。 | 必须证明 final upstream body 与 preflight 一致，direct-client 也不能绕过。 |
| Pricing storage | `src/core/providers/gemini/models/mod.rs:83-105,127-140` | pricing helper 接收 per-million 值并换算到 per-1k fields，metadata 与 limits 同记录。 | 新价格的单位换算、cost parity 与 GH1113 边界。 |
| Credential config | `src/core/providers/gemini/config.rs:14-15,82-105,134-141` | Developer config 从 env 读 key；当前 type derive `Debug`。 | live smoke 不得格式化/落盘 key；production credential redaction 由 GH1112 T4 所有。 |
| Existing live-test pattern | `tests/live_bedrock.rs:1-27,63-68` | `#[ignore]` + 单一 opt-in env，未开启时不联网。 | 复用成熟手动验证形态，不创建后台自动化。 |
| Dependency contract | `specs/GH1112/tech.md`、`specs/GH1112/tasks.md` | 已声明 neutral `google/models` owner、Developer overlay、shared request contract，T1→T2 串行。 | GH1108 implementation 的唯一合法 owner/base gate。 |

## Planned Changes

```specrail-planned-changes
{
  "issue": 1108,
  "complete": true,
  "paths": [
    "src/core/providers/google/models/registry.rs",
    "src/core/providers/google/models/request_contract.rs",
    "src/core/providers/google/models/catalog/mod.rs",
    "src/core/providers/google/models/catalog/gemini35.rs",
    "src/core/providers/google/models/catalog/gemini36.rs",
    "src/core/providers/google/models/tests.rs",
    "src/core/providers/gemini/provider.rs",
    "src/core/providers/gemini/provider_tests.rs",
    "src/core/providers/gemini/client.rs",
    "tests/gemini_router_fallback_routes.rs",
    "tests/live_gemini.rs",
    "docs/providers/README.md",
    "docs/providers/gemini.md"
  ],
  "spec_refs": [
    "specs/GH1108/product.md#behavior-invariants",
    "specs/GH1108/product.md#验收标准",
    "specs/GH1108/tech.md#implementation-gate",
    "specs/GH1112/tech.md",
    "specs/GH1112/tasks.md"
  ]
}
```

`src/core/providers/google/**` 是 GH1112 计划并拥有的路径，当前尚不存在。implementation
开始前必须以 merged GH1112 head 重新验证以上清单；任何必要路径差异通过 spec amendment
处理，不能把旧 `src/core/providers/gemini/models/**` 加回 manifest。

## 设计方案

### 1. 证据驱动的 Developer catalog delta

在 GH1112 neutral registry 上增加两个 exact records：

- `gemini-3.6-flash`：Developer `available_exact`、GA、1,048,576 input、
  65,536 output、1.50/7.50 USD per million；
- `gemini-3.5-flash-lite`：Developer `available_exact`、GA、相同 limits、
  0.30/2.50 USD per million。

每个 record 保留 Developer official URL、reviewed-at、lifecycle stage 和 shutdown date
（当前为 none）。`gemini36.rs` 是新 family owner；`gemini35.rs` 只扩展现有 3.5 family。
Developer availability 只写入 Developer overlay；Vertex overlay 保持只读且不得由本 PR
改变。

对 migration 前 Developer catalog 的每个 ID 生成 deterministic disposition fixture：

- `available_exact`：继续公开；
- `retired` / `shutdown`：停止公开；
- `unverified` / `other_product`：默认不公开。

fixture 必须列出 pre/post advertised IDs 与 reason/source。registry 初始化继续由 GH1112
规则拒绝 duplicate、missing evidence、missing contract 和非法 lifecycle。

### 2. 新模型请求契约

在 shared `request_contract.rs` 中为两个 exact IDs 声明 closed allowed-param set 和
illegal-state policy：

- `temperature`、`top_p`、`top_k` 不在 allowlist；
- 请求对象只要显式携带任一字段（包括 `null`/默认值）即 typed invalid-request；
- contents 规范化后，最后一个非空 role 为 `model` 即 prefill rejection；
- user/tool 结尾保持合法；
- 只有官方明确声明相同契约的未来 model record 才能复用，禁止 family substring 推断。

Gemini provider 的 supported params、`validate_request`、`map_openai_params` 和
`GeminiClient::transform_chat_request` 都查询同一 contract。最终 body serializer 只消费
已经校验的 provider-neutral fields；direct client entry point 也先执行同一 preflight。
网络计数 fixture 证明所有负例在 auth/HTTP 之前终止。

### 3. Pricing 与能力边界

价格仍作为 neutral model metadata 的现有字段写入，保持 GH1112 已定义的 access API。
测试从“官方 per-million 数值 → stored per-1k 数值 → cost for fixed tokens”逐层断言单位，
避免 1000 倍换算错误。

本 issue 只为两个新模型提供确定 pricing facts：

- 不更改 pricing authority、fallback、unknown-cost 或 spend/budget/callback 路径；
- 不新增零成本 fallback；
- pricing consumer 若需要新 accessor，必须先走 GH1113 spec amendment，而不是在本 PR
  建第二套价格表。

能力来自官方 exact model pages，并限于当前 Gemini provider 已实现且可兑现的能力。
官方产品页面存在、但 provider 尚未实现的 Computer Use、Interactions-only state 等不得
只靠 metadata 宣称支持。

### 4. Opt-in live smoke

新增 `tests/live_gemini.rs`，复用现有 live Bedrock pattern：

- `#[ignore]`；
- 只有 `LITELLM_RS_LIVE_GEMINI=1` 才允许联网；
- key 只从 `GEMINI_API_KEY`/既有 Developer credential path 读取，不写入命令、Debug、
  error 或 artifact；
- 依次执行 static snapshot、official models get/list、两个 exact ID 的最小
  generate-content call；
- 结果写为 typed in-memory/temporary artifact，字段只含 model、step、status、
  error_class、HTTP status（若可安全公开）与时间，不含 request URL query/header/body
  credential；
- 错误分类闭集 `{auth, quota, not_found, protocol, network}`；
- cancellation/timeout 标记 incomplete，不把部分步骤聚合为 pass。

offline unit test 使用 sentinel key 和 loopback/fake response 覆盖分类与 redaction；
普通 `cargo test` 只运行 offline tests，live cases 保持 ignored 且 opt-in 双门禁。

### 5. 文档与发布 snapshot

`docs/providers/gemini.md` 记录：

- 新模型 exact IDs、limits、价格与 sampling/prefill migration；
- live smoke 的 opt-in 命令、所需 env、错误分类和“不向 issue/PR 粘贴原始错误”的安全说明；
- Developer/Vertex 分离；
- 被停止公开的旧 ID disposition。

`docs/providers/README.md` 只增加 provider doc 索引。不得修改高上下文
`AGENTS.md`/`CLAUDE.md` 或用户配置。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | neutral exact records + Developer overlay | `cargo test --locked google_model_catalog_2026_07_exact_ids`；大小写/前后缀负例均不命中。 |
| B-002 | `gemini35.rs`/`gemini36.rs` limits and pricing | `cargo test --locked google_model_catalog_2026_07_metadata`；断言 per-million 与 stored per-1k/cost。 |
| B-003 | Developer evidence filter | `cargo test --locked google_model_catalog_2026_07_dispositions`；retired/shutdown/unverified/other-product 不公开。 |
| B-004 | deterministic disposition fixture | 同一 test 输出 pre/post set、status/source/reason 并断言每个旧 ID 恰好一个 disposition。 |
| B-005 | shared request allowlist + provider mapping | `cargo test --locked gemini_2026_07_deprecated_sampling_rejected`；三字段含 null/default/non-default 均 pre-network error。 |
| B-006 | normalized contents prefill gate | `cargo test --locked gemini_2026_07_prefill_rejected`；model+trailing-empty 拒绝，user/tool 结尾通过。 |
| B-007 | provider/client contract parity | `cargo test --locked gemini_2026_07_request_contract_parity`；supported params、preflight、final JSON table fixture 一致。 |
| B-008 | neutral pricing facts only | `cargo test --locked gemini_2026_07_cost`；fixed-token cost 精确，unknown behavior snapshot 不变。 |
| B-009 | immutable stable snapshot | `cargo test --locked google_model_catalog_2026_07_stability`；重复/并发查询结果完全相等、升序、无重复。 |
| B-010 | double-gated live test | `cargo test --locked --test live_gemini -- --ignored` 在无 opt-in/key 时 network counter=0；真实调用只用显式命令。 |
| B-011 | typed live result aggregation | offline fixture `cargo test --locked live_gemini_result_classification` 覆盖五类错误与 missing-step false-pass；手工 opt-in 跑 list/get/call。 |
| B-012 | credential redaction | `cargo test --locked live_gemini_redaction`；sentinel 在 stdout/stderr/error/debug/artifact capture 中零命中。 |
| B-013 | Vertex overlay/read paths | `git diff --name-only <base>...HEAD` 不含 Vertex production path；catalog fixture 证明 Vertex set 未变。 |
| B-014 | live cancellation/retry state | `cargo test --locked live_gemini_interruption`；timeout/cancel 为 incomplete，重试生成新 run id。 |
| B-015 | existing model compatibility | `cargo test --locked gemini_provider` 与 migration snapshot；只允许 fixture 声明的 advertised-ID delta。 |
| B-016 | evidence manifest validation | `cargo test --locked google_model_catalog_2026_07_evidence`；missing/conflicting/stale/unofficial evidence 初始化失败。 |

## 数据流

```text
official Developer evidence fixture (offline, immutable)
  -> GH1112 neutral Google catalog validation
  -> Developer availability filter
  -> stable Gemini models()
  -> shared model request contract
       -> supported params
       -> preflight validation
       -> final Developer request body
  -> existing Developer endpoint + query API key

explicit live opt-in + Developer credential
  -> static snapshot
  -> official list/get
  -> minimal call
  -> typed redacted result (pass | incomplete | classified failure)
```

正常构建、单元测试和运行时不读取远端模型目录，也不根据 live smoke 修改 catalog。

## 备选方案

1. **直接写回旧 `gemini/models/**`**：拒绝。会绕过 GH1112 single authority，并让后续
   Vertex/pricing/tool work 再次分叉。
2. **先实现两个 ID，GH1112 以后再迁移**：拒绝。当前 GH1112 是明写依赖；短期第二套
   authority 会产生不可审计的中间状态。
3. **静默删除 deprecated sampling fields**：拒绝。调用方会误以为参数生效；按 B-005
   返回 stable error 才能阻止 silent degradation。
4. **只依赖 live list-models**：拒绝。单次连通性不等于 lifecycle、通用 chat、pricing
   或长期可用证据。
5. **把 live smoke 加进默认 CI**：拒绝。凭证、quota、网络波动会破坏确定性，并把手动
   验证过早自动化。

## 风险

- **Security**：live smoke 接触 API key；必须双门禁、sentinel redaction、禁止原始
  URL/header/error artifact。SEC-11 要求 exact-head 人工/独立审查。
- **Compatibility**：deprecated params 从可能 ignored 变为 deterministic error；这是
  有意收紧，发布说明必须明确。停止公开旧 ID 也必须逐项列 disposition。
- **Data correctness**：official pages 可能在 spec 到 implementation 之间变化；实现时
  fresh re-verify reviewed-at/lifecycle/pricing，冲突按 fail closed。
- **Dependency**：GH1112 当前 parked；在其 API 合并前本实现不可开始。若其 API 改名，
  先修 tech manifest，不猜路径。
- **Performance**：catalog 增量和 contract lookup 为 O(1)；排序只在 snapshot 构造时
  发生。live smoke 不进入生产热路径。
- **Maintenance**：未来 model 不能靠 family substring 自动继承 request contract；
  每个 record 显式绑定证据与 contract。

## 测试计划

- [ ] Catalog: `cargo test --locked google_model_catalog_2026_07`
- [ ] Provider contract: `cargo test --locked gemini_2026_07`
- [ ] Router/network negatives: `cargo test --locked gemini_router_fallback`
- [ ] Offline live-smoke fixtures: `cargo test --locked live_gemini`
- [ ] Manual opt-in:
      `LITELLM_RS_LIVE_GEMINI=1 cargo test --locked --test live_gemini -- --ignored`
- [ ] Format/build: `cargo fmt --all -- --check && cargo check --locked`
- [ ] Strict lint: `cargo clippy --locked --all-targets -- -D warnings`
- [ ] Full suite: `cargo test --locked`
- [ ] SpecRail:
      `python3 checks/check_workflow.py --repo . --spec-dir specs/GH1108 &&
       python3 checks/check_workflow.py --repo .`
- [ ] Diff integrity: `git diff --check`
- [ ] Coverage:
      `cargo llvm-cov --locked --all-features --workspace --branch --lcov
       --output-path artifacts/coverage/GH1108/lcov.info`，以 exact
      `COVERAGE_BASE_SHA...COVERAGE_HEAD_SHA` 的新增 production Rust 可执行行为为分母，
      要求 line coverage ≥80%，并要求新增 catalog evidence/validation、deprecated-param
      rejection、prefill rejection、live classification/redaction branch records 100% 命中。

full suite、strict Clippy、coverage 与 SpecRail gates 在 exact implementation head 各执行
一次；reviewer 默认 inspection/focused，避免重复 full run。

## 回滚方案

以完整 implementation PR 为单位回滚 catalog delta、request contract 和 live smoke。
不得只恢复 silent sampling drop、保留新 model IDs 但删除 evidence，或把 ID 写回旧
Gemini registry 作为“部分回滚”。回滚 binary 前，operator 应停止配置两个新 ID；live
smoke 无持久状态，只删除/禁用新测试和文档入口。若回滚原因是官方 lifecycle 变化，应
先将受影响 model fail closed，再通过独立 evidence update PR 恢复。
