# Vibe Coding 事后分析：PR 混乱根因与预防方案

> 本文档基于 litellm-rs 18 个 PR 的 review/合并过程，分析 AI 自主编码产生的问题模式，并提出防护方案。

## 1. 本次发现的问题

### 1.1 PR 分支爆炸（7 个安全 PR 共享同一基础提交）

**现象**：PR #26-32 全部包含 commit `6f6de03`（安全审计修复），各自独立创建分支但共享 80%+ 的代码变更。

**根因**：多个 AI agent 从同一 commit 并行 fork 分支，各自独立完成修复后推送。缺少集中协调机制，导致：
- 7 个 PR 修改相同的 14 个文件
- 共享提交出现在多个分支中（`ca6121e` 出现在 #30 和 #31，`9930048` 出现在 #29 和 #32）
- 合并时需要解决大量冲突

**影响**：
- 7 个 PR 最终合并为 1 个（#33），其余全部关闭
- 手动解决 3 轮合并冲突
- 浪费 CI 资源（7 × 8 checks = 56 次 CI 运行）

### 1.2 CI 串行瓶颈

**现象**：每次合并一个 PR 后，所有其他 PR 都变为 BEHIND/BLOCKED 状态，需要更新分支并等待 CI 重新运行。

**根因**：仓库启用了 "Require branches to be up to date before merging" 规则。合并一个 PR → 更新其余所有分支 → 等待 CI → 合并下一个 → 重复。18 个 PR 的合并耗时超过 1 小时。

### 1.3 Clippy 版本漂移

**现象**：所有安全 PR 都因同一个 clippy lint 失败（`manual_pattern_char_comparison`），但本地开发时通过。

**根因**：CI 使用的 Rust 工具链 (1.94) 比开发环境新，引入了新的 clippy lint。AI agent 在旧版本上验证通过就推送了。

### 1.4 提交原子性不足

**现象**：PR #29 同时包含 "public route bypass fix" 和 "router concurrency fix"（commit `9930048`）。PR #30 同时包含 "OAuth redirect fix" 和 "env mutation fix"（commit `ca6121e`）。

**根因**：AI agent 在一次工作流中修复了多个问题，但没有按问题粒度拆分提交和分支。

### 1.5 PR #13 直接冲突废弃

**现象**：PR #13（Router error hardening）与 main 有不可调和的冲突，CI 也 fail，最终直接关闭。

**根因**：长期未合并的 PR 被后续改动覆盖，变成死分支。

## 2. 问题分类与行业数据

### Vibe Coding 的定义

由 Andrej Karpathy（OpenAI 联合创始人）在 2025 年 2 月提出：用自然语言描述需求，让 LLM 生成代码，不仔细审查就接受。Collins 英语词典将其评为 2025 年度词汇。

Simon Willison 做了重要区分：**不是所有 AI 辅助编程都是 vibe coding**。关键差异在于是否审查生成的代码。

### 行业数据

| 指标 | 数据 | 来源 |
|------|------|------|
| AI 代码引入安全缺陷 | 45% | Veracode 2025 |
| AI PR 被拒绝率 | 67.3%（人类 PR 为 15.6%）| LinearB |
| AI 代码错误配置率 | 比人类高 75% | SANS 2025 |
| AI 代码漏洞密度 | 比人类高 2.74 倍 | 多项研究 |
| 高级工程师使用 AI 后速度 | 实际慢 19%（自认为快 20%）| METR 2025 |

### 真实事故

- **Lovable**：1,645 个应用中 170 个存在严重漏洞
- **Tea App**：暴露 72,000 张图片，含 13,000 个政府 ID
- **Replit Agent**：删除了生产数据库
- **供应链攻击**：LLM 幻觉不存在的包名，攻击者注册这些名字分发恶意软件

## 3. 防护方案

### 3.1 分支策略：Trunk-Based Development

```
main ← 唯一主干
  ├── fix/issue-123     # 生命周期 < 1 天
  ├── feat/feature-456  # 生命周期 < 3 天
  └── (禁止从 feature 分支 fork 子分支)
```

**规则**：
- 每个 AI agent 只创建一个分支，解决一个问题
- 分支生命周期不超过 24 小时
- 禁止从非 main 分支创建分支
- 合并前必须 rebase 到最新 main

### 3.2 Agent 隔离：Git Worktree

```bash
# 每个 agent 任务使用独立 worktree
git worktree add /tmp/agent-task-{id} -b fix/issue-{id} main

# Agent 在 worktree 中工作，完成后
cd /tmp/agent-task-{id}
git push origin fix/issue-{id}
git worktree remove /tmp/agent-task-{id}
```

