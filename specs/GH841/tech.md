# Tech Spec

## Linked Issue

GH-841 / #841

## Product Spec

Link to `product.md`.

## Codebase Context

| Area | Files | Current behavior | Why relevant |
| --- | --- | --- | --- |
| 主缓存 | `src/core/cache/memory.rs:20-37` | `DashMap<CacheKey, CacheEntry<T>>` 存储 entry，另有全局 `lru_order: Arc<Mutex<LruCache<CacheKey, ()>>>` | 需要移除全局热锁 |
| 读取热路径 | `src/core/cache/memory.rs:88-127` | 命中后 `entry.touch()`，再 `update_lru(key).await` | 每次命中都等待同一把 mutex |
| 写入热路径 | `src/core/cache/memory.rs:140-180` | 容量满时先 `evict_one().await`，插入后 `update_lru` 或 `add_to_lru` | 写入和命中争用同一 LRU 结构 |
| LRU 辅助方法 | `src/core/cache/memory.rs:257-275` | `update_lru` / `add_to_lru` / `remove_from_lru` 都锁全局 LRU | 迁移目标 |
| LFU/TTL/FIFO 淘汰 | `src/core/cache/memory.rs:306-368` | LFU / TTL / FIFO 通过 `cache.iter()` 查找 min 或 expired | 容量满插入时 O(n) |
| Entry 元数据 | `src/core/cache/types.rs:80-151` | `CacheEntry` 保存 `access_count`、`last_accessed`、`created_at`、TTL | 新索引必须与这些元数据一致 |

## 设计方案

1. **引入淘汰索引层**
   - 在 `memory.rs` 内新增私有 `EvictionIndex`，由固定 shard 组成。
   - 每个 shard 维护该 shard 内 key 的 LRU / LFU / TTL / FIFO 排序元数据；锁粒度为 shard，而不是全局缓存。
   - shard 选择使用 `CacheKey::hash_value()`，避免额外 hash 分配。
   - 全局淘汰从各 shard 的 front candidate 中选择最小/最旧候选，复杂度为 `O(shards + log shard_size)`，不随 entry 总数线性增长。

2. **访问元数据更新**
   - 在 `InMemoryCache` 增加单调 `AtomicU64 access_tick`。
   - 命中时先更新主 entry 的 `touch` 语义，再用新 tick 更新对应 shard 的索引项。
   - 若实现选择将访问计数/最近访问 tick 拆到专用 metadata 结构，必须保持 `get_entry` 返回的 `CacheEntry` 元数据与当前 API 兼容。
   - 不能用异步全局后台队列作为唯一索引更新路径；读取后立即淘汰必须能看到本次访问或有明确测试证明不会违反 LRU/LFU 语义。

3. **淘汰路径**
   - `evict_lru` 从 shard fronts 中选全局最小 `last_access_tick`。
   - `evict_lfu` 从 shard fronts 中选全局最小 `(access_count, last_access_tick, key)`，用 tick 作为稳定 tie-breaker。
   - `evict_ttl` 先从 TTL 索引取已过期候选；没有过期候选时选最早过期 entry。
   - `evict_fifo` 用 `created_at` / 插入 tick 的索引选最旧 entry；不得继续 `cache.iter().min_by_key`。
   - 淘汰从索引取候选后再 `cache.remove(&key)`；若主存储已被并发删除，清理索引后重试有限次数，避免死循环。

4. **一致性维护**
   - `set_with_ttl` / `set_with_size` 覆盖旧 key 时，先移除旧索引项，再插入新索引项，size 统计按当前语义更新。
   - `delete`、前台过期 remove、后台 `cleanup_expired`、`clear` 必须调用同一套 index remove/clear helper。
   - 索引 helper 保持私有，不暴露给 `LLMCache` 调用方。

5. **测试与 benchmark**
   - 新增聚焦单元测试覆盖每个策略的插入、命中、覆盖、删除、过期、clear。
   - 新增并发压力测试或 criterion benchmark：固定大缓存、多 task 读同一热点、多 task 写入触发淘汰、读写混合。
   - benchmark 输出写入 PR body；仓库不强制提交生成的 benchmark 报告。

## Product-to-Test Mapping

| Product invariant | Implementation area | Verification |
| --- | --- | --- |
| P1 热路径无全局 mutex | `InMemoryCache::get{,_entry}` + index update | `rg -n "update_lru|lru_order\\.lock" src/core/cache/memory.rs` 只允许迁移兼容/测试残留；聚焦命中测试 |
| P2 淘汰无全表扫描 | `evict_lru` / `evict_lfu` / `evict_ttl` / `evict_fifo` | `rg -n "cache\\.iter\\(\\).*min_by_key|cache\\.iter\\(\\).*find" src/core/cache/memory.rs` 零命中或仅测试 |
| P3 索引一致性 | delete / expired / cleanup / clear | 单测：删除后淘汰不返回旧 key；过期前台/后台并发 remove 不 panic |
| P4 统计兼容 | stats 更新点 | 现有缓存统计测试 + 新增 size/entry_count 断言 |
| P5 并发安全 | shard index + DashMap | 多 task 压测或 criterion bench |

## 风险

- Concurrency: shard 索引与主存储双写可能产生短暂不一致；必须通过 remove miss 重试和索引清理兜底。
- Compatibility: `CacheEntry` 是公开结构，不能随意把字段改为 atomic 类型并破坏 Clone/Serialize 转换路径。
- Performance: shard 锁数量过少会残留争用，数量过多会增加 eviction front 合并成本；初始 shard 数应固定且有基准支撑。

## 测试计划

- [ ] `cargo test core::cache::memory --lib --all-features`
- [ ] `cargo test core::cache --lib --all-features`
- [ ] `cargo test --all-features cache`
- [ ] 并发 benchmark 或压测命令在 PR body 记录迁移前后吞吐、p95 latency、entry 数量、task 数。
- [ ] `cargo test --all-features`

## 回滚方案

单 PR revert。实现应保持在 `memory.rs` 私有结构内，外部 API 不变，回滚不需要迁移调用方。
