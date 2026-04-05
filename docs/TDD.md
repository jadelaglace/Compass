# Compass Phase 1 — Technical Design Document

| 字段 | 内容 |
|------|------|
| 版本 | v1.0 |
| 日期 | 2026-04-06 |
| 作者 | CTO (太子) |
| 状态 | 正式接手，待团队评审 |

---

## 1. 系统架构

### 1.1 分层总览

```
┌──────────────────────────────────────────────────────────────┐
│  接入层                                                         │
│  ┌─────────────┐  ┌──────────────┐  ┌──────────────────────┐  │
│  │ 飞书Bot     │  │ Obsidian     │  │ Agent (Claude Code) │  │
│  │ (WebSocket) │  │ 桌面端(人)   │  │ /agent/context 调   │  │
│  └──────┬──────┘  └──────┬───────┘  └──────────┬───────────┘  │
└─────────┼────────────────┼─────────────────────┼──────────────┘
          │                │                     │
┌─────────▼─────────────────────────────────────▼──────────────┐
│  FastAPI 网关层                                                  │
│                                                                    │
│  POST /entities          # 创建实体                               │
│  GET  /entities/search   # FTS5 全文搜索                          │
│  GET  /entities/{id}     # 单个实体 + 评分 + 引用                 │
│  POST /entities/{id}/score   # 手动评分                           │
│  GET  /scores/{id}       # 评分详情                               │
│  GET  /feed              # 浮现 feed（评分排序）                  │
│  POST /agent/context     # Agent 上下文注入                       │
│  POST /fetch             # URL 抓取（Phase 1: Agent 调用）       │
│  GET  /graph/neighbors/{id}  # 邻居查询（Phase 3 预埋）         │
│  GET  /graph/evolution   # 评分演化（Phase 3 预埋）              │
└──────────────────────────────────────────────────────────────────┘
          │
┌─────────▼──────────────────────────────────────────────────────┐
│  核心引擎层                                                       │
│                                                                    │
│  FileWatcher    ──watchdog──▶  Vault 目录监听                     │
│  ReferenceParser         解析 [[id]] 双向链接                    │
│  ScoringEngine           三维评分 + decay 计算                   │
│  TimelineEventSystem     事件驱动 boost 记录                     │
│  SearchIndex             FTS5 全文索引 + SQLite                   │
└──────────────────────────────────────────────────────────────────┘
          │
┌─────────▼──────────────────────────────────────────────────────┐
│  数据层                                                           │
│                                                                    │
│  Obsidian Vault (Markdown 文件，根数据)                            │
│  SQLite (索引、元数据、评分历史、FTS5 倒排)                         │
└──────────────────────────────────────────────────────────────────┘
```

### 1.2 技术选型

| 组件 | 选型 | 理由 |
|------|------|------|
| **语言** | Python 3.11+ | FastAPI 生态成熟，asyncio 原生支持 |
| **Web 框架** | FastAPI | 类型安全，自动 OpenAPI 文档，agent 友好 |
| **数据库** | SQLite + FTS5 | 单用户 MVP 最简方案，无运维成本；FTS5 全文检索 |
| **文件监听** | watchdog | Python 标准，跨平台，支持 debounce |
| **飞书 SDK** | lark-oapi | 官方 SDK，支持 events_api + 卡片消息 |
| **YAML 解析** | pyyaml | Front-matter 解析 |
| **Markdown** | markdown-it-py | 引用解析 |
| **HTTP 客户端** | httpx | 异步，/fetch 依赖 |
| **任务队列** | 内存队列 + asyncio | Phase 1 规模不需要 Redis |

### 1.3 项目结构

```
compass/
├── app/
│   ├── __init__.py
│   ├── main.py              # FastAPI 入口
│   ├── config.py            # 环境配置
│   ├── api/
│   │   ├── __init__.py
│   │   ├── entities.py      # /entities 路由
│   │   ├── scores.py        # /scores 路由
│   │   ├── feed.py          # /feed 路由
│   │   ├── agent.py          # /agent/* 路由
│   │   ├── graph.py          # /graph/* 路由（Phase 3 预埋）
│   │   └── fetch.py          # /fetch 路由
│   ├── core/
│   │   ├── __init__.py
│   │   ├── file_watcher.py   # watchdog 封装
│   │   ├── reference_parser.py  # [[id]] 解析
│   │   ├── scoring_engine.py  # 三维评分 + decay
│   │   ├── timeline.py       # 事件系统
│   │   └── search_index.py   # FTS5 + SQLite 操作
│   ├── models/
│   │   ├── __init__.py
│   │   ├── entity.py         # Entity 数据模型
│   │   ├── score.py          # Score 数据模型
│   │   └── event.py          # Timeline Event 模型
│   ├── services/
│   │   ├── __init__.py
│   │   ├── vault_service.py   # Vault 文件操作
│   │   ├── feishu_bot.py      # 飞书 Bot 命令处理
│   │   └── agent_service.py   # Agent API 业务逻辑
│   └── db/
│       ├── __init__.py
│       ├── database.py       # SQLite 连接管理
│       └── schema.sql        # DDL
├── vault/                    # Obsidian Vault（运行时挂载）
│   ├── Inbox/
│   ├── Direction/
│   ├── Knowledge/
│   ├── Logs/
│   └── Insights/
├── tests/
│   ├── unit/
│   ├── integration/
│   └── e2e/
├── scripts/
│   └── init_vault.py         # Vault 初始化脚本
├── requirements.txt
├── Dockerfile
└── README.md
```

