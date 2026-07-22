# Task Plan

## Linked Issue

GH-838 / #838

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP838-T1` Owner: coordinator. Done when: `specs/GH838/` 三件套通过 SpecRail packet validation. Verify: `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH838"`.
- [ ] `SP838-T2` Owner: coordinator. Done when: 可达性证据表（每子系统的 `rg` 命中数、依赖、测试规模、churn）作为附录追加到本 spec，范围包含 audit logging 与 webhooks. Verify: `git diff -- specs/GH838/`; 每子系统一行且附判定命令。
- [x] `SP838-T3` Owner: maintainer. Done when: 维护者在 #838 批复处置矩阵（wire/remove/experimental-gate），特别是 mcp/a2a/realtime 的产品意向与 guardrails 默认开关（SpecRail human gate `spec_approval`）. Verify: [#838 comment 4982856136](https://github.com/majiayu000/litellm-rs/issues/838#issuecomment-4982856136) 明确批复处置矩阵、`remove` 行的 0.6.0 deprecation → 0.7.0 removal 窗口及 release-workflow 前置门禁；`experimental-gate` 行保持 default-off，删除须另有 spec 批准。
- [ ] `SP838-T4` Owner: coordinator. Done when: 守护检查合入 CI——`src/core/mod.rs` 顶层 `pub mod` 先分类为 gateway-facing / library-only / internal-support / feature-gated；gateway-facing 模块必须被启动装配实际构造并接入请求路径/中间件/路由/后台任务，或在豁免清单，或被真 default-off feature gate；default-on support feature（如 storage/sqlite）不算 experimental gate，config/admin/status 文本引用不算可达. Verify: 检查脚本/测试绿色；人为添加 config-only/admin-only 未接线 gateway 模块的负测试验证后移除；人为把 gateway-facing 模块只挂到 default-on support feature 的负测试会失败；`completion`、`function_calling`、`traits`、`secret_managers` 作为 library-only 正测试不被误拦。
- [ ] `SP838-T5` Owner: coordinator. Done when: wire lane 执行完毕——每子系统一个 PR，含 `GatewayConfig` 字段、启动初始化。Verify: 见下列命令与行为证据。
      中间件/路由挂载、smoke 测试（U-26 三要件）；observability+integrations 还必须在真实 LLM request 生命周期触发
      `on_llm_start` 与 `on_llm_end`/`on_llm_error`；audit logging 必须保留 `enterprise.audit_logging` 作为 runtime
      enablement knob，证明 true 时真实执行 `AuditLogger`/`AuditMiddleware`、false 时默认不执行。Verify: 每 PR
      `cargo test --all-features` + 冒烟请求记录；observability PR 用 test integration 或 Langfuse/OTel 测试替身证明事件分发，
      不以 `/metrics` 单独作为通过证据；audit PR 用开关两种状态的请求路径测试证明 knob 已接线，不得用 gate/remove
      no-op-knob 扫描作为验收。
- [ ] `SP838-T6` Owner: coordinator. Done when: gate lane 执行完毕——被 gate 模块默认不编译，README、`docs/README.md`、相关当前能力 docs。Verify: 见下列命令与行为证据。
      （只扫现存 tracked docs，不追历史/归档计划）全文标注 experimental 或删除可用性示例，docs.rs feature 列表同步；对应
      config schema/env/example 要么 gated，要么对禁用 feature 返回显式 validation error；若影响 `litellm_rs::core::<module>`
      import，完成 semver、CHANGELOG、deprecation/迁移说明；`experimental-gate` 行在 0.7.0 后仍保持 default-off，除非独立
      spec 批准删除。`user_management` gate 前必须迁移/重构 storage/SeaORM 对 legacy `User`/`Team`/`Organization` 的无条件依赖，
      保留 legacy/canonical 同步语义，不得因 gate 默认关闭而破坏 SQLite/storage build。Verify: `cargo check`（默认 gateway 用户路径）、
      `cargo check --no-default-features --features "sqlite,metrics,tracing"`、`cargo check --no-default-features --features "metrics,tracing"` 与
      `cargo check --all-features` 均通过；对相关 SeaORM repository 运行 legacy/canonical 转换与同步回归；
      `rg -n "crate::core::user_management|core::user_management" src/storage` 不存在对关闭 feature 的无条件引用；
      `git ls-files README.md CLAUDE.md 'docs/**/*.md' | xargs rg -n "MCP Gateway|A2A Protocol|A2A Gateway|A2AGateway|Model Context Protocol|litellm_rs::core::(mcp|a2a)"`
      输出与 gate 处置一致且允许相关 protocol docs 被删除；`git ls-files config src/config docs/README.md 'docs/protocols/**/*.md' | xargs rg -n "mcp|a2a|realtime|webhooks|user_management"`
      仅检查 gate 行的 config/schema/examples 与当前能力 docs，历史 docs/plan/specs 不作为阻塞且不会因已删除 protocol 文件失败。
