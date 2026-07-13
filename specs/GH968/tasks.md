# Task Plan

## Linked Issue

GH-968 / #968

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP968-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: coordinator. Dependencies: none. Done when: GH968 三件套存在，product invariant、tech mapping 与 task coverage 集合完整，latest SpecRail packet/route gate 通过，并记录十段 serial PR 计划. Verify: `python3 checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH968"`; 比较 product/tasks 的 `B-[0-9]{3}` 集合。
- [x] `SP968-T2` Covers: B-001, B-002, B-005, B-006, B-009. Owner: policy implementation owner. Dependencies: SP968-T1. Done when: policy PR 提供 endpoint access/policy、scheme/host/effective-port 私网绑定、统一 URL/IP 校验、globally-routable-only IPv6 分类和永久 metadata 拒绝；不新增尚未接线的 Gateway config 或 HTTP client；PR 使用 `Refs #968`. Verify: focused `core::net` tests；`cargo check --all-targets --all-features --locked`; scope/overlap；independent current-head review + CI + required PR gate。
- [x] `SP968-T3` Covers: B-002, B-003, B-004, B-005, B-006, B-008, B-009, B-012. Owner: HTTP client implementation owner. Dependencies: SP968-T2 merged. Done when: HTTP client PR 提供不泄漏裸 `reqwest::Client` 且支持 headers/auth/query/body/json/form/multipart 的 request builder、普通/streaming/no-redirect policy client 与可注入 resolver；deterministic rebinding/literal/redirect/private tests 证明负例未建立 socket；PR 使用 `Refs #968`. Verify: focused `utils::net::http` tests、all-target check、strict clippy、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T4` Covers: B-007, B-011, B-012. Owner: image-edit fixture decomposition owner. Dependencies: SP968-T3 merged. Done when: `image_edit_variation_routes.rs` 拆到 800 行以内，移动的测试主体、断言与覆盖集合不变；PR 使用 `Refs #968`. Verify: moved-tail byte diff、test function set、focused/full tests、file-size/scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T5` Covers: B-007, B-011, B-012. Owner: image-router fixture decomposition owner. Dependencies: SP968-T4 merged. Done when: `image_router_fallback_routes.rs` 以相同约束拆分；PR 使用 `Refs #968`. Verify: moved-tail byte diff、test listing、focused/full tests、file-size/scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T6` Covers: B-007, B-011, B-012. Owner: completions fixture decomposition owner. Dependencies: SP968-T5 merged. Done when: `integration/completions_route_tests.rs` 以相同约束拆分；PR 使用 `Refs #968`. Verify: moved-tail byte diff、test listing、focused/full tests、file-size/scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T7` Covers: B-007, B-011, B-012. Owner: fixture consolidation owner. Dependencies: SP968-T6 merged. Done when: 14 处 OpenAI/OpenAI-like loopback mock config 使用 `tests/common/providers.rs` 共享构造器，行为与断言不变，不添加尚未声明或会被忽略的 access 值；PR 使用 `Refs #968`. Verify: 全量测试前后同绿、source search 无剩余重复 mock provider 基础字面量、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T8` Covers: B-001, B-005, B-007, B-011. Owner: config construction normalization owner. Dependencies: SP968-T7 merged. Done when: 四处动态/默认 OpenAI 构造保留 `Default` 字段，request-level override 仍无私网 opt-in；不发请求的 factory 单测使用公网形态 URL，运行时行为不变；PR 使用 `Refs #968`. Verify: focused router/factory tests、all-feature check/test、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T9P` Covers: B-007, B-011, B-012. Owner: runtime fixture preparation owner. Dependencies: SP968-T8 merged. Done when: crate/internal OpenAI、OpenAI-like 与 integration loopback runtime config 收敛到 3 个 test-only helper；不声明 access 字段，不改变测试断言、listener、test purpose 或运行时行为；无网络的构造测试使用公网形态 URL；PR 使用 `Refs #968`. Verify: helper consumer/source invariants、OpenAI/OpenAI-like/health/legacy focused tests、全量 tests、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T9Q` Covers: B-007, B-011, B-012. Owner: Gemini fallback fixture preparation owner. Dependencies: SP968-T9P merged. Done when: `gemini_router_fallback_routes.rs` 的最后一处 loopback `ProviderConfig` 改用既有 `tests/common/providers.rs` helper，不声明 access 字段且测试行为与断言不变；PR 使用 `Refs #968`. Verify: focused Gemini fallback tests、全量 tests、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T9S` Covers: B-007, B-011, B-012. Owner: Gemini SDK fixture preparation owner. Dependencies: SP968-T9Q merged. Done when: 全仓 inventory 确认的最后一处 direct runtime loopback `ProviderConfig`（`gemini_sdk_routes/support.rs`）改用既有 integration helper，不声明 access 字段且测试行为与断言不变；PR 使用 `Refs #968`. Verify: inventory invariant、focused Gemini SDK tests、全量 tests、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T9A` Covers: B-001, B-005, B-009, B-011. Owner: staged configuration contract owner. Dependencies: SP968-T9S merged. Done when: Gateway ProviderConfig/default/serde/env/builder 暴露 typed `endpoint_access`，settings alias 被拒绝；在 runtime routes 全部接线前，Gateway validation、create_provider 与 direct registry 对 private 或显式 direct access fail closed；不切换任何 runtime client、不改变现有 loopback 测试行为；PR 使用 `Refs #968`. Verify: model/env/builder/validation/factory matrix、全量 gates、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T10P` Covers: B-007, B-011. Owner: BaseConfig construction normalization owner. Dependencies: SP968-T9A merged. Done when: 全仓 12 处完整 `BaseConfig` 字面量（11 个 production 文件）改为保留非默认值并使用 `Default` 补齐；不改变当前配置值或运行时行为，为共享 policy 字段接入消除构造扩散；PR 使用 `Refs #968`. Verify: complete-literal source invariant、focused config/router/provider tests、全量 gates、scope/overlap、independent current-head review + CI + PR gate。
- [x] `SP968-T10A` Covers: B-003, B-005, B-007, B-009, B-011. Owner: shared access contract owner. Dependencies: SP968-T10P merged. Done when: ProviderConfig trait、BaseConfig、OpenAI/OpenAI-like config 与 factory JSON 显式携带 typed endpoint access；T9R 前现有 Gateway/direct fail-closed 行为保持不变；PR 使用 `Refs #968`. Verify: BaseConfig/trait/factory propagation matrix、source invariant、全量 gates与 PR gate。
- [ ] `SP968-T10B` Covers: B-003, B-004, B-005, B-007, B-008, B-009, B-011. Owner: shared HTTP provider owner. Dependencies: SP968-T10A merged. Done when: BaseHttpClient 以及 Mistral/Cohere/Bedrock live 路径接入 policy client，移除裸 client escape，redirect/proxy 约束保持；PR 使用 `Refs #968`. Verify: provider matrix、redirect/no-proxy negatives、source bypass、全量 gates与 PR gate。
- [ ] `SP968-T10C` Covers: B-003, B-004, B-005, B-007, B-008, B-009, B-011. Owner: shared-runtime extras owner. Dependencies: SP968-T10B merged. Done when: live provider macros、Custom/Amazon Nova、OpenAI multipart 与 custom health/no-redirect 路径接入 policy client；已迁移 shared 路径不存在 raw client escape、普通 fallback 或 proxy 降级；PR 使用 `Refs #968`. Verify: multipart/health/provider-macro matrix、redirect/no-proxy negatives、source bypass、全量 gates与 PR gate。
- [ ] `SP968-T11` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: native-route implementation owner. Dependencies: SP968-T10C merged. Done when: native providers和 Gemini/batches/images/moderations/fine-tuning/rerank route 全部接入统一 policy，unsafe proxy 配置失败，architecture guard 对范围内 production client bypass 为零并接入 PR/main CI，完整验收矩阵通过；最终 PR 使用 `Fixes #968`. Verify: native/route matrix、guard self-test/production scan、全量 repository gates、current-head independent security review、GitHub CI/review threads/merge state/required PR gate。
- [ ] `SP968-T9R` Covers: B-001, B-003, B-005, B-007, B-009, B-011. Owner: Gateway/shared-runtime activation owner. Dependencies: SP968-T11 merged. Done when: GlobalPoolManager 将 OpenAI/OpenAI-like ordinary/stream/health 接入 policy client；factory 把顶层 access 传播到已完整接线的 provider；3 个共享 loopback helper 显式 private opt-in，self-hosted 无 opt-in fail closed，所有已声明 route 无普通 fallback；PR 使用 `Refs #968`. Verify: config/factory matrix、ordinary/stream/health/authority isolation、source bypass、全量 gates、scope/overlap、independent current-head review + CI + PR gate。
- [ ] `SP968-T12` Covers: B-002, B-003, B-004, B-005, B-006, B-008, B-009, B-012. Owner: security verification owner. Dependencies: SP968-T9R. Done when: 保存 red/green evidence，至少覆盖 public-at-validation 到 loopback/RFC1918/metadata/IPv6-at-connect、initial metadata literal、redirect 私网目标、private loopback 正例与 metadata/redirect 负例；每个负例均有 listener 未 accept 证据，且无真实 DNS 依赖. Verify: focused rebinding integration suite 在 default、`--no-default-features` 适用组合与 `--all-features` 下通过。
- [ ] `SP968-T13` Covers: B-001, B-007, B-009, B-010, B-011. Owner: coordinator + independent reviewer. Dependencies: SP968-T2, SP968-T3, SP968-T4, SP968-T5, SP968-T6, SP968-T7, SP968-T8, SP968-T9P, SP968-T9A, SP968-T10P, SP968-T10A, SP968-T10B, SP968-T10C, SP968-T11, SP968-T9R, SP968-T12. Done when: 每段 PR 都绑定最终 head 的独立 native reviewer verdict、current-head CI、resolved review threads、clean merge state、offline PR gate 和 runtime ledger；最终段合并后 #968 关闭，远端分支删除，closure audit 无遗留 bypass. Verify: GitHub evidence adapter、`pr_gate.py --mode required`、`runtime_ledger_gate.py`，以及 merge 后 PR/issue/branch/main 查询。

