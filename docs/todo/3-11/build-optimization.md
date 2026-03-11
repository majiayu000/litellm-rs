# 构建优化计划

> 日期: 2026-03-11
> 目标: 将 `cargo check` 从 ~2min 降到 <40s，减少日常开发摩擦

## 现状诊断

| 指标 | 数值 |
|------|------|
| 总代码行数 | 328,811 |
| `.rs` 文件数 | 1,097 |
| 外部依赖 crate | 607 |
| `cargo check --all-features` | **1m 57s** |
| `cargo test --all-features` | **41s** (10,274 单元 + 140 集成 + 136 doc) |
| Debug 二进制 | 95 MB |
| `target/` 磁盘占用 | 469 GB（已清理） |

### 代码分布

```
src/core/providers/   138k 行 (42%)  ← 主要膨胀来源
src/core/ (其他)      111k 行 (34%)
src/utils/             30k 行 (9%)
src/auth/              12k 行 (4%)
src/config/             9k 行 (3%)
src/server/             9k 行 (3%)
src/storage/            7k 行 (2%)
src/monitoring/         5k 行 (2%)
src/sdk/                4k 行 (1%)
```

### 膨胀根因

1. **单 crate monolith** — 328k 行全部在一个编译单元，改一行重编译整个 crate
2. **53 个空壳 provider 目录** — 已被 `catalog.rs` 数据替代，代码是历史残留
3. **默认 features 全开** — `sqlite + redis + providers-extra + providers-extended` 全部编译
4. **重型云 SDK 无隔离** — AWS/GCP/Azure SDK 各带几十个传递依赖，全量拉入

---

## 优化计划

### P0: 删除 53 个 Tier 1 provider 空壳目录

**背景**: 这 53 个 provider 是纯 OpenAI-compatible 包装，已经在 `src/core/providers/registry/catalog.rs` 中用数据条目定义。代码目录是迁移前的残留，运行时通过 `Provider::OpenAILike(OpenAILikeProvider)` 统一处理。

**待删除列表**:

```
groq, together, fireworks, perplexity, cerebras, openrouter, deepinfra,
deepseek, novita, nvidia_nim, nebius, nscale, hyperbolic, featherless,
galadriel, sambanova, heroku, friendliai, xai, vllm, vllm_hosted,
moonshot, dashscope, qwen, baichuan, minimax, volcengine, xiaomi_mimo,
zhipu, lemonade, linkup, poe, wandb, nanogpt, aiml_api, aleph_alpha,
anyscale, bytez, comet_api, compactifai, maritalk, siliconflow, yi,
lambda_ai, ovhcloud, codestral, sambanova, gigachat, empower, topaz
```

**保留列表** (10 个有自定义逻辑的 Tier 2/3 provider):

```
openai         — 基础实现，所有 compatible provider 的模板
anthropic      — 自有 API 格式 (Messages API)
azure          — Azure-specific auth + assistants + batches
azure_ai       — Azure AI Studio 独立认证流程
bedrock        — AWS SigV4 签名 + 独立模型配置
vertex_ai      — Google OAuth + 13 个子模块
gemini         — Google AI Studio 独立模型定义
mistral        — 独立 API 格式差异
cloudflare     — Workers AI 独立路由
meta_llama     — Meta 官方 API
```

**同时保留的非 LLM provider** (有独立协议，不适用 catalog):

```
base/          — 基类和 SSE 工具
registry/      — catalog 注册表
custom_api/    — 用户自定义 provider
deepgram       — 语音 (非 LLM)
deepl          — 翻译 (非 LLM)
elevenlabs     — TTS (非 LLM)
fal_ai         — 图像生成 (非 LLM)
firecrawl      — 网页抓取 (非 LLM)
jina           — 嵌入 + 搜索 (独立协议)
google_pse     — Google 搜索 (非 LLM)
exa_ai         — 搜索 (非 LLM)
cohere         — 独立 Rerank/Embed API
databricks     — 独立认证 (OAuth M2M)
github         — GitHub Models API
github_copilot — Copilot 认证流程
huggingface    — HF Inference API
ai21           — 独立 API 格式
clarifai       — 独立平台协议
baseten        — 独立部署协议
datarobot      — 独立平台协议
amazon_nova    — AWS Nova 独立配置
```

**执行步骤**:
1. 确认 53 个目录在 `mod.rs` 中仅通过 `#[cfg(feature)]` 引入
2. 删除目录 + 移除 `mod.rs` 中的引用
3. `cargo check --all-features` 验证
4. `cargo test --all-features` 验证

**预期效果**: 删除 ~60-80k 行代码，check 时间减少 30-40%

**风险**: 低。catalog 已验证可替代这些 provider，删除不影响运行时行为。

---

### P1: 默认 features 瘦身

**当前** (`Cargo.toml`):
```toml
default = ["sqlite", "redis", "metrics", "tracing", "providers-extra", "providers-extended"]
```

**改为**:
```toml
default = ["metrics", "tracing"]
```

