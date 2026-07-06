# T1.4 SQLite 索引层 — 独立 Review

> 审阅对象：`compass-core/src/db.rs`（新）+ `config.rs`（加 `db_path`）
> 依据：PRD_v3.0 §4.2 / PLAN T1.4 / 评分不变量 / T1.3 冷却接口
> 模式：批判性自审（环境无 subagent），findings 在本分支修复后合并。

## 验收对照（PLAN T1.4）

| 验收项 | 证据 | 结论 |
|--------|------|------|
| entities/score_history/timeline/entities_fts 表 | `init_schema` 四表 + 索引；`test_open_in_memory_creates_schema` | ✅ |
| 从 vault 全量重建索引 | `rebuild_from_vault` + `test_rebuild_indexes_vault`/`_is_idempotent`/`_nested`/`_real_vault_sample` | ✅ |
| FTS5 可查 | `fts_search` + 7 个 fts 测试（命中/AND/无命中/空/limit/更新/删除） | ✅ |
| cargo test 全绿 | 66 passed（原 40 + 新 26） | ✅ |

## 不变量对照

1. **frontmatter 权威**：rebuild 从 vault 解析 frontmatter 重建；SQLite 仅缓存。✅
2. **删库可重建**：rebuild 清空 entities+fts 重建。⚠ score_history/timeline 不可从 vault 重建（PRD 设计：历史不进 frontmatter）——备份策略待 T1.5+。
4. **写回不破坏正文**：T1.4 不写 frontmatter（仅读+索引），不涉及。✅

## 接口对接

- T1.3 `apply_trigger_if_eligible(last_for_type)` ← `Db::last_trigger_time(entity_id, trigger)`：按 `score_history.id DESC` 取最新（插入序=时间序，规避 RFC3339 时区排序歧义）。✅ 测试 `test_last_trigger_time_returns_latest_by_insert_order` 反向验证。

## Findings

| ID | 优先级 | 问题 | 处置 |
|----|--------|------|------|
| F1 | 中 | rebuild 遇重复 id 静默覆盖（最后胜出），数据完整性 | **修**：HashSet 检测 + warn + `duplicates` 统计，跳过后续 |
| F2 | 中 | entities 加 interest/strategy/consensus 偏离 PRD §4.2 字面 | **修**：更新 PRD §4.2 注明缓存扩展 |
| F3 | 低 | `unchecked_transaction` vs `transaction()` | **不修**：`transaction()` 需 `&mut self` 致 API 蔓延；`unchecked_transaction` 为 `&self` 无嵌套场景设计（clippy 不报），单连接顺序调用合理，保留 |
| F4 | 低 | rebuild 每文件一事务，可批量 | 记录，T1.5 增量场景不阻塞 |
| F5 | 低 | config db_path 解析无单测 | 记录 |
| F6 | 前瞻 | Db 非 Sync，T1.6 axum 共享需 Mutex/pool | 记录，非 T1.4 阻塞 |

## 安全

- SQL 全参数化（`params!`），无拼接。✅
- FTS `fts_query` 拆词去引号加双引号，防 FTS 语法注入（`-`/`*`/`OR`）。✅ 测试 `test_fts_query_sanitizes`。

## 结论

F1/F2/F3 修复后可合并。F4/F5/F6 记录后续。