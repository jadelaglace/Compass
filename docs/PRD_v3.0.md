# Compass PRD v3.0 — 重做版精简规格

> 版本：v3.0 ｜ 日期：2026-07-05
> 性质：实施规格（替代 `PRD_v2.1` 的臃肿路线）
> 依据：`docs/REVIEW_整体分析结论.md` + 归档原始意志（`archive/`）
> 一句话：回到"评分引力场 + Obsidian 当 UI + 纯 Rust 单二进制 + 极薄 Web"。

---

## 0. 为什么有 v3.0

v2.x 的根本问题（详见 `REVIEW_整体分析结论.md`）：
- 评分从 frontmatter 搬进 SQLite → Obsidian 看不到引力场 → 砍 Obsidian 插件 → 另起一套与 Obsidian 重复的重 JS Web UI；
- 主力在 Python，Rust 反成带 bug 的点缀（衰减模型/权重漂移、死代码、漏 await、关键件未接线）；
- 130h+84h 大量重建 Obsidian 已有能力，稀释唯一差异化。

v3.0 的转向：**评分写回 frontmatter（Obsidian 可见）+ 单一 Rust 二进制 + Obsidian 当主 UI + 极薄 Web 只做引力场/Feed + Agent/Skill 主交互**。砍掉一切 Obsidian 已有能力。

---

## 1. 产品定位

**个人知识宇宙**——以"我"为核心、高度结构化且动态演进。

**核心引擎 = 动态相关度评分系统**，是整个知识库的"**引力场**"（最高准则），决定每个知识元素的"价值"与"位置"。让高价值内容自然浮现，让过时内容优雅衰减。

三维评分（语义对应 现在/未来/过去；命名采用短形式）：

| 维度 | 字段 | 语义 |
|------|------|------|
| 现在·兴趣 | `interest` | 当下热情所在 |
| 未来·战略 | `strategy` | 面向未来的战略布局 |
| 过去·共识 | `consensus` | 已验证的、基石性知识 |

---

## 2. 设计原则（四条原始决策 + 两条新约束）

1. **数据主权优先**：Markdown + frontmatter 是根数据，50 年后仍可读；SQLite 只是索引/缓存/历史。
2. **Agent 优先设计**：自然语言是主交互方式，UI 是辅助。
3. **评分是灵魂**：三维评分是唯一差异化；**AI 建议、人类决策**（保留手动覆盖权）。
4. **渐进式复杂**：从 Markdown+评分起步，逐步加浮现/Agent。
5. **【新】纯 Rust**：单一二进制，无 Python 胶水层、无 subprocess。
6. **【新】少量 JS**：Web 端不建 SPA、不引构建链，HTMX + 少量 D3。

---

## 3. 三方分工（v3.0 核心：明确谁做什么，杜绝重复造轮子）

| 能力 | 负责方 | 说明 |
|------|--------|------|
| Markdown 编辑/阅读 | **Obsidian** | 原生 |
| 双向链接 `[[id]]` / 反向链接 | **Obsidian** | 原生，Compass 不自建 refs 维护 |
| 标签 `#tag` | **Obsidian** | 原生 |
| 图谱可视化 | **Obsidian** | 原生 Graph View |
| 全文搜索 | **Obsidian** | 原生搜索（Compass 内置 FTS5 仅供 Agent/Skill 用） |
| 时间线/日志 | **Obsidian** | Daily Notes + Dataview |
| 评分计算 / 衰减 / 触发 | **Compass 引擎** | 核心，唯一差异化 |
| 分数写回 frontmatter | **Compass 引擎** | 让引力场在 Obsidian 可见 |
| Feed / 浮现排序 | **Compass 引擎** | 核心 |
| 引力场视图（节点大小=评分） | **Web（极薄）** | Obsidian 图谱不能按自定义评分定大小，这是增量 |
| 评分手动调整 | **Obsidian(Dataview)** 或 Web | 改 frontmatter 即可，引擎感知 |
| Agent 自然语言交互 | **Agent + Skill（已有）** | 见 §3.1 |
| 飞书消息通道 | **飞书 ws（已有，外部）** | Compass 不实现 |

> 铁律：**凡 Obsidian 已有的能力，Compass 一律不自建。** Compass 只做"评分→衰减→浮现"闭环 + 给 Agent/Skill/Web 提供数据 API。

### 3.1 接入层架构（飞书/Agent/Skill 均已有，Compass 只提供 API）