---

## 2. 数据模型

### 2.1 SQLite Schema

```sql
-- 实体表（对应 Obsidian 每个 Markdown 文件）
CREATE TABLE entities (
    id          TEXT PRIMARY KEY,          -- YAML front-matter id
    file_path   TEXT NOT NULL UNIQUE,      -- 绝对路径
    vault_path  TEXT NOT NULL,             -- vault 内相对路径
    title       TEXT NOT NULL,
    category    TEXT NOT NULL,             -- Inbox|Direction|Knowledge|Logs|Insights
    created_at  TEXT NOT NULL,              -- ISO 8601
    updated_at  TEXT NOT NULL,              -- ISO 8601
    last_boosted_at TEXT,                   -- 最近一次 boost 时间
    has_attachments INTEGER DEFAULT 0,
    attachment_refs TEXT,                    -- JSON 数组
    metadata    TEXT                        -- YAML front-matter 其余字段（JSON）
);

-- 评分表
CREATE TABLE scores (
    entity_id   TEXT PRIMARY KEY REFERENCES entities(id),
    interest    REAL DEFAULT 5.0,           -- 1-10
    strategy    REAL DEFAULT 5.0,           -- 1-10
    consensus   REAL DEFAULT 0.0,           -- 0-n（被引用次数）
    final_score REAL DEFAULT 0.0,           -- 加权计算结果
    interest_half_life_days REAL DEFAULT 30.0,
    strategy_half_life_days REAL DEFAULT 365.0,
    consensus_half_life_days REAL DEFAULT 60.0,
    manual_override INTEGER DEFAULT 0,      -- 1=手动覆盖，引擎不自动改
    updated_at  TEXT NOT NULL
);

-- 引用表（双向链接）
CREATE TABLE references (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id   TEXT NOT NULL REFERENCES entities(id),
    target_id   TEXT NOT NULL REFERENCES entities(id),
    created_at  TEXT NOT NULL,
    UNIQUE(source_id, target_id)
);

-- 时间线事件表
CREATE TABLE timeline_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id   TEXT NOT NULL REFERENCES entities(id),
    event_type  TEXT NOT NULL,             -- boost|decay|manual_update|created
    trigger     TEXT,                        -- 触发原因描述
    created_at  TEXT NOT NULL
);

-- FTS5 全文索引
CREATE VIRTUAL TABLE entities_fts USING fts5(
    id,
    title,
    content,
    category,
    content='entities',
    content_rowid='rowid'
);

-- 索引
CREATE INDEX idx_entities_category ON entities(category);
CREATE INDEX idx_scores_final_score ON scores(final_score DESC);
CREATE INDEX idx_references_source ON references(source_id);
CREATE INDEX idx_references_target ON references(target_id);
CREATE INDEX idx_timeline_entity ON timeline_events(entity_id);
```

### 2.2 YAML Front-matter 约定

```yaml
---
id: unique-entity-id           # 必需，全局唯一（建议 UUID 或 slug）
created: 2026-04-06
updated: 2026-04-06
category: Knowledge           # Inbox|Direction|Knowledge|Logs|Insights
scores:
  interest: 7
  strategy: 9
  consensus: 3
last_boosted: 2026-04-06      # 可选
tags: []                      # 可选，Phase 1 不做自动分类
---
```

### 2.3 评分计算公式

```
final_score = (interest × 0.4 + strategy × 0.35 + consensus × 0.25) × decay_factor

decay_factor:
  decay_factor = 0.5 ^ (days_since_last_event / half_life_days)

说明：
  - interest 半衰期 30 天（快速衰减，保持新鲜感）
  - strategy 半衰期 365 天（缓慢衰减，战略内容锚定）
  - consensus 半衰期 60 天（被遗忘时自然下降）
  - days_since_last_event = (now - last_boosted_or_created)
  - consensus 不直接参与 decay_factor 计算，而是通过 boost 机制更新
```

