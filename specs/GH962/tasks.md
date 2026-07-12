# Task Plan

## Linked Issue

GH-962 / #962

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP962-T1` Owner: coordinator. Dependencies: none. Done when: `specs/GH962/` 三件套与 issue、代码证据和 stable IDs 一致，并通过 SpecRail packet check. Verify: `python3 /Users/lifcc/Desktop/code/AI/tools/specrail/checks/check_workflow.py --repo /Users/lifcc/Desktop/code/AI/tools/specrail --spec-dir "$PWD/specs/GH962"`。
- [x] `SP962-T2` Owner: implementation worker. Dependencies: `SP962-T1`. Done when: mock upstream contract tests 通过 live `chat_completion` 路径捕获 OpenAI 与 OpenAI-compatible outbound JSON，并在生产修复前以缺失 `functions` / `function_call` 失败. Verify: `cargo test --test openai_legacy_function_forwarding --all-features --locked`，red 与 green 输出分别保存为 artifact。
- [x] `SP962-T3` Owner: implementation worker. Dependencies: `SP962-T2`. Done when: OpenAI transform 精确转发 typed `functions` 与 `function_call`，且 typed 值优先于同名 `extra_params`. Verify: mock upstream OpenAI case + focused transform assertions。
- [x] `SP962-T4` Owner: implementation worker. Dependencies: `SP962-T2`. Done when: OpenAI-compatible transform 满足相同契约并保留既有 serialization error mapping. Verify: mock upstream OpenAI-compatible case + focused transform assertions。
- [ ] `SP962-T5` Owner: reviewer. Dependencies: `SP962-T3`, `SP962-T4`. Done when: 独立 reviewer 对当前 head 比较 issue、三件套、diff 与测试，且无 blocking finding. Verify: reviewer evidence 记录 current head SHA 与 verdict。

## 并行拆分

- `SP962-T3` 与 `SP962-T4` 修改不同生产文件，理论上可并行；本 tranche 使用单一 implementation owner 串行完成，避免共享 integration test 文件。
- reviewer lane 只读，只在 implementation head 固定后启动。
- 禁止修改 `AGENTS.md`、`CLAUDE.md`、`.github/workflows/**`、provider capability 与 supported-parameter 列表。

## 验证

- [ ] `SP962-T6` Owner: verification owner. Dependencies: `SP962-T3`, `SP962-T4`, `SP962-T5`. Done when: focused test、format、check、strict Clippy、全量 test、scope/overlap guards 与 PR gate 都绑定最终 head 并通过. Verify: `cargo test --test openai_legacy_function_forwarding --all-features --locked`; `cargo fmt --all -- --check`; `cargo check --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked`; `bash scripts/guards/check_pr_scope.sh`; `bash scripts/guards/check_pr_overlap.sh`。

## Handoff Notes

- `pr_kind: mixed_impl`；spec 与实现放在同一 issue PR，不计为 spec-only progress。
- `completion_mode: final`；只有全部验收标准满足后，PR 才使用 `Fixes #962`。
- 本 issue 不修改 modern tools、response delta、其他 provider、capability 或 supported-parameter 声明。
- `auth_mode: auto` 提供 merge authorization，但仍需 current-head CI、独立 reviewer、review threads、clean merge state 与 PR gate 全部通过。