```
用户 → 飞书消息 → 飞书 ws(已有) → Agent(OpenClaw, 已有) → compass skill(已有 CLI)
                                                              │ HTTP JSON
                                                              ▼
                                                     Compass HTTP API (Rust 二进制)
                                                              │
                                                     返回 JSON → skill render → 飞书卡片
```

- **飞书 ws / Agent / compass skill 都是已存在的基础设施**，Compass 不重新实现。
- Compass 的责任：**提供符合 Skill 协议的 HTTP API**（见 §7，对齐 skill 的 7 个 action）。
- `render`（JSON→人话/卡片）由 **skill 侧**负责，Compass 不做。
- Skill 脚本与 `SKILL.md` 需随 v3.0 适配（启动命令、衰减描述）——列入计划，非 Compass 核心。

---

## 4. 数据模型

### 4.1 frontmatter 是根（分数写在这里，Obsidian/Dataview 直接读）

```yaml
---
id: know-000001                      # know-/case-/log-/ins- 前缀 + 6位序号
title: 博弈论基础：纳什均衡
layer: knowledge                     # direction|knowledge|case|log|insight
category: [学科系列, 数学, 博弈论]     # 架构层路径（三大界·架构层）
tags: [#数学, #决策科学, #意识系列]
score:                               # ← 引力场（Compass 写回，Dataview 可读）
  interest: 85
  strategy: 92
  consensus: 78
  composite: 85.3                    # 自动算，引擎写回
  weights: {interest: 0.4, strategy: 0.35, consensus: 0.25}  # 可选覆盖
  updated_at: 2026-07-05T10:00:00+08:00
  last_boosted_at: 2026-07-01T...    # 衰减基准
  access_count: 12
status: active                       # active|archived|orphan
source: {type: book, title: 策略思维, url: null}
created_at: 2026-01-15T...
updated_at: 2026-03-20T...
---
# 脑图（Mermaid）+ 正文（Markdown）
```

- `score` 块由 Compass 引擎**写回**；其余字段用户在 Obsidian 编辑。
- **历史轨迹不放 frontmatter**（会膨胀），放 SQLite `score_history` 表。
- 写回采用"解析 frontmatter → 改 score 块 → 原子替换文件" + 文件锁；Obsidian 检测外部变更自动重载。

### 4.2 SQLite 仅作索引/缓存/历史（可随时从 vault 重建）

```sql
PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;

CREATE TABLE entities (               -- vault 文件的索引镜像
  id TEXT PRIMARY KEY, file_path TEXT UNIQUE, title TEXT,
  layer TEXT, status TEXT, composite REAL,
  access_count INTEGER, last_boosted_at TEXT,
  content_hash TEXT, updated_at TEXT
);
CREATE TABLE score_history (          -- 评分历史（frontmatter 不存）
  id INTEGER PK, entity_id TEXT, dimension TEXT,
  old REAL, new REAL, reason TEXT, trigger TEXT, created_at TEXT
);
CREATE TABLE timeline (               -- 访问/引用/评分事件流
  id INTEGER PK, entity_id TEXT, event_type TEXT,
  intensity REAL, source TEXT, created_at TEXT
);
CREATE VIRTUAL TABLE entities_fts USING fts5(title, content);  -- 仅 Agent/Skill 用
```

> 删除 v2.x 的 `references`/`taggings`/`refs` 自建表——双向链接与标签交回 Obsidian。

### 4.3 三大界内容结构（原始意志）

- **架构层**（宇宙地图）：`Direction/` — 基石→学科→分支金字塔，受评分调控。
- **内容层**（原子与标本）：`Knowledge/`（理论原子）+ `Cases/`（实践标本），相互 `[[ ]]` 引用成闭环。
- **日志感悟层**：`Logs/`（长期/中期/短期/实时时间切片）+ `Insights/`（感悟，必须自己写）。
- `Inbox/`：实时收集箱。

---

## 5. 核心引擎：评分系统

### 5.1 综合分（回到原始默认权重，消除 v2.x 漂移）

```
composite = interest*0.40 + strategy*0.35 + consensus*0.25   # 范围 [0,100]，四舍五入 1 位
```
- 权重可被 frontmatter `score.weights` 单条覆盖；默认 0.40/0.35/0.25。
- **修正 v2.x bug**：decay 预览不得再用 0.4/0.4/0.2。

### 5.2 衰减（回到原始规格：只衰 interest）

```
# 只衰减 interest 维度；strategy/consensus 不衰减
new_interest = max(interest * 0.5, interest * 0.98 ^ days_inactive)
# 每日 2%，地板 50%（防止完全遗忘重要知识）
```
- **修正 v2.x bug**：Rust 不得对三维度都做半衰期衰减。
- 跳过条件：`status=archived`；`last_boosted_at` 距今 <3 天（boost 保护期）；`layer=direction` 衰减减半。

