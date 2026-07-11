# Compass 罗盘

> 个人知识宇宙系统 — 以三维评分（interest/strategy/consensus）为"引力场"，让高价值内容自然浮现，让过时内容优雅衰减。

## 核心理念

- **评分是灵魂**：三维评分是唯一差异化，决定每个知识元素的"价值"与"位置"
- **frontmatter 是根**：Markdown + frontmatter 50 年后仍可读，SQLite 仅作索引/缓存/历史
- **纯 Rust 单二进制**：无 Python、无 subprocess、无构建链
- **Obsidian 当 UI**：编辑/链接/标签/图谱/搜索全交 Obsidian，Compass 只做评分->衰减->浮现
- **Agent 优先**：飞书 ws -> Agent -> compass skill -> Compass HTTP API（接入层均已有）

## 架构

```
用户 -> 飞书消息 -> 飞书 ws(已有) -> Agent(已有) -> compass skill(已有 CLI)
                                                        │ HTTP JSON
                                                        ▼
                                               Compass HTTP API (Rust 二进制)
                                               ├── axum HTTP server   /api/*
                                               ├── FileWatcher (notify) 监听 vault
                                               ├── ScoringEngine      评分/衰减/触发器
                                               └── SQLite 索引         entities/FTS5
                                                        │
                                               返回 JSON -> skill render -> 飞书卡片
```

## 快速开始

### 前置

- Rust MSVC 工具链（`rustup default stable-x86_64-pc-windows-msvc`）
- Obsidian + Dataview 插件 + Templater 插件

### 构建

```bash
cd compass-core
cargo build --release
```

### 配置

编辑 `compass-core/compass.toml`：

```toml
vault_path = "../vault"      # Obsidian vault 路径（相对配置文件）
port = 8080                  # HTTP API 端口

[weights]                    # 三维默认权重（sum=1.0）
interest = 0.40
strategy = 0.35
consensus = 0.25

[decay]                      # 衰减参数（只衰 interest）
daily_rate = 0.98            # 每日衰减率
floor = 0.5                  # 地板（防完全遗忘）
boost_protection_days = 3    # boost 保护期
direction_layer_factor = 0.5 # direction 层衰减减半
```

### 运行

```bash
cd compass-core
cargo run --release
```

启动后：
- 自动从 vault 全量重建索引
- FileWatcher 监听 vault 变更（新建/修改/删除）
- HTTP API 监听 `http://localhost:8080`

## API 端点

| 方法 | 路径 | 作用 |
|------|------|------|
| GET | `/health` | 健康检查 |
| GET | `/feed?mode=explore` | 浮现列表（按 composite 降序） |
| GET | `/entities/top?layer=&limit=` | Top 评分实体 |
| GET | `/entities/{id}` | 实体详情（含 `[[id]]` refs） |
| GET | `/search?q=&limit=` | FTS5 搜索 |
| POST | `/entities` | 创建实体（写 .md） |
| PATCH | `/entities/{id}/score` | 手动调分（写回 frontmatter） |
| PATCH | `/entities/{id}/access` | 记录访问（触发 boost） |

## 评分模型

### 三维评分

| 维度 | 字段 | 语义 |
|------|------|------|
| 现在·兴趣 | `interest` | 当下热情所在 |
| 未来·战略 | `strategy` | 面向未来的战略布局 |
| 过去·共识 | `consensus` | 已验证的、基石性知识 |

```
composite = interest*0.40 + strategy*0.35 + consensus*0.25
```

### 衰减（只衰 interest）

```
new_interest = max(interest * 0.5, interest * 0.98 ^ days_inactive)
```

### 触发器

| 触发条件 | 维度 | 调整 | 冷却 |
|----------|------|------|------|
| 被引用 | consensus | +2 | 1 天 |
| 创建关联链接 | interest | +1 | 7 天 |
| 添加案例 | strategy | +3 | - |
| 手动标记重点 | interest | +10 | - |
| 完成复习 | consensus | +2 | 7 天 |