## 并行拆分

- implementation slice 有严格依赖，必须按 policy -> HTTP client -> image-edit split -> image-router split -> completions split -> fixture consolidation -> config normalization -> runtime fixture preparation -> Gemini fallback fixture preparation -> Gemini SDK fixture preparation -> staged Gateway config -> BaseConfig construction normalization -> shared access contract -> shared HTTP providers -> shared-runtime extras -> native-route -> Gateway activation 串行合并，避免同一
  HTTP/config 文件并行写入和 open PR overlap。
- 每段内部可派发只读 planner/explorer/reviewer；若使用 writable worker，必须分配不重叠文件，shared verification
  由 coordinator 执行。
- #966 与 Gemini route 文件重叠，在 GH968 最终段合并前不得并行修改同一文件。

## 验证

- Product invariant set 与 tasks `Covers:` union 均精确为 B-001 至 B-012，无 orphan。
- 每段 PR 通过最新 SpecRail packet、implement route、scope/overlap 和 current-head review/CI/PR gate。
- 最终全量命令：`cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`;
  `cargo clippy --all-targets --all-features --locked -- -D warnings`;
  `cargo test --all-features --locked -- --test-threads=1`。

## Handoff Notes

- 不得用只改 trait 默认值、配置期 DNS 校验或真实 DNS 测试声称完成。
- private-network 是按 provider + exact authority 的启动配置授权，不是全局开关；metadata/link-local/reserved 永久拒绝。
- public-only 与 private-network 都禁用 proxy；private-network 不跟随 redirect。
- Policy/HTTP client/preparation/shared-runtime PR 只能 `Refs #968`；仅最终 native-route/guard PR 在所有 B-xxx 满足后使用 `Fixes #968`。
