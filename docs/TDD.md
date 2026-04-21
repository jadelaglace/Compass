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
| **P2-Graph-1** | `GET /graph/neighbors/{id}` 基础邻居查询 | `feat/p2-graph-1-neighbors-basic` | P0 | 无 | 4h |
| **P2-Graph-2** | `GET /graph/neighbors/{id}?depth=N` 深度查询 | `feat/p2-graph-2-neighbors-depth` | P0 | P2-Graph-1 | 3h |
| **P2-Graph-3** | `GET /graph/neighbors/{id}?min_strength=X` 强度过滤 | `feat/p2-graph-3-neighbors-filter` | P1 | P2-Graph-1 | 3h |
| **P2-Graph-4** | `GET /graph/path?from=X&to=Y` 最短路径 | `feat/p2-graph-4-path` | P1 | P2-Graph-1 | 6h |
| **P2-Fetch-1** | `POST /fetch` URL 抓取（原始内容） | `feat/p2-fetch-1-url-fetch` | P0 | 无 | 4h |
| **P2-Fetch-2** | `POST /fetch/clean` 内容清洗结构化 | `feat/p2-fetch-2-clean` | P0 | P2-Fetch-1 | 6h |
| **P2-Fetch-3** | `POST /fetch/save` 清洗结果写入 Vault | `feat/p2-fetch-3-save` | P0 | P2-Fetch-2 | 4h |
| **P2-Search-1** | `GET /search?q=...` 语义搜索（FAISS） | `feat/p2-search-semantic` | P1 | 无 | 8h |
| **P2-MCP-1** | MCP Server 适配层（基础 Tools） | `feat/p2-mcp-server` | P2 | 无 | 6h |

### 9.2 任务详细设计

[详细设计内容见原 TDD，此处省略以保持简洁]

---

*Phase 2 任务拆解完成。TDD 已更新，可直接进入开发。*

---

*CTO 评估完成。Rust 核心 + Python 胶水层架构可行，8 周可达。核心增量是 Rust 学习成本，风险可控。*