### 5.3 评分触发器（来自原始规划A §4.1.2）

| 触发条件 | 维度 | 调整 | 冷却 |
|----------|------|------|------|
| 被引用（Agent/Skill/Obsidian 链接） | consensus | +2 | 1 天 |
| 创建关联链接 | interest | +1 | 7 天 |
| 添加案例（理论被实践验证） | strategy | +3 | — |
| 30 天未访问 | interest | −2%/天 | — |
| 手动标记重点 | interest | +10 | — |
| 完成复习 | consensus | +2 | 7 天 |

访问深度 boost：`glance` +0 / `read` +1(interest) / `study` +3(interest) / `apply` +2(interest)+5(strategy)。

### 5.4 浮现（Feed）

`GET /feed` 按 `composite` 降序返回——衰减让过时内容自然下沉，高价值自然浮现。三种模式：`explore`（全量按分）/ `consolidate`（待复习）/ `strategic`（strategy 维度 Top）。

---

## 6. 技术架构（纯 Rust 单二进制）

```
┌──────────────────────────────────────────────────────────┐
│  compass（单一 Rust 二进制，一个进程）                       │
│  ├─ axum HTTP server        /api/* + /web 静态 + 健康检查   │
│  ├─ FileWatcher (notify)    监听 vault → 解析 frontmatter  │
│  │                          → 更新 SQLite + 重算评分        │
│  ├─ ScoringEngine           评分/衰减/触发器/写回 frontmatter│
│  ├─ Scheduler (tokio)       每日衰减、孤儿检测、周报         │
│  └─ rusqlite (WAL/FTS5)     索引/缓存/历史                  │
└──────────────────────────────────────────────────────────┘
        │ 读写                        │ serve                    │ HTTP
        ▼                             ▼                          ▼
  Obsidian Vault (Markdown 根)    极薄 Web（引力场/Feed）    compass skill(已有) → Agent → 飞书 ws(已有)
```

- **crates**：`axum`、`rusqlite`(bundled)、`notify`(文件监听)、`serde`+`serde_yaml`(frontmatter)、`chrono`、`regex`(引用解析)、`tower-http`(静态文件/CORS)、`tokio`(运行时/调度)。
- **进程模型**：单进程，内置 HTTP + watcher + scheduler；无 subprocess、无 Python（skill 侧 Python 属已有外部基础设施）。
- **配置**：`compass.toml`（vault 路径、端口、衰减参数）。
- **部署**：`cargo build --release` → 一个二进制 + vault 目录，守护进程即跑。

---

## 7. API（精简，对齐 Skill 协议）

对齐 `skills/compass` 的 7 个 action（`search/top/get/feed/context/create/score`）。`render` 由 skill 侧负责，Compass 只返回 JSON。

| 方法 | 路径 | 对应 skill action | 作用 | 写 frontmatter |
|------|------|-------------------|------|----------------|
| GET | `/feed` | `feed` | 浮现列表（按 composite，三模式） | — |
| GET | `/entities/top` | `top` | Top 评分实体（可按 layer 过滤） | — |
| GET | `/entities/{id}` | `get` | 详情（含 outgoing refs，供 Agent） | — |
| GET | `/search` | `search` | FTS5 搜索（Agent/Skill 用） | — |
| POST | `/entities` | `create` | 创建（Skill 快速记录→写 .md） | ✅ 新建文件 |
| PATCH | `/entities/{id}/score` | `score` | 手动调分（AI 建议/人类决策） | ✅ 写回 score |
| PATCH | `/entities/{id}/access` | — | 记录访问（触发 boost + 重算） | ✅ 写回 score |
| POST | `/agent/context` | `context` | Agent 上下文组装（语义+评分加权） | — |
| GET | `/graph` | — | 引力场数据（节点+边+评分，供 Web） | — |
| GET | `/health` | — | 健康检查 | — |

> 相比 v2.x 的 35 端点大幅精简；删掉自建的 refs/graph-path/timeline-ui 类端点（交回 Obsidian）。默认端口 `8080`（与 skill `COMPASS_API_URL` 默认一致）。

---

## 8. Web（极薄，HTMX + 少量 D3）

