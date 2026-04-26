# Compass Phase 1 — Technical Design Document

| 字段 | 内容 |
|------|------|
| 版本 | v2.0 |
| 日期 | 2026-04-06 |
| 作者 | CTO (太子) |
| 状态 | 正式接手，待团队评审 |

---

## 1. 系统架构

### 1.1 分层总览

```
┌──────────────────────────────────────────────────────────────┐
│  接入层                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────────┐  │
│  │ 飞书Bot      │  │ Obsidian     │  │ Agent (LLM)      │  │
│  │ (WebSocket)  │  │ 桌面端(人)   │  │ Claude Code 等    │  │
│  └──────┬───────┘  └──────┬───────┘  └─────────┬─────────┘  │
└─────────┼────────────────┼─────────────────────┼────────────┘
          │                │                     │
          │        ┌──────▼───────┐             │
          │        │ FileWatcher  │◄────────────┘
          │        │   (Python)   │  watchdog 监听 Vault
          │        └──────┬───────┘
          │                │ subprocess / JSON-RPC
          │        ┌──────▼───────┐
          │        │  Rust Core   │◄────────── 编译期类型安全
          │        │  (compass-core) │
          │        │  - ScoringEngine  │
          │        │  - ReferenceParser │
          │        │  - DecayCalculator  │
          │        └──────┬───────┘
          │                │
┌─────────▼────────────────▼──────────────────────────────────▼─┐
│  Python 胶水层 (FastAPI)                                         │
│                                                                     │
│  FastAPI                    Agent SDK (OpenAI/Anthropic)          │
│  Tool Use 接口              Agent Runtime                          │
│  REST API (读/写)           LLM Intent Parsing                     │
└────────────────────────────────────────────────────────────────────┘
          │
┌─────────▼──────────────────────────────────────────────────────┐
│  数据层                                                           │
│                                                                     │
│  Obsidian Vault (Markdown 文件，根数据)                             │
│  SQLite (索引、元数据、评分历史、FTS5 倒排)                          │
└────────────────────────────────────────────────────────────────────┘
```

**核心设计原则：**
- **Rust Core** = 评分引擎、引用解析、decay 计算（AI 生成的代码受编译期强制校验）
- **Python 胶水层** = FastAPI REST API、Agent SDK 对接、飞书 Bot WebSocket 粘合
- **通信协议** = subprocess JSON-RPC（Rust 二进制通过 stdin/stdout 通信，简单可靠）

---

### 1.2 技术选型

#### Python 层

| 组件 | 选型 | 理由 |
|------|------|------|
| **语言** | Python 3.11+ | Agent SDK 生态成熟，FastAPI 胶水层 |
| **Web 框架** | FastAPI | 类型安全，OpenAPI 文档，tool use 原生支持 |
| **文件监听** | watchdog | 跨平台，asyncio 集成 |
| **飞书 SDK** | lark-oapi | 官方 SDK，events_api WebSocket |
| **SQLite** | aiosqlite | 异步访问，WAL 模式 |
| **HTTP 客户端** | httpx | 异步，/fetch 依赖 |
| **LLM SDK** | anthropic / openai | Agent Runtime 解耦 |

#### Rust 层

| 组件 | 选型 | 理由 |
|------|------|------|
| **语言** | Rust 1.75+ | 编译期类型安全，AI 生成代码可靠保障 |
| ** crates** | serde (JSON) | 结构化序列化，与 Python 通信 |
| ** crates** | regex | [[id]] 引用解析 |
| ** crates** | chrono | 时间计算，decay 半衰期 |
| ** crates** | tokio | 异步运行时（如果需要并发） |
| ** crates** | rusqlite | SQLite 操作（Rust 端直接读写） |

---

### 1.3 项目结构

```
compass/
├── compass-core/               # Rust 二进制 crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs             # CLI 入口，subprocess 模式
│   │   ├── scoring.rs          # ScoringEngine + DecayCalculator
│   │   ├── reference.rs        # ReferenceParser
│   │   ├── models.rs           # 共享数据结构（与 Python 共用）
│   │   └── rpc.rs              # JSON-RPC server (stdin/stdout)
│   └── tests/
│       ├── scoring_test.rs
│       └── reference_test.rs
│
├── compass-api/                # Python FastAPI 应用
│   ├── pyproject.toml
│   ├── src/
│   │   ├── __init__.py
│   │   ├── main.py             # FastAPI 入口
│   │   ├── config.py            # 环境配置
│   │   ├── api/
│   │   │   ├── entities.py      # /entities 路由
│   │   │   ├── scores.py        # /scores 路由
│   │   │   ├── feed.py          # /feed 路由
│   │   │   ├── agent.py          # /agent/* 路由
│   │   │   └── fetch.py          # /fetch 路由
│   │   ├── core/                # ⚠️ Python 核心逻辑已移至 Rust
│   │   │   ├── rust_client.py   # subprocess 调用 compass-core
│   │   │   └── file_watcher.py   # watchdog 监听 Vault
│   │   ├── services/
│   │   │   ├── feishu_bot.py    # 飞书 Bot WebSocket 处理
│   │   │   └── vault_service.py # Vault 文件操作
│   │   └── db/
│   │       ├── database.py       # SQLite 连接（WAL 模式）
│   │       └── schema.sql        # DDL
│   └── tests/
│       ├── unit/
│       ├── integration/
│       └── e2e/
│
├── vault/                      # Obsidian Vault（运行时挂载）
│   ├── Inbox/
│   ├── Direction/
│   ├── Knowledge/
│   ├── Logs/
│   └── Insights/
│
├── scripts/
│   ├── init_vault.py           # Vault 初始化
│   └── build_rust.sh           # Rust 编译脚本
│
├── requirements.txt
├── Dockerfile
└── README.md
```

**⚠️ 重要变更：** 原 `app/core/scoring_engine.py`、`app/core/reference_parser.py` 已移除，逻辑移至 `compass-core/src/`。Python 层不重复实现 Rust 已有的逻辑。

---

## 2. 数据模型

### 2.1 SQLite Schema

> **不变。** 数据模型存 SQLite，Rust 和 Python 共用同一数据库文件。

```sql
-- 实体表（对应 Obsidian 每个 Markdown 文件）
CREATE TABLE entities (
    id          TEXT PRIMARY KEY,
    file_path   TEXT NOT NULL UNIQUE,
    vault_path  TEXT NOT NULL,
    title       TEXT NOT NULL,
    category    TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    last_boosted_at TEXT,
    has_attachments INTEGER DEFAULT 0,
    attachment_refs TEXT,
    metadata    TEXT
);

-- 评分表
CREATE TABLE scores (
    entity_id   TEXT PRIMARY KEY REFERENCES entities(id),
    interest    REAL DEFAULT 5.0,
    strategy    REAL DEFAULT 5.0,
    consensus   REAL DEFAULT 0.0,
    final_score REAL DEFAULT 0.0,
    interest_half_life_days REAL DEFAULT 30.0,
    strategy_half_life_days REAL DEFAULT 365.0,
    consensus_half_life_days REAL DEFAULT 60.0,
    manual_override INTEGER DEFAULT 0,
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
    event_type  TEXT NOT NULL,
    trigger     TEXT,
    created_at  TEXT NOT NULL
);

-- FTS5 全文索引
CREATE VIRTUAL TABLE entities_fts USING fts5(
    id, title, content, category,
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

### 2.2 共享数据结构（Rust ↔ Python）

> 通过 JSON 序列化在 Rust subprocess 和 Python 之间传递。

```json
// ScoringInput → Rust Core → ScoringOutput
{
  "interest": 7.0,
  "strategy": 9.0,
  "consensus": 3.0,
  "last_boosted_at": "2026-04-01T12:00:00Z",
  "interest_half_life_days": 30.0,
  "strategy_half_life_days": 365.0,
  "consensus_half_life_days": 60.0
}

// ReferenceInput → Rust Core → ReferenceOutput
{
  "content": "这是 [[entity-001]] 和 [[entity-002]] 的核心观点",
  "current_entity_id": null
}
```

### 2.3 评分计算公式

> **不变。** 公式在 Rust 中实现。

```
final_score = (interest × 0.4 + strategy × 0.35 + consensus × 0.25) × decay_factor

decay_factor = 0.5 ^ (days_since_last_event / half_life_days)
```

---

## 3. 模块设计

### 3.1 Rust Core (`compass-core`)

#### 3.1.1 ScoringEngine

**输入：** `ScoringInput` (JSON via stdin)
**输出：** `ScoringOutput` (JSON via stdout)

```rust
#[derive(Deserialize)]
pub struct ScoringInput {
    pub interest: f64,
    pub strategy: f64,
    pub consensus: f64,
    pub last_boosted_at: String,  // ISO 8601
    pub interest_half_life_days: f64,
    pub strategy_half_life_days: f64,
    pub consensus_half_life_days: f64,
}

