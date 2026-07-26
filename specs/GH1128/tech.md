# Tech Spec

## Linked Issue

GH-1128 / #1128

## Product Spec

见 `product.md`（B-001 ～ B-009）。

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| Guardrail adapter | `src/server/guardrails.rs` | `content_text` 只借用普通 text part | 根因与唯一 enforcement boundary |
| Chat DTO | `src/core/models/openai/messages.rs` | `ContentPart` 包含 document/tool result/tool use | 列出必须规范化的载体 |
| Function DTO | `src/core/models/openai/tools.rs` | legacy/modern function arguments 都会发给 provider | 等价结构化输入不能继续遗漏 |
| Guardrail engine | `src/core/guardrails/{traits,engine,openai_moderation,pii,prompt_injection}.rs` | 接收单个 `&str`；OpenAI moderation 每次调用发一个远程请求；mask 在 gateway boundary fail-closed | 需要 record-aware batch，避免字段拼接和外部调用放大 |

## 设计方案

1. 用一个返回 owned `Result<Vec<GuardrailInputRecord>, GatewayError>` 的规范化函数替代
   `content_text -> Vec<&str>`。owned 结果允许安全容纳 JSON 序列化和 base64 解码
   后的正文；record 只在进程内携带 typed provenance 与原始扫描值，provenance 不编码
   进扫描文本，错误不能被 iterator/filter 静默丢弃。
2. builder 按稳定顺序生成互补记录：
   - 对每条 message 中语义连续的 content parts 生成 adjacency view，不插入标签，
     使跨 part 拆词保持可检测；document/tool 等不连续 surface 不加入该连续串。
   - 对 message name、普通 content、message-level legacy `function_call`
     name/arguments、modern `tool_calls[].function` name/arguments 和
     `ToolUse.name` 生成独立 typed records。每个 record 的值单独交给 engine，
     禁止把 kind、长度、换行或其他人工标签拼入扫描值，禁止 regex 跨 record 匹配。
   - request-level `ChatCompletionRequest.function_call.name/arguments` 在 messages
     之后按固定 typed kind 加入；顺序与 request DTO 一致。
3. `ToolResult.content` 与 `ToolUse.input` 先使用 `serde_json::to_string` 加入完整、
   确定性表示，再深度遍历 `serde_json::Value`，按稳定对象 key 顺序分别加入解码
   后的 string keys/values。合法 JSON function arguments 使用同一遍历并同时保留
   原 argument string；解析失败不跳过原字符串，也不把原本允许的非 JSON argument
   改成请求错误。序列化错误 fail-closed。
4. `Document.source.data` 先按 base64 解码。仅接受明确文本媒体类型：
   `text/*`、`application/json`、`application/*+json`、`application/xml` 和
   `application/*+xml`；解码结果必须是 UTF-8。malformed base64、非 UTF-8 或
   其他媒体类型在 input guardrail 开启且 `check_input` 为 true 时 fail-closed。
   MIME 比较忽略 ASCII 大小写并剥离参数。malformed base64、非 UTF-8 和 unsupported
   MIME 均返回 `GatewayError::validation`，外部固定为 HTTP 400、
   `type=invalid_request_error`、`code=invalid_request`；安全消息只说明
   base64/UTF-8/media-type 类别，不回显输入。
   `application/json` 与 `application/*+json` 的解码正文必须作为 raw record
   扫描，并解析成 `serde_json::Value` 后按第 3 条生成 semantic records；声明为
   JSON 但语法无效时返回同一稳定 400（消息类别为 invalid JSON，不回显正文），
   不得仅扫描带 `\uXXXX` 转义的 raw 表示后放行。
   不读取 URL、不解析 PDF/Office、不扫描 image/audio base64。
5. 新规范化只改变传给 guardrail 的扫描字符串，绝不重写原 DTO。现有
   `GuardrailAction::Mask`/modified 结果继续由 `enforce` 显式失败，避免把扁平文本
   错误回写到结构。
6. 在 `check_input` 中先构造全部 records，成功后通过一次 batch 调用交给 engine。
   gateway 不循环调用现有单字符串入口，而是调用一次新增的
   `GuardrailEngine::check_input_records(&[GuardrailInputRecord])`。engine 按优先级
   对每个 guardrail 调用一次 batch 方法；PII/prompt-injection 在该方法内逐 record
   做本地匹配并聚合，不能跨 record；OpenAI moderation 将全部非空 record values
   作为 API 支持的 string array 放进一次 `/moderations` 请求，按 response index
   聚合，结果数量不匹配时 fail-closed。任一 guardrail 的聚合结果为
   block/error/modified 时沿用现有 `enforce` 语义结束；只有全部 allow 才继续。
   单字符串 `check_input` 通过一元素 batch 保持兼容。engine disabled 或
   `check_input: false` 时保持现有无检查行为。
