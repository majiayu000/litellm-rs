# Task Plan

## Linked Issue

GH-968 / #968

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP968-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: coordinator. Dependencies: none. Done when: GH968 三件套存在，product invariant、tech mapping 与 task coverage 集合完整，latest SpecRail packet/route gate 通过，并记录三段 serial PR 计划. Verify: `python3 checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH968"`; 比较 product/tasks 的 `B-[0-9]{3}` 集合。
- [x] `SP968-T2` Covers: B-001, B-002, B-005, B-006, B-009. Owner: policy implementation owner. Dependencies: SP968-T1. Done when: policy PR 提供 endpoint access/policy、scheme/host/effective-port 私网绑定、统一 URL/IP 校验、globally-routable-only IPv6 分类和永久 metadata 拒绝；不新增尚未接线的 Gateway config 或 HTTP client；PR 使用 `Refs #968`. Verify: focused `core::net` tests；`cargo check --all-targets --all-features --locked`; scope/overlap；independent current-head review + CI + required PR gate。
- [ ] `SP968-T3` Covers: B-002, B-003, B-004, B-005, B-006, B-008, B-009, B-012. Owner: HTTP client implementation owner. Dependencies: SP968-T2 merged. Done when: HTTP client PR 提供不泄漏裸 `reqwest::Client` 且支持 headers/auth/query/body/json/form/multipart 的 request builder、普通/streaming/no-redirect policy client 与可注入 resolver；deterministic rebinding/literal/redirect/private tests 证明负例未建立 socket；PR 使用 `Refs #968`. Verify: focused `utils::net::http` tests、all-target check、strict clippy、scope/overlap、independent current-head review + CI + PR gate。
- [ ] `SP968-T4` Covers: B-001, B-003, B-005, B-007, B-009, B-011. Owner: shared-provider implementation owner. Dependencies: SP968-T3 merged. Done when: shared-provider PR 将 Gateway config/default/env/validation、trait/BaseConfig/factory/GlobalPoolManager/BaseHttpClient/provider macros/OpenAI/OpenAI-like 普通、流式与 health 路径接入 policy client，self-hosted 无 opt-in fail closed，且没有普通 fallback；PR 使用 `Refs #968`. Verify: config/env/default 与 factory propagation matrix、shared provider ordinary/stream/health tests、all-feature check/test、scope/overlap、independent current-head review + CI + PR gate。
- [ ] `SP968-T5` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011, B-012. Owner: native-route implementation owner. Dependencies: SP968-T4 merged. Done when: native providers和 Gemini/batches/images/moderations/fine-tuning/rerank route 全部接入统一 policy，unsafe proxy 配置失败，architecture guard 对范围内 production client bypass 为零并接入 PR/main CI，完整验收矩阵通过；最终 PR 使用 `Fixes #968`. Verify: native/route matrix、guard self-test/production scan、全量 repository gates、current-head independent security review、GitHub CI/review threads/merge state/required PR gate。
- [ ] `SP968-T6` Covers: B-002, B-003, B-004, B-005, B-006, B-008, B-009, B-012. Owner: security verification owner. Dependencies: SP968-T5. Done when: 保存 red/green evidence，至少覆盖 public-at-validation 到 loopback/RFC1918/metadata/IPv6-at-connect、initial metadata literal、redirect 私网目标、private loopback 正例与 metadata/redirect 负例；每个负例均有 listener 未 accept 证据，且无真实 DNS 依赖. Verify: focused rebinding integration suite 在 default、`--no-default-features` 适用组合与 `--all-features` 下通过。
- [ ] `SP968-T7` Covers: B-001, B-007, B-009, B-010, B-011. Owner: coordinator + independent reviewer. Dependencies: SP968-T2, SP968-T3, SP968-T4, SP968-T5, SP968-T6. Done when: 每段 PR 都绑定最终 head 的独立 native reviewer verdict、current-head CI、resolved review threads、clean merge state、offline PR gate 和 runtime ledger；最终段合并后 #968 关闭，远端分支删除，closure audit 无遗留 bypass. Verify: GitHub evidence adapter、`pr_gate.py --mode required`、`runtime_ledger_gate.py`，以及 merge 后 PR/issue/branch/main 查询。

## 并行拆分

- 四个 implementation slice 有严格依赖，必须按 policy -> HTTP client -> shared-provider -> native-route 串行合并，避免同一
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
- Policy/HTTP client/shared-provider PR 只能 `Refs #968`；仅最终 native-route/guard PR 在所有 B-xxx 满足后使用 `Fixes #968`。
