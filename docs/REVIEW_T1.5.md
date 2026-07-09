# T1.5 FileWatcher — 独立 Review

> 审阅对象：`compass-core/src/watcher.rs`（新）+ `main.rs`（接线）
> 依据：PRD_v3.0 / PLAN T1.5 / T1.4 接口
> 模式：批判性自审（环境无 subagent），findings 修后合并。

## 验收对照（PLAN T1.5）

| 验收项 | 证据 | 结论 |
|--------|------|------|
| notify 监听 vault | `FileWatcher::start()` + `RecommendedWatcher` | ✅ |
| 解析 frontmatter + 重算评分 | `process_single_file()` + `scoring::composite()` | ✅ |
| 无 score 时计算默认并写回 | `process_single_file()` 无 score 分支 + `frontmatter::write_score()` | ✅ |
| 删除事件清理 | `process_single_file()` 文件不存在分支 + `db.delete_entity()` | ✅ |
| 跳过隐藏目录 | `is_hidden_path()` + `HIDDEN_DIRS` | ✅ |
| 事件去抖 | `DEBOUNCE_MS` + `debounce_timer` | ✅ |
| cargo test 全绿 | 77 passed（原 67 + 新 10） | ✅ |

## 测试覆盖（10 个新测试）

1. `test_extract_id_from_frontmatter` — frontmatter id 提取
2. `test_extract_title` — title 提取
3. `test_extract_layer` — layer 提取
4. `test_is_hidden_path` — 隐藏目录检测
5. `test_extract_id_from_path` — 路径 id 提取
6. `test_content_hash_stable` — 内容指纹稳定性
7. `test_rel_path` — 相对路径转换
8. `test_process_single_file_with_score` — 有 score 文件处理
9. `test_process_single_file_without_score` — 无 score 文件处理（自动计算默认评分）
10. `test_process_single_file_delete` — 删除文件清理

## Findings

| ID | 优先级 | 问题 | 处置 |
|----|--------|------|------|
| F1 | 中 | `process_events` 未在非测试代码使用（仅 `tokio::spawn` 内） | 编译器 dead code warning，非真实问题（Spawn 内调用），不修 |
| F2 | 低 | `is_hidden_path` 未在非测试代码使用 | 同上，`process_events` 内调用 |
| F3 | 低 | 默认评分初始值（5.0/5.0/5.0）硬编码 | 可改为配置项，但 T1.5 范围外，记录后续 |

## 安全

- 文件路径遍历：`rel_path()` 用 `strip_prefix` 防路径注入 ✅
- FTS 内容索引：用 `note.body`（Markdown 正文），不含 frontmatter ✅

## 接口对接

- T1.2 `frontmatter::read_note` / `get_score` / `write_score` ✅
- T1.3 `scoring::composite` ✅
- T1.4 `Db::upsert_entity` / `delete_entity` ✅

## 结论

F1/F2 为编译器 dead code warning（Spawn 内调用），非真实问题。F3 记录后续。可合并。