- [ ] `SP838-T7v` Owner: release-workflow owner. Dependencies: T3. Done when: 修改 `.github/workflows/version-bump.yml`，使 breaking 版本策略正确产出 `0.5.0 → 0.6.0`、`0.6.0 → 0.7.0`、`1.2.3 → 2.0.0`，且能从 subject/body/footer 检测 `feat!:`、`fix!:`、`refactor!:` 与 `BREAKING CHANGE:`；新增 deterministic runner `checks/version_bump_policy.py` 与 fixtures `checks/fixtures/version_bump_cases.json`. Verify: `python3 checks/version_bump_policy.py --workflow .github/workflows/version-bump.yml --fixtures checks/fixtures/version_bump_cases.json`；fixture 逐项断言三组版本迁移与四类 breaking marker detection；`git diff --check`。
- [ ] `SP838-T7a` Owner: batch compatibility owner. Dependencies: T3. Done when: 0.6.x partial tranche 保留公开 `BatchProcessor` 的签名与既有行为并标记 deprecated，补齐 CHANGELOG 与迁移说明；`/v1/batches` 继续保持现有 provider proxy 语义；不得删除或改变 `AsyncBatchExecutor`、共享 batch 类型、database schema/history. Verify: public compile/behavior compatibility fixture；provider proxy fixture；`cargo check --all-features`; `cargo test --all-features`; `git diff --check`。
- [ ] `SP838-T7c` Owner: remove-compatibility owner. Dependencies: T3. Done when: 0.6.x compatibility/deprecation tranche 为 `semantic_cache` 与 `analytics` public modules 添加 deprecation，补齐 CHANGELOG 与迁移说明；保留 `litellm_rs::core::{semantic_cache,analytics}` public imports，保留 `cache.semantic_cache` / `enterprise.advanced_analytics` 现有 config parse/rejection/runtime 行为，且保留 `analytics`、`enterprise`、`full`、docs.rs 现有 Cargo feature surface，不在 0.6.x 提前删除。Verify: 扩展 `tests/public_api_compat.rs` 覆盖两个 public import；config compatibility fixtures；`cargo test --test public_api_compat`；`cargo check --no-default-features --features analytics`；`cargo check --features enterprise`；`cargo check --features full`；deprecation/CHANGELOG/migration diff；`git diff --check`。
- [ ] `SP838-T7b` Owner: batch removal owner. Dependencies: T7a, T7v；0.6 deprecation 已有可验证 release artifact. Done when: 0.7.0 breaking tranche 仅删除公开 `BatchProcessor` 及其专属实现，清理对应 `core/mod.rs`/文档/CHANGELOG/migration 引用，同时保留 `/v1/batches` provider proxy、`AsyncBatchExecutor`、共享 batch 类型、database schema/history；任何扩大 removal scope 的变化必须先有独立 spec 批准. Verify: `SP838-T7v` fixture 证据与含 `BatchProcessor` deprecation 的已验证 0.6 release artifact；`cargo check --all-features`; `cargo test --all-features`; proxy 回归；public removal compile fixture；`git diff --check`。
- [ ] `SP838-T7` Owner: coordinator. Dependencies: T7v, T7a, T7b, T7c；所有 remove 行的 deprecation 已有可验证 0.6 release artifact. Done when: 批准的完整 remove lane（仅矩阵中 `remove` 行）均已执行。Verify: 见下列命令与行为证据。
      `core/mod.rs`、config schema/env/example、README、CLAUDE.md、`docs/README.md`、相关当前能力 docs（只扫现存 tracked docs）
      同步清理；所有 public removal 均满足各自 semver、CHANGELOG、deprecation/迁移与 release gate；analytics removal 同时删除
      `Cargo.toml` 的 `analytics` feature 定义、`enterprise`/`full` 成员资格和 `package.metadata.docs.rs.features` 公开面，并提供
      停用 `--features analytics` 的迁移说明。Verify: `cargo check --all-features`；`rg -n '^analytics\s*=|analytics' Cargo.toml` 不再命中
      feature 定义、bundle 成员或 docs.rs feature 列表；
      `git ls-files config src/config README.md CLAUDE.md docs/README.md 'docs/protocols/**/*.md' | xargs rg -n "semantic_cache|advanced_analytics|core::analytics|--features analytics"`
      仅检查 remove 行的 config/schema/examples、当前能力 docs 与迁移说明，历史 docs/plan/specs 不作为阻塞；public import/feature 破坏有发布记录。
