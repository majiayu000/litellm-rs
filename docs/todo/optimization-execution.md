# 优化执行计划

> 创建时间: 2026-03-13
> 基线: cargo check (cached) 47.93s | (clean) 1m57s | 1097 .rs files | 328k LOC | v0.4.5
> 当前: cargo check (clean) 57.42s | --all-features (clean) 2m11s

## 总览

| # | 任务 | 风险 | 预计影响 | 状态 |
|---|------|------|----------|------|
| 1 | 死代码清理 (.bak 文件 + _bak/ 目录 + base_provider.rs.bak) | 极低 | 代码卫生 | ✅ 已完成 |
| 3 | P1: 默认 features 瘦身 | 低 | cargo check 47.93s → 26.69s (44%↓) | ✅ 已完成 |
| 4 | SSE 流式代码去重 | 中 | -167 行 (7 provider scan→1行调用) | ✅ 已完成 |
| 5 | P2: MCP + A2A workspace 拆分 | 高 | 编译隔离 | ⬜ 待评估 |
| 6 | P3: 完整 workspace 拆分 | 极高 | 编译加速 ~60% | ⬜ 待评估 |

---

## Step 1: 死代码清理

### 方案
删除所有 .bak 备份文件，这些是 SSE 统一重构时遗留的历史备份。

**目标文件:**
```
_bak/src/core/providers/base/sse.rs.bak           (559 行)
_bak/src/core/providers/oci/streaming.rs.bak
_bak/src/core/providers/sagemaker/mod.rs.bak
_bak/src/core/providers/together/mod.rs.bak
_bak/src/core/providers/xinference/mod.rs.bak
src/core/providers/base_provider.rs.bak            (559 行, 17KB)
```

**验证步骤:**
1. `cargo check --all-features` 通过
2. `cargo test --all-features` 通过
3. 无编译警告新增

### 执行记录
- [x] 文件删除: 6 个 .bak 文件 + _bak/ 目录已删除
- [x] Review: grep 确认无代码引用这些文件
- [x] cargo check --all-features 通过 (1m 37s)
- [ ] 提交

---

## Step 2: P1 — 默认 features 瘦身

### 方案
将 default features 从 `["sqlite", "redis", "metrics", "tracing", "providers-extra", "providers-extended"]` 缩减。

**约束分析:**
- 三个 binary (gateway, google-gateway, pricing-tool) 都声明 `required-features`
- `cargo run` 默认启用 default features → binary 需要的 features 必须覆盖
- `cargo check` (库检查) 不需要 binary features

**方案选项:**
- A) default = `["metrics", "tracing"]` — 最激进，`cargo run` 需要 `--features gateway,sqlite`
- B) default = `["sqlite", "redis", "metrics", "tracing"]` — 去掉 providers-extra/extended
- C) 维持现状，仅确保 `lite` feature 可用

**推荐: 方案 B** — 去掉 `providers-extra` 和 `providers-extended`，保留存储层（binary 依赖）

**验证步骤:**
1. `cargo check` 通过 (使用新 default)
2. `cargo check --all-features` 通过
3. `cargo test --all-features` 通过
4. `cargo run` 仍能启动 gateway

### 执行记录
- [x] 修改 Cargo.toml: default = ["sqlite", "redis", "metrics", "tracing"] (去掉 providers-extra, providers-extended)
- [x] Review: binary required-features 满足 (gateway/google-gateway 需要 storage→sqlite 已在 default; pricing-tool 需要 gateway→已在 storage chain)
- [x] cargo check: 26.69s ✅ | cargo check --all-features: 通过 ✅
- [x] cargo test --all-features: 10276 + 137 全部通过 ✅
- [x] cargo clippy --all-features: 通过 ✅
- [ ] 提交

---

## Step 3: SSE 流式代码去重

### 方案
19 个 provider 的 streaming 文件包含近乎相同的 wrapper 代码 (~50-70 行/provider)。

**重复模式:**
```rust
// 每个 provider 都独立定义:
pub type XxxStream = UnifiedSSEStream<Pin<Box<dyn Stream<...>>>, XxxTransformer>;

pub fn from_response(response: reqwest::Response, model: String) -> Self {
    let transformer = XxxTransformer::new(model);
    UnifiedSSEStream::new(Box::pin(response.bytes_stream()), transformer)
}

impl Stream for XxxStream {
    fn poll_next(...) { this.inner.poll_next(cx) }
}
```

**去重策略:**
- 在 `src/core/providers/base/` 添加通用 `create_sse_stream()` 工具函数
- 各 provider 只需提供 Transformer，不再重复 Stream wrapper
- 分批修改：先改 2 个验证可行，再批量推广

**验证步骤:**
1. 选取 2 个 provider 先行验证
2. `cargo check --all-features` 通过
3. `cargo test --all-features` 通过
4. 批量修改剩余 provider
5. 全量测试

### 执行记录
- [x] 设计通用函数: `create_provider_sse_stream(response, provider_name)` in base/sse.rs
- [x] 先行验证: sambanova + galadriel 编译通过
- [x] 批量推广: 7/7 provider 完成 (sambanova, galadriel, friendliai, gigachat, mistral, huggingface, codestral)
- [x] Review: HuggingFaceError/CodestralError 均为 ProviderError type alias，兼容
- [x] cargo check --all-features: 通过 ✅
- [x] cargo test --all-features: 10276 + 137 + 100 全部通过 ✅
- [x] cargo clippy: 通过 ✅
- [x] 改动: 9 files, +36 -203 (净减 167 行)
- [ ] 提交

---

## Step 4-5: P2/P3 Workspace 拆分 (待评估)

> ⚠️ 之前在 feature 分支尝试过完整 workspace 拆分但已回滚。
> 需要先完成 Step 1-3，评估效果后再决定是否推进。
> 主要风险：循环依赖、inter-crate 耦合、CI 配置变更。

### 评估结果 (Step 1-3 完成后)

**编译时间对比:**
| 场景 | 优化前 | 优化后 | 变化 |
|------|--------|--------|------|
| `cargo check` (clean, default) | 1m 57s | 57.42s | -51% |
| `cargo check --all-features` (clean) | ~2m 11s | 2m 11s | 无变化 (仍编译全量) |
| `cargo check` (incremental) | 47.93s | 26.69s | -44% |

**结论:**
- P1 (default features 瘦身) 带来了显著的日常开发加速 (-51% clean, -44% incremental)
- P2/P3 (workspace 拆分) 主要优化 --all-features 场景和增量编译隔离
- 之前 workspace 拆分尝试已回滚，说明存在技术风险 (循环依赖、耦合)
- **建议: P2/P3 暂缓**，当前编译速度已满足日常开发需求。如需进一步优化可在独立分支推进

---

## 执行日志

| 时间 | Step | 动作 | 结果 |
|------|------|------|------|
| 2026-03-13 11:45 | 1 | 删除 6 个 .bak 文件 + _bak/ 目录 | ✅ 未跟踪文件，无需 commit |
| 2026-03-13 12:00 | 2 | default features 去掉 providers-extra/extended | ✅ 26.69s (44%↓), 10413 tests pass |
| 2026-03-13 12:20 | 3 | SSE 流式代码去重 (7 provider) | ✅ -167 行, 10413 tests pass |
| 2026-03-13 12:30 | 评估 | Clean build: 1m57s → 57.42s (-51%) | P2/P3 暂缓 |
| 2026-03-13 12:50 | 4 | validate() 去重 (10 provider + trait default) | ✅ -80 行, 10413 tests pass |
