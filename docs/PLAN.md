# Compass 开发计划 (PLAN)

> 版本：v1.0 ｜ 日期：2026-07-05
> 依据：`docs/PRD_v3.0.md`
> 原则：渐进式复杂，每个 Phase 有可验收闭环。

---

## 0. 路线图

| Phase | 名称 | 周期 | 状态 | 验收闭环 |
|-------|------|------|------|----------|
| 1 | 核心闭环 | 2-3 周 | 待开发 | Obsidian 新建笔记→引擎算分写回 frontmatter→Dataview 排序可见 |
| 2 | 浮现与可视化 | 2 周 | ✅ 完成 | `/feed` 浮现正确；Web 引力场节点大小=评分 |
| 3 | Agent/Skill 对接 | 1-2 周 | 待开发 | 飞书"记一下 X"→vault 新增 .md；"今天有什么"→feed 卡片 |
| 4 | 智能增强 | 按需 | 待开发 | 自动标签/关联推荐/周报 |
| 5 | 打磨 | 按需 | 待开发 | Dataview 模板库 + Git 备份 + 跨端同步 |

**总周期：5-8 周。**

---

## 1. Phase 1 · 核心闭环（详细任务分解）

> 目标：建立"写笔记→引擎算分→写回 frontmatter→Obsidian 可见"的最小闭环。
> 技术栈：Rust 单二进制（axum + rusqlite + notify + serde_yaml + tokio + tower-http）。

### 任务分解

| ID | 任务 | 产出 | 依赖 | 工时 | 验收 |
|----|------|------|------|------|------|
| T1.1 | 项目骨架 | Cargo workspace + crate 依赖 + `compass.toml` 配置加载 | — | 4h | `cargo build` 通过；`/health` 返回 ok |
| T1.2 | frontmatter 读写模块 | 解析 YAML → 改 `score:` 块 → 原子写回 + 文件锁 | T1.1 | 8h | 读 .md frontmatter；改 score 写回不破坏正文；Obsidian 自动重载 |
| T1.3 | 评分引擎 | composite 公式 + 衰减（只衰 interest）+ 触发器表 | T1.1 | 8h | 单元测试：权重 0.4/0.35/0.25；衰减 0.98^天 50% 地板；触发器 boost |
| T1.4 | SQLite 索引层 | `entities/score_history/timeline/entities_fts` + 可重建 | T1.1 | 6h | 从 vault 全量重建索引；FTS5 可查 |
| T1.5 | FileWatcher | `notify` 监听 vault → 解析 → 索引 + 重算评分 → 写回 frontmatter | T1.2,T1.3,T1.4 | 10h | Obsidian 新建/改笔记 → 30s 内索引+算分+写回 |
| T1.6 | 基础 API | `GET /feed` `/entities/top` `/entities/{id}` `/search` `PATCH /score` `/access` `POST /entities` | T1.4 | 8h | curl 各端点返回正确 JSON；score/access 写回 frontmatter |
| T1.7 | 验收测试 | 端到端：新建笔记→算分→写回→Dataview 排序 | T1.5,T1.6 | 4h | 闭环跑通；Dataview 查询按 composite 排序 |
| T1.8 | 文档与样例 | Templater 模板（带 score 骨架）+ README 更新 | T1.7 | 2h | 新建笔记模板含完整 frontmatter |

**Phase 1 合计：~50h（2-3 周）。**

### Phase 1 关键不变量（验收必须满足）

1. **frontmatter 是权威**：`score.composite` 由引擎计算并写回；SQLite 仅缓存，删库可从 vault 重建。
2. **衰减只衰 interest**：`new_interest = max(interest*0.5, interest*0.98^days)`；strategy/consensus 不衰减。
3. **权重默认 0.40/0.35/0.25**：不得出现 0.4/0.4/0.2（修正 v2.x bug）。
4. **写回不破坏正文**：只改 `score:` 块，Mermaid/正文/其他 frontmatter 字段不变。
5. **单二进制**：无 Python、无 subprocess；`cargo build --release` 产出一个可执行文件。

---

## 2. Phase 2 · 浮现与可视化

| ID | 任务 | 依赖 | 工时 |
|----|------|------|------|
| T2.1 | 衰减调度（tokio 定时，每日 02:00） | P1 | 4h |
| T2.2 | Feed 三模式（explore/consolidate/strategic） | P1 | 6h |
| T2.3 | `/graph` 引力场数据端点（节点+边+评分） | P1 | 6h |
| T2.4 | Web 极薄页（HTMX + D3，引力场 + Feed） | T2.3 | 12h |
| T2.5 | 验收：30 天衰减曲线 + 节点大小=评分 | T2.4 | 4h |

**Phase 2 合计：~32h（2 周）。**

---

## 3. Phase 3 · Agent/Skill 对接

> 飞书 ws / Agent / compass skill 均已有。本 Phase 只适配 skill 调 Compass API。

| ID | 任务 | 依赖 | 工时 |
|----|------|------|------|
| T3.1 | 适配 `skills/compass/compass` 脚本（端口 8080、端点路径对齐 §7） | P1 | 4h |
| T3.2 | 更新 `SKILL.md`（启动命令改 Rust 二进制、衰减描述修正） | T3.1 | 2h |
| T3.3 | 全链路验收：飞书→Agent→skill→compass→vault | T3.2 | 6h |

**Phase 3 合计：~12h（1-2 周）。**

---

## 4. Phase 4 · 智能增强（可选）

- 自动标签建议（LLM / 标题分词）
- 关联推荐（FTS5 相似度 + 图谱距离）
- 认知演化周报（评分变化 Top5 推送飞书）

---

## 5. Phase 5 · 打磨

- Dataview 查询模板库（`docs/dataview-queries.md`）
- Git 自动提交备份（每日 diff）
- 跨端同步（Syncthing/WebDAV）

---

## 6. 立即下一步

**启动 Phase 1 · T1.1 项目骨架**：
1. 创建 `compass-core/` Cargo workspace（复用目录名，清空旧 Rust 代码）
2. `Cargo.toml` 引入：axum、rusqlite(bundled)、notify、serde、serde_yaml、chrono、regex、tokio、tower-http
3. `compass.toml` 配置骨架（vault_path、port=8080、decay 参数）
4. axum `/health` 端点

> 待你确认即可开工。