- [ ] `SP838-T8` Owner: verification owner. Done when: 全量回归通过，README/CLAUDE.md/`docs/` 能力表与实际可达能力一致，public API 变更记录完整. Verify: `cargo fmt --all -- --check`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --all-features`.

## 并行拆分

- SP838-T2（纯文档）与 SP838-T4（守护检查，只动 CI/测试文件）可并行。
- SP838-T5 各子系统 PR 文件不相交可并行（W-14），但 observability+integrations 因依赖必须同组。
- SP838-T6/T7 依赖 T3 批复；T5 中安全语义子系统（guardrails/ip_access）优先。

## 验证

- [ ] `SP838-T9` Owner: verification owner. Done when: 被 wire 的每个子系统有一条本会话可复现的端到端证据记录在对应 PR body；observability+integrations 必须证明 request lifecycle event dispatch（`on_llm_start` 与 `on_llm_end`/`on_llm_error`），不是只有 `/metrics` 输出. Verify: PR body 中的命令输出（W-16：本会话证据）。

## Handoff Notes

- 与 #837 的边界：本 issue 只处理 core 子系统层；provider 目录归 #837。两者的守护检查可共享豁免清单机制但分开断言。
- batch 决策已由 [#838 comment 4982856136](https://github.com/majiayu000/litellm-rs/issues/838#issuecomment-4982856136) 批准：0.6.x 保留公开 `BatchProcessor` 签名与行为并 deprecated，`/v1/batches` provider proxy 保持不变；仅在 version-workflow breaking-release gate 与已验证 0.6 release 均满足后，0.7.0 才删除 `BatchProcessor`。`AsyncBatchExecutor`、共享 batch 类型、database schema/history 均不在该 removal scope，除非独立 spec 另行批准。
- guardrails 若 wire，注意其 `check_output` 每次调用重新编译正则（`src/core/guardrails/prompt_injection.rs:294-303`），接线前先改为预编译（`LazyLock`），否则把性能问题带上热路径。
- `virtual_keys` 不是 stub-only：已有迁移、manager 与 SeaORM CRUD，后续处置应围绕 gateway/API 接线或 public API gate，而不是按空壳删除。
- `webhooks` 不是 stub-only：已有 delivery processor / signing / outbound POST，处置应围绕 gateway event path 接线或 gate，而不是因未挂载直接删除。
- `audit` / `enterprise.audit_logging` 属于 wire lane；配置示例存在但 runtime 不执行时属于 U-26 缺口。
  验收应证明 knob 真实接线，不得把它归入 gate/remove 的 no-op-knob 清理扫描。
- `user_management` gate 开始前先处理 SeaORM 对 legacy domain types 的无条件依赖，并保留默认 SQLite/storage
  build 与 legacy/canonical 数据同步。
- `analytics` removal 必须将 Cargo feature 定义、`enterprise`/`full` bundle 和 docs.rs feature 发布面一并清理；
  否则删除模块后会留下可启用的空 feature。
- remove/gate lane 可能破坏 `src/lib.rs` 暴露的 `pub mod core` import；合入前必须保留 human gate，确认 semver/CHANGELOG/deprecation/迁移说明。
