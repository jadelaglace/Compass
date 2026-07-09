# T1.7 + T1.8 验收测试 + 文档 - 独立 Review

> 审阅对象：`compass-core/src/e2e_tests.rs`（新）+ `vault/Templates/*` + `docs/dataview-queries.md` + `README.md`（重写）
> 依据：PLAN T1.7/T1.8 / PRD §9/§10
> 模式：批判性自审。

## T1.7 验收对照

| 验收项 | 证据 | 结论 |
|--------|------|------|
| 新建笔记 -> 算分写回 | `test_e2e_api_full_loop` create_entity -> 验证 frontmatter 含 composite | ✅ |
| 算分写回 frontmatter | score/access 写回 + 验证 frontmatter 可读 | ✅ |
| Dataview 排序（composite） | `test_e2e_api_full_loop` feed 按 composite 降序 | ✅ |
| watcher 自动算分 | `test_e2e_watcher_assigns_default_score` 无 score -> 默认 5/5/5 写回 | ✅ |
| watcher 重算 composite | `test_e2e_watcher_recalculates_existing_score` composite=999 -> 81 | ✅ |
| search 可查 | `test_e2e_search_after_create` | ✅ |

## T1.8 验收对照

| 验收项 | 证据 | 结论 |
|--------|------|------|
| Templater 模板含 score 骨架 | `vault/Templates/{knowledge,case,direction}-note.md` | ✅ |
| Dataview 查询模板 | `docs/dataview-queries.md`（5 个查询） | ✅ |
| README 更新 | 重写为 v3.0（纯 Rust 架构 + API + 评分模型 + 目录结构 + 开发状态） | ✅ |

## 端到端测试（4 个新）

1. `test_e2e_api_full_loop` - create -> get -> score 调分 -> access boost -> feed 排序 -> frontmatter 验证
2. `test_e2e_watcher_assigns_default_score` - 无 score 笔记 -> watcher 算默认 -> 写回 + 索引
3. `test_e2e_watcher_recalculates_existing_score` - composite 不一致 -> 重算
4. `test_e2e_search_after_create` - create 后 FTS 搜索

## Findings

| ID | 优先级 | 问题 | 处置 |
|----|--------|------|------|
| F1 | 低 | FTS5 默认 tokenizer 对中文分词支持有限（e2e search 用英文） | 记录：Phase 4 评估中文 FTS5 tokenizer（unicode61/jieba） |
| F2 | 低 | Templater 模板 id 用日期+cursor，非序号 | 可接受：手动新建用日期，API create 用序号，两种方式并存 |

## 结论

Phase 1 核心闭环验收通过。F1/F2 低优先记录。可合并。