**使用方式变更**:
```bash
# 日常开发（快速）
cargo check                    # 只编译核心 + metrics + tracing

# 完整构建（CI / 发布）
cargo check --all-features     # 全部编译

# 按需组合
cargo check --features="gateway,sqlite"           # gateway + SQLite
cargo check --features="gateway,postgres,redis"    # 生产配置
```

**执行步骤**:
1. 修改 `Cargo.toml` 的 `default` features
2. 确保 `cargo check`（无 flag）能通过编译
3. 确保 `cargo check --all-features` 仍然通过
4. 更新 CLAUDE.md 和 Makefile 中的命令说明

**预期效果**: 日常 `cargo check` 从 ~2min 降到 ~30-40s

**风险**: 中。需要检查 `cargo check`（无 features）时代码是否有未 gate 的引用导致编译失败。

---

### P2: 拆 MCP + A2A 为独立 workspace crate

**背景**: MCP (3,643 行) 和 A2A (3,434 行) 是独立协议实现，与 gateway 核心零耦合。

**目标结构**:
```
litellm-rs/
├── Cargo.toml            ← workspace root
├── crates/
│   ├── litellm-mcp/      ← Model Context Protocol gateway
│   │   ├── Cargo.toml
│   │   └── src/
│   └── litellm-a2a/      ← Agent-to-Agent protocol
│       ├── Cargo.toml
│       └── src/
└── src/                   ← 主 gateway binary
```

**执行步骤**:
1. 创建 workspace 根 `Cargo.toml`（`[workspace] members = [".", "crates/*"]`）
2. 将 `src/core/mcp/` 迁移到 `crates/litellm-mcp/src/`
3. 将 `src/core/a2a/` 迁移到 `crates/litellm-a2a/src/`
4. 主 crate 通过 `litellm-mcp = { path = "crates/litellm-mcp" }` 引入
5. 验证编译和测试

**预期效果**:
- 修改 MCP/A2A 代码不触发主 crate 重编译
- 为后续拆分 auth/storage/providers 建立 workspace 模式

**风险**: 中。需要处理 MCP/A2A 对主 crate types 的引用（可能需要提取共享类型 crate）。

---

### P3: 完整 workspace 拆分

**目标结构**:
```
litellm-rs/
├── Cargo.toml                 ← workspace root
├── crates/
│   ├── litellm-core/          ← types, error, config（公共依赖）
│   ├── litellm-providers/     ← 10 核心 provider + catalog
│   ├── litellm-auth/          ← JWT, API Key, RBAC
│   ├── litellm-router/        ← 路由策略
│   ├── litellm-storage/       ← PostgreSQL, Redis, S3
│   ├── litellm-mcp/           ← MCP gateway
│   ├── litellm-a2a/           ← A2A protocol
│   └── litellm-monitoring/    ← Prometheus, tracing
├── src/                       ← gateway binary（组装层）
└── apps/
    └── google-gateway/        ← Google 专用 gateway binary
```

**执行步骤**:
1. 先完成 P2（建立 workspace 模式）
2. 提取 `litellm-core`（types + error + config，被所有 crate 依赖）
3. 逐步迁移 auth → storage → providers → router → monitoring
4. 每一步独立验证编译和测试

**预期效果**:
- 增量编译：改一行 auth 只重编译 auth crate（几秒）
- 并行编译：独立 crate 可被 Rust 编译器并行处理
- 总 `cargo check` 预计降到 <30s

**风险**: 高。涉及大量模块间引用拆解，需要解决循环依赖。建议用 `/vibeguard:interview` 设计 spec 后再执行。

---

### P3-ext: 粒度化 provider features

```toml
provider-azure = ["dep:azure_identity", "dep:azure_core"]
provider-bedrock = ["dep:aws-sdk-bedrock", "dep:aws-config"]
provider-vertex = ["dep:google-cloud-auth"]
provider-anthropic = []
```

只开发 OpenAI + Anthropic 的场景下不编译 AWS/GCP/Azure SDK（各带 30-50 个传递依赖）。

**预期效果**: 按需编译，最小配置 `cargo check` <20s

---

## 执行优先级总览

| 优先级 | 任务 | 工作量 | 编译提速 | 风险 |
|--------|------|--------|----------|------|
| **P0** | 删 53 个空壳 provider 目录 | 1-2h | ~35% | 低 |
| **P1** | 改 default features 为最小集 | 30min | ~50%（日常） | 中 |
| **P2** | 拆 MCP + A2A 为 workspace crate | 半天 | ~10% | 中 |
| **P3** | 完整 workspace 拆分 | 1-2 天 | ~60%（增量） | 高 |
| **P3-ext** | 粒度化 provider features | 2-3h | 按需 -40% | 中 |

**建议**: P0 + P1 先做，投入 2h 获得最大收益。P2 作为 workspace 化的试点。P3 需要设计 spec。

---

## 附录: 杂项清理

- [ ] 删除 `src/core/providers/base_provider.rs.bak`（559 行历史备份）
- [ ] 移除 `catalog.rs` 中 `def_with_prefix()` 的 `#[allow(dead_code)]`（确认是否使用）
- [ ] 清理 `target/` 定期策略（建议 CI 中加 `cargo cache --autoclean`）