**优势**：并行 agent 不会相互干扰（不共享工作目录和 checkout 状态）。

### 3.3 提交原子性强制

```yaml
# .github/pr-rules.yml
rules:
  max_files_changed: 10        # 超过 10 文件要求拆分
  max_lines_changed: 500       # 超过 500 行要求拆分
  single_concern: true          # 一个 PR 只解决一个问题
  no_shared_commits: true       # 禁止多个 PR 共享相同的 commit
```

**实践**：
- 一个 issue → 一个 branch → 一个 PR
- PR title 必须引用 issue 编号
- CI 检查 PR diff 中的文件数和行数，超阈值自动标记 needs-split

### 3.4 多层守卫（Guardrails Stack）

```
Layer 1: Policy checks (安全规则、架构边界)
    ↓
Layer 2: Test proof (强制覆盖率阈值)
    ↓
Layer 3: Diff heuristics (文件数、影响范围、blast radius)
    ↓
Layer 4: AI review gate (第二个 AI 审查第一个 AI 的输出)
    ↓
Layer 5: Human escalation (风险分级，高风险强制人工审查)
```

**风险分级**：
| 级别 | 类别 | 审查要求 |
|------|------|----------|
| Low | docs, formatting, comments | 自动化 gate 通过即可 |
| Medium | 新功能、测试、非核心重构 | AI review + 1 人工 |
| High | auth, payments, infra, security | 强制 senior 人工审查 |

### 3.5 CI 优化：合并队列

GitHub Merge Queue 可以解决串行瓶颈：
```yaml
# Repository Settings → Rules → Merge queue
merge_queue:
  enabled: true
  merge_method: squash
  max_entries_to_build: 5    # 并行测试 5 个 PR
  min_entries_to_merge: 1
```

合并队列会自动将多个 PR 的变更合并测试，通过后批量合并，避免逐个更新→等待→合并的循环。

### 3.6 版本锁定：Rust Toolchain

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.87.0"    # 固定版本
components = ["rustfmt", "clippy"]
```

确保 CI 和本地使用完全相同的工具链版本，消除 clippy lint 版本漂移。

### 3.7 Agent 编排模式

```
                    ┌─────────────┐
                    │ Orchestrator │  (Tech Lead 角色)
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
        ┌─────┴─────┐ ┌───┴───┐ ┌─────┴─────┐
        │ Agent A    │ │Agent B│ │ Agent C    │
        │ (worktree) │ │(wt)  │ │ (worktree) │
        └────────────┘ └───────┘ └────────────┘
```

**规则**：
1. Orchestrator 分配任务前检查文件所有权冲突
2. 两个 agent 不得修改同一文件
3. Agent 完成后 orchestrator 负责合并顺序
4. Agent 只推送到自己创建的分支

## 4. 本项目具体行动项

### 立即执行

- [x] 启用 `rust-toolchain.toml` 锁定 Rust 版本
- [ ] 启用 GitHub Merge Queue
- [ ] 添加 PR 大小检查 CI（超 10 文件 / 500 行标记 needs-split）

### 短期（1-2 周）

- [ ] 实现 agent worktree 隔离（在 Harness task_runner.rs 中添加 `git worktree add` 逻辑）
- [ ] 添加文件所有权冲突检测（orchestrator 分配任务前扫描目标文件）
- [ ] CI 添加 "AI-generated" 标签自动检测和额外审查要求

### 长期

- [ ] 建立风险分级审查流程（Low/Medium/High）
- [ ] 实现 AI review gate（第二个模型审查第一个模型的 PR）
- [ ] 监控指标：AI PR 合并率、冲突率、CI 浪费率

## 5. 核心结论

> Vibe coding 的核心风险不在于 AI 生成了错误的代码——而在于人类不审查就接受。

本次 PR 清理揭示了三个关键教训：

1. **并行 ≠ 高效**：7 个并行 agent 创建 7 个重叠 PR，最终合并为 1 个。串行但有协调的工作流更高效。
2. **CI 是瓶颈**：代码生成速度提升后，测试、审查、部署成为新的瓶颈。必须在扩大 AI 输出前强化 CI/CD 基础设施。
3. **Agent 需要边界**：不设边界的自主 agent 就像不设 scope 的开发者——产出很多但方向分散。bounded autonomy（有界自主权）是唯一可行路径。

---

*References: Wikipedia (Vibe coding), Veracode 2025, LinearB, METR 2025, Anthropic Agentic Coding Trends 2026, AWS Blog, Evil Martians, Kaspersky Security*