7. builder 使用固定常量 `MAX_INPUT_GUARDRAIL_RECORDS = 256` 与
   `MAX_INPUT_GUARDRAIL_SCAN_BYTES = 2 * 1024 * 1024`。每次加入 record 前用 checked
   arithmetic 统计 records 数与所有 record values 的 UTF-8 byte 总和；任一超限
   返回 `GatewayError::validation`，公开固定 HTTP 400
   `invalid_request_error`/`invalid_request`，安全消息只说明 fragment/size limit。
   完整 batch 在任何 guardrail（尤其远程 moderation）调用前验证完毕。内置 remote
   guardrail 的外部请求次数必须按 guardrail 数量有界，而不是按 record 数量增长。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| B-001 | request/message/function/content 规范化 | 每种载体独立 allow/block fixture |
| B-002/B-003 | adjacency + typed independent records | 跨 content part、顺序、record 隔离、JSON/document raw+semantic、Unicode escape snapshot |
| B-004 | `check_input` 调用顺序 | mock provider 未被调用 |
| B-005 | `enforce` modified 分支 | mask 仍 fail-closed 且请求 DTO 未改变 |
| B-006 | fallible builder | malformed document/serialization 返回安全稳定 400，且不调用 engine/provider 后续 |
| B-007 | document media gate | 文本正文解码；PDF/image/audio/URL 无网络且 fail-closed/保持范围 |
| B-008 | disabled fast path | 同一多模态请求在未启用 guardrail 时保持兼容 |
| B-009 | batch engine + builder limits | 256/2 MiB 边界、越界 zero-call、moderation single-batch/count mismatch |

## 数据流

`ChatCompletionRequest.messages` 按顺序进入 bounded fallible fragment builder；连续 content
形成 adjacency record，独立字段形成 typed independent records。JSON 同时保留完整
表示和解码 string nodes；允许的 document base64 解码为 UTF-8，JSON MIME 进一步
生成 raw + semantic records；request-level `function_call` 最后加入。builder 完整
成功且通过 256/2 MiB 上限后，把 typed batch 一次交给 engine；engine 让每个
guardrail 在保持 record 边界的前提下处理该 batch，OpenAI moderation 使用一次 array
请求。再由 `enforce` 统一映射 allow/block/modified/error。原请求对象始终不变。

## 备选方案

- 对整个 `ChatCompletionRequest` 直接 JSON 序列化：拒绝，因为会扫描 image/audio
  base64、混入无关配置，并且 document 正文仍是编码数据。
- 只扫描 JSON/base64 原字符串：拒绝，因为不能识别 document 解码后的自然语言。
- 对不支持 document 类型放行：拒绝，因为输入 guardrail 会继续存在公开 bypass。
- 自动提取 PDF/Office：超出范围且会新增复杂解析/资源消耗面。

## 风险

- Security: 支持媒体类型列表必须 fail-closed，不能被 MIME 大小写/参数绕过。
- Compatibility: 启用 input guardrail 的二进制 document 将被拒绝；发布说明需明确。
- Performance: 文档解码和 owned 载荷增加内存；沿用请求大小限制并避免重复复制。
- Availability/Cost: records 有 256/2 MiB 硬上限；内置 OpenAI moderation 必须 batch
  为单次远程调用，并验证 response count，避免 JSON fan-out。
- Maintenance: DTO 新增文本载体时应在 exhaustiveness test 中显式分类。

## 测试计划

- [ ] Unit tests: 全 variant、request/message legacy/modern function、JSON raw+semantic、record isolation、跨 part、Unicode。
- [ ] Document tests: textual MIME、JSON raw+semantic/invalid、`+json/+xml`、MIME 参数、bad base64、bad UTF-8、PDF。
- [ ] Batch tests: 256/2 MiB 边界、checked overflow/越界 zero external calls、local record isolation、OpenAI array single-call、response count mismatch fail-closed。
- [ ] Integration tests: blocked 发生在 provider 前，400 error envelope 稳定，engine disabled 与 `check_input: false` 都保持 DTO。
- [ ] Repository gates: `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试、`cargo test`。

## 回滚方案

回滚规范化函数与测试即可，无持久化迁移。若二进制 document 兼容性需要恢复，
必须另行设计可审核的解析器或按端点禁用 input guardrail；不得重新静默放行。
