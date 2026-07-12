# Task Plan

## Linked Issue

GH-964 / #964

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [x] `SP964-T1` Owner: coordinator. Dependencies: none. Done when: GH964 product/tech/tasks packet 通过 SpecRail 校验，duplicate-work 与 implement route gate 为 allowed。Verify: packet check and route gate JSON。
- [x] `SP964-T2` Owner: config-contract. Dependencies: T1. Done when: partial expected codes 默认一致，endpoint 使用 URL API 解析并通过 SSRF/跨字段/status code 校验，direct factory 不能绕过 Validate。Verify: focused provider config and validation tests。
- [x] `SP964-T3` Owner: runtime-mapping. Dependencies: T2. Done when: `DeploymentConfig` 可选保存 normalized health policy，同 provider 的所有 model deployment 获得同一 policy，程序化默认保持 None。Verify: focused gateway mapping/deployment tests。
- [x] `SP964-T4` Owner: probe-engine. Dependencies: T3. Done when: 每 provider 一个 live task 执行 native 或 custom endpoint probe，interval/threshold/recovery/expected codes 均被消费，global disabled 不 spawn，Drop 取消 task。Verify: focused health-probe tests。
- [x] `SP964-T5` Owner: state-safety. Dependencies: T4. Done when: probe success/failure驱动 Healthy/Degraded/Unhealthy，active Cooldown 和 probe Unhealthy 不被并发请求路径错误覆盖。Verify: transition and existing cooldown/router suites。
- [x] `SP964-T6` Owner: verification. Dependencies: T2-T5. Done when: focused、fmt、check、strict clippy、串行全量 tests 和 packet validation 全部通过。Verify: commands below。
- [ ] `SP964-T7` Owner: coordinator. Dependencies: T6. Done when: one implementation PR 使用 `Closes #964`，current-head CI、独立 reviewer、review threads、PR gate 与 runtime gate 全部通过并远端确认合并。Verify: SpecRail PR evidence and closure audit。

## 并行拆分

T2-T5 共享 config -> policy -> task -> state contract，按 W-14 串行实现。只读 reviewer 在 focused
verification 后启动；共享全量验证由 coordinator 独占。

## 验证

- `cargo fmt --all -- --check`
- `cargo check --all-features --locked`
- `cargo clippy --all-targets --all-features --locked -- -D warnings`
- focused provider config/validation tests
- focused `core::router::health_probe` and cooldown tests
- `cargo test --all-features --locked -- --test-threads=1`
- `python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH964"`

## Handoff Notes

- custom endpoint 是经过 SSRF 校验的无认证 GET；需要 provider 认证时使用 native path。
- 不接入使用随机模拟的 `core::health::checker::perform_health_check`。
- one task per provider config name，结果应用到该 provider 的全部 model deployment。
- reviewer 必须检查 task 生命周期、endpoint/expected code、threshold/recovery 和 Cooldown race。
