# Product Spec

## Linked Issue

GH-841 / #841

## 用户问题

`InMemoryCache` 的主存储使用 `DashMap`，但缓存热路径仍被一个全局
`Arc<tokio::Mutex<LruCache<CacheKey, ()>>>` 串行化。`get` / `get_entry`
在命中后都会更新同一把 LRU mutex，`set_with_ttl` / `set_with_size` 在插入后也会更新同一结构。
当缓存容量满时，LFU 与 TTL 淘汰还会遍历整张 `DashMap` 查找候选项。结果是并发缓存读写无法获得
`DashMap` 分片并发收益，容量越大淘汰越慢。

## 目标

- 缓存命中与写入路径不再等待全局 LRU mutex。
- LRU / LFU / TTL / FIFO 淘汰保持现有对外语义，但淘汰候选选择不再做全表扫描。
- 统计、TTL 过期清理、显式删除、`clear`、`keys`、`stats` 等对外 API 行为保持兼容。
- 用聚焦并发测试或 benchmark 证明高并发读写吞吐改善，并用单元测试证明淘汰语义未回退。

## 非目标

- 不改变 `InMemoryCache` / `LLMCache` 的公开 API。
- 不改变默认缓存配置、Redis 缓存行为或 response cache 调用方语义。
- 不把缓存策略重写成近似随机淘汰，除非产品验收明确接受可解释的近似边界。

## Behavior Invariants

1. `get` / `get_entry` 在未过期命中时返回值、访问计数、最近访问语义与当前实现一致，但不得获取全局 LRU mutex。
2. `set` / `set_with_ttl` / `set_with_size` 在容量已满时仍按配置的 `EvictionPolicy` 淘汰一个候选项后插入；
   LFU / TTL / FIFO 不得通过 `cache.iter().min_by_key` 或 `find` 扫描全部 entry。
3. 删除、过期命中剔除、后台清理和 `clear` 必须同时更新主存储、淘汰索引和统计；不能留下索引孤儿项导致后续淘汰漏删或误删。
4. 过期 entry 在读取时仍返回 miss，并更新 miss / size / entry_count 统计。
5. 并发读写下，缓存不会出现 panic、死锁、负 size 统计、entry_count 与 `cache.len()` 长期不一致。

## 验收标准

- [ ] `src/core/cache/memory.rs` 中 `get` / `get_entry` / `set_with_ttl` / `set_with_size` 的正常热路径不再调用全局 `lru_order.lock().await`。
- [ ] LFU / TTL / FIFO 淘汰路径不再对 `DashMap` 做全表扫描；如保留扫描，只允许在测试或一次性诊断代码中出现。
- [ ] 淘汰索引与主存储一致性有单元测试覆盖：命中更新、删除、过期剔除、覆盖写入、`clear`。
- [ ] 并发压测或 criterion bench 覆盖多 task 命中、写入、淘汰混合场景，并在 PR body 附迁移前后对比。
- [ ] 现有缓存测试和新增聚焦测试通过。

## 边界情况

- `max_size == 0` 当前通过 `NonZeroUsize::MIN` 兜底；实现不得引入 panic 或无限淘汰循环。
- `CacheEntry::touch` 当前同时更新 `access_count` 与 `last_accessed`；迁移后 LFU 与 LRU 索引必须从同一访问事件更新。
- 覆盖写入同 key 时，旧 entry 的 size 必须扣减，新 entry 必须只在索引中出现一次。
- 后台 TTL cleanup 与前台淘汰可能并发删除同一个 key，必须容忍 remove miss。

## 发布说明

内部性能优化，无公开 API 变化。CHANGELOG 以 `perf(cache)` 记录。