---

## 3. 模块设计

### 3.1 FileWatcher

**职责：** 监听 Vault 目录，增量同步到 SQLite。

**触发场景：**
- 文件创建 → 解析 front-matter，插入 entity + score 记录
- 文件修改 → 重新解析，更新 entity，触发引用重算
- 文件删除 → 软删除（is_deleted 标记），保留评分历史

**Debounce 策略：** 500ms 内连续事件合并为一次处理，避免 Obsidian 批量保存刷屏。

**实现要点：**
```python
# watchdog 的 FileSystemEventHandler
# 只监听 .md 文件，排除 .md.bak / .md.tmp
# 事件入队 asyncio.Queue，后台 worker 串行处理
```

### 3.2 ReferenceParser

**职责：** 解析 Markdown 中所有 `[[id]]` 引用，建立双向链接。

**正则：** `\[\[([a-zA-Z0-9_-]+)\]\]`

**流程：**
1. 文件保存时触发
2. 解析所有 `[[id]]` → 提取 outgoing refs
3. 事务写入 references 表（先删后插，保证幂等）
4. 更新 target 端的 consensus = incoming_refs.count

### 3.3 ScoringEngine

**职责：** 计算并维护每个 entity 的 final_score。

**核心方法：**
```python
def calculate_decay(half_life_days: float, days_since_event: float) -> float:
    return 0.5 ** (days_since_event / half_life_days)

def compute_final_score(entity_id: str) -> float:
    score = get_score(entity_id)
    last_event = get_last_event_time(entity_id)
    days = (now - last_event).days
    
    decayed_interest = score.interest * calculate_decay(30, days)
    decayed_strategy = score.strategy * calculate_decay(365, days)
    decayed_consensus = score.consensus * calculate_decay(60, days)
    
    return decayed_interest * 0.4 + decayed_strategy * 0.35 + decayed_consensus * 0.25
```

**触发时机：**
- 文件保存时（引用更新 → consensus boost）
- 定时任务：每小时全量重算 top 100，实时重算受影响的局部实体
- `/s` 命令手动评分 → `manual_override=1`，引擎跳过自动评分

### 3.4 Feishu Bot

**架构：**

```
用户消息 → 飞书 WebSocket 长连接 → Agent Runtime (LLM) → FastAPI Tool Calls
```

- 飞书 Bot 使用 **WebSocket 模式**（lark-oapi events_api）
- **LLM 负责意图理解**（intent parsing），不靠在 FastAPI 层做规则匹配
- FastAPI 只暴露 **tool use 接口**（符合 Agent 调用规范）
- 飞书 Bot 的"智能"在 Agent 侧，FastAPI 是纯工具层

**LLM 调用链路（Phase 1）：**

```
用户: "/q 今天看了篇关于 Notion 定价的文章"
  → 飞书 WebSocket 接收
  → Agent (LLM) 理解意图：需要记录到 Inbox
  → Agent 调用 FastAPI POST /entities
  → FastAPI 写入 Vault + SQLite
  → 返回 entity_id 给 Agent
  → Agent 生成回复发回飞书
```

**FastAPI Tool Use 接口（Phase 1 必须实现）：**

| Tool Name | 用途 |
|-----------|------|
| `create_entity` | 创建实体到 Inbox |
| `search_entities` | FTS5 全文搜索 |
| `update_score` | 手动调整评分 |
| `get_strategic_focus` | 获取战略焦点（strategy > 7） |
| `get_entity` | 获取单个实体详情 |
| `get_context` | Agent 上下文注入 |

**⚠️ 网络挑战：飞书 WebSocket 需要公网 HTTPS 回调地址。开发环境用 ngrok 透传。**

### 3.5 Agent API

```python
POST /agent/context
Body: { "task": "分析竞品 Notion 的定价策略" }
Response: {
    "context": [
        {
            "id": "entity-001",
            "title": "Notion 定价分析",
            "score": 8.7,
            "excerpt": "...相关段落...",
            "source": "Knowledge/notion-pricing.md"
        }
    ],
    "suggested_entities": ["entity-002", "entity-003"],
    "reasoning": "基于 strategy=9 和 consensus=5 筛选..."
}
```

**设计原则：** Agent 调用的返回要带 reasoning 字段，让 Agent 知道为什么返回这些内容（可解释性）。

---

## 4. 开发周期评估

### 4.1 8 周能否交付？

