# Product Spec

## Linked Issue

GH-1127 / #1127

complexity: high

## 用户问题

网关只在非流式响应上执行输出 guardrail。相同请求一旦启用 `stream: true`，
`/v1/chat/completions`、`/v1/completions` 和 `/v1/responses` 就会把模型输出
直接发送给客户端，使已配置的内容审核、PII 与 secret-leak 策略失去作用。
流式调用不能成为绕过输出安全策略的等价开关。

## 目标

- 让三条流式文本路径执行与非流式路径一致的输出 guardrail 策略。
- 在每个待发送窗口对客户端可见前完成累计审核，覆盖跨 chunk 规则。
- 明确审核粒度、缓冲上限、额外延迟、SSE 错误与取消语义。
- 保持预算结算、provider lease、callback 和客户端断连语义完整且恰好终止一次。

## 非目标

- 改变 guardrail provider 的检测规则、action 或 `fail_open` 策略。
- 对图片、音频或其他二进制增量执行 OCR、转写或内容识别。
- 撤回已经离开网关进程的历史响应。
- 改变 provider 的上游 SSE 协议、重试策略或计费模型。
- 提供 `full_response`、per-event 或运行时可切换的审核模式；本 Issue 固定为
  `windowed_cumulative`。

## Behavior Invariants

1. B-001 当输出 guardrail 生效时，`/v1/chat/completions`、`/v1/completions` 和 `/v1/responses` 的模型生成文本必须在发送给客户端前通过输出检查。
2. B-002 审核粒度固定为 `windowed_cumulative`：三条路径使用 `guardrails.stream_output_check_chars` 作为 event-aligned 检查触发阈值，默认 `256` 个新增 Unicode 字符，合法范围 `1..=4096`；包含使新增字符数达到或超过阈值的完整 provider event，并对截至该 event 的累计模型文本执行检查，禁止只检查独立 provider chunk。
3. B-003 跨 chunk 敏感词、跨 chunk PII 与多字节 UTF-8 必须通过累计文本检测；provider chunk 切分不得造成独立 chunk 扫描绕过。由于原 SSE event 不拆分，单个 event 可使实际 pending 窗口超过配置字符阈值。
4. B-004 当前 pending 窗口通过审核前不得发送其中的模型文本、reasoning summary 或 tool/function arguments；允许立即发送的内容仅限不包含模型数据且不破坏事件顺序的连接级元数据或 SSE 注释。
5. B-005 pending event 与累计扫描文本都必须有固定、非零的内部字节上限。达到上限前不得无界增长；超过上限时必须按输出 guardrail 错误 fail-closed，且不得发送当前 pending 内容。
6. B-006 审核通过后，原有 SSE events、usage event、`response.completed` 与 `[DONE]` 必须按原顺序和原字段回放，不得合并、重排或重复模型 delta。
7. B-007 guardrail 明确拒绝时，客户端只收到不含被拒内容的稳定错误 envelope，随后收到一个传输终止 `[DONE]`；不得发送成功 `response.completed` 或成功 callback。
8. B-008 guardrail 执行错误、超时或不可解释结果必须交由现有 engine 的 `fail_open` 契约处理；streaming 层不得将 `Err` 静默转换为通过。默认 `fail_open: false` 时必须按 `guardrail_error` 终止。
9. B-009 provider 在审核前失败或 idle timeout 时，必须保留现有 provider error envelope 与结算语义；不得把上游错误误报为 guardrail violation。
10. B-010 guardrail 拒绝或执行错误时，已发生的 provider 用量仍按现有 partial/final usage 与 reservation fallback 结算；callback 以 `guardrail_output` 失败，provider lease 不得被错误惩罚为 provider failure。
11. B-011 客户端在上游读取、guardrail 等待或审核通过后的回放阶段断连时，等待检查与上游读取必须可取消，且 lease、callback、预算 reservation 恰好结束一次。
12. B-012 纯 usage、role、空 delta、heartbeat、finish reason 与 `[DONE]` 不作为模型文本扫描，但必须保留；completion 的 `echo` 文本属于客户端可见文本，必须包含在累计审核载荷中。
13. B-013 `/v1/responses` 的 output text、reasoning summary 与 function-call arguments 都属于客户端可见模型文本，必须进入同一累计扫描序列。
14. B-014 输出 guardrail 未启用或 `check_output: false` 时，三条路径继续逐事件转发，不增加窗口缓冲或额外 guardrail 延迟。
15. B-015 `windowed_cumulative` 的首内容延迟至少为首个窗口生成时间加一次输出 guardrail 检查时间；已审核并释放的前缀无法因后续窗口形成的新违规上下文而撤回，该风险必须在配置文档和发布说明中明确。
16. B-016 只有通过输出 guardrail 的 `/v1/responses` 结果才能进入 response persistence；拒绝、错误、超限或取消的结果不得保存为成功响应。

## 验收标准

- [ ] 三条端点均有测试证明每个安全窗口在累计审核后按原 SSE 顺序回放。
- [ ] 三条端点均有测试证明被拒窗口中的文本、reasoning 与 function arguments 不出现在任何已发送 event 中。
- [ ] 跨 chunk 敏感模式与多字节 UTF-8 使用累计文本检测；测试证明阈值按 Unicode 字符计数、触发 event 不拆分且其完整模型文本先审核后释放。
- [ ] pending/cumulative buffer overflow、guardrail 拒绝、默认 fail-closed 错误与显式 `fail_open` 均有确定测试。
- [ ] violation/error 只发送稳定错误 envelope 与一个 `[DONE]`，且不发送成功完成事件。
- [ ] usage-only、空 delta、provider error、idle timeout 与三个阶段的客户端断连均保持正确结算和单一 terminal callback。
- [ ] 输出 guardrail 关闭时有测试证明首 chunk 仍可立即转发，且不分配窗口或累计响应缓冲。
- [ ] `stream_output_check_chars` 的默认值 `256`、合法范围 `1..=4096`、延迟影响、内部字节上限和 `windowed_cumulative` 固定粒度有文档。
- [ ] `cargo fmt --check`、`cargo check`、严格 Clippy、相关测试及完整测试通过。

## 边界情况

- 敏感模式可能从一个 chunk 的末尾延伸到下一个 chunk 的开头。
- provider 可能先发送 role、usage 或空 delta，再发送文本。
- upstream EOF 可能没有 usage；仍须使用既有 reserved-spend fallback。
- guardrail 可能在上游已经产生完整 usage 后拒绝内容；拒绝不免除已发生费用。
- 客户端可能在等待上游、等待 guardrail 或回放安全事件时断开。
- 回放阶段断连可能让客户端只看到已经审核通过的部分安全文本；不得重试或重复结算。
- 一个响应可能包含多个 choice、reasoning 与并行 tool call；边界和顺序必须确定。

## 发布说明

启用输出 guardrail 的流式请求采用 `windowed_cumulative` 粒度。发布说明必须公开
`stream_output_check_chars` 默认值 `256` 与范围 `1..=4096`、首窗口延迟、已批准
前缀不可撤回、buffer overflow 的 fail-closed 行为以及 error + `[DONE]` 终止契约。
未启用输出 guardrail 的部署保持现有低延迟流式行为。
