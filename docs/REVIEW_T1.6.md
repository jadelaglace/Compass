# T1.6 基础 API - 独立 Review

> 审阅对象：`compass-core/src/api.rs`（重写）+ `main.rs`（接线 Db+FileWatcher+AppState）
> 依据：PRD §7 / PLAN T1.6 / #160 字段统一
> 模式：批判性自审，findings 修后合并。

## 验收对照

| 端点 | 方法 | 测试 | 结论 |
|------|------|------|------|
| /feed?mode= | GET | test_feed_sorted_by_composite | ✅ |
| /entities/top?layer= | GET | test_entities_top_layer_filter | ✅ |
| /entities/{id} | GET | test_get_entity_with_refs + not_found | ✅ |
| /search?q= | GET | test_search_hits | ✅ |
| /entities | POST | test_create_entity | ✅ ✅写.md |
| /entities/{id}/score | PATCH | test_update_score_recalculates | ✅ ✅写回 |
| /entities/{id}/access | PATCH | test_record_access_study + invalid_depth | ✅ ✅写回 |

## 字段统一（#160）
- 响应统一 `id`/`composite`（非 v2.x `entity_id`/`final_score`）✅
- 端点路径对齐 PRD §7 ✅

## 测试覆盖（15 新测试）
- 辅助：layer_prefix / layer_dir / parse_access_depth / next_id / extract_refs
- feed 排序 / top 层过滤 / get 详情+refs / search FTS / create 写.md / score 重算+写回 / access boost+写回 / not_found / invalid_depth

## Findings

| ID | 优先级 | 问题 | 处置 |
|----|--------|------|------|
| F1 | 中 | feed 三模式（strategic/consolidate）暂都用 composite 排序，未按 strategy 维度/last_boosted_at 过滤 | 记录：db 需扩展 list_by_strategy / list_for_review，T2.2 Feed 三模式完善 |
| F2 | 低 | create_entity 写 frontmatter 用 format! 字符串拼接，非 score_to_block | 可复用 frontmatter::write_score，但 create 需先写文件再改 score，两步。当前一次性写完整 frontmatter 可接受 |
| F3 | 低 | extract_refs 每次读文件（get_entity 时），无缓存 | 记录：refs 可存 db，T1.6 范围外 |

## 安全
- SQL 全参数化 ✅
- 路径拼接用 strip_prefix 防遍历 ✅
- score 输入 clamp [0,100] ✅

## 结论
F1 记录后续（T2.2 Feed 完善），F2/F3 低优先。可合并。