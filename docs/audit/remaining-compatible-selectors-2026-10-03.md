# 余下兼容选择器的非聊天边界

日期：2026-10-03；基线 `b19e8fc9bd7c2b2baf3cf056aab306aa67b9e28a`；issue #1431。此批核验 W&B、CompactifAI、Aleph Alpha、Anyscale、Linkup、Lambda AI。没有供应商账户实调，不从价格表推导 callable 模型。

决策：复用既有 factory、OpenAI-like 传输和 custom_headers，仅修正 W&B 错误地址。其他选择器保留具体未完成条件，不增加 SDK、配置或泛化框架。

| 选择器 | 官方依据与已确认事实 | 运行时决定及剩余限制 |
| --- | --- | --- |
| wandb | [当前 CoreWeave Serverless API 参考](https://docs.coreweave.com/products/inference/serverless/api-reference)（2026-10-02 更新）明确 base 为 `https://api.inference.wandb.ai/v1`，列出 Chat Completions 与 List Models；Bearer Forge key。[示例](https://docs.coreweave.com/products/inference/serverless)保留同一 W&B 域名 | 修正原 `api.wandb.ai/v1`。既有 `settings.custom_headers.OpenAI-Project` 可指定 team/project；当前参考说此字段可选，未指定用默认 entity 和 inference project，不沿用旧翻译页的必填说法。未找到该 serverless 接口的独立 embeddings/images/audio 合同，保持不声明；Dedicated Inference 是不同服务。 |
| compactifai | [转录合同](https://docs.compactif.ai/api_reference/#audio-transcriptions)是 multipart `/v1/audio/transcriptions`，JSON 返回 text 和 duration 型 usage.seconds；[Whisper 模型卡](https://docs.compactif.ai/models/whisper_large_v3_turbo_slim/)是 callable 证据；[定价](https://docs.compactif.ai/pricing/#speech-to-text-pricing)明确逐秒收费、最少一分钟 | 当前普通转录成本按实际秒数计算，不能保证一分钟最低费用；因此本批不启用 ASR。后续必须在预留与结算同时处理最低费用并测试短音频。文档格式/忽略参数不等于 streaming 能力。没有独立 embeddings/images/TTS 合同；chat 音频输入不是这些端点。 |
| aleph_alpha | 官方 [Python client 固定提交](https://github.com/Aleph-Alpha/aleph-alpha-client/tree/a509b182204213eec9df65416969a779e6dd2f05) 的 `embeddings` 和 EmbeddingV2Request/Response 已定义兼容输入、dimensions、encoding_format、prompt_tokens/total_tokens；旧 semantic_embed 是另一协议。client 8.0 起要求显式 host | 确认供应商存在兼容 embedding 能力，但尚未确认当前 selector 默认 `api.aleph-alpha.com/v1` 对应可用部署及模型，不能直接宣称默认地址可用。需明确实际 Pharia/托管部署地址和模型后启用与测试；不把存在旧协议误写为没有 embeddings。images/audio 未取得该默认地址的合同。 |
| anyscale | [当前官方 serving 文档](https://docs.anyscale.com/llm/serving)介绍用户部署的 Ray Serve/vLLM 服务 | 这不能证明旧 `api.endpoints.anyscale.com/v1` 的 embeddings/images/audio 仍公开可用。未找到足够官方退役日期证据，不猜测删除；自部署 vLLM 已有独立选择器。默认端点及账户可用性仍待确认。 |
| linkup | [当前官方介绍](https://docs.linkup.so/pages/documentation/get-started/introduction)列出 Search、Fetch、Research、Tasks、Extract；这些是检索/提取合同 | 没有取得 OpenAI embeddings/images/audio 合同，也没有确认旧 catalog 的 chat 声明；不将有自然语言输出的 search 自动等同 chat。chat 声明复核交 F10，检索接入不在本批扩展。 |
| lambda_ai | [官方产品页](https://lambda.ai/inference)说明 Inference API 正在结束；[官方 staff 的日期确认](https://deeptalk.lambda.ai/t/sunsetting-chat-sunsetting-inference/4744)为 2025-09-25，[后续确认](https://deeptalk.lambda.ai/t/hermes3-405b-api-not-responding/4762)一致 | 已将确证退役交 F10 #1373 清理 callable 声明。本批不扩非聊天能力，不把 GPU 租赁或部署模型当成已存在的共享推理接口。历史价格不作为恢复 callable 的依据。 |

W&B 配置示例（凭据使用环境配置，不写入文档）：

```yaml
provider_type: wandb
settings:
  custom_headers:
    OpenAI-Project: your-team/your-project
```

验证使用本地 HTTP 上游：factory → Router Chat 选择 → 实际请求确认 Bearer 和 OpenAI-Project，返回 usage 正常解析；Embeddings 选择明确失败。新地址由同一测试确认。此测试不代表真实账户调用成功，也不验证未启用的五家非聊天路径。

本地结果：`cargo fmt --check`、`cargo check`、默认完整 `cargo test`（库 7,132 通过、1 忽略及集成/doc tests）、默认 all-target clippy、gateway/sqlite 完整测试（库 9,426 通过、1 忽略；catalog HTTP 28 通过及其余集成/doc tests）及 feature all-target clippy 全部通过。日志前缀 `/tmp/litellm-final-selectors-`。

保存下来的原始来源哈希（未下载成功的页面只记录上述链接，不伪造快照）：

| 来源 | SHA-256 |
| --- | --- |
| [linkup-index](https://docs.linkup.so/llms.txt) | `70b812c3b1808fb4820c03ab98c12fe68bcfd5c2841908204d3a6610469abd89` |
| [lambda-sunset](https://lambda.ai/inference) | `05826422c266261c69bbe5af65289e3bca24ec54b4bd2db621a66ef0ae027405` |
| [aleph-client](https://raw.githubusercontent.com/Aleph-Alpha/aleph-alpha-client/a509b182204213eec9df65416969a779e6dd2f05/aleph_alpha_client/aleph_alpha_client.py) | `8956f523e844fec0b7f34f466073fa1877c3d86ddac4813ffa2654820474748e` |
| [aleph-embedding](https://raw.githubusercontent.com/Aleph-Alpha/aleph-alpha-client/a509b182204213eec9df65416969a779e6dd2f05/aleph_alpha_client/embedding.py) | `a3cfa5096658bd3fd64a057b6ab1f1d57c0a06e51959fadde814ac1d8ef4ab72` |
| [compact-reference](https://docs.compactif.ai/api_reference/) | `d0cd19b6933fbfaf690cd7e2a034f4a6ba2aa2b8888c9d6dc391efa16c0f0686` |
| [compact-pricing](https://docs.compactif.ai/pricing/) | `adcb0132a5006ac82a2cb986d1059b6c40b761ed62d1c8ccecd5f90b34930389` |