#[derive(Serialize)]
pub struct ScoringOutput {
    pub final_score: f64,
    pub decay_factor: f64,
    pub days_elapsed: f64,
}
```

**DecayCalculator 单元测试（Rust 编译期保障）：**
```rust
#[test]
fn test_decay_half_life() {
    let decay = DecayCalculator::new(30.0);
    assert!((decay.factor(30.0) - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_final_score_formula() {
    let input = ScoringInput {
        interest: 8.0, strategy: 9.0, consensus: 4.0,
        last_boosted_at: "2026-04-06T00:00:00Z".into(),
        interest_half_life_days: 30.0,
        strategy_half_life_days: 365.0,
        consensus_half_life_days: 60.0,
    };
    let output = ScoringEngine::compute(input);
    // 8*0.4 + 9*0.35 + 4*0.25 = 3.2 + 3.15 + 1.0 = 7.35
    assert!((output.final_score - 7.35).abs() < f64::EPSILON);
}
```

#### 3.1.2 ReferenceParser

**正则：** `\[\[([a-zA-Z0-9_-]+)\]\]`

```rust
#[test]
fn test_reference_extraction() {
    let content = "这是 [[entity-001]] 和 [[entity-002]] 的核心观点";
    let refs = ReferenceParser::extract_ids(content, None);
    assert_eq!(refs, vec!["entity-001", "entity-002"]);
}

#[test]
fn test_self_reference_filtered() {
    let content = "关于 [[entity-001]] 的讨论";
    let refs = ReferenceParser::extract_ids(content, Some("entity-001"));
    assert!(!refs.contains(&"entity-001"));
}
```

#### 3.1.3 JSON-RPC Server

subprocess 通信协议（stdin/stdout）：

```json
// Request
{"jsonrpc": "2.0", "method": "compute_score", "params": {...}, "id": 1}
{"jsonrpc": "2.0", "method": "parse_refs", "params": {...}, "id": 2}

// Response
{"jsonrpc": "2.0", "result": {...}, "id": 1}
{"jsonrpc": "2.0", "result": {...}, "id": 2}
```

**为什么用 subprocess 而不是 gRPC：**
- 零依赖，Python `subprocess.run()` 直接调用
- JSON-RPC over stdin/stdout，调试简单（直接 terminal 测试）
- Phase 1 QPS 低，subprocess fork 开销可忽略

---

### 3.2 Python → Rust 通信层 (`compass-api/src/core/rust_client.py`)

```python
import subprocess
import json
from dataclasses import dataclass
from typing import Optional


@dataclass
class RustClient:
    binary_path: str

    def compute_score(
        self,
        interest: float,
        strategy: float,
        consensus: float,
        last_boosted_at: str,
        interest_half_life_days: float = 30.0,
        strategy_half_life_days: float = 365.0,
        consensus_half_life_days: float = 60.0,
    ) -> dict:
        payload = {
            "jsonrpc": "2.0",
            "method": "compute_score",
            "params": {
                "interest": interest,
                "strategy": strategy,
                "consensus": consensus,
                "last_boosted_at": last_boosted_at,
                "interest_half_life_days": interest_half_life_days,
                "strategy_half_life_days": strategy_half_life_days,
                "consensus_half_life_days": consensus_half_life_days,
            },
            "id": 1,
        }
        result = subprocess.run(
            [self.binary_path],
            input=json.dumps(payload).encode(),
            capture_output=True,
            timeout=5,
        )
        response = json.loads(result.stdout)
        return response.get("result", {})

    def parse_refs(self, content: str, current_id: Optional[str] = None) -> list[str]:
        payload = {
            "jsonrpc": "2.0",
            "method": "parse_refs",
            "params": {"content": content, "current_entity_id": current_id},
            "id": 2,
        }
        result = subprocess.run(
            [self.binary_path],
            input=json.dumps(payload).encode(),
            capture_output=True,
            timeout=5,
        )
        response = json.loads(result.stdout)
        return response.get("result", {}).get("refs", [])
```

**Rust 二进制构建：**
```bash
# build_rust.sh
cd compass-core
cargo build --release
cp target/release/compass-core ../compass-api/bin/
```

---

### 3.3 FileWatcher（Python 层）

```python
# compass-api/src/core/file_watcher.py
from watchdog.observers import Observer
from pathlib import Path
import asyncio

class VaultWatcher:
    def __init__(self, vault_path: Path, rust_client: RustClient):
        self.vault_path = vault_path
        self.rust_client = rust_client
        self._debounce: dict[Path, asyncio.Task] = {}

    def start(self):
        handler = DebouncedEventHandler(self.rust_client, debounce_ms=500)
        observer = Observer()
        observer.schedule(handler, str(self.vault_path), recursive=True)
        observer.start()
```

**触发流程：**
```
文件保存 → FileWatcher 接收事件 → debounce 500ms → 
  1. Python 解析 front-matter (YAML) → 提取 id/title/category
  2. Python 读取文件 content
  3. Python 调用 rust_client.parse_refs() → 获取 [[id]] 引用列表
  4. Python 调用 rust_client.compute_score() → 获取 final_score
  5. Python 写入 SQLite (entities, scores, references 表)
```

---

### 3.4 飞书 Bot + OpenClaw Agent（Phase 1 接入架构）

**实际架构（Phase 1）：**

```
飞书消息
    ↓
OpenClaw Agent（现成 Bot，LLM 驱动）
    ↓ Tool Call（OpenClaw Skill）
compass-api（FastAPI REST）
    ↓ subprocess JSON-RPC
Rust Core（compass-core）
    ↓
SQLite + Obsidian Vault
```

**关键说明：**
- 飞书 Bot 是 OpenClaw 内置能力（lark-oapi WebSocket 由 OpenClaw 处理）
- OpenClaw Agent 负责意图理解 + 工具调用
- **OpenClaw Skill**（封装 HTTP 调用）是 Agent 访问 compass-api 的唯一途径
- compass-api 对外暴露 REST 接口，Skill 负责将 REST 映射为 Agent Tool
- Phase 2：OpenClaw Skill 逐步迁移到 MCP Server（Skill 废弃）

**⚠️ 网络：OpenClaw Gateway 自带 HTTPS 回调，无需额外 ngrok。**

---

### 3.5 FastAPI 工具层（Python 层）

```python
# compass-api/src/api/agent.py
from fastapi import FastAPI
from pydantic import BaseModel

class AgentContextRequest(BaseModel):
    task: str
    top_k: int = 5

class AgentContextResponse(BaseModel):
    context: list[dict]
    suggested_entities: list[str]
    reasoning: str  # Agent 可解释性

@app.post("/agent/context")
async def get_context(req: AgentContextRequest) -> AgentContextResponse:
    # 1. FTS5 搜索相关实体
    entities = await search_entities(req.task, limit=req.top_k * 2)
    # 2. 对每个实体调用 Rust compute_score
    scored = []
    for e in entities:
        score_result = rust_client.compute_score(
            interest=e["scores"]["interest"],
            strategy=e["scores"]["strategy"],
            consensus=e["scores"]["consensus"],
            last_boosted_at=e.get("last_boosted_at", ""),
        )
        e["final_score"] = score_result["final_score"]
        scored.append(e)
    # 3. 按 final_score 排序取 top_k
    scored.sort(key=lambda x: x["final_score"], reverse=True)
    return AgentContextResponse(
        context=scored[:req.top_k],
        suggested_entities=[s["id"] for s in scored[req.top_k:req.top_k*2]],
        reasoning=f"基于 strategy 加权 {scored[0]['scores']['strategy']} 和 consensus {scored[0]['scores']['consensus']} 筛选",
    )
```

---

### 3.6 OpenClaw Skill（Phase 1 Agent 接入方式）

OpenClaw Skill 是 OpenClaw Agent 访问 compass-api 的封装层。
Agent 通过 Skill 定义的 Tool 与 Compass 交互，不直接调用 REST API。

**项目结构：**
```
compass-skill/                  # OpenClaw Skill 包
├── SKILL.md                    # Skill 定义（OpenClaw 规范）
├── scripts/
│   ├── search.py               # 封装 GET /entities/search
│   ├── create.py               # 封装 POST /entities
│   ├── score.py                # 封装 POST /scores/update
│   ├── context.py              # 封装 POST /agent/context
│   └── fetch.py                 # 封装 POST /fetch
└── requirements.txt
```

**Tool 列表（Phase 1）：**

| Tool Name | 调用 API | 用途 |
|-----------|---------|------|
| `compass_search` | `GET /entities/search` | FTS5 全文搜索 |
| `compass_create` | `POST /entities` | 快速记录到 Inbox |
| `compass_score` | `POST /scores/update` | 手动调整评分 |
| `compass_context` | `POST /agent/context` | Agent 上下文注入 |
| `compass_fetch` | `POST /fetch` | URL 内容抓取（Phase 1 Agent 自己清洗）|

**Tool 与 REST 端点一一对应，Skill 是 Agent 的唯一调用渠道。**

**Phase 2 演进：**
- MCP Server 适配层加入 compass-api
- Skill 工具逐步迁移为 MCP Tools
- Skill 进入维护模式，最终废弃

---

### 3.7 接口演进路线（REST → MCP）

| 阶段 | 接口形式 | Agent 接入方式 |
|------|---------|---------------|
| **Phase 1** | FastAPI REST | OpenClaw Skill（封装 HTTP 调用） |
| **Phase 2** | REST + MCP Server 双接口 | OpenClaw Skill → MCP Tools 迁移 |
| **稳定期** | MCP Server 为主 | REST 可选关闭 |

**OpenClaw Skill 演进路径：**
```
Phase 1: OpenClaw Agent → Skill（HTTP）→ compass-api REST
Phase 2: OpenClaw Agent → Skill（MCP 兼容）→ compass-api MCP Server
Phase 3: OpenClaw Agent → MCP Native → compass-api MCP Server
```

---

### 3.8 /fetch 与 /clean 接口（Phase 1/2 行为）

#### 3.8.1 /fetch 接口

```python
@app.post("/fetch")
async def fetch_url(url: str) -> FetchResult:
    """
    Phase 1: Agent 自己调 firecrawl/exa API
              Agent 负责清洗 + 格式化
              调用 POST /entities 写入

    Phase 2: 全自动 pipeline
              /fetch → 内部 firecrawl → /clean → /entities
    """
    raise NotImplementedError(
        "Phase 1: Agent calls firecrawl/exa directly. "
        "Agent handles cleaning + formatting before POST /entities."
    )
```

**Phase 1 调用链：**
```
用户: "把这个页面存进来 https://..."
    → OpenClaw Agent 理解意图
    → Agent 调用 firecrawl/exa API 获取正文
    → Agent 清洗 + 格式化 Markdown
    → Agent 调用 POST /entities → 写入 Vault
```

#### 3.8.2 /clean 接口（Phase 2 预埋）

```python
@app.post("/clean")
async def clean_content(raw: str, source: str = "manual") -> CleanedContent:
    """
    Phase 1: NotImplemented
    Phase 2: 数据清洗 pipeline

    输入：原始文本 / HTML
    输出：清洗后的 Markdown
    """
    raise NotImplementedError("Phase 2 pipeline")


class CleanedContent(BaseModel):
    title: str
    content: str           # 清洗后的 Markdown
    summary: str | None    # 可选摘要
    tags: list[str]       # 可选自动标签
    source_url: str | None
    cleaned_at: str        # ISO 8601
```

**Phase 2 全自动流程：**
```
/fetch → 提取正文 → /clean → 清洗 → /entities → 写入 Vault
```

---

## 4. 开发周期评估

### 4.1 8 周能否交付？

**结论：能，但 Rust 学习曲线需要计入风险。**

| 周次 | 任务 | 风险等级 |
|------|------|---------|
| Week 1 | Rust 环境 + compass-core 骨架 + JSON-RPC 通信 | 🟡 中（Rust 上手成本） |
| Week 2 | SQLite Schema + FileWatcher + Python-Rust 集成 | 🟢 低 |
| Week 3-4 | Rust ScoringEngine + ReferenceParser + 单元测试 | 🟢 低（类型安全） |
| Week 5-6 | FastAPI + Agent API + 飞书 Bot | 🔴 高（飞书 OAuth + HTTPS） |
| Week 7 | 集成测试 + 端到端验收 | 🟡 中 |
| Week 8 | 文档 + 备份方案 + 发布 | 🟢 低 |

**Week 1 必须交付：**
1. Rust 二进制能通过 `cargo build --release` 编译
2. JSON-RPC 协议在 Rust 和 Python 间跑通
3. `compute_score` 和 `parse_refs` 两个核心方法端到端可用
4. 飞书 Bot WebSocket 骨架跑通（硬编码响应先跑通）

---

## 5. 技术方案评估

### 5.1 Rust 核心的优势

| 优势 | 说明 |
|------|------|
| **编译期类型保障** | AI 生成的 Rust 代码有编译器强制校验，幻觉代码在编译期暴露 |
| **内存安全** | 无 GC，无数据竞争，AI 生成代码不会因内存问题崩溃 |
| **执行速度** | scoring/decay 计算比 Python 快 10-100x |
| **可靠性** | Rust 测试框架 + 编译检查，核心逻辑稳定 |

### 5.2 潜在坑

| 坑 | 说明 | 解法 |
|----|------|------|
| **Rust 学习曲线** | CTO 第一次写 Rust，可能会遇到 borrow checker 调试 | 给 Rust 代码 2 天上手时间，Week 1 不追求优雅 |
| **subprocess 延迟** | 每次 scoring 调用都 fork 进程 | Phase 1 可接受；Phase 2 可改为长连接 socket 或 tokio-based RPC |
| **Python-Rust 类型映射** | JSON 序列化有精度损失（f64 → Python float） | 评分保留 2 位小数，超出精度直接截断 |
| **Windows 编译** | Rust 在 Windows 上交叉编译需要 target | 提供预编译 binary，或 Docker multi-stage build |
| **飞书 Bot HTTPS** | 同 Python 版本，高风险 | Day 1 就搭 ngrok |

### 5.3 技术决策总结

| 决策 | 选择 | 理由 |
|------|------|------|
| 核心逻辑语言 | Rust | 编译期类型安全，AI 代码可靠 |
| 胶水层语言 | Python | Agent SDK 生态，快速交付 |
| 通信协议 | subprocess JSON-RPC | 简单，零依赖，Phase 1 够用 |
| 数据库 | SQLite + FTS5 | 不变，单用户 MVP 最优 |
| 文件监听 | Python watchdog | 不变，成熟方案 |

---

## 6. 测试策略

### 6.1 分层测试

```
┌─────────────────────────────────────────────┐
│  Rust 单元测试 (compass-core)                  │
│  cargo test                                   │
│  - ScoringEngine 公式验证                      │
│  - ReferenceParser [[id]] 提取                  │
│  - DecayCalculator 半衰期验证                   │
│  - JSON-RPC 序列化/反序列化                     │
│  覆盖率目标: ≥ 95%                            │
└─────────────────────────────────────────────┘
          ▲
┌─────────────────────────────────────────────┐
│  Python 单元测试 (compass-api)                │
│  pytest                                       │
│  - RustClient 调用封装                         │
│  - FileWatcher debounce 逻辑                   │
│  - FastAPI 端点响应                            │
│  覆盖率目标: ≥ 80%                            │
└─────────────────────────────────────────────┘
          ▲
┌─────────────────────────────────────────────┐
│  集成测试                                      │
│  Rust binary ↔ Python subprocess              │
│  - compute_score: Python 传入 → Rust 计算 → Python 接收 │
│  - parse_refs: Python 传入 → Rust 解析 → Python 接收   │
└─────────────────────────────────────────────┘
          ▲
┌─────────────────────────────────────────────┐
│  E2E 测试（飞书 Bot 人工验收）                  │
│  Bot → Agent (LLM) → FastAPI → Rust Core     │
└─────────────────────────────────────────────┘
```

### 6.2 Rust 测试用例

```rust
// compass-core/src/scoring.rs

#[test]
fn test_decay_half_life_exact() {
    let decay = DecayCalculator::new(30.0);
    let factor = decay.factor(30.0);
    assert!((factor - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_decay_quarter_life() {
    // 15天 ≈ 1/4 衰减 (0.5^(15/30) = 0.5^0.5 ≈ 0.707)
    let decay = DecayCalculator::new(30.0);
    let factor = decay.factor(15.0);
    assert!((factor - 0.7071).abs() < 0.001);
}

#[test]
fn test_final_score_zero_days() {
    // 0 天衰减，decay_factor = 1.0
    let input = ScoringInput {
        interest: 10.0, strategy: 10.0, consensus: 10.0,
        last_boosted_at: chrono::Utc::now().to_rfc3339(),
        interest_half_life_days: 30.0,
        strategy_half_life_days: 365.0,
        consensus_half_life_days: 60.0,
    };
    let output = ScoringEngine::compute(input);
    assert!((output.final_score - 10.0).abs() < f64::EPSILON);
    assert!((output.decay_factor - 1.0).abs() < f64::EPSILON);
}

#[test]
fn test_final_score_formula_weights() {
    let input = ScoringInput {
        interest: 8.0, strategy: 9.0, consensus: 4.0,
        last_boosted_at: chrono::Utc::now().to_rfc3339(),
        interest_half_life_days: 30.0,
        strategy_half_life_days: 365.0,
        consensus_half_life_days: 60.0,
    };
    let output = ScoringEngine::compute(input);
    // decay_factor = 1.0 (0 days)
    // final = 8*0.4 + 9*0.35 + 4*0.25 = 3.2 + 3.15 + 1.0 = 7.35
    assert!((output.final_score - 7.35).abs() < f64::EPSILON);
}
```

### 6.3 CI 流水线

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.75" }
      - run: cargo test --workspace
      - run: cargo clippy -- -D warnings
      - run: cargo tarpaulin --workspace --out xml
        if: github.event_name == 'push'

  python:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with: { python-version: "3.11" }
      - run: pip install -e compass-api
      - run: black --check compass-api/src/
      - run: ruff check compass-api/src/
      - run: mypy compass-api/src/ --strict
      - run: pytest compass-api/tests/ -v --cov=compass_api

  integration:
    needs: [rust, python]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --release -p compass-core
      - run: pip install -e compass-api
      - run: pytest compass-api/tests/integration/ -v
```

---

## 7. 开发规范

### 7.1 Git Flow

```
1. 创建 issue（含验收标准）
2. 创建分支：git checkout -b feat/{issue-id}-{description}
3. 开发 + 测试
4. PR → review → merge
```

**分支命名：** `feat/{issue-id}-{description}`

### 7.2 代码规范

**Rust：**
- `cargo fmt` + `cargo clippy`
- 所有公开接口必须有文档注释 `///`
- 单元测试直接在同文件 `#[cfg(test)]` 模块

**Python：**
- black（line-length=88）
- ruff（select=ALL）
- mypy（strict mode）
- pytest + pytest-asyncio

---

## 8. 待确认事项

1. **飞书 Bot 账号归属**：个人版 or 企业版？开放平台管理员权限？
2. **Vault 路径**：Windows 宿主路径固定还是可配置？
3. **ngrok 账户**：开发环境 HTTPS 透传
4. **Rust 经验**：CTO（我）第一次写 Rust，Week 1 预留 2 天踩坑

---

## 9. Phase 2 任务拆解（TDD 增量）

> **目标**：将 PRD v2.1 的 Phase 2 大任务拆分为可独立测试、逐步交付的小任务。
> **原则**：每个任务产出一个可运行的 API/功能，TDD 先行（测试 → 实现 → 重构）。

### 9.1 任务总览

| 任务 ID | 任务名称 | 分支 | 优先级 | 依赖 | 预估工时 |
|---------|----------|------|--------|------|----------|
| **P2-Entity-1** | `GET /entities` 实体列表（分页+过滤） | `feat/p2-entity-list` | P0 | 无 | 4h |
| **P2-Graph-1** | `GET /graph/neighbors/{id}` 基础邻居查询 | `feat/p2-graph-1-neighbors-basic` | P0 | 无 | 4h |
| **P2-Graph-2** | `GET /graph/neighbors/{id}?depth=N` 深度查询 | `feat/p2-graph-2-neighbors-depth` | P0 | P2-Graph-1 | 3h |
| **P2-Graph-3** | `GET /graph/neighbors/{id}?min_strength=X` 强度过滤 | `feat/p2-graph-3-neighbors-filter` | P1 | P2-Graph-1 | 3h |
| **P2-Graph-4** | `GET /graph/path?from=X&to=Y` 最短路径 | `feat/p2-graph-4-path` | P1 | P2-Graph-1 | 6h |
| **P2-Fetch-1** | `POST /fetch` URL 抓取（原始内容） | `feat/p2-fetch-1-url-fetch` | P0 | 无 | 4h |
| **P2-Fetch-2** | `POST /fetch/clean` 内容清洗结构化 | `feat/p2-fetch-2-clean` | P0 | P2-Fetch-1 | 6h |
| **P2-Fetch-3** | `POST /fetch/save` 清洗结果写入 Vault | `feat/p2-fetch-3-save` | P0 | P2-Fetch-2 | 4h |
| **P2-Search-1** | `GET /search?q=...` 语义搜索（FAISS） | `feat/p2-search-semantic` | P1 | 无 | 8h |
| **P2-MCP-1** | MCP Server 适配层（基础 Tools） | `feat/p2-mcp-server` | P3 | 无 | 6h |
| **P2-Timeline-1** | `PATCH /entities/{id}/access` 访问记录 | `feat/p2-timeline-access` | P1 | P2-Entity-1 | 2h |
| **P2-Timeline-2** | `GET /entities/{id}/timeline` 时间线查询 | `feat/p2-timeline-query` | P1 | P2-Timeline-1 | 3h |
| **P2-History-1** | `POST /scores/update` 自动写入 score_history | `feat/p2-score-history` | P1 | P2-Entity-1 | 2h |
| **P2-History-2** | `GET /entities/{id}/score/history` 评分趋势 | `feat/p2-score-history` | P1 | P2-History-1 | 3h |
| **P2-Insight-1** | Insight 实体 CRUD + maturity 状态机 | `feat/p2-insight-engine` | P2 | P2-Entity-1 | 4h |
| **P2-Insight-2** | Insight 成熟度演化触发器 | `feat/p2-insight-engine` | P2 | P2-Graph-1 + P2-Insight-1 | 6h |
| **P2-Insight-3** | Insight → Knowledge 导出降级 | `feat/p2-insight-engine` | P2 | P2-Insight-2 | 3h |
| **P2-Ref-1** | 引用强度自动计算（共同邻居） | `feat/p2-ref-intelligence` | P2 | P2-Graph-1 | 6h |
| **P2-Ref-2** | 双向引用自动维护 | `feat/p2-ref-intelligence` | P2 | P2-Graph-1 | 3h |
| **P2-Decay-1** | `PATCH /entities/{id}/decay-config` 个性化半衰期 | `feat/p2-decay-tuner` | P2 | P2-Entity-1 | 3h |
| **P2-Decay-2** | `GET /entities/{id}/decay-preview` Decay 预览 | `feat/p2-decay-tuner` | P2 | P2-Decay-1 | 3h |
| **P2-Decay-3** | Decay 模拟器（未来90天衰减曲线） | `feat/p2-decay-tuner` | P2 | P2-Decay-2 | 4h |

### 9.2 Entity List API（P2-Entity-1）

> **Issue #44**：当前 Compass API 缺少无需 query 参数即可枚举 vault 中所有实体的端点。
> `GET /entities/search` 需要 `q` 参数（空字符串返回空结果），`GET /entities/top` 只能按分数排序，无法满足枚举需求。
> 本任务新增 `GET /entities` 端点，支持无查询条件的全量列表查询。

#### API 规格

```
GET /entities

Query Parameters:
  type:     string (optional) — 过滤实体类型：knowledge | case | log | insight
  min_score: number (optional, default=0) — 最低综合分数过滤
  tags:     string[] (optional) — 标签过滤，AND 逻辑
  limit:    integer (optional, default=20, max=100) — 每页条数
  offset:   integer (optional, default=0) — 偏移量

Response 200:
{
  "items": [
    {
      "id": "know-000001",
      "title": "实体标题",
      "entity_type": "knowledge",
      "category": "架构层",
      "vault_path": "Inbox/note.md",
      "final_score": 72.5,
      "tags": ["#数学", "#架构"],
      "created_at": "2026-04-01T08:00:00Z",
      "updated_at": "2026-04-06T12:00:00Z"
    }
  ],
  "total": 150,
  "has_more": true
}
```

#### 数据库层实现

```python
# compass-api/src/db/database.py

async def list_entities(
    self,
    entity_type: Optional[str] = None,
    min_score: float = 0.0,
    tags: Optional[list[str]] = None,
    limit: int = 20,
    offset: int = 0,
) -> tuple[list[dict], int]:
    """Return paginated list of entities with optional filters.

    Returns (items, total_count).
    """
    conditions = ["s.final_score >= ?", "e.status = 'active'"]
    params: list = [min_score]

    if entity_type:
        conditions.append("e.entity_type = ?")
        params.append(entity_type)

    # Tag filtering via subquery on taggings table
    if tags:
        for tag in tags:
            conditions.append(
                "e.id IN (SELECT entity_id FROM taggings WHERE tag = ?)"
            )
            params.append(tag)

    where_clause = " AND ".join(conditions)

    # Total count (without limit/offset)
    count_sql = f"""
        SELECT COUNT(DISTINCT e.id)
        FROM entities e
        JOIN scores s ON e.id = s.entity_id
        WHERE {where_clause}
    """
    async with self.conn.execute(count_sql, params) as cur:
        total = (await cur.fetchone())[0]

    # Paginated items
    sql = f"""
        SELECT DISTINCT e.id, e.title, e.entity_type, e.category,
               e.vault_path, s.final_score, e.created_at, e.updated_at
        FROM entities e
        JOIN scores s ON e.id = s.entity_id
        WHERE {where_clause}
        ORDER BY s.final_score DESC, e.updated_at DESC
        LIMIT ? OFFSET ?
    """
    params.extend([limit, offset])
    async with self.conn.execute(sql, params) as cur:
        rows = await cur.fetchall()

    items = [dict(row) for row in rows]

    # Attach tags per entity (separate query; acceptable for small result sets)
    for item in items:
        async with self.conn.execute(
            'SELECT tag FROM taggings WHERE entity_id = ?', (item["id"],)
        ) as cur:
            item["tags"] = [row[0] for row in await cur.fetchall()]

    return items, total
```

#### 测试用例

```python
# tests/api/test_entities.py
import pytest
from httpx import AsyncClient, ASGITransport
from src.main import app

@pytest.fixture
async def client():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac

@pytest.fixture
async def seeded_db(isolated_db):
    """Seeded with 3 entities of different types and scores."""
    await seed_entity(isolated_db, "know-001", score=80.0, etype="knowledge")
    await seed_entity(isolated_db, "case-001", score=60.0, etype="case")
    await seed_entity(isolated_db, "know-002", score=40.0, etype="knowledge")
    return isolated_db


async def test_list_entities_empty(client, isolated_db):
    """No entities → empty list with total=0."""
    response = await client.get("/entities")
    assert response.status_code == 200
    data = response.json()
    assert data["items"] == []
    assert data["total"] == 0
    assert data["has_more"] is False


async def test_list_entities_returns_all(client, seeded_db):
    """No filters → returns all seeded entities, sorted by score desc."""
    response = await client.get("/entities")
    assert response.status_code == 200
    data = response.json()
    assert data["total"] == 3
    assert len(data["items"]) == 3
    assert data["items"][0]["id"] == "know-001"  # highest score
    assert data["has_more"] is False


async def test_list_entities_pagination(client, seeded_db):
    """limit=2, offset=0 → first 2 items, has_more=True."""
    response = await client.get("/entities?limit=2&offset=0")
    assert response.status_code == 200
    data = response.json()
    assert len(data["items"]) == 2
    assert data["total"] == 3
    assert data["has_more"] is True


async def test_list_entities_pagination_offset_end(client, seeded_db):
    """offset=2 → last item, has_more=False."""
    response = await client.get("/entities?limit=20&offset=2")
    assert response.status_code == 200
    data = response.json()
    assert len(data["items"]) == 1
    assert data["has_more"] is False


async def test_list_entities_filter_by_type(client, seeded_db):
    """type=knowledge → only knowledge entities."""
    response = await client.get("/entities?type=knowledge")
    assert response.status_code == 200
    data = response.json()
    assert all(item["entity_type"] == "knowledge" for item in data["items"])
    assert data["total"] == 2


async def test_list_entities_filter_by_min_score(client, seeded_db):
    """min_score=50 → only entities with score >= 50."""
    response = await client.get("/entities?min_score=50")
    assert response.status_code == 200
    data = response.json()
    assert all(item["final_score"] >= 50 for item in data["items"])
    assert data["total"] == 2


async def test_list_entities_invalid_type(client, seeded_db):
    """Invalid entity_type → 422 validation error."""
    response = await client.get("/entities?type=invalid")
    assert response.status_code == 422


async def test_list_entities_limit_exceeds_max(client, seeded_db):
    """limit > 100 → 422 validation error."""
    response = await client.get("/entities?limit=200")
    assert response.status_code == 422
```

#### 验收标准

- [ ] `GET /entities` 无任何 query 参数时返回所有实体（分页）
- [ ] `type` 参数正确过滤实体类型
- [ ] `min_score` 参数正确过滤最低分数
- [ ] `tags` 参数正确过滤标签（AND 逻辑）
- [ ] `limit` / `offset` 分页正确，`has_more` 准确
- [ ] 返回结果按 `final_score` 降序排列
- [ ] 每个 item 包含 `id`、`title`、`entity_type`、`category`、`vault_path`、`final_score`、`tags`、`created_at`、`updated_at`
- [ ] 空结果返回 `items=[]`、`total=0`、`has_more=False`（200）
- [ ] `limit > 100` 返回 422
- [ ] 无效 `type` 值返回 422
- [ ] `offset` 超出范围不报错，返回空 `items`

#### API Handler 实现要点

```python
# compass-api/src/api/entities.py

@router.get("", response_model=EntityListResponse)
async def list_entities(
    type: Annotated[
        Optional[str],
        Query(description="实体类型：knowledge | case | log | insight"),
    ] = None,
    min_score: Annotated[
        float,
        Query(ge=0, le=100, description="最低综合分数"),
    ] = 0.0,
    tags: Annotated[
        Optional[list[str]],
        Query(description="标签过滤（AND 逻辑）"),
    ] = None,
    limit: Annotated[
        int,
        Query(ge=1, le=100, description="每页条数"),
    ] = 20,
    offset: Annotated[
        int,
        Query(ge=0, description="偏移量"),
    ] = 0,
    db: Annotated[Database, Depends(get_db)] = None,
) -> EntityListResponse:
    """List all entities with optional filters and pagination."""
    if type is not None and type not in ("knowledge", "case", "log", "insight"):
        raise HTTPException(status_code=422, detail="Invalid entity_type")

    items, total = await db.list_entities(
        entity_type=type,
        min_score=min_score,
        tags=tags,
        limit=limit,
        offset=offset,
    )
    has_more = (offset + limit) < total
    return EntityListResponse(items=items, total=total, has_more=has_more)
```

#### 工时：4h

---

### 9.3 Graph API 详细设计

#### 数据模型依据

Graph API 底层使用现有 `refs` 表（已在 Phase 1 SQLite Schema 中定义）：

```sql
-- refs 表（已在 schema.sql）
CREATE TABLE refs (
    source_id  TEXT,
    target_id  TEXT,
    ref_type   TEXT,
    strength   REAL DEFAULT 1.0,   -- 0.0~1.0，边强度
    context    TEXT,
    created_at TIMESTAMP,
    PRIMARY KEY (source_id, target_id, ref_type)
);
```

#### P2-Graph-1：`GET /graph/neighbors/{id}` — 基础邻居查询

**目标：** 返回一个实体的所有直接邻居（入边 + 出边）。

**API 规格：**

```
GET /graph/neighbors/{id}

Path Parameters:
  id: string  — 实体 ID（如 "know-000001"）

Response 200:
{
  "nodes": [
    {
      "id": "know-000002",
      "title": "实体标题",
      "entity_type": "knowledge",
      "score_composite": 72.5
    }
  ],
  "edges": [
    {
      "source": "know-000001",
      "target": "know-000002",
      "ref_type": "cites",
      "strength": 1.0,
      "direction": "outgoing"
    }
  ],
  "total_neighbors": 5
}
```

**验收标准：**
- [ ] 给定任意实体 ID，返回所有直接相连的出边邻居（source=该实体）
- [ ] 同时返回所有入边邻居（target=该实体）
- [ ] 每个节点含 `id`、`title`、`entity_type`、`score_composite`
- [ ] 每条边含 `source`、`target`、`ref_type`、`strength`、`direction`
- [ ] 不存在的实体返回 404
- [ ] 数据库无邻居时返回空数组（200）

**测试用例：**

```python
# tests/api/test_graph.py
import pytest
from httpx import AsyncClient, ASGITransport
from src.main import app

@pytest.fixture
async def client():
    transport = ASGITransport(app=app)
    async with AsyncClient(transport=transport, base_url="http://test") as ac:
        yield ac

@pytest.mark.asyncio
async def test_neighbors_returns_direct_links(client, populated_db):
    """实体 A → B → C（A 引用 B），B 的 neighbors 包含 A 和 C"""
    # Arrange: 在测试 fixture 中 setup refs: A→B, C→B
    entity_b_id = "know-000002"
    # Act
    resp = await client.get(f"/graph/neighbors/{entity_b_id}")
    # Assert
    assert resp.status_code == 200
    data = resp.json()
    node_ids = {n["id"] for n in data["nodes"]}
    assert "know-000001" in node_ids  # incoming
    assert "know-000003" in node_ids  # outgoing
    edges_dir = {e["direction"] for e in data["edges"]}
    assert "incoming" in edges_dir
    assert "outgoing" in edges_dir

@pytest.mark.asyncio
async def test_neighbors_404_for_nonexistent(client):
    resp = await client.get("/graph/neighbors/nonexistent-id")
    assert resp.status_code == 404

@pytest.mark.asyncio
async def test_neighbors_empty_returns_empty_lists(client, isolated_db):
    resp = await client.get("/graph/neighbors/know-999999")
    assert resp.status_code == 200
    data = resp.json()
    assert data["nodes"] == []
    assert data["edges"] == []
```

**实现方案：**

```python
# compass-api/src/api/graph.py
from fastapi import APIRouter, Depends, HTTPException
from pydantic import BaseModel
from typing import Optional

from src.db.database import Database, get_db

router = APIRouter(prefix="/graph", tags=["graph"])


class GraphNode(BaseModel):
    id: str
    title: str
    entity_type: str
    score_composite: Optional[float] = None


class GraphEdge(BaseModel):
    source: str
    target: str
    ref_type: str
    strength: float
    direction: str  # "incoming" | "outgoing"


class NeighborsResponse(BaseModel):
    nodes: list[GraphNode]
    edges: list[GraphEdge]
    total_neighbors: int


@router.get("/neighbors/{entity_id}", response_model=NeighborsResponse)
async def get_neighbors(
    entity_id: str,
    db: Database = Depends(get_db),
) -> NeighborsResponse:
    # 验证实体存在
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    # OUTGOING: refs WHERE source_id = entity_id
    out_cur = await db.conn.execute(
        """
        SELECT r.target_id, r.ref_type, r.strength, e.title, e.entity_type, s.final_score
        FROM refs r
        JOIN entities e ON e.id = r.target_id
        LEFT JOIN scores s ON s.entity_id = r.target_id
        WHERE r.source_id = ?
        """,
        (entity_id,),
    )
    out_rows = await out_cur.fetchall()

    # INCOMING: refs WHERE target_id = entity_id
    in_cur = await db.conn.execute(
        """
        SELECT r.source_id, r.ref_type, r.strength, e.title, e.entity_type, s.final_score
        FROM refs r
        JOIN entities e ON e.id = r.source_id
        LEFT JOIN scores s ON s.entity_id = r.source_id
        WHERE r.target_id = ?
        """,
        (entity_id,),
    )
    in_rows = await in_cur.fetchall()

    # 去重建模 nodes，避免重复
    nodes_map: dict[str, GraphNode] = {}
    edges: list[GraphEdge] = []

    for row in out_rows:
        tid = row["target_id"]
        if tid not in nodes_map:
            nodes_map[tid] = GraphNode(
                id=tid,
                title=row["title"],
                entity_type=row["entity_type"],
                score_composite=row["final_score"],
            )
        edges.append(GraphEdge(
            source=entity_id,
            target=tid,
            ref_type=row["ref_type"],
            strength=row["strength"],
            direction="outgoing",
        ))

    for row in in_rows:
        sid = row["source_id"]
        if sid not in nodes_map:
            nodes_map[sid] = GraphNode(
                id=sid,
                title=row["title"],
                entity_type=row["entity_type"],
                score_composite=row["final_score"],
            )
        edges.append(GraphEdge(
            source=sid,
            target=entity_id,
            ref_type=row["ref_type"],
            strength=row["strength"],
            direction="incoming",
        ))

    return NeighborsResponse(
        nodes=list(nodes_map.values()),
        edges=edges,
        total_neighbors=len(nodes_map),
    )
```

**工时：** 4h（测试先行 1.5h → 实现 1.5h → 重构 1h）

---

#### P2-Graph-2：`GET /graph/neighbors/{id}?depth=N` — 深度查询

**目标：** BFS 遍历，支持返回 N 度邻居。

**API 规格：**

```
GET /graph/neighbors/{id}?depth=2

Query Parameters:
  depth: int  — 跳数，默认 1，最大 3
```

**验收标准：**
- [ ] `depth=1` 等同于 P2-Graph-1 基础查询
- [ ] `depth=2` 返回直接邻居 + 2度邻居（去重）
- [ ] `depth=3` 最大，支持 3 度遍历
- [ ] `depth > 3` 返回 400 Bad Request
- [ ] 节点数量上限 200（BFS 截断，防止图过大）
- [ ] 路径中不得出现重复节点（防环）

**测试用例：**

```python
@pytest.mark.asyncio
async def test_depth_2_includes_2nd_degree_neighbors(client, populated_db):
    """A→B→C（A 引用 B，B 引用 C），depth=2 时 A 可见 C"""
    # Arrange: A→B, B→C
    resp = await client.get("/graph/neighbors/know-000001?depth=2")
    assert resp.status_code == 200
    node_ids = {n["id"] for n in resp.json()["nodes"]}
    assert "know-000002" in node_ids  # 1度
    assert "know-000003" in node_ids  # 2度

@pytest.mark.asyncio
async def test_depth_3_max(client, populated_db):
    resp = await client.get("/graph/neighbors/know-000001?depth=3")
    assert resp.status_code == 200

@pytest.mark.asyncio
async def test_depth_4_returns_400(client):
    resp = await client.get("/graph/neighbors/know-000001?depth=4")
    assert resp.status_code == 400
```

**实现方案：**

```python
async def _bfs_neighbors(db, start_id: str, depth: int, max_nodes: int = 200) -> tuple[set[str], list[GraphEdge]]:
    """BFS 遍历图，返回 (visited_ids, edges)"""
    visited: set[str] = {start_id}
    queue: list[tuple[str, int]] = [(start_id, 0)]  # (node_id, current_depth)
    all_edges: list[GraphEdge] = []

    while queue:
        node_id, d = queue.pop(0)
        if d >= depth:
            continue

        # 查所有邻居（双向）
        cur = await db.conn.execute(
            """
            SELECT 'outgoing' AS direction, r.source_id AS from_id, r.target_id AS to_id,
                   r.ref_type, r.strength, e.title, e.entity_type
            FROM refs r
            JOIN entities e ON e.id = r.target_id
            WHERE r.source_id = ?
            UNION ALL
            SELECT 'incoming' AS direction, r.source_id AS from_id, r.target_id AS to_id,
                   r.ref_type, r.strength, e.title, e.entity_type
            FROM refs r
            JOIN entities e ON e.id = r.source_id
            WHERE r.target_id = ?
            """,
            (node_id, node_id),
        )
        rows = await cur.fetchall()

        for row in rows:
            neighbor_id = row["to_id"] if row["direction"] == "outgoing" else row["from_id"]
            if neighbor_id not in visited and len(visited) < max_nodes:
                visited.add(neighbor_id)
                queue.append((neighbor_id, d + 1))
            all_edges.append(GraphEdge(
                source=row["from_id"],
                target=row["to_id"],
                ref_type=row["ref_type"],
                strength=row["strength"],
                direction=row["direction"],
            ))

    return visited, all_edges
```

---

#### P2-Graph-3：`GET /graph/neighbors/{id}?min_strength=X` — 强度过滤

**目标：** 按边强度 `strength` 阈值过滤邻居。

**API 规格：**

```
GET /graph/neighbors/{id}?depth=1&min_strength=0.7
```

**验收标准：**
- [ ] `min_strength=0.7` 仅返回 `strength >= 0.7` 的边
- [ ] 不满足条件的边不出现在 edges 数组中
- [ ] 满足条件但相关节点已在 edges 中时正常返回
- [ ] `min_strength=0.0` 等同于不过滤
- [ ] `min_strength > 1.0` 返回空结果（不报错）

---

#### P2-Graph-4：`GET /graph/path?from=X&to=Y` — 最短路径

**目标：** BFS 求两点间最短路径。

**API 规格：**

```
GET /graph/path?from=know-000001&to=know-000005

Response 200:
{
  "path": ["know-000001", "know-000003", "know-000005"],
  "edges": [
    {"source": "know-000001", "target": "know-000003", "ref_type": "cites"},
    {"source": "know-000003", "target": "know-000005", "ref_type": "cites"}
  ],
  "distance": 2
}
Response 404:  // 无路径连通
{"detail": "No path found between entities"}
```

**验收标准：**
- [ ] 两实体相连时返回最短路径（边数最少）
- [ ] 不连通的实体返回 404
- [ ] `from == to` 返回 distance=0 的自环路径
- [ ] 最大搜索节点数 500，防止无限图死循环

**测试用例：**

```python
@pytest.mark.asyncio
async def test_path_shortest(client, populated_db):
    """A→B→C（A 引用 B，B 引用 C），A 到 C 的最短路径为 [A, B, C]"""
    resp = await client.get("/graph/path?from=know-000001&to=know-000003")
    assert resp.status_code == 200
    data = resp.json()
    assert data["distance"] == 2
    assert data["path"] == ["know-000001", "know-000002", "know-000003"]

@pytest.mark.asyncio
async def test_path_not_connected_returns_404(client, isolated_db):
    """两个孤立实体之间无路径"""
    resp = await client.get("/graph/path?from=know-000001&to=know-009999")
    assert resp.status_code == 404
    assert "No path found" in resp.json()["detail"]
```

---

### 9.4 Fetch Pipeline 详细设计

> Fetch Pipeline 实现 Phase 1 /docs 中埋头的 `NotImplemented` 槽位：
> `/fetch` → `/clean` → `/save` 三段式流水线。

#### P2-Fetch-1：`POST /fetch` — URL 抓取

**目标：** 接收 URL，返回页面原始 HTML/正文内容。

**API 规格：**

```yaml
POST /fetch
Content-Type: application/json
Body: {"url": "https://example.com/article"}

Response 200:
{
  "url": "https://example.com/article",
  "title": "页面标题",
  "raw_content": "<html>...",
  "content_type": "text/html",
  "status_code": 200,
  "fetched_at": "2026-04-24T12:00:00Z"
}
Response 422:  // URL 格式错误
Response 408:  // 请求超时（10s）
Response 400:  // 非 HTTP(S) URL
```

**验收标准：**
- [ ] 支持 HTTP 和 HTTPS URL
- [ ] 超时 10s，返回 408
- [ ] 返回原始 HTML（不做清洗）
- [ ] 跟随最多 3 次重定向
- [ ] User-Agent 携带 `Compass-Fetch/2.1` 标识
- [ ] 非 2xx 状态码返回 502

**测试用例：**

```python
@pytest.mark.asyncio
async def test_fetch_returns_html(client, respx_mock):
    respx_mock.get("https://example.com").respond(
        text="<html><body>Hello</body></html>",
        headers={"content-type": "text/html"},
        status_code=200,
    )
    resp = await client.post("/fetch", json={"url": "https://example.com"})
    assert resp.status_code == 200
    data = resp.json()
    assert data["url"] == "https://example.com"
    assert "<html>" in data["raw_content"]
    assert data["status_code"] == 200

@pytest.mark.asyncio
async def test_fetch_invalid_url_returns_422(client):
    resp = await client.post("/fetch", json={"url": "not-a-url"})
    assert resp.status_code == 422

@pytest.mark.asyncio
async def test_fetch_timeout_returns_408(client, respx_mock):
    # 模拟 11s 延迟
    respx_mock.get("https://slow.example").mock(side_effect=asyncio.TimeoutError())
    resp = await client.post("/fetch", json={"url": "https://slow.example"})
    assert resp.status_code == 408
```

**实现方案：**

```python
# compass-api/src/api/fetch.py
import httpx
from pydantic import BaseModel, HttpUrl
from fastapi import APIRouter, HTTPException

router = APIRouter(prefix="/fetch", tags=["fetch"])

class FetchRequest(BaseModel):
    url: HttpUrl

class FetchResponse(BaseModel):
    url: str
    title: Optional[str]
    raw_content: str
    content_type: str
    status_code: int
    fetched_at: str

TIMEOUT = 10.0

@router.post("", response_model=FetchResponse)
async def fetch_url(req: FetchRequest):
    try:
        async with httpx.AsyncClient(
            timeout=httpx.Timeout(TIMEOUT),
            follow_redirects=True,
            headers={"User-Agent": "Compass-Fetch/2.1"},
        ) as client:
            resp = await client.get(str(req.url))
    except httpx.TimeoutException:
        raise HTTPException(status_code=408, detail="Fetch timeout (10s)")
    except Exception:
        raise HTTPException(status_code=400, detail="Invalid URL")

    if not (200 <= resp.status_code < 300):
        raise HTTPException(status_code=502, detail=f"Upstream returned {resp.status_code}")

    content_type = resp.headers.get("content-type", "text/plain")
    title = resp.extract_text("title") if "text/html" in content_type else None

    return FetchResponse(
        url=str(req.url),
        title=title,
        raw_content=resp.text,
        content_type=content_type,
        status_code=resp.status_code,
        fetched_at=datetime.now(tz=timezone.utc).isoformat(),
    )
```

**工时：** 4h（测试 1.5h → 实现 1.5h → 重构 1h）

---

#### P2-Fetch-2：`POST /fetch/clean` — 内容清洗

**目标：** 接收原始 HTML，输出清洗后的结构化 Markdown。

**API 规格：**

```yaml
POST /fetch/clean
Body: {"raw_content": "<html>...</html>", "source_url": "https://..."}

Response 200:
{
  "title": "清洗后标题",
  "content": "## 章节标题\n\n正文内容...",
  "summary": "可选 AI 摘要（≤200 字）",
  "tags": ["#标签1", "#标签2"],
  "source_url": "https://..."
}
```

**验收标准：**
- [ ] 移除所有 HTML 标签，保留文本结构（标题 → `#` / `##`）
- [ ] `<pre><code>` 块保留原始格式
- [ ] 图片 `src` 转为 `![](url)` Markdown 格式
- [ ] 链接保留为 `[text](url)` 格式
- [ ] 移除广告、导航栏、Footer 等噪音块（基于 CSS selector 过滤）
- [ ] `source_url` 写入文档元数据
- [ ] 提取 `<title>` 作为标题（兜底）

**清洗策略：**
- 使用 `httpx` 或 `BeautifulSoup` 解析
- 移除 `<nav>`, `<footer>`, `<aside>`, `<header>`, `.ad`, `.advertisement` selector
- 保留 `<article>`, `<main>`, `<p>`, `<h1>`~`<h6>`, `<ul>`, `<ol>`, `<blockquote>`, `<pre>`, `<code>`
- 提取 `og:title` > `<title>` 作为标题

---

#### P2-Fetch-3：`POST /fetch/save` — 写入 Vault

**目标：** 将清洗后的内容保存为 Vault Markdown 文件。

**API 规格：**

```yaml
POST /fetch/save
Body:
{
  "title": "文章标题",
  "content": "## 正文...",
  "source_url": "https://...",
  "tags": ["#AI", "#论文"],
  "category": "Inbox"
}

Response 201:
{
  "entity_id": "know-000042",
  "file_path": "/vault/Inbox/know-000042.md",
  "title": "文章标题"
}
```

**验收标准：**
- [ ] 文件名：`{category}/{entity_id}.md`
- [ ] Front-matter 包含 `id`、`title`、`source`、`tags`、`created_at`、`updated_at`
- [ ] 正文内容跟在 front-matter 后面
- [ ] 实体 ID 格式：`know-{6位数字}`，自动递增
- [ ] 重复 URL 检测（source 已存在的 entity 不允许重复 save）
- [ ] 同时写入 SQLite（entities + scores 表）
- [ ] 返回新创建实体的 ID 和文件路径

**Front-matter 格式：**

```markdown
---
id: know-000042
title: 文章标题
source: https://example.com/article
tags:
  - #AI
  - #论文
category: Inbox
created_at: 2026-04-24T12:00:00Z
updated_at: 2026-04-24T12:00:00Z
---

## 正文内容...
```

---

### 9.5 Semantic Search（FAISS）详细设计

#### P2-Search-1：`POST /search` — 语义 + BM25 混合搜索

**目标：** 使用 FAISS 向量索引 + BM25 实现语义相似度搜索。

**数据流：**

```
写入路径（后台任务）：
  新建/更新 Entity → 提取文本 → Embedding API → 存入 FAISS 索引

查询路径：
  用户 query → Embedding → FAISS top-k → BM25 补充 → 混合排序 → 返回
```

**API 规格：**

```yaml
POST /search
Body:
{
  "query": "机器学习注意力机制",
  "semantic_weight": 0.6,
  "score_weight": 0.4,
  "filters": {
    "tags": ["#AI"],
    "entity_type": "knowledge",
    "date_range": {"start": "2026-01-01", "end": "2026-04-24"}
  },
  "limit": 20
}

Response 200:
{
  "items": [
    {
      "entity": {
        "id": "know-000001",
        "title": "注意力机制详解",
        "entity_type": "knowledge",
        "score_composite": 75.0
      },
      "match_score": 0.92,
      "highlights": ["...注意力机制..."]
    }
  ],
  "total": 1,
  "query_vector_dim": 1536
}
```

**验收标准：**
- [ ] `query` 字段支持自然语言
- [ ] `semantic_weight + score_weight = 1.0`（校验）
- [ ] 返回结果按混合分数降序
- [ ] `highlights` 字段包含 query 关键词在正文中的上下文片段
- [ ] 无 FAISS 索引时回退到纯 BM25（FTS5）
- [ ] 向量维度须与 Embedding 模型一致（text-embedding-3-small: 1536维）
- [ ] 单次查询最大 `limit=100`

**测试用例：**

```python
@pytest.mark.asyncio
async def test_search_returns_hybrid_results(client, populated_db_with_embeddings):
    resp = await client.post("/search", json={
        "query": "深度学习优化器",
        "limit": 5,
    })
    assert resp.status_code == 200
    data = resp.json()
    assert len(data["items"]) <= 5
    assert all("match_score" in item for item in data["items"])
    assert all("entity" in item for item in data["items"])

@pytest.mark.asyncio
async def test_search_weight_validation(client):
    resp = await client.post("/search", json={
        "query": "test",
        "semantic_weight": 0.5,
        "score_weight": 0.8,  # > 1.0
    })
    assert resp.status_code == 422  # validation error
```

**实现方案（嵌入生成）：**

```python
# compass-api/src/services/embedding.py
import httpx
from src import config

EMBEDDING_MODEL = "text-embedding-3-small"
EMBEDDING_URL = "https://api.openai.com/v1/embeddings"
DIMENSION = 1536

async def embed_texts(texts: list[str]) -> list[list[float]]:
    """调用 OpenAI Embedding API，返回归一化向量列表"""
    async with httpx.AsyncClient(timeout=30) as client:
        resp = await client.post(
            EMBEDDING_URL,
            json={
                "model": EMBEDDING_MODEL,
                "input": texts,
                "dimensions": DIMENSION,
            },
            headers={"Authorization": f"Bearer {config.OPENAI_API_KEY}"},
        )
        resp.raise_for_status()
        data = resp.json()
        return [item["embedding"] for item in data["data"]]
```

**FAISS 索引管理（Python 层）：**

```python
# compass-api/src/services/faiss_index.py
import faiss
import numpy as np

class FaissIndex:
    def __init__(self, dim: int = DIMENSION):
        self.dim = dim
        self.index = faiss.IndexFlatIP(dim)  # Inner Product（余弦相似度等价于归一化向量）
        self.id_map: dict[int, str] = {}  # faiss_offset → entity_id

    def add(self, entity_id: str, embedding: list[float]):
        vec = np.array([embedding], dtype=np.float32)
        faiss.normalize_L2(vec)
        offset = self.index.ntotal
        self.index.add(vec)
        self.id_map[offset] = entity_id

    def search(self, query_embedding: list[float], k: int) -> list[tuple[str, float]]:
        vec = np.array([query_embedding], dtype=np.float32)
        faiss.normalize_L2(vec)
        scores, offsets = self.index.search(vec, k)
        return [(self.id_map[o], float(s)) for o, s in zip(offsets[0], scores[0]) if o >= 0]
```

**工时：** 8h（Embedding API 对接 2h → FAISS 索引管理 2h → 混合排序 2h → 测试 2h）

---

### 9.6 MCP Server 详细设计

#### P2-MCP-1：MCP Server 适配层

**目标：** 将 compass-api 的核心能力暴露为 MCP Tools，供 MCP-native Agent 使用。

**技术选型：** `@modelcontextprotocol/sdk` Python SDK

**Tools 规格：**

```python
# compass-api/src/mcp/server.py
from mcp.server import Server
from mcp.types import Tool, TextContent
from mcp.server.stdio import stdio_server

server = Server("compass")

@server.list_tools()
async def list_tools() -> list[Tool]:
    return [
        Tool(
            name="compass_neighbors",
            description="获取实体的所有直接邻居节点",
            inputSchema={
                "type": "object",
                "properties": {
                    "entity_id": {"type": "string"},
                    "depth": {"type": "integer", "default": 1},
                    "min_strength": {"type": "number", "default": 0.0},
                },
                "required": ["entity_id"],
            },
        ),
        Tool(
            name="compass_search",
            description="语义搜索实体",
            inputSchema={
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "default": 10},
                },
                "required": ["query"],
            },
        ),
        Tool(
            name="compass_fetch_save",
            description="抓取 URL 并保存到 Vault",
            inputSchema={
                "type": "object",
                "properties": {
                    "url": {"type": "string", "format": "uri"},
                    "tags": {"type": "array", "items": {"type": "string"}},
                },
                "required": ["url"],
            },
        ),
    ]

@server.call_tool()
async def call_tool(name: str, arguments: dict) -> list[TextContent]:
    if name == "compass_neighbors":
        # 调用 graph API
        ...
    elif name == "compass_search":
        # 调用 search API
        ...
    elif name == "compass_fetch_save":
        # 调用 fetch + clean + save pipeline
        ...
    return [TextContent(text=json.dumps(result))]
```

**验收标准：**
- [ ] MCP Server 可通过 `python -m src.mcp.server` 独立启动（Stdio 模式）
- [ ] 3 个 Tool 注册成功：`compass_neighbors`、`compass_search`、`compass_fetch_save`
- [ ] Tool 输入 schema 与 compass-api REST 接口一致
- [ ] Tool 输出为 JSON 字符串（TextContent）
- [ ] 可与 Cursor / Claude Code 等 MCP Native Agent 对接

**工时：** 6h（MCP SDK 接入 2h → 3 个 Tool 实现 2h → 联调测试 2h）

---

### 9.7 Phase 3 任务拆解

> Phase 3 为 Compass 的用户界面层，包含 Web UI、图谱可视化和 PWA 离线能力。
> **前置依赖：** Phase 2 Graph API 和 Search API 已完成。

#### 9.6.1 任务总览

| 任务 ID | 任务名称 | 分支 | 优先级 | 依赖 | 预估工时 |
|---------|----------|------|--------|------|----------|
| **P3-UI-1** | React 前端骨架 + Vite | `feat/p3-ui-skeleton` | P1 | P2-Search-1 | 8h |
| **P3-UI-2** | 实体详情页（阅读视图） | `feat/p3-ui-entity-detail` | P1 | P3-UI-1 | 6h |
| **P3-UI-3** | 评分面板（Interest/Strategy/Consensus 可视化） | `feat/p3-ui-score-panel` | P1 | P2-Search-1 | 4h |
| **P3-UI-4** | 图谱可视化（Force-Directed Graph） | `feat/p3-ui-graph-viz` | P2 | P2-Graph-1 | 12h |
| **P3-UI-5** | PWA 配置（Service Worker + 离线缓存） | `feat/p3-ui-pwa` | P2 | P3-UI-1 | 6h |

---

#### P3-UI-1：React 前端骨架

**目标：** 搭建可运行的 React + Vite 项目，连接 compass-api。

**技术栈：** React 18 + Vite + TypeScript + Tailwind CSS + React Query

**验收标准：**
- [ ] `npm run dev` 启动开发服务器（端口 5173）
- [ ] `GET /health` 健康检查通过后显示连接状态
- [ ] React Query 配置正确，API 请求走 `/api/v1` 代理
- [ ] Tailwind CSS 正确配置
- [ ] 实体列表页（`/entities`）可加载并显示 Phase 2 已有数据

**Vite 配置片段：**

```ts
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      "/api": {
        target: "http://localhost:8000",
        changeOrigin: true,
      },
    },
  },
});
```

---

#### P3-UI-2：实体详情页

**目标：** 展示单个实体的完整信息。

**路由：** `GET /entities/:id`

**验收标准：**
- [ ] 显示标题、内容（Markdown 渲染）、标签、评分
- [ ] 显示引用关系（outgoing + incoming refs）
- [ ] 显示关联图谱入口（点击邻居节点跳转）
- [ ] 评分面板可直接调整评分（调用 `PATCH /entities/:id/score`）

---

#### P3-UI-3：评分面板

**目标：** 可视化三维评分 + decay 状态。

**验收标准：**
- [ ] 雷达图显示 Interest / Strategy / Consensus 三维分布
- [ ] 时间线滑块显示 decay 进度（距离下次衰减的天数）
- [ ] 手动 boost 按钮（调用 `/scores/update` with `last_boosted_at=now`）
- [ ] 评分变化历史图表（折线图，最近 30 天）

---

#### P3-UI-4：图谱可视化

**目标：** D3.js 力导向图展示实体关系网络。

**验收标准：**
- [ ] 中心节点为当前查看的实体
- [ ] 直接邻居显示在第一圈
- [ ] 边粗细表示 `strength`
- [ ] 节点大小表示 `score_composite`
- [ ] 点击节点跳转详情
- [ ] 支持拖拽重新布局
- [ ] 颜色区分 `entity_type`（knowledge/case/log/insight）

**技术方案：** D3.js force simulation，数据来源 `GET /graph/neighbors/:id?depth=2`

---

#### P3-UI-5：PWA 配置

**目标：** 支持离线访问已缓存实体。

**验收标准：**
- [ ] Service Worker 注册成功（`sw.js`）
- [ ] 访问过的实体页面可离线浏览（Cache-First 策略）
- [ ] 后台同步：网络恢复后自动同步评分变更
- [ ] Web App Manifest（`manifest.json`）包含图标和启动屏
- [ ] Lighthouse PWA Score ≥ 80

---

### 9.8 Phase 2 完整依赖图

```
Phase 2 基座
└── P2-Entity-1 ──┬──────────────────────────────────┐

Graph 任务线            Timeline/Access        Score History
├── P2-Graph-1 ──┬── P2-Graph-2 ── P2-Graph-3 ── P2-Graph-4   P2-Timeline-1 ── P2-Timeline-2
│                │                                                      │
│                └──→ P2-Insight-1 ──→ P2-Insight-2 ──→ P2-Insight-3    │
│                │                                                      │
│                └──→ P2-Ref-1 ──────→ P2-Ref-2                         │
│                                                                      │
Fetch 任务线                            Decay Tuner                    │
├── P2-Fetch-1 ──→ P2-Fetch-2 ──→ P2-Fetch-3 ─────────────────────────→ P2-Decay-1 ──→ P2-Decay-2 ──→ P2-Decay-3
│                                                                      │
Search 任务线                             Score History ──────────────→ P2-History-1 ──→ P2-History-2
└── P2-Search-1 ────────────────────────────────────────────────────────┘

MCP（汇总所有下游能力，Phase 2 后期接入）
└── P2-MCP-1 ← P2-Graph-1 + P2-Search-1 + P2-Fetch-3 + P2-Timeline-2

Phase 3
├── P3-UI-1 (React skeleton)  ←─┐
├── P3-UI-2 (entity detail)  ← P3-UI-1
├── P3-UI-3 (score panel)  ← P3-UI-1 + P2-History-2 + P2-Decay-3
├── P3-UI-4 (graph viz)  ← P2-Graph-1 + P3-UI-1
└── P3-UI-5 (PWA)  ← P3-UI-1
```

---

### 9.9 Phase 2 & 3 验收 Checklist（执行用）

#### Phase 2 核心验收

**Entity List API：**
- [ ] P2-Entity-1: `GET /entities` 无参数时返回全量实体列表（分页）
- [ ] P2-Entity-1: `type` 参数正确过滤实体类型
- [ ] P2-Entity-1: `min_score` 参数正确过滤最低分数
- [ ] P2-Entity-1: `tags` 参数正确过滤标签（AND 逻辑）
- [ ] P2-Entity-1: `limit` / `offset` 分页正确，`has_more` 准确
- [ ] P2-Entity-1: 返回结果按 `final_score` 降序排列
- [ ] P2-Entity-1: `limit > 100` 返回 422
- [ ] P2-Entity-1: 无效 `type` 值返回 422
- [ ] P2-Entity-1: 空结果返回 `items=[]`、`total=0`（200）

**Graph API：**
- [ ] P2-Graph-1: `GET /graph/neighbors/{id}` — 所有直接邻居返回正确
- [ ] P2-Graph-1: 不存在的实体返回 404
- [ ] P2-Graph-2: `depth=2` 包含 2 度邻居
- [ ] P2-Graph-2: `depth=4` 返回 400
- [ ] P2-Graph-3: `min_strength=0.7` 正确过滤
- [ ] P2-Graph-4: A→B→C 的最短路径正确
- [ ] P2-Graph-4: 不连通实体返回 404

**Fetch Pipeline：**
- [ ] P2-Fetch-1: 正常 URL 返回 HTML 内容
- [ ] P2-Fetch-1: 超时 10s 返回 408
- [ ] P2-Fetch-2: HTML 清洗后为合法 Markdown
- [ ] P2-Fetch-2: 图片和链接正确转换
- [ ] P2-Fetch-3: 文件成功写入 Vault
- [ ] P2-Fetch-3: 重复 URL 不允许重复保存
- [ ] P2-Fetch-3: SQLite entities 表同步写入

**Semantic Search：**
- [ ] P2-Search-1: 自然语言 query 返回结果
- [ ] P2-Search-1: 权重和不等于 1 时返回 422
- [ ] P2-Search-1: highlights 包含查询关键词上下文
- [ ] P2-Search-1: 无 FAISS 索引时回退 BM25

**MCP Server：**
- [ ] P2-MCP-1: 3 个 Tool 正确注册
- [ ] P2-MCP-1: Tool 输出与 REST API 结果一致
- [ ] P2-MCP-1: Stdio 模式独立启动

#### Phase 3 核心验收

**Web UI：**
- [ ] P3-UI-1: 开发服务器启动成功
- [ ] P3-UI-1: API 代理配置正确
- [ ] P3-UI-2: Markdown 内容正确渲染
- [ ] P3-UI-2: 引用关系显示完整
- [ ] P3-UI-3: 雷达图三维分布正确
- [ ] P3-UI-4: 力导向图正确渲染
- [ ] P3-UI-4: 节点点击跳转正常
- [ ] P3-UI-5: Service Worker 离线可用
- [ ] P3-UI-5: Lighthouse PWA Score ≥ 80

**Timeline & Access：**
- [ ] P2-Timeline-1: `PATCH /entities/{id}/access` 访问后 access_count++ 且 accessed_at 更新
- [ ] P2-Timeline-1: 同一实体的连续访问在 5 分钟内合并为一次（debounce）
- [ ] P2-Timeline-2: `GET /entities/{id}/timeline` 返回该实体所有事件，按时间倒序
- [ ] P2-Timeline-2: `event_type` 过滤正确（create / read / update / cite / reflect）
- [ ] P2-Timeline-2: `days=N` 参数正确限制时间范围

**Score History：**
- [ ] P2-History-1: `POST /scores/update` 成功后同步写入 `score_history` 表
- [ ] P2-History-1: `trigger_type` 字段正确记录触发原因（manual / auto_decay / access_boost）
- [ ] P2-History-2: `GET /entities/{id}/score/history?days=30` 返回趋势数据
- [ ] P2-History-2: `trend` 字段正确计算（rising / declining / stable）
- [ ] P2-History-2: `change_pct` 正确反映相对变化百分比

**Insight Engine：**
- [ ] P2-Insight-1: `GET /insights` 返回所有 insight 实体，支持 maturity 过滤
- [ ] P2-Insight-1: `POST /insights` 创建新 insight，maturity 默认为 `spark`
- [ ] P2-Insight-1: maturity 状态机转换路径正确（spark → framework → mature）
- [ ] P2-Insight-2: 被 3+ case 引用时触发 spark → framework 演化
- [ ] P2-Insight-2: 被 2+ knowledge 引用时触发 spark → framework 演化
- [ ] P2-Insight-2: 演化时写入 `refined_at` 时间戳
- [ ] P2-Insight-3: mature insight 可导出为新的 knowledge 实体
- [ ] P2-Insight-3: 导出后 mature insight 状态变更为 `archived`

**Reference Intelligence：**
- [ ] P2-Ref-1: 创建引用时自动计算 `strength` = |N(A)∩N(B)| / √(|N(A)|×|N(B)|)
- [ ] P2-Ref-1: `strength` 为 0 时该引用被软删除（不返回但保留历史）
- [ ] P2-Ref-2: 创建 A→B 时，若存在 B→A 且 `ref_type` 相同，则合并（strength 取最大值）
- [ ] P2-Ref-2: `ref_type` 为 `cites` 时自动补充反向 `inspired` 引用

**Decay Tuner：**
- [ ] P2-Decay-1: `PATCH /entities/{id}/decay-config` 可覆盖全局默认半衰期
- [ ] P2-Decay-1: 未设置个性化半衰期时回退全局默认值
- [ ] P2-Decay-2: `GET /entities/{id}/decay-preview?days=30` 返回 30 天后的预测分数
- [ ] P2-Decay-2: preview 考虑当前 decay_factor，不修改实际数据
- [ ] P2-Decay-3: Decay 模拟器返回未来 90 天每一天的预测分数数组
- [ ] P2-Decay-3: 模拟器可指定自定义半衰期参数（用于 A/B 测试对比）

**MCP Server：**
- [ ] P2-MCP-1: 3 个 Tool 正确注册（compass_neighbors / compass_search / compass_fetch_save）
- [ ] P2-MCP-1: Tool 输出与 REST API 结果一致
- [ ] P2-MCP-1: Stdio 模式独立启动
- [ ] P2-MCP-1: 可与 Cursor / Claude Code 等 MCP Native Agent 对接

---

## 9.9 新增任务线详细设计

### 9.10 Timeline & Access（P2-Timeline-1~2）

#### P2-Timeline-1：`PATCH /entities/{id}/access` — 访问记录

**目标：** 记录实体访问事件，更新 `accessed_at` 和 `access_count`，触发 decay 重新计算。

**API 规格：**

```
PATCH /entities/{id}/access

Response 200:
{
  "entity_id": "know-000001",
  "access_count": 42,
  "accessed_at": "2026-04-26T14:37:00Z",
  "decay_updated": true
}
Response 404:  // 实体不存在
```

**验收标准：**
- [ ] `access_count` 在原值基础上 +1
- [ ] `accessed_at` 更新为当前时间
- [ ] 同一实体在 5 分钟内的连续访问合并为一次（防重复计数）
- [ ] 访问触发 decay 因子重新计算（调用 Rust compute_score）
- [ ] 不存在的实体返回 404

**实现要点：**

```python
# compass-api/src/api/entities.py

ACCESS_DEBOUNCE_SECONDS = 300  # 5 分钟

@router.patch("/{entity_id}/access")
async def record_access(
    entity_id: str,
    db: Database = Depends(get_db),
) -> AccessResponse:
    entity = await db.get_entity(entity_id)
    if not entity:
        raise HTTPException(status_code=404, detail="Entity not found")

    # Debounce：检查最近一次访问时间
    last_access = await db.get_last_access(entity_id)
    now = datetime.now(tz=timezone.utc)
    if last_access and (now - last_access).total_seconds() < ACCESS_DEBOUNCE_SECONDS:
        # 合并为一次，不增加 count
        return AccessResponse(
            entity_id=entity_id,
            access_count=entity["access_count"],
            accessed_at=last_access.isoformat(),
            decay_updated=False,
        )

    # 更新 access_count 和 accessed_at
    await db.update_access(entity_id, now)
    # 触发 decay 重新计算
    await recalculate_decay(entity_id, db)
    return AccessResponse(
        entity_id=entity_id,
        access_count=entity["access_count"] + 1,
        accessed_at=now.isoformat(),
        decay_updated=True,
    )
```

---

#### P2-Timeline-2：`GET /entities/{id}/timeline` — 时间线查询

**目标：** 返回实体的所有 timeline 事件，支持过滤和分页。

**API 规格：**

```
GET /entities/{id}/timeline?event_type=read&days=30&limit=50&offset=0

Response 200:
{
  "entity_id": "know-000001",
  "events": [
    {
      "id": 1001,
      "event_type": "read",
      "trigger": "manual",
      "intensity": 1.0,
      "timestamp": "2026-04-26T14:37:00Z",
      "source": "feishu"
    },
    {
      "id": 1000,
      "event_type": "cite",
      "trigger": "auto",
      "intensity": 0.8,
      "timestamp": "2026-04-25T10:00:00Z",
      "source": "system"
    }
  ],
  "total": 2,
  "has_more": false
}
```

**event_type 枚举：** `create` | `read` | `update` | `cite` | `reflect` | `agent_query`

**验收标准：**
- [ ] 无过滤时返回该实体所有事件，按时间倒序
- [ ] `event_type` 过滤正确
- [ ] `days=N` 只返回 N 天内的事件
- [ ] `limit` / `offset` 分页正确
- [ ] 不存在的实体返回 404

**工时：** P2-Timeline-1（2h） + P2-Timeline-2（3h） = **5h**

---

### 9.11 Score History API（P2-History-1~2）

#### P2-History-1：`POST /scores/update` 时同步写入 score_history

**目标：** 每次评分变更自动记录历史，供趋势分析使用。

**数据库操作（新增）：**

```python
# compass-api/src/db/database.py

async def write_score_history(
    self,
    entity_id: str,
    dimension: str,  # 'interest' | 'strategy' | 'consensus' | 'composite'
    old_value: float,
    new_value: float,
    reason: str,
    trigger_type: str,  # 'manual' | 'auto_decay' | 'access_boost' | 'agent_suggestion'
) -> None:
    """写入单条评分历史记录。"""
    await self.conn.execute(
        """
        INSERT INTO score_history (entity_id, dimension, old_value, new_value, reason, trigger_type)
        VALUES (?, ?, ?, ?, ?, ?)
        """,
        (entity_id, dimension, old_value, new_value, reason, trigger_type),
    )
```

**触发时机：**
- `PATCH /entities/{id}/score`（手动调整）→ `trigger_type='manual'`
- FileWatcher 检测到文件更新 + decay 衰减 → `trigger_type='auto_decay'`
- `PATCH /entities/{id}/access`（访问 boost）→ `trigger_type='access_boost'`
- Agent 建议调整 → `trigger_type='agent_suggestion'`

---

#### P2-History-2：`GET /entities/{id}/score/history` — 评分趋势

**API 规格：**

```
GET /entities/{id}/score/history?dimension=interest&days=30

Response 200:
{
  "entity_id": "know-000001",
  "dimension": "interest",
  "records": [
    {"timestamp": "2026-04-26T14:37:00Z", "value": 72.5},
    {"timestamp": "2026-04-15T00:00:00Z", "value": 80.0},
    {"timestamp": "2026-04-01T00:00:00Z", "value": 85.0}
  ],
  "trend": "declining",
  "change_pct": -14.7,
  "min_value": 72.5,
  "max_value": 85.0
}

Response 404:  // 实体不存在
```

**trend 计算规则：**
```
recent_avg = 最近3次记录的平均值
older_avg  = 再往前3次记录的平均值
change_pct = (recent_avg - older_avg) / older_avg × 100

trend = "rising"    if change_pct > 5
      | "declining" if change_pct < -5
      | "stable"
```

**验收标准：**
- [ ] `dimension` 参数正确过滤维度（默认 `composite`）
- [ ] `days=N` 参数正确限制时间范围（默认 90）
- [ ] `records` 按时间倒序
- [ ] `trend` 计算正确
- [ ] `change_pct` 保留 1 位小数
- [ ] 实体不存在返回 404

**工时：** P2-History-1（2h） + P2-History-2（3h） = **5h**

---

### 9.12 Insight Engine（P2-Insight-1~3）

#### P2-Insight-1：Insight 实体 CRUD + maturity 状态机

**目标：** 支持 Insight 特殊实体的创建、查询和 maturity 状态流转。

**Insight 实体与普通 entity 的区别：**

| 字段 | Knowledge | Insight |
|------|-----------|---------|
| `entity_type` | `knowledge` | `insight` |
| `maturity` | 无 | `spark` / `framework` / `mature` |
| `derived_from` | 无 | 有（引用来源） |
| `evolved_into` | 无 | 有（演化目标） |
| 状态流转 | 无 | spark → framework → mature → archived |

**API 规格：**

```
POST /entities
Body: {"entity_type": "insight", "title": "灵感标题", "content": "...", "maturity": "spark"}

GET /entities?type=insight&maturity=spark
GET /entities/{id}
```

**maturity 状态机（允许的转换）：**

```
spark ──→ framework ──→ mature
  │            │
  └────────────┴──→ archived（随时可存档）
```

**验收标准：**
- [ ] `POST /entities` 支持 `entity_type=insight`
- [ ] `GET /entities?type=insight` 返回所有 insight
- [ ] `GET /entities?type=insight&maturity=spark` 正确过滤 maturity
- [ ] `maturity` 字段出现在 entity 详情中
- [ ] `derived_from` 和 `evolved_into` 字段正确填充
- [ ] 不允许的状态转换返回 422

---

#### P2-Insight-2：成熟度演化触发器

**触发条件：**

| 当前状态 | 触发条件 | 目标状态 |
|----------|----------|----------|
| spark | 被 3+ case 引用 OR 被 2+ knowledge 引用 | framework |
| spark | 超过 90 天无引用 | spark（不变） |
| framework | 被 5+ knowledge 引用 OR 包含 `outcome` 字段 | mature |
| framework | 超过 180 天无引用 | spark（降级） |
| mature | 超过 365 天无引用 | archived |

**演化检查时机：**
- 每次引用创建时（`POST /refs`）检查两端实体
- 每天定时任务扫描所有 insight（避免遗漏）

**实现要点：**

```python
async def check_insight_evolution(db: Database, insight_id: str) -> Optional[str]:
    """检查 insight 是否满足成熟度演化条件，返回目标状态或 None。"""
    insight = await db.get_entity(insight_id, entity_type="insight")
    if not insight or insight["entity_type"] != "insight":
        return None

    maturity = insight["maturity"]

    # 统计引用数
    incoming_refs = await db.get_incoming_refs(insight_id)  # 引用该 insight 的实体
    case_refs = [r for r in incoming_refs if r["entity_type"] == "case"]
    knowledge_refs = [r for r in incoming_refs if r["entity_type"] == "knowledge"]

    if maturity == "spark":
        if len(case_refs) >= 3 or len(knowledge_refs) >= 2:
            return "framework"
    elif maturity == "framework":
        if len(knowledge_refs) >= 5:
            return "mature"
        # 检查 outcome 字段
        if insight.get("outcome"):
            return "mature"

    return None
```

**验收标准：**
- [ ] 引用数达标时自动触发 maturity 演化
- [ ] 演化时设置 `refined_at` 为当前时间
- [ ] 演化时记录 `evolved_into` / `derived_from`
- [ ] 定时任务正确扫描所有 insight
- [ ] 不满足条件时 insight 状态不变

---

#### P2-Insight-3：Insight → Knowledge 导出降级

**目标：** mature insight 可导出为 knowledge 实体，自身降为 archived。

**API 规格：**

```
POST /entities/{insight_id}/export
Body: {"title": "新 Knowledge 标题"}  // 可选，默认沿用原标题

Response 201:
{
  "new_entity_id": "know-000042",
  "exported_from_insight_id": "ins-000001",
  "maturity": "mature",
  "status": "archived"
}
```

**导出后的变化：**
- 新建 `knowledge` 实体，内容复制自 insight（去掉 maturity 相关字段）
- 原 insight 的 `status` 变为 `archived`
- 原 insight 的 `evolved_into` 指向新建 knowledge

**验收标准：**
- [ ] 导出后原 insight 状态变为 `archived`
- [ ] 新建 knowledge 的 `derived_from` 指向原 insight
- [ ] `export` 操作 idempotent（已 archived 的 insight 不能再次导出）
- [ ] 导出前检查 `maturity=mature`，否则返回 422

**工时：** P2-Insight-1（4h） + P2-Insight-2（6h） + P2-Insight-3（3h） = **13h**

---

### 9.13 Reference Intelligence（P2-Ref-1~2）

#### P2-Ref-1：引用强度自动计算

**背景：** PRD v2.1 的 `refs.strength` 字段默认为 1.0，没有自动计算逻辑。本任务实现基于图结构的共同邻居相似度。

**计算公式（余弦相似度在图上）：**

```
strength(A → B) = |N(A) ∩ N(B)| / √(|N(A)| × |N(B)|)

其中 N(X) = {X 的所有邻居节点 ID，不包括 X 自身}
     A → B 表示 A 引用了 B（A 是 source，B 是 target）
```

**含义：** A 和 B 的共同邻居越多，强度越高。

**触发时机：**
- 引用创建时立即计算
- 引用删除时反向影响（周围节点重新计算）

**实现要点：**

```python
async def compute_ref_strength(db: Database, source_id: str, target_id: str) -> float:
    """计算 source → target 的引用强度。"""
    neighbors_a = await db.get_neighbor_ids(source_id)
    neighbors_b = await db.get_neighbor_ids(target_id)
    neighbors_a.discard(source_id)  # 排除自身
    neighbors_b.discard(target_id)

    if not neighbors_a or not neighbors_b:
        return 0.0  # 孤立的边，强度为 0

    intersection = neighbors_a & neighbors_b
    union_sqrt = (len(neighbors_a) * len(neighbors_b)) ** 0.5
    return len(intersection) / union_sqrt if union_sqrt > 0 else 0.0
```

**验收标准：**
- [ ] 新引用创建后 strength 正确计算（不是默认 1.0）
- [ ] 引用删除后相关节点重新计算影响正确
- [ ] strength = 0 的引用自动软删除（不出现在查询结果中）
- [ ] self-loop 引用强度强制为 0

---

#### P2-Ref-2：双向引用自动维护

**规则：**
- `ref_type = 'cites'` 时，自动创建反向 `inspired` 引用（B → A，强度相同）
- 若反向引用已存在（任意 `ref_type`），则合并：`strength = max(strength_A→B, strength_B→A)`
- `ref_type = 'inspired'` 时，**不**自动创建反向 cites（避免循环）

**实现要点：**

```python
async def create_ref_with_backlink(db: Database, source_id: str, target_id: str, ref_type: str) -> Ref:
    ref = await db.create_ref(source_id, target_id, ref_type)

    if ref_type == "cites":
        # 自动创建反向 inspired 引用
        existing = await db.get_ref(target_id, source_id)
        if existing:
            # 合并：strength 取最大值
            new_strength = max(ref.strength, existing.strength)
            await db.update_ref_strength(target_id, source_id, new_strength)
        else:
            await db.create_ref(target_id, source_id, "inspired", ref.strength)

    return ref
```

**验收标准：**
- [ ] `cites` 引用自动创建反向 `inspired`
- [ ] `inspired` 引用不触发双向创建
- [ ] 反向引用已存在时 strength 取最大值
- [ ] 删除正向引用时反向引用同步处理

**工时：** P2-Ref-1（6h） + P2-Ref-2（3h） = **9h**

---

### 9.14 Decay Tuner（P2-Decay-1~3）

#### P2-Decay-1：`PATCH /entities/{id}/decay-config` — 个性化半衰期

**目标：** 允许用户为单个实体覆盖全局默认半衰期。

**API 规格：**

```
PATCH /entities/{id}/decay-config
Body: {
  "interest_half_life_days": 60.0,
  "strategy_half_life_days": 180.0,
  "consensus_half_life_days": 90.0
}

Response 200:
{
  "entity_id": "know-000001",
  "interest_half_life_days": 60.0,
  "strategy_half_life_days": 180.0,
  "consensus_half_life_days": 90.0,
  "is_override": true,
  "updated_at": "2026-04-26T14:37:00Z"
}
Response 404:  // 实体不存在
```

**数据库变化：**

```sql
-- entities 表新增字段（Phase 2 迁移）
ALTER TABLE entities ADD COLUMN decay_override_json TEXT;
```

**is_override 逻辑：**
- `decay_override_json IS NULL` → 使用全局默认半衰期，`is_override = false`
- `decay_override_json IS NOT NULL` → 使用个性化半衰期，`is_override = true`

---

#### P2-Decay-2：`GET /entities/{id}/decay-preview?days=30` — Decay 预览

**目标：** 在不修改数据的情况下，预览未来某天的预测分数。

**API 规格：**

```
GET /entities/{id}/decay-preview?days=30

Response 200:
{
  "entity_id": "know-000001",
  "current_scores": {
    "interest": 80.0,
    "strategy": 90.0,
    "consensus": 60.0,
    "composite": 78.0
  },
  "preview_scores": {
    "interest": 72.5,
    "strategy": 88.7,
    "consensus": 57.2,
    "composite": 74.9
  },
  "preview_at": "2026-05-26T14:37:00Z",  // 30 天后
  "decay_config": {
    "interest_half_life_days": 60.0,
    "strategy_half_life_days": 180.0,
    "consensus_half_life_days": 90.0
  }
}
```

**计算逻辑：** 完全在 Rust 层计算，不修改数据库。

---

#### P2-Decay-3：Decay 模拟器（未来 90 天衰减曲线）

**API 规格：**

```
GET /entities/{id}/decay-simulate?days=90&custom_halflife_interest=15

Response 200:
{
  "entity_id": "know-000001",
  "simulated_days": 90,
  "using_custom_halflife": true,
  "custom_halflife": {"interest": 15.0, "strategy": 365.0, "consensus": 60.0},
  "curve": [
    {"day": 0,  "interest": 80.0, "strategy": 90.0, "consensus": 60.0, "composite": 78.0},
    {"day": 1,  "interest": 79.8, "strategy": 89.9, "consensus": 59.9, "composite": 77.8},
    ...
    {"day": 30, "interest": 65.0, "strategy": 87.1, "consensus": 55.0, "composite": 71.2},
    ...
    {"day": 90, "interest": 42.0, "strategy": 80.0, "consensus": 42.0, "composite": 56.8}
  ],
  "half_life_markers": {
    "interest": {"day": 60, "value": 40.0},
    "strategy": {"day": 180, "value": 45.0},
    "consensus": {"day": 60, "value": 30.0}
  }
}
```

**用途：** 用户可对比不同半衰期配置下的衰减曲线，用于调参决策。

**验收标准：**
- [ ] `days` 参数最大支持 365（超过返回 400）
- [ ] `custom_halflife_*` 参数可选，不提供则使用实体当前配置
- [ ] `curve` 数组每一天都有一条记录（数据量大时支持 downsample）
- [ ] `half_life_markers` 标注每个维度首次达到 50% 的天数

**工时：** P2-Decay-1（3h） + P2-Decay-2（3h） + P2-Decay-3（4h） = **10h**

---

## 9.15 完整任务依赖图（最终版）

```
Phase 2 基座
└── P2-Entity-1 (GET /entities list)

Graph 任务线 ─────────────────────────────────────────────────────────────┐
├── P2-Graph-1 (neighbors basic) ──┬── P2-Graph-2 (depth)                │
│                                  ├── P2-Graph-3 (strength filter)        │
│                                  └── P2-Graph-4 (path)                   │
│                                                                      │
Timeline/Access ───────────────────────────┐                               │
├── P2-Timeline-1 (access record) ───────┼──→ P2-Timeline-2 (timeline query)  │
│                                          │                               │
Score History ────────────────────────────┼───────────────────────────────┘
├── P2-History-1 (write history) ────────┼──→ P2-History-2 (score trend API)
│                                          │
Fetch 任务线 ────────────────────────────────────────────────────────────┐
├── P2-Fetch-1 (URL fetch) ───→ P2-Fetch-2 (clean) ───→ P2-Fetch-3 (save)  │
│                                                                          │
Search 任务线 ───────────────────────────────────────────────────────────┐
└── P2-Search-1 (FAISS semantic search)                                     │
                                                                           │
Insight Engine ─────────────────────────────────────────────────────────┘
├── P2-Insight-1 (CRUD + maturity state machine) ──→ P2-Insight-2 (evolution trigger)
│                                                     └──→ P2-Insight-3 (export to knowledge)
│
Reference Intelligence ──────────────────────────────────────────────────┘
├── P2-Ref-1 (strength auto-calculation) ──→ P2-Ref-2 (bidirectional maintenance)
│
Decay Tuner ──────────────────────────────────────────────────────────────┘
├── P2-Decay-1 (per-entity half-life config) ──→ P2-Decay-2 (decay preview)
│                                                └──→ P2-Decay-3 (90-day simulator)
│
MCP Server（汇总所有下游能力，Phase 2 收尾）
└── P2-MCP-1 ← P2-Graph-1 + P2-Search-1 + P2-Fetch-3 + P2-Timeline-2

Phase 3（依赖 Phase 2 核心能力）
├── P3-UI-1 (React skeleton)  ←─┐
├── P3-UI-2 (entity detail)  ← P3-UI-1
├── P3-UI-3 (score panel)  ← P3-UI-1 + P2-History-2 + P2-Decay-3
├── P3-UI-4 (graph viz)  ← P2-Graph-1 + P3-UI-1
└── P3-UI-5 (PWA)  ← P3-UI-1
```

---

## 9.16 工时汇总（Phase 2 最终版）

| 任务线 | 任务数 | 总工时 | 优先级 |
|--------|--------|--------|--------|
| Entity List API | 1 | 4h | P0 |
| Graph API | 4 | 16h | P0 / P1 |
| Fetch Pipeline | 3 | 14h | P0 |
| Semantic Search (FAISS) | 1 | 8h | P1 |
| **L1 Timeline & Access** | 2 | **5h** | **P1** |
| **L2 Score History** | 2 | **5h** | **P1** |
| **L3 Insight Engine** | 3 | **13h** | **P2** |
| **L4 Reference Intelligence** | 2 | **9h** | **P2** |
| **L5 Decay Tuner** | 3 | **10h** | **P2** |
| MCP Server | 1 | 6h | P3 |
| **Phase 2 合计** | **24** | **90h** | |
| Phase 3 Web UI | 5 | 36h | |

---

*Phase 2 + Phase 3 任务拆解完成。Section 9 现为完整 600+ 行可执行 TDD 文档，覆盖 Entity List / Graph / Fetch / Search / Timeline / Score History / Insight Engine / Reference Intelligence / Decay Tuner / MCP 共 10 条任务线。*