访问深度 boost：`glance` +0 / `read` +1 / `study` +3 / `apply` +2+5

## Obsidian 集成

- **Dataview**：读 `score.composite` 排序/表格（查询模板见 `docs/dataview-queries.md`）
- **Templater**：用 `vault/Templates/` 下的模板新建笔记（含完整 score 骨架）
- **无需插件**：分数写回 frontmatter，Obsidian/Dataview 自动反映

## 目录结构

```
Compass/
├── compass-core/              # Rust 单二进制
│   ├── src/
│   │   ├── main.rs            # 入口：配置 + DB + FileWatcher + API
│   │   ├── config.rs          # compass.toml 加载
│   │   ├── models.rs          # Score / Weights / Layer
│   │   ├── scoring.rs         # 综合分 + 衰减 + 触发器
│   │   ├── frontmatter.rs     # YAML 解析 + score 块替换 + 原子写
│   │   ├── db.rs              # SQLite 索引层（entities/FTS5/history）
│   │   ├── watcher.rs         # notify 文件监听
│   │   ├── api.rs             # axum HTTP 路由
│   │   └── e2e_tests.rs       # 端到端验收测试
│   └── compass.toml           # 运行时配置
├── vault/                     # Obsidian vault
│   ├── Direction/             # 架构层
│   ├── Knowledge/             # 内容层·理论原子
│   ├── Cases/                 # 内容层·实践标本
│   ├── Logs/                  # 日志层
│   ├── Insights/              # 感悟层
│   ├── Inbox/                 # 收集箱
│   └── Templates/             # Templater 模板
├── skills/compass/            # compass skill CLI（已有，Python）
├── docs/                      # 文档
│   ├── PRD_v3.0.md            # 实施规格
│   ├── PLAN.md                # 开发计划
│   ├── dataview-queries.md    # Dataview 查询模板
│   └── REVIEW_*.md            # 各任务 review
└── archive/                   # v2.x 归档
```

## 开发状态

### Phase 1 · 核心闭环 ✅

- [x] T1.1 项目骨架（Cargo + config + /health）
- [x] T1.2 frontmatter 读写（YAML 解析 + score 块替换 + 原子写 + 文件锁）
- [x] T1.3 评分引擎（composite + 衰减 + 触发器 + 冷却）
- [x] T1.4 SQLite 索引层（entities/score_history/timeline/FTS5 + rebuild）
- [x] T1.5 FileWatcher（notify 监听 + 去抖 + 解析 + 索引 + 写回）
- [x] T1.6 基础 API（7 端点 + main.rs 接线）
- [x] T1.7 验收测试（端到端闭环）
- [x] T1.8 文档与样例（Templater 模板 + README）

### Phase 2 · 浮现与可视化 ✅

衰减调度 + Feed 三模式 + 引力场 Web（HTMX+D3）已完成。

### Phase 3 · Agent/Skill 对接 ✅

已完成 skill 脚本适配、`POST /agent/context`、search 响应对齐及本地全链路验收。

验收路径：`skill action → Rust HTTP API → vault/frontmatter → FileWatcher → skill render`。

### Phase 4 · 智能增强（准备完成，待实现）

实施入口：[`docs/PHASE4_PREP.md`](docs/PHASE4_PREP.md)。

Phase 4 只做可解释建议和结构化周报：LLM 由已有 Agent/skill 调用，Compass 不自动覆盖标签/链接、不实现 Feishu ws；Phase 4 的建议写回必须经过显式确认和 content hash 校验，Phase 1-3 的 score/access/create 合同保持不变。

## 测试

```bash
cd compass-core
cargo test
```

119 个 Rust 测试覆盖：评分引擎 / frontmatter 读写 / SQLite 索引 / FTS5 / FileWatcher / API / 端到端闭环。

skill 侧另有 18 个 renderer 单测和 14 个 HTTP E2E：

```bash
cd skills/compass
python -m unittest test_compass.py
python -m unittest test_e2e.py
```

## License

Compass Open License (COL) — 见 [OPENCLAW.md](OPENCLAW.md)
