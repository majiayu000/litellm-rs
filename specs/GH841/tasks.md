# Task Plan

## Linked Issue

GH-841 / #841

## Spec Packet

- Product: `product.md`
- Tech: `tech.md`

## 实现任务

- [ ] `SP841-T1` Owner: coordinator. Done when: `specs/GH841/` 三件套通过 SpecRail packet validation. Verify: `SPEC_RAIL=/path/to/specrail; python3 "$SPEC_RAIL/checks/check_workflow.py" --repo "$SPEC_RAIL" --spec-dir "$PWD/specs/GH841"`.
- [ ] `SP841-T2` Owner: maintainer. Done when: #841 批复 shard eviction index 方向、是否允许近似淘汰、benchmark 口径和 shard 数默认值（SpecRail human gate `spec_approval`）. Verify: #841 issue thread 明确批复。
- [ ] `SP841-T3` Owner: coordinator. Done when: `InMemoryCache` 增加私有 shard `EvictionIndex` 与 `access_tick`，LRU 索引不再使用全局 `lru_order`；`get` / `get_entry` 命中路径更新 shard 索引且不锁全局 mutex. Verify: `cargo test core::cache::memory --lib --all-features`; `rg -n "lru_order|update_lru|add_to_lru|remove_from_lru" src/core/cache/memory.rs` 输出只剩兼容注释或零命中。
- [ ] `SP841-T4` Owner: coordinator. Done when: `set_with_ttl` / `set_with_size` / 覆盖写入 / delete / clear / 前台过期剔除 / 后台 cleanup 全部维护索引与统计一致性. Verify: 新增单测覆盖 overwrite、delete、expired get、cleanup、clear；`cargo test core::cache --lib --all-features`。
- [ ] `SP841-T5` Owner: coordinator. Done when: LFU / TTL / FIFO 淘汰迁移为索引候选选择，不再对 `DashMap` 全表扫描；并发 remove miss 有有限重试/清理兜底. Verify: `rg -n "cache\\.iter\\(\\).*min_by_key|cache\\.iter\\(\\).*find" src/core/cache/memory.rs` 零命中或仅测试；策略单测证明候选选择。
- [ ] `SP841-T6` Owner: verification owner. Done when: 并发压测或 criterion bench 覆盖热点命中、写入淘汰、读写混合，PR body 附迁移前后吞吐和 p95 对比. Verify: 记录实际 bench/压测命令与输出路径。
- [ ] `SP841-T7` Owner: verification owner. Done when: 全仓确定性验证通过. Verify: `cargo test --all-features`。

## 并行拆分

- SP841-T3 与 SP841-T5 都修改 `src/core/cache/memory.rs`，不得并行写同文件。
- SP841-T6 可在实现分支完成后由只读验证 lane 并行运行。

## Handoff Notes

- 不要在本 issue 中调整 Redis 缓存、response-cache key 语义或缓存配置默认值。
- 如果实现发现精确 LRU/LFU 与无全局锁之间存在不可接受成本，先回到 #841 说明 tradeoff，不能静默改为随机/近似淘汰。
