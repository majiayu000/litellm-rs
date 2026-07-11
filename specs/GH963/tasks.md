# Task Plan

## Linked Issue

GH-963 / #963

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP963-T1` Owner: coordinator. Dependencies: none. Done when: GH963 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work 与 implement route gate 为 allowed。Verify: `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH963"`; `python3 "$SPEC_RAIL/checks/route_gate.py" --repo "$SPEC_RAIL" --route implement --issue 963 --state ready_to_implement --duplicate-evidence "$EVIDENCE" --json`。
- [x] `SP963-T2` Owner: runtime-config. Dependencies: T1. Done when: `DeploymentConfig` 可选保存 normalized retry schedule，gateway provider mapping 保留 `base_delay`、`max_delay`、`backoff_multiplier`、`jitter`，程序化 deployment 的默认值保持 global fallback。Verify: focused `gateway_config` 与 `deployment` unit tests。
- [x] `SP963-T3` Owner: retry-policy. Dependencies: T2. Done when: 唯一 `RetryPolicy` 支持 selected-deployment schedule，公式、jitter、cap、retry-after precedence 与 global fallback 都有确定性测试。Verify: `cargo test core::router::retry_policy --lib --all-features --locked`; `cargo test core::router::execution --lib --all-features --locked`。
- [x] `SP963-T4` Owner: live-callers. Dependencies: T3. Done when: core router unary/capability 与 server unary/streaming-pre-output 的 selected-deployment failure 调用新 policy；selection error 仍走 global policy。Verify: source scan plus focused router/server execution tests。
- [x] `SP963-T5` Owner: verification. Dependencies: T2-T4. Done when: mapping、250/500/900 backoff、jitter bounds、global fallback、hint precedence 和现有安全停止条件均通过。Verify: focused tests and fresh full checks。
- [ ] `SP963-T6` Owner: coordinator. Dependencies: T5. Done when: one mixed implementation PR 使用 `Closes #963`，current-head CI、独立 reviewer、review threads 和 PR gate 全部通过。Verify: SpecRail PR evidence and gate artifacts。

## 并行拆分

T2-T4 共享 `DeploymentConfig`、`RetryPolicy` 与 caller contract，按 W-14 串行执行，避免跨 lane 修改同一 API。
只读 reviewer 在实现和 focused verification 完成后启动；共享全量验证由 coordinator 独占。

## 验证

- `cargo fmt --all -- --check`
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cargo test core::router --lib --all-features --locked`
- `cargo test server::routes::ai::execution --lib --all-features --locked`
- `cargo test --all-features --locked -- --test-threads=1`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH963"`

## Handoff Notes

- `ProviderConfig.max_retries` 与 provider client retry 不在本 issue 内重构。
- Provider factory 不拥有 router schedule；消费边在 `gateway_config` → `DeploymentConfig`。
- `RetrySchedule::None` 是程序化 deployment 的兼容路径，不得默认为 provider YAML schedule。
- reviewer 必须重点检查所有 selected-deployment call site、retry-after precedence、jitter hard cap 和 streaming 已输出后的禁止 retry。