**结论：能，但前提是严格控制范围，且没有意外阻断因素。**

| 周次 | 任务 | 风险等级 |
|------|------|---------|
| Week 1-2 | Vault 结构 + SQLite Schema + FileWatcher | 🟢 低 |
| Week 3-4 | 评分引擎 + Timeline + 引用解析 + FTS5 | 🟡 中（decay 公式需验证） |
| Week 5-6 | FastAPI + Agent API + 飞书 Bot | 🔴 高（飞书 OAuth + HTTPS 是变数） |
| Week 7 | 集成测试 + 端到端验收 | 🟡 中 |
| Week 8 | 文档 + 备份方案 + 发布 | 🟢 低 |

### 4.2 主要风险点

| 风险 | 概率 | 影响 | 应对 |
|------|------|------|------|
| **飞书 Bot HTTPS + OAuth** | 高 | 高 | Day 1 就搭 ngrok，Week 1 末让 Bot 跑通 |
| **评分公式调参** | 中 | 中 | Phase 1 用固定参数，收集反馈再调 |
| **SQLite FTS5 并发写** | 低 | 中 | Phase 1 单用户，用 WAL 模式 + 文件锁 |
| **watchdog 误触发** | 中 | 低 | debounce + 事件类型过滤 |
| **范围蔓延** | 高 | 高 | PRD 已明确，变更必须过 CTO 审批 |

### 4.3 关键路径（不可并行）

```
飞书Bot ↔ FastAPI ↔ FileWatcher ↔ SQLite
         ↑
     必须先通
```

**Week 1 必须交付：** SQLite Schema + FileWatcher 骨架 + 飞书 Bot 本地跑通（哪怕硬编码响应）。

---

## 5. 技术方案评估

### 5.1 需要调整的地方

**1. 飞书 Bot 架构已更正**

Phase 1 **必须有** LLM intent understanding，但这层跑在外部 Agent Runtime 里（WebSocket 模式），不在 FastAPI 层。FastAPI 只做 tool use 接口，供 Agent 调用。架构已更新（见 3.4 节）。

**2. `/fetch` 接口定位需明确**

PRD 说 Phase 1 Agent 自己调 firecrawl/exa，但 FastAPI 层还是要有 `/fetch` 端点作为 thin proxy，避免 Agent 知道底层细节。

```python
# Phase 1 实现（简单转发）
@app.post("/fetch")
async def fetch_url(url: str):
    async with httpx.AsyncClient() as client:
        resp = await client.get(url, timeout=10)
        return {"raw": resp.text[:5000]}  # Agent 自己清洗
```

**3. SQLite WAL 模式必须开**

单用户不等于没并发。watchdog 写事件和 API 读请求可能并发。启用 WAL 模式避免锁阻塞。

```python
conn.execute("PRAGMA journal_mode=WAL")
```

**4. 飞书 Bot 调试周期长，建议 Day 1 就破解**

飞书开放平台注册 + ngrok + 验证 SSL + 事件订阅调试，这个链路最快也要 2-3 天。不要等到 Week 5 和其他功能一起调。

### 5.2 潜在坑

| 坑 | 说明 | 解法 |
|----|------|------|
| **Obsidian 同一文件高频保存** | Obsidian 有 auto-save，可能 1 秒内多次写 | watchdog debounce 500ms，不处理相邻 500ms 内的重复事件 |
| **Vault 路径跨盘符（Windows）** | Windows 路径大小写不敏感，Linux 敏感 | 路径存小写，解析时 normalize |
| **consensus 衰减和 boost 的时序** | 被引用时 boost，之后又衰减，时序不对会震荡 | boost 直接更新 last_boosted_at，decay 基于此时间计算 |
| **FTS5 索引和 SQLite 数据不一致** | 文件删了但 FTS 没更新 | 用 SQLite 事务包住文件写和索引更新，保证原子性 |
| **中文文件名和 id** | `[[中文id]]` 引用是否支持 | Phase 1 只支持 ASCII id（slug 格式），中文 id 转拼音或报错提示 |

---

## 6. 测试策略

### 6.1 分层测试

```
┌─────────────────────────────────────────┐
│  E2E 测试（真实验证）                       │
│  飞书 Bot → API → SQLite → 文件系统        │
│  覆盖：完整命令流程                         │
└─────────────────────────────────────────┘
          ▲
┌─────────────────────────────────────────┐
│  集成测试                                 │
│  API 端点 → ScoringEngine → DB           │
│  FileWatcher → 引用解析 → 评分联动        │
└─────────────────────────────────────────┘
          ▲
┌─────────────────────────────────────────┐
│  单元测试                                 │
│  - ScoringEngine.decay 计算               │
│  - ReferenceParser [[id]] 提取           │
│  - YAML front-matter 解析                 │
│  - 公式逻辑（interest × 0.4 + ...）       │
└─────────────────────────────────────────┘
```

