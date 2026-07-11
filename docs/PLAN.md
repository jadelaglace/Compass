# Compass 开发计划 (PLAN)

> 版本：v1.0 ｜ 日期：2026-07-05
> 依据：`docs/PRD_v3.0.md`
> 原则：渐进式复杂，每个 Phase 有可验收闭环。

---

## 0. 路线图

| Phase | 名称 | 周期 | 状态 | 验收闭环 |
|-------|------|------|------|----------|
| 1 | 核心闭环 | 2-3 周 | ✅ 完成 | Obsidian 新建笔记→引擎算分写回 frontmatter→Dataview 排序可见 |
| 2 | 浮现与可视化 | 2 周 | ✅ 完成 | `/feed` 浮现正确；Web 引力场节点大小=评分 |
| 3 | Agent/Skill 对接 | 1-2 周 | ✅ 完成 | skill action→Compass API→vault；本地 E2E 覆盖 action + render + FileWatcher |
| 4 | 智能增强 | 按需 | ✅ 完成 | 可解释标签建议/关联推荐/周报 |
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

## 4. Phase 4 · 智能增强（T4.0-T4.7 已完成）

Phase 4 的实施入口是 [`docs/PHASE4_PREP.md`](PHASE4_PREP.md)。先冻结协议和责任边界，再进入运行时开发：

| ID | 任务 | 依赖 | 状态 | 验收摘要 |
|----|------|------|------|----------|
| T4.0 | 协议、标签格式、事件与 schema migration | — | 已完成（PR #211） | 固定 JSON fixture；迁移可重复 |
| T4.1 | 事件与标签/链接可重建索引 | T4.0 | 已完成（PR #214） | rebuild 后索引恢复 |
| T4.2 | 带 content hash 的 metadata patch | T4.0,T4.7 | 已完成（PR #218） | stale 返回 409；只改目标字段 |
| T4.3 | 标签候选 + accept/reject | T4.1,T4.2,T4.7 | 已完成（PR #221） | 候选只读；accept 幂等；reject 无写入 |
| T4.4 | 关联推荐 | T4.1,T4.2,T4.7 | 已完成（PR #222） | 推荐只读；accept/reject 幂等；排除既有链接 |
| T4.5 | 认知周报聚合 | T4.1 | 已完成 | 固定时区可重复；覆盖空/缺失数据 |
| T4.6 | skill action/render 与 E2E | T4.3-T4.5,T4.7 | 已完成 | action → API → Vault/SQLite → render |
| T4.7 | HTTP 暴露面安全门禁 | Issue #206 | 已完成（PR #208） | 默认 localhost；非本机需显式配置 |

约束：Agent/skill 调用已有 LLM，Compass 不内嵌 LLM；Feishu ws 不回迁；Obsidian 标签/链接不自动覆盖。具体字段、非目标和完成定义见 [`docs/PHASE4_PREP.md`](PHASE4_PREP.md)。

---

## 5. Phase 5 · 打磨

- Dataview 查询模板库（`docs/dataview-queries.md`）
- Git 自动提交备份（每日 diff）
- 跨端同步（Syncthing/WebDAV）

---

## 6. 当前状态与下一步

Phase 1、Phase 2、Phase 3 已完成。Phase 3 的本地验收从 `skills/compass` 开始，覆盖：

`skill action → Rust HTTP API → frontmatter/SQLite → FileWatcher → skill render`

证据：`cargo test --release`（149 个 Rust 测试）、`skills/compass/test_e2e.py`（17 个 HTTP/skill E2E）和 `skills/compass/test_compass.py`（21 个 renderer 单测）。完整验收见 [`docs/PHASE1_4_TEST_REPORT.md`](PHASE1_4_TEST_REPORT.md)。

Phase 4 已完成 T4.0-T4.7。skill 已覆盖标签候选、关联推荐、accept/reject 与周报，并通过 action → Rust API → Vault/SQLite → render 的本地 E2E。
