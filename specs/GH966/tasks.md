# Task Plan

## Linked Issue

GH-966 / #966

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP966-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011. Owner: coordinator. Dependencies: none. Done when: GH966 product/tech/tasks 三件套存在，behavior invariant、implementation mapping 与 task coverage 集合完整，SpecRail workflow 与 write-spec route gate 通过；规格 PR 使用 `Refs #966` 并经独立 current-head review、CI、threads 与 PR gate 合并。 Verify: `python3 checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH966"`; 比较 product/tech/tasks 的 `B-[0-9]{3}` 集合；GitHub evidence adapter + offline PR gate。
- [ ] `SP966-T2` Covers: B-001, B-002, B-003, B-004, B-005, B-009, B-010, B-011. Owner: provider dispatch implementation owner. Dependencies: SP966-T1 merged. Done when: 现有 typed `ProviderCapability` enum 获得 additive Gemini native marker，closed `Provider` enum 提供 crate-private execute contract；native Gemini 与三个受限命名的 OpenAI-like runtime 使用各自 immutable config/client 执行，其他 provider 在 lease selection 前被排除；provider-owned non-success handling 在错误离开 runtime 前脱敏 raw/URL-encoded API key；没有 `Any`、公开无类型 payload、第二 client 或 config scan。 Verify: focused provider/client/capability 与 error redaction tests；source guard；all-target/all-feature check；strict Clippy。
- [ ] `SP966-T3` Covers: B-001, B-002, B-005, B-006, B-007, B-008, B-009, B-010. Owner: Gemini route implementation owner. Dependencies: SP966-T2 in same branch. Done when: route 只消费 selected runtime `Provider`、deployment id 与原始 requested Gemini model；adapter 只保留 selected provider + requested-model 的 pricing/spend identity；native URL/budget/spend 不使用 empty-model named compatibility 的 provider-name deployment model，unary retry、budget fallback、health 与 stream lease 绑定本次 selected deployment；`state.config().providers()`、`RouteHttpClient` 与敏感 runtime 字段从 Gemini route provider 模块删除。 Verify: focused Gemini SDK/fallback/spend/execution tests，含 `models=[]` named compatibility URL/pricing/model-budget/spend identity；source guard；scope/overlap。
- [ ] `SP966-T4` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007, B-008, B-009, B-010, B-011. Owner: integration verification owner + independent reviewer. Dependencies: SP966-T2, SP966-T3. Done when: router 构造后 Gateway config mutation 的 unary/stream 双 listener测试、named compatibility 闭集、fallback/budget/health/lease/spend 与 guard 全部通过；client cancel 明确断言 health neutral、lease 归零和已观察 spend 结算，上游 read failure 断言 health failure；raw 与 URL-encoded API key upstream echo 均在 provider 边界脱敏；实现遵守 tech spec 列出的 10 文件预算、500 changed lines与 800 行上限。若 fresh scope 预计超限，只允许先合并 byte-equivalent test decomposition `Refs #966` PR，随后最终 implementation PR 使用 `Fixes #966`；current-head CI、0 unresolved threads、independent reviewer 与 required PR gate 全绿后合并并 closure audit。 Verify: `cargo fmt --all -- --check`; `cargo check --all-targets --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `scripts/guards/check_pr_scope.sh`; `scripts/guards/check_pr_overlap.sh`; GitHub evidence adapter + `pr_gate.py --mode required`。

## 并行拆分

- T2 与 T3 同属一个 implementation PR，但 provider dispatch 必须先稳定；两者会共同影响 request/response contract，
  不分配给并行 writable agents。
- T4 可由独立只读 reviewer lane 与 coordinator 的验证并行执行；reviewer 不写 production/test 文件，只在 exact
  head 给 verdict并由 reviewer 身份解析 review threads。
- tech spec 的 10 文件清单是完整 writable ownership；不修改 799 行的 `execution.rs`。若 line budget 触发
  decomposition，prep PR 只移动既有测试且串行合并，final implementation 再从最新 main 开始。
- 任何 writable worker 必须使用独立 worktree且显式声明不重叠文件；本 issue 默认由 coordinator 串行实现，
  避免 `gemini.rs`、provider dispatch 与 integration fixture 交叉写入。

## 验证

- Product、tech mapping 与 tasks `Covers:` union 精确为 B-001 至 B-011，无 orphan 或 undeclared invariant。
- Spec PR 只包含 `specs/GH966/` 三个文档；implementation PR 从规格合并后的最新 `main` 创建。
- Implementation PR 的 fresh exact-head focused、check、strict Clippy、全量 test、scope/overlap、CI、reviewThreads
  与 offline gate 全部通过，合并后 #966 closed、远端分支删除、`main` 与 closure audit 一致。

## Handoff Notes

- selected runtime provider 是唯一执行器；禁止再从 Gateway config 复原认证、endpoint 或 client。
- `GeminiRouteProvider` 只可保留 selected provider name + original requested Gemini model 的 budget/spend identity，
  不可保存 API key、base URL、headers、timeout、client，亦不可把 named deployment model 当作请求 model。
- 兼容范围仅为显式命名 `gemini|googleai|googleaistudio` 的 OpenAI-compatible runtime；不得扩大为任意实例。
- upstream error 脱敏必须在持有 runtime key 的 provider 内完成，覆盖 raw 与 URL-encoded key；route 不得取回 key。
- 仅 implementation PR 使用 `Fixes #966`；spec PR 使用 `Refs #966`。