- **单页**：引力场视图（D3 force-directed，**节点大小=composite，颜色=layer**）+ Feed 排行榜。
- 不引 Vue/Vite/Pinia/构建链；用 **HTMX**（服务端渲染片段 + 局部刷新）+ D3（CDN 或本地）。
- Rust server 用 `tower-http::ServeDir` 直接 serve `web/` 静态目录；HTMX 片段由 axum 模板返回。
- 数据走 `GET /graph` 和 `GET /feed`。
- 评分调整：直接在 Obsidian 改 frontmatter（Dataview 可做滑块/表格），Web 不做编辑器。

> 引力场视图是唯一真正增量——Obsidian 原生图谱不会按你的自定义评分给节点定大小。

---

## 9. Obsidian 集成（不开发插件，靠 Dataview + frontmatter）

- **必装**：Dataview（读 frontmatter `score.composite` 排序/表格）、Templater（新建文件自动带 frontmatter 骨架）、Periodic Notes（日志分层）。
- **提供**：一套 Dataview 查询模板库（`docs/dataview-queries.md`）：Top 10 高分、待复习、战略焦点、按层聚合。
- 分数随引擎写回 frontmatter，Obsidian/Dataview 实时反映引力场——无需插件。
- 若未来需"评分滑块"交互，再评估轻量插件；MVP 不做。

---

## 10. Phase 规划（渐进式，呼应原始"渐进式复杂"）

### Phase 1 · 核心闭环（2-3 周）
Rust 骨架 + frontmatter 读写 + 评分引擎 + SQLite 索引 + FileWatcher + 基础 API。
**验收**：在 Obsidian 新建笔记 → 引擎算分写回 frontmatter → Dataview 能看到分数排序。

### Phase 2 · 浮现与可视化（2 周）
衰减调度 + Feed 三模式 + 引力场 Web 极薄页（HTMX+D3）。
**验收**：`GET /feed` 浮现正确；Web 引力场节点大小=评分；30 天衰减曲线合理。

### Phase 3 · Agent/Skill 对接（1-2 周）
适配 `compass` skill 脚本与 `SKILL.md`（启动命令改 Rust 二进制、衰减描述修正、端口对齐 8080）；打通 飞书 ws→Agent→skill→compass 全链路。
**验收**：飞书发"记一下 X"→ skill create → vault 新增 .md；"今天有什么"→ feed 卡片返回。

### Phase 4 · 智能增强（可选，按需）
自动标签建议、关联推荐（LLM/图算法）、认知演化周报。

### Phase 5 · 打磨
Dataview 查询模板库、备份（Git 自动提交）、跨端同步（Syncthing/WebDAV）。

**总周期：5-8 周**（vs v2.x 的 12-16 周 245h）。

---

## 11. 与 v2.x 的差异对照

| 维度 | v2.x | v3.0 |
|------|------|------|
| 评分存储 | SQLite only（Obsidian 看不到） | frontmatter 为主，SQLite 缓存 |
| Obsidian 插件 | 砍掉，Obsidian 当文件管理器 | 不开发插件，但靠 Dataview+frontmatter 让分数可见 |
| Web UI | Vue3+TS+D3+Pinia+PWA（84h SPA） | 极薄单页 HTMX + D3（引力场/Feed） |
| 后端 | Python FastAPI 主力 + Rust 点缀(subprocess) | 单一 Rust 二进制 |
| 飞书 | compass 内置 feishu_bot.py（未实现） | 飞书 ws/Agent/skill 均已有，compass 只提供 API |
| 自建能力 | refs/标签/图谱/搜索/时间线/详情页全自建 | 全交 Obsidian，只做评分/衰减/浮现 |
| 衰减模型 | Rust 三维度半衰期（漂移） | 只衰 interest，0.98^天，50% 地板（回原始） |
| 端点数 | 35 | ~10 |
| 周期 | 12-16 周 / 245h | 5-8 周 |

---

## 12. 风险与应对

| 风险 | 应对 |
|------|------|
| 引擎写 frontmatter 与 Obsidian 编辑冲突 | 文件锁 + 原子写 + 只改 score 块；Obsidian 自动重载外部变更 |
| 纯 Rust 缺少成熟语义检索 | Phase 1-3 不做语义，用 FTS5；Phase 4 评估 `candle`/`ort` 本地嵌入 |
| skill 脚本是 Python（urllib） | 属已有外部基础设施，非 Compass 二进制；按需适配，不阻塞 Phase 1 |
| 评分写回触发 Git 频繁 diff | score 块单独；Git 提交合并到每日定时（可配置） |

---

*v3.0 起步。实施前以本文档为准，废弃 v2.x 的 Phase 4/5 前端与部署规格。开发计划见 `docs/PLAN.md`。*