### 6.2 测试覆盖目标

| 模块 | 覆盖率目标 |
|------|-----------|
| scoring_engine | ≥ 95% |
| reference_parser | ≥ 90% |
| api/entities | ≥ 80% |
| api/scores | ≥ 80% |
| 整体 | ≥ 70% |

### 6.3 关键测试用例

**评分引擎：**
```python
def test_decay_half_life():
    """30天半衰期，30天后 interest 应为原来的一半"""
    engine = ScoringEngine()
    original = 10.0
    decayed = engine.calculate_decay(30.0, 30.0)
    assert abs(decayed - 0.5) < 0.001

def test_final_score_formula():
    """final_score = interest × 0.4 + strategy × 0.35 + consensus × 0.25"""
    score = Score(interest=8, strategy=9, consensus=4)
    result = engine.compute_final_score(score)
    expected = 8 * 0.4 + 9 * 0.35 + 4 * 0.25  # = 7.25
    assert abs(result - expected) < 0.001
```

**引用解析：**
```python
def test_reference_extraction():
    content = "这是 [[entity-001]] 和 [[entity-002]] 的核心观点"
    refs = ReferenceParser.extract_ids(content)
    assert set(refs) == {"entity-001", "entity-002"}

def test_self_reference():
    """[[self-id]] 引用自己应过滤"""
    content = "关于 [[entity-001]] 的讨论"
    refs = ReferenceParser.extract_ids(content, current_id="entity-001")
    assert "entity-001" not in refs
```

**E2E（飞书 Bot）：**
```python
def test_q_command_creates_entity():
    bot = FeishuBot()
    response = bot.handle("/q 这是一条测试记录")
    assert response.entity_id is not None
    assert response.category == "Inbox"
```

### 6.4 CI 流水线

```yaml
# .github/workflows/ci.yml
on: [push, pull_request]
jobs:
  quality:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with: { python-version: "3.11" }
      - run: pip install -r requirements.txt
      - run: black --check app/ tests/
      - run: ruff check app/ tests/
      - run: mypy app/ --strict
      - run: pytest tests/ -v --cov=app --cov-report=xml

  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: docker compose up -d
      - run: pytest tests/e2e/ -v
```

---

## 7. 开发规范

### 7.1 Git Flow（issue → 分支 → PR）

```
1. 创建 issue（描述要完整，包含验收标准）
2. 创建分支：git checkout -b feat/{issue-id}-{short-description}
3. 开发 + 测试
4. PR 描述包含：解决什么问题、怎么验证、测试截图
5. CTO review → approve → merge to master
```

**分支命名：**
- `feat/{issue-id}-{description}`
- `fix/{issue-id}-{description}`
- `docs/{issue-id}-{description}`

**PR 规范：**
- Title: `{type}: {简短描述}` （如 `feat: implement scoring engine decay`）
- Description 必须包含测试验证说明
- 最小 PR 原则：一个 PR 只解决一个问题

### 7.2 代码规范

| 规则 | 工具 |
|------|------|
| 格式化 | black（line-length=88） |
| Lint | ruff（select=ALL，ignore=...） |
| 类型检查 | mypy（strict mode） |
| 测试 | pytest + pytest-asyncio |
| 覆盖率 | pytest-cov |

### 7.3 API 规范

- 所有端点返回 `application/json`
- 错误格式：`{"detail": "错误描述", "code": "ERROR_CODE"}`
- 分页：cursor-based，不做 offset 翻页
- 时间格式：ISO 8601（`2026-04-06T02:36:00Z`）
- ID：UUID v4 或 URL-safe slug

### 7.4 FastAPI 路由模块划分原则

每个路由模块不超过 200 行。复杂业务逻辑下沉到 `services/` 或 `core/`。

---

## 8. 待确认事项（Blocking Issues）

在进入 Week 1 之前需要明确：

1. **飞书 Bot 账号归属**：个人版飞书 or 企业版？是否有开放平台管理员权限？
2. **Vault 路径**：Windows 宿主路径固定还是需要可配置？
3. **ngrok 账户**：开发环境 HTTPS 透传谁提供？
4. **数据所有权**：Phase 1 数据本地存储，是否有备份/恢复需求？

---

*CTO 评估完成。技术方案可行，8 周是积极但可达成的目标。关键是 Week 1 先破飞书 Bot 的网络链，扫清最大风险。*
