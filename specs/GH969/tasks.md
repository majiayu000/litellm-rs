# Task Plan

## Linked Issue

GH-969 / #969

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP969-T1` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: coordinator. Dependencies: none. Done when: GH969 三件套存在、全部 `B-xxx` 连续且 product-to-test/task coverage 集合完整，packet validator 通过. Verify: `python3 checks/check_workflow.py --repo <specrail> --spec-dir "$PWD/specs/GH969"`; `rg -o "B-[0-9]{3}" specs/GH969/product.md | sort -u`; `rg -o "B-[0-9]{3}" specs/GH969/tasks.md | sort -u`.
- [ ] `SP969-T2` Covers: B-006, B-007. Owner: implementation owner. Dependencies: SP969-T1. Done when: `check_log_pii.sh` 使用独立零基线与 token-tree scanner 检查 log macro 中的 `session_id|session_token|sid`，scanner 不依赖分号且自测覆盖 semicolon/tail/multiline、隐式 format capture、prose/escaped-brace 与字符串/注释负例；当前三处泄漏精确红灯，两套 lint workflow 调用该 guard. Verify: `python3 scripts/guards/check_log_session_identifiers.py --self-test`；`bash scripts/guards/check_log_pii.sh` 预期非零且输出三个 known paths；`LITELLM_LOG_PII_BASELINE_MAX=999 bash scripts/guards/check_log_pii.sh` 仍预期非零；`rg -n "Log PII guard|check_log_pii" .github/workflows/ci.yml .github/workflows/ci-main-full.yml`.
- [ ] `SP969-T3` Covers: B-001, B-002, B-003, B-004, B-005. Owner: implementation owner. Dependencies: SP969-T2. Done when: 三个日志保留原 level/branch 和静态事件结果，但不读取 session 值，生产/session 协议代码没有其他变化. Verify: `git diff --unified=40 origin/main...HEAD -- src/auth/user_management.rs src/auth/oauth/middleware.rs src/auth/oauth/handlers.rs`; `bash scripts/guards/check_log_pii.sh` 报 session count 0；人工确认无 prefix/suffix/hash/length 替代。
- [ ] `SP969-T4` Covers: B-001, B-002, B-003, B-004, B-007. Owner: security review owner. Dependencies: SP969-T3. Done when: auth/server 全部 session-related log hits 都有 `path:line | value class | sink | disposition | issue`，known 三处已修复，protocol/email/path/provider/error hits 未误归为 session credential. Verify: `rg -n --glob '*.rs' '(trace|debug|info|warn|error)!' src/auth src/server | rg -i 'session|sid|logout|oauth|token'`; `python3 scripts/guards/check_log_session_identifiers.py src/` 必须无输出且成功。
- [ ] `SP969-T5` Covers: B-005, B-006, B-007. Owner: verification owner. Dependencies: SP969-T3, SP969-T4. Done when: mixed implementation diff 不超过 10 个 expected files/500 code lines，格式、编译、lint、全量测试、guards、scope 与 overlap 全部成功. Verify: `cargo fmt --all -- --check`; `cargo check --all-features --locked`; `cargo clippy --all-targets --all-features --locked -- -D warnings`; `cargo test --all-features --locked -- --test-threads=1`; `python3 scripts/guards/check_log_session_identifiers.py --self-test`; `bash scripts/guards/check_log_pii.sh`; `bash scripts/guards/check_pr_scope.sh origin/main`; `bash scripts/guards/check_pr_overlap.sh`.
- [ ] `SP969-T6` Covers: B-001, B-002, B-003, B-004, B-005, B-006, B-007. Owner: independent security reviewer + coordinator. Dependencies: SP969-T5. Done when: 非 coordinator 的 native reviewer 对最终 current head 返回 0 findings，PR current-head CI、review threads、merge state、offline PR gate、runtime gate 与 maintainer authorization 全部通过后才 merge，且 #969 远端关闭. Verify: 保存 reviewer lane ID/head/verdict；运行 current-head GitHub evidence adapter、`pr_gate.py --mode required`、`runtime_ledger_gate.py`; merge 后查询 PR/issue/branch/main。

## 并行拆分

- Spec、guard、production logs 与 workflow wiring 存在依赖，单一 implementation owner 串行写入，避免 red baseline 被提前消除。
- planner/reviewer 只读；最终 reviewer 必须绑定最后一次提交后的 current head，不能复用 planning evidence。

## 验证

- Product invariant set 与 tasks `Covers:` union 均精确为 `B-001` 至 `B-007`，无 orphan。
- Guard 先在未修复 production logs 上红灯，再在同一规则下转绿；不得用 baseline override 掩盖 session hits。
- PR 使用 `Fixes #969`，只有在 B-001 至 B-007 全部满足后才关闭 issue。

## Handoff Notes

- 本 issue 与 GH957 完全分离；不得修改 `AuthMethod` formatter。
- OAuth redirect/response 中的 session fields 属于协议输出，不是日志泄漏，不在本 PR 删除。
- `check_log_pii.sh` 的 raw-body 与 session-identifier baseline 必须独立，默认均为 0。
