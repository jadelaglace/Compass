这是一个**个人认知操作系统（PCOS, Personal Cognitive Operating System）**的MVP版本，聚焦核心闭环：记录→关联→评分→反馈。

---

# PCOS v1.0 产品需求文档（PRD）

## 一、产品定位

**一句话定义**：基于Obsidian的本地优先个人知识引力场系统，通过三维评分引擎驱动信息相关度，支持Agent原生交互。

**核心价值主张**：
- 不制造"第二大脑"，而是制造"认知罗盘"
- 让高价值内容自然浮现，让过时内容优雅衰减
- 所有数据本地可控，Agent仅作为增强层

---

## 二、系统架构（简化版）

```
┌─────────────────────────────────────────────┐
│  交互层：Obsidian Mobile + 飞书Bot（A+C方案）  │
├─────────────────────────────────────────────┤
│  API网关：FastAPI（本地服务，Agent/外部访问）   │
├─────────────────────────────────────────────┤
│  核心引擎：                                   │
│  ├─ 文件监听服务（watchdog）                  │
│  ├─ 评分计算引擎（定时任务）                   │
│  ├─ 引用图谱管理器                            │
│  └─ 全文检索（FTS5）                          │
├─────────────────────────────────────────────┤
│  数据层：                                     │
│  ├─ Markdown文件（Obsidian Vault）            │
│  └─ SQLite（_system/pcos.db）                 │
└─────────────────────────────────────────────┘
```

---

## 三、功能模块详设

### 模块1：基础数据层（M1）

#### 1.1 Vault结构规范

```yaml
# 强制目录结构（安装时自动创建）
PCOS_Vault/
├── 00-Inbox/                    # 实时暂存（飞书Bot默认投递）
│   └── _template.md             # 模板：时间戳+来源+原始内容
├── 10-Direction/
│   ├── 11-Foundations/          # 基石维度（时间/空间/物质/意识）
│   ├── 12-Disciplines/          # 学科大类
│   └── 13-Specialties/          # 专业分支
├── 20-Knowledge/
│   ├── 21-Concepts/             # 高浓缩脑图/模型
│   └── 22-Cases/                # 应用案例
├── 30-Logs/
│   ├── 31-Realtime/             # 飞书Bot快速记录
│   ├── 32-ShortTerm/            # 短期片段（周回顾）
│   ├── 33-MidTerm/              # 中期压缩（月/季度）
│   └── 34-LongTerm/             # 长期存档（年度/人生节点）
├── 40-Insights/
│   ├── 41-Seeds/                # 灵感种子
│   ├── 42-Drafts/               # 概念草稿
│   └── 43-Systems/              # 体系化产出
├── _system/                     # 系统目录（Obsidian忽略）
│   ├── pcos.db                  # SQLite主库
│   ├── migrations/              # 数据库版本
│   └── cache/                   # 临时文件
└── _templates/                  # 模板库
    ├── direction.md
    ├── concept.md
    ├── case.md
    ├── log.md
    └── seed.md
```

#### 1.2 Frontmatter规范（强制）

```yaml
---
# 身份标识
id: "{{ulid}}"                    # 全局唯一标识
title: "字符串"
created: "2026-04-03T22:30:00+08:00"
modified: "2026-04-03T22:30:00+08:00"

# 层级定位（从Direction层继承）
layer: "direction|knowledge|case|log|insight"
subtype: "foundation|discipline|specialty|concept|case|realtime|short|mid|long|seed|draft|system"

# 标签系统（多维）
tags:
  - "#时间系/物理/相对论"         # 基石维度路径
  - "#战略权重高"                 # 评分标记
  - "#飞书导入"                   # 来源标记

# 评分（手动/自动）
scores:
  interest: 75                    # 0-100
  strategy: 60
  consensus: 80
  composite: 71.5                 # 自动计算
  calculated_at: "2026-04-03T22:30:00+08:00"
  history: []                     # 历史轨迹

# 引用（双向）
references:
  outgoing:                       # 我引用谁
    - {id: "xxx", type: "cites", context: "片段"}
  incoming:                       # 谁引用我（自动维护）
    - {id: "yyy", type: "applies", context: "片段"}

# 状态
status: "active|archived|orphan"
---
```

#### 1.3 SQLite Schema（M1核心）

```sql
-- 数据库版本：v1.0.0
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- 1. 实体主表
CREATE TABLE entities (
    id TEXT PRIMARY KEY,           -- ULID
    file_path TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL,    -- SHA256前16位
    layer TEXT NOT NULL CHECK(layer IN ('direction','knowledge','case','log','insight')),
    subtype TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active','archived','orphan')),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    metadata_json TEXT             -- 完整Frontmatter JSON
);

-- 2. 评分表（与实体1:1，但独立更新）
CREATE TABLE scores (
    entity_id TEXT PRIMARY KEY,
    interest REAL DEFAULT 50 CHECK(interest BETWEEN 0 AND 100),
    strategy REAL DEFAULT 50 CHECK(strategy BETWEEN 0 AND 100),
    consensus REAL DEFAULT 50 CHECK(consensus BETWEEN 0 AND 100),
    temporal_decay REAL DEFAULT 1.0,
    composite REAL GENERATED ALWAYS AS (
        (interest * 0.4 + strategy * 0.35 + consensus * 0.25) * temporal_decay
    ) STORED,
    calculated_at TIMESTAMP,
    history_json TEXT DEFAULT '[]',
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

-- 3. 标签表（扁平化，路径存储）
CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,     -- 如 "#时间系/物理/相对论"
    category TEXT,                 -- foundation|weight|source|temporal
    parent_path TEXT,
    usage_count INTEGER DEFAULT 0
);

CREATE TABLE entity_tags (
    entity_id TEXT,
    tag_path TEXT,
    confidence REAL DEFAULT 1.0,
    PRIMARY KEY (entity_id, tag_path),
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

-- 4. 引用关系表（有向图，双向维护）
CREATE TABLE refs (
    source_id TEXT,
    target_id TEXT,
    ref_type TEXT CHECK(ref_type IN ('cites','applies','inspired','parent','child')),
    strength REAL DEFAULT 1.0,
    context TEXT,                  -- 引用上下文摘要
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (source_id, target_id, ref_type),
    FOREIGN KEY (source_id) REFERENCES entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES entities(id) ON DELETE CASCADE
);

-- 5. 时间轴事件（所有交互记录）
CREATE TABLE timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT,
    event_type TEXT CHECK(event_type IN ('create','read','update','cite','reflect','agent_query')),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    intensity REAL DEFAULT 1.0,    -- 时长/重要性
    source TEXT,                   -- obsidian|feishu|agent|system
    metadata_json TEXT,
    FOREIGN KEY (entity_id) REFERENCES entities(id)
);

-- 6. 全文搜索（FTS5）
CREATE VIRTUAL TABLE search USING fts5(
    title, content,
    content='entities',
    content_rowid='rowid'
);

-- 触发器：自动同步FTS
CREATE TRIGGER entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO search(rowid, title, content) 
    VALUES (new.rowid, new.title, '');
END;

CREATE TRIGGER entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO search(search, rowid, title, content) 
    VALUES ('delete', old.rowid, '', '');
    INSERT INTO search(rowid, title, content) 
    VALUES (new.rowid, new.title, '');
END;

-- 索引
CREATE INDEX idx_entities_layer ON entities(layer, status);
CREATE INDEX idx_entities_modified ON entities(modified_at);
CREATE INDEX idx_scores_composite ON scores(composite DESC);
CREATE INDEX idx_timeline_entity ON timeline(entity_id, timestamp DESC);
CREATE INDEX idx_refs_source ON refs(source_id);
CREATE INDEX idx_refs_target ON refs(target_id);
```

---

### 模块2：文件同步引擎（M2）

#### 2.1 核心功能

| 功能 | 描述 | 触发条件 |
|------|------|---------|
| 文件监听 | 监控Vault变动 | watchdog文件系统事件 |
| 增量索引 | 解析Frontmatter更新数据库 | 文件修改且hash变化 |
| 引用解析 | 提取`[[id]]`或`[[标题\|id]]`格式链接 | 文件内容解析 |
| 反向链接维护 | 自动更新被引用文件的incoming | 引用变更时 |
| 冲突检测 | hash不一致时标记 | 定时校验 |

#### 2.2 引用语法规范

```markdown
<!-- 标准双向引用格式 -->
详见[[量子纠缠概念|ent_01hxyz123]]在[[意识系哲学|dir_01hxyz456]]中的讨论。

<!-- 自动生成引用区块（文件末尾） -->
---
## 引用网络
###  outgoing（我引用）
- [[ent_01hxyz123]] (cites): "关于量子纠缠的..."
- [[dir_01hxyz456]] (applies): "意识系哲学框架"

### incoming（引用我）
- [[log_01hxyz789]] (inspired): "2026-04-03 阅读感悟"
  > "这个概念让我想到..."
```

---

### 模块3：评分引擎（M3）

#### 3.1 评分维度算法

```python
# scoring_engine.py
from datetime import datetime, timedelta
import math

class ScoringEngine:
    def __init__(self, db):
        self.db = db
        self.weights = {
            'interest': 0.4,
            'strategy': 0.35,
            'consensus': 0.25
        }
    
    def calculate_interest(self, entity_id, days=30):
        """
        兴趣维度 = f(近期交互频率, 交互深度, 主动搜索次数)
        """
        events = self.db.get_timeline(entity_id, days=days)
        
        if not events:
            return 50  # 默认值
        
        # 时间衰减（越近越重要）
        now = datetime.now()
        scores = []
        for e in events:
            days_ago = (now - e['timestamp']).days
            time_weight = math.exp(-days_ago / 7)  # 7天半衰期
            
            # 事件类型权重
            type_weights = {
                'agent_query': 1.5,   # 主动询问 = 高兴趣
                'reflect': 1.3,       # 深度思考
                'update': 1.2,        # 编辑更新
                'cite': 1.0,          # 被引用
                'read': 0.8,          # 阅读
                'create': 0.5         # 创建（初始较低）
            }
            
            intensity = e['intensity'] * type_weights.get(e['event_type'], 1.0)
            scores.append(intensity * time_weight)
        
        # 归一化到0-100
        raw_score = sum(scores) * 10
        return min(100, max(0, raw_score))
    
    def calculate_strategy(self, entity_id):
        """
        战略维度 = 手动标记 + 趋势对齐 + 网络中心性
        """
        entity = self.db.get_entity(entity_id)
        
        # 基础：手动评分（如果存在）
        base = entity.get('manual_strategy', 50)
        
        # 增强：引用网络中的战略节点引用
        strategic_refs = self.db.query("""
            SELECT COUNT(*) FROM refs r
            JOIN entities e ON r.target_id = e.id
            WHERE r.source_id = ? 
            AND e.tags LIKE '%战略权重高%'
        """, (entity_id,))[0]
        
        boost = min(20, strategic_refs * 5)  # 每个战略引用+5，上限20
        
        return min(100, base + boost)
    
    def calculate_consensus(self, entity_id):
        """
        共识维度 = 被引用次数 + 跨层引用 + 标签通用性
        """
        # 入度中心性
        in_degree = self.db.count_incoming_refs(entity_id)
        
        # 跨层引用（如Knowledge被Direction引用）
        cross_layer = self.db.count_cross_layer_refs(entity_id)
        
        # 基础标签（如数学、物理）vs 细分标签
        tags = self.db.get_tags(entity_id)
        foundation_tags = [t for t in tags if t.startswith('#时间系') or 
                          t.startswith('#空间系') or 
                          t.startswith('#物质系') or 
                          t.startswith('#意识系')]
        generality = len(foundation_tags) * 10
        
        return min(100, in_degree * 5 + cross_layer * 10 + generality)
    
    def calculate_temporal_decay(self, entity_id):
        """
        时效衰减 = 指数衰减 + 突发激活
        """
        entity = self.db.get_entity(entity_id)
        last_active = self.db.get_last_activity(entity_id)
        
        days_inactive = (datetime.now() - last_active).days
        
        # 基础衰减：30天半衰期
        decay = math.exp(-days_inactive / 30)
        
        # 检查是否有突发激活（近期高互动）
        recent_burst = self.db.check_burst_activity(entity_id, days=7)
        if recent_burst:
            decay = min(1.0, decay * 1.5)  # 激活上限1.0
        
        return decay
    
    def recalculate(self, entity_id):
        """完整重算单个实体"""
        interest = self.calculate_interest(entity_id)
        strategy = self.calculate_strategy(entity_id)
        consensus = self.calculate_consensus(entity_id)
        decay = self.calculate_temporal_decay(entity_id)
        
        composite = (interest * self.weights['interest'] + 
                    strategy * self.weights['strategy'] + 
                    consensus * self.weights['consensus']) * decay
        
        # 保存历史
        history = self.db.get_score_history(entity_id)
        history.append({
            'timestamp': datetime.now().isoformat(),
            'interest': interest,
            'strategy': strategy,
            'consensus': consensus,
            'composite': composite,
            'decay': decay
        })
        history = history[-20:]  # 保留最近20次
        
        self.db.update_scores(entity_id, {
            'interest': interest,
            'strategy': strategy,
            'consensus': consensus,
            'temporal_decay': decay,
            'calculated_at': datetime.now(),
            'history': json.dumps(history)
        })
        
        return composite
    
    def batch_recalculate(self, active_only=True):
        """批量重算（定时任务）"""
        if active_only:
            entities = self.db.get_active_entities()
        else:
            entities = self.db.get_all_entities()
        
        for entity in entities:
            self.recalculate(entity['id'])
            time.sleep(0.01)  # 避免CPU占用过高
```

#### 3.2 定时任务

```yaml
# scheduler.yml
jobs:
  - name: "快速评分更新"
    cron: "0 * * * *"              # 每小时
    task: "update_recent"          # 最近7天活跃实体
    
  - name: "全量评分重算"
    cron: "0 3 * * 0"              # 每周日3点
    task: "recalculate_all"
    
  - name: "孤儿节点检测"
    cron: "0 4 1 * *"              # 每月1日4点
    task: "detect_orphans"
    params:
      threshold_days: 90
      action: "mark_archived"      # 或发送提醒
      
  - name: "数据库备份"
    cron: "0 2 * * *"              # 每日2点
    task: "backup_db"
```

---

### 模块4：API服务层（M4）

#### 4.1 FastAPI端点设计

```python
# main.py
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from typing import List, Optional

app = FastAPI(title="PCOS API", version="1.0.0")

# ========== 实体管理 ==========

@app.post("/entities", response_model=EntityResponse)
async def create_entity(req: CreateEntityRequest):
    """
    创建新实体，自动分配ID，写入文件和数据库
    """
    pass

@app.get("/entities/{entity_id}", response_model=EntityDetail)
async def get_entity(entity_id: str, include_refs: bool = True):
    """
    获取实体详情，包含评分和引用网络
    """
    pass

@app.put("/entities/{entity_id}")
async def update_entity(entity_id: str, req: UpdateRequest):
    """
    更新实体内容或元数据
    """
    pass

@app.delete("/entities/{entity_id}")
async def delete_entity(entity_id: str, hard: bool = False):
    """
    软删除（标记orphan）或硬删除
    """
    pass

# ========== 查询接口（核心） ==========

@app.post("/query", response_model=QueryResult)
async def dynamic_query(req: QueryRequest):
    """
    动态查询（Agent主要接口）
    
    Request:
    {
        "layers": ["knowledge", "case"],
        "tags": ["#空间系"],
        "score_min": 60,
        "fulltext": "渲染管线",
        "temporal_range": ["2026-01-01", "2026-04-03"],
        "sort_by": "composite_score",
        "limit": 20,
        "offset": 0
    }
    """
    pass

@app.get("/feed/{mode}")
async def get_feed(mode: str, limit: int = 10):
    """
    个性化信息流
    modes: explore(探索), consolidate(巩固), strategic(战略)
    """
    pass

@app.get("/graph/neighbors/{entity_id}")
async def get_neighbors(entity_id: str, depth: int = 1, min_strength: float = 0.5):
    """
    获取邻近节点（用于上下文准备）
    """
    pass

# ========== 评分接口 ==========

@app.post("/scores/{entity_id}/manual")
async def set_manual_score(entity_id: str, dimension: str, value: float):
    """
    手动设置评分维度（覆盖自动计算）
    """
    pass

@app.post("/scores/recalculate")
async def trigger_recalculate(entity_ids: Optional[List[str]] = None):
    """
    触发评分重算（不传则全部）
    """
    pass

# ========== 时间轴 ==========

@app.post("/timeline/events")
async def record_event(req: EventRequest):
    """
    记录交互事件（Agent调用、用户阅读等）
    """
    pass

@app.get("/timeline/{entity_id}")
async def get_timeline(entity_id: str, days: int = 30):
    pass

# ========== 系统管理 ==========

@app.get("/health")
async def health_check():
    return {
        "status": "ok",
        "db_connected": True,
        "vault_path": "/path/to/vault",
        "last_sync": "2026-04-03T22:30:00Z"
    }

@app.post("/sync/force")
async def force_sync():
    """
    强制全量同步（文件↔数据库）
    """
    pass

@app.get("/stats")
async def get_stats():
    """
    系统统计：实体数、评分分布、孤儿节点数等
    """
    pass
```

#### 4.2 Agent专用接口（扩展）

```python
@app.post("/agent/context")
async def get_agent_context(current_topic: str, max_tokens: int = 4000):
    """
    为Agent准备上下文（核心功能）
    
    返回结构：
    {
        "topic": "当前主题",
        "relevant_entities": [
            {
                "id": "xxx",
                "title": "xxx",
                "content_snippet": "..."  # 前500字
                "composite_score": 85,
                "relationship": "direct_cite|semantic_similar|strategic_align"
            }
        ],
        "strategic_focus": [...],      # 当前战略焦点
        "recent_logs": [...],          # 近期日志摘要
        "knowledge_gaps": [...]        # 高战略低共识领域（建议探索）
    }
    """
    pass

@app.post("/agent/suggest_links")
async def suggest_links(entity_id: str, top_k: int = 5):
    """
    基于内容相似度和网络结构建议潜在链接
    """
    pass

@app.post("/agent/action")
async def agent_action(actions: List[AgentAction]):
    """
    Agent批量操作（原子性）
    
    actions: [
        {"type": "create", "params": {...}},
        {"type": "link", "params": {...}},
        {"type": "score_feedback", "params": {...}}
    ]
    """
    pass
```

---

### 模块5：移动端方案A+C（M5）

#### 5.1 方案A：Obsidian Mobile适配

**功能限制下的最大化：**

| PCOS功能 | Obsidian Mobile实现 | 备注 |
|---------|-------------------|------|
| 快速记录 | 核心插件：QuickAdd | 绑定快捷键，自动入00-Inbox |
| 评分查看 | Dataview查询 | 表格展示composite_score |
| 引用创建 | 双向链接`[[` | 原生支持 |
| 标签选择 | 模板+建议 | Templater脚本 |
| 时间轴 | Daily Notes + Dataview | 按日聚合 |

**必装插件清单：**
```yaml
required_plugins:
  - QuickAdd:        # 快速捕获
      config: 
        - name: "Quick Log"
          type: capture
          target: "00-Inbox/{{date:YYYY-MM-DD}}-{{time:HHmm}}.md"
          template: "_templates/quick_log.md"
          
  - Dataview:        # 动态查询
      used_for: [评分展示, 引用列表, 时间轴聚合]
      
  - Templater:       # 模板自动化
      triggers: [文件创建, 标签继承]
      
  - Periodic Notes:  # 日志分层
      daily:  32-ShortTerm/
      weekly: 33-MidTerm/
      yearly: 34-LongTerm/
```

#### 5.2 方案C：飞书Bot（核心创新）

**架构：**
```
用户手机 → 飞书消息 → 自建Bot Server → PCOS API → 本地Vault
                ↑                                    ↓
         推送通知 ←──────────────────────── 文件变更Webhook
```

**Bot功能命令：**

| 命令 | 功能 | 示例 |
|------|------|------|
| `/q <内容>` | 快速记录到Inbox | `/q 量子纠缠与意识的关系值得探索` |
| `/r <关键词>` | 搜索相关实体 | `/r 量子纠缠` |
| `/s <id>` | 查看实体评分 | `/s ent_01hxyz123` |
| `/l <from> <to>` | 创建引用 | `/l ent_01hxyz123 ent_01hxyz456` |
| `/f` | 获取今日战略焦点 | `/f` |
| `/t <内容>` | 今日日志（直接进ShortTerm） | `/t 今天完成了PCOS架构设计` |

**消息格式设计：**

```yaml
# 快速记录响应（飞书卡片）
response_card:
  header: "已记录到 00-Inbox"
  content: "量子纠缠与意识的关系值得探索"
  metadata:
    - id: "ent_01hxyz789"
    - time: "2026-04-03 22:45"
    - suggested_tags: ["#意识系", "#物理"]
  actions:
    - button: "查看"
      url: "obsidian://open?vault=PCOS&file=00-Inbox/..."
    - button: "关联知识"
      command: "/link ent_01hxyz789"
    - button: "标记战略"
      command: "/score ent_01hxyz789 strategy 80"

# 搜索结果响应
search_result:
  list:
    - title: "量子纠缠概念"
      score: 85
      snippet: "量子纠缠是量子力学中..."
      tags: ["#物理", "#空间系"]
      actions: [查看, 引用, 评分]
```

**Bot技术栈：**
```python
# feishu_bot.py
from fastapi import FastAPI, Request
from feishu_bot_sdk import FeishuBot
import httpx

app = FastAPI()
bot = FeishuBot(app_id="xxx", app_secret="xxx")

@app.post("/webhook")
async def handle_message(request: Request):
    data = await request.json()
    message = bot.parse_message(data)
    
    if message.text.startswith("/q "):
        content = message.text[3:]
        # 调用PCOS API创建Inbox条目
        result = await create_inbox_entry(content, source="feishu")
        return bot.reply_card(message, format_quick_add_card(result))
    
    elif message.text.startswith("/r "):
        query = message.text[3:]
        results = await search_entities(query, limit=5)
        return bot.reply_card(message, format_search_card(results))
    
    # ... 其他命令

async def create_inbox_entry(content: str, source: str):
    async with httpx.AsyncClient() as client:
        resp = await client.post(
            "http://localhost:8000/entities",
            json={
                "layer": "log",
                "subtype": "realtime",
                "title": f"飞书记录 {datetime.now().strftime('%m-%d %H:%M')}",
                "content": content,
                "tags": [f"#{source}"],
                "source": source
            }
        )
        return resp.json()
```

---

### 模块6：B方案接口预留（M6，不实现）

```python
# 仅定义接口，返回501 Not Implemented

@app.post("/mobile/pwa/auth")
async def mobile_auth(): pass

@app.get("/mobile/pwa/sync")
async def mobile_sync(): pass

@app.post("/mobile/pwa/offline-queue")
async def offline_queue(): pass

# 数据库预留表（空）
CREATE TABLE mobile_sync_queue (
    id INTEGER PRIMARY KEY,
    device_id TEXT,
    action_json TEXT,
    created_at TIMESTAMP
) WITHOUT ROWID;
```

---

## 四、开发计划

### 阶段划分（8周）

```
Week 1-2:  M1+M2  基础数据层+文件同步
Week 3-4:  M3     评分引擎（核心难点）
Week 5-6:  M4+M5A API服务+Obsidian Mobile适配
Week 7:    M5C    飞书Bot开发
Week 8:    集成测试+文档+发布
```

### 详细任务分解

#### Week 1：基础架构搭建

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1-2 | Vault结构设计 | 目录结构、模板文件 | 结构符合PRD，模板包含完整Frontmatter |
| 3 | SQLite Schema实现 | migration文件 | 所有表创建成功，外键约束生效 |
| 4 | 文件监听服务 | watcher.py | 能检测文件增删改，解析Frontmatter |
| 5 | 基础索引逻辑 | indexer.py | 文件变更后30秒内同步到数据库 |
| 6-7 | 引用解析器 | ref_parser.py | 能提取`[[id]]`格式，维护refs表 |

#### Week 2：双向引用与冲突处理

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1-2 | 反向链接自动维护 | link_manager.py | 创建A→B时，B的incoming自动更新 |
| 3 | 引用区块生成 | ref_section.py | 文件末尾自动生成引用列表 |
| 4-5 | 冲突检测与标记 | conflict_detector.py | hash不一致时生成.conflict.md |
| 6 | 全量同步工具 | full_sync.py | 可手动重建整个数据库索引 |
| 7 | Week 1-2集成测试 | 测试报告 | 100个文件增删改查无错误 |

#### Week 3：评分算法（上）

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | 时间轴事件系统 | timeline.py | 所有操作记录到timeline表 |
| 2 | 兴趣维度算法 | interest_calc.py | 基于事件的兴趣评分计算正确 |
| 3 | 战略维度算法 | strategy_calc.py | 手动标记+自动增强逻辑 |
| 4 | 共识维度算法 | consensus_calc.py | 网络中心性计算正确 |
| 5 | 时效衰减算法 | decay_calc.py | 指数衰减+突发激活逻辑 |
| 6 | 综合评分合成 | composite.py | 加权公式正确，历史记录保存 |
| 7 | 单实体评分测试 | 单元测试 | 10个测试用例全部通过 |

#### Week 4：评分引擎（下）

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | 批量重算任务 | batch_recalc.py | 能处理1000个实体不OOM |
| 2 | 定时任务调度 | scheduler.py | cron表达式解析，任务执行 |
| 3 | 孤儿节点检测 | orphan_detector.py | 90天无引用自动标记 |
| 4 | 评分API端点 | /scores/* | REST接口符合PRD |
| 5 | 评分查询优化 | 索引优化 | 评分排序查询<100ms |
| 6-7 | 评分引擎集成测试 | 测试报告 | 模拟30天交互，评分曲线合理 |

#### Week 5：API服务（上）

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | FastAPI项目骨架 | main.py | 服务启动，/health正常 |
| 2 | 实体CRUD端点 | /entities/* | Postman测试通过 |
| 3 | 查询接口实现 | /query | 支持所有过滤条件 |
| 4 | Feed接口实现 | /feed/* | 三种模式返回正确 |
| 5 | 图查询接口 | /graph/* | 邻居查询支持depth参数 |
| 6 | API文档生成 | Swagger UI | 自动文档可访问 |
| 7 | API单元测试 | pytest | 覆盖率>80% |

#### Week 6：API服务（下）+ Obsidian Mobile

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | Agent上下文接口 | /agent/context | 返回结构符合PRD |
| 2 | Agent建议接口 | /agent/suggest_links | 建议准确性人工评估 |
| 3 | Obsidian插件开发 | pcos-plugin/ | 能显示评分、快速记录 |
| 4 | Dataview查询模板 | 查询库 | 10个常用查询 |
| 5 | Templater自动化 | 脚本库 | 自动标签继承、ID生成 |
| 6 | Mobile界面适配 | CSS调整 | 关键信息手机可读 |
| 7 | Week 5-6集成测试 | 测试报告 | API+插件联调通过 |

#### Week 7：飞书Bot

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | 飞书Bot注册配置 | 机器人 | 能接收和发送消息 |
| 2 | 快速记录命令 | /q | 消息→Inbox文件，<3秒 |
| 3 | 搜索命令 | /r | 返回卡片格式正确 |
| 4 | 评分/引用命令 | /s, /l | 操作同步到数据库 |
| 5 | 今日焦点 /f | 战略Feed | 返回Top 5战略实体 |
| 6 | 消息模板优化 | 卡片JSON | 关键信息一屏可见 |
| 7 | Bot集成测试 | 测试报告 | 5个核心命令全部通过 |

#### Week 8：收尾

| 天数 | 任务 | 产出 | 验收标准 |
|------|------|------|---------|
| 1 | 端到端测试 | 完整流程 | 飞书记录→Obsidian查看→评分更新→Agent查询 |
| 2 | 性能测试 | 报告 | 1000实体，查询<200ms |
| 3 | 文档编写 | README, API文档 | 新用户可独立部署 |
| 4 | 部署脚本 | install.sh | 一键安装，自动配置 |
| 5 | 备份与恢复 | backup.py | 数据库+文件完整备份 |
| 6 | Bug修复 | - | 所有P0、P1修复 |
| 7 | 发布v1.0.0 | GitHub Release | 版本标签，Release Note |

---

## 五、测试验收标准

### 5.1 功能测试矩阵

| 模块 | 测试项 | 测试方法 | 通过标准 |
|------|--------|---------|---------|
| **M1** | Vault结构创建 | 运行install.sh | 所有目录、模板文件存在 |
| | Frontmatter解析 | 单元测试 | 100种变体解析正确 |
| | 数据库迁移 | 升级测试 | v1.0.0→v1.0.1数据不丢失 |
| **M2** | 文件监听 | 自动化测试 | 100次文件操作，99次30秒内同步 |
| | 引用解析 | 单元测试 | `[[id]]`, `[[text\|id]]`都识别 |
| | 反向链接 | 集成测试 | 创建A→B后，B.incoming包含A |
| | 冲突检测 | 模拟测试 | 手动修改文件后触发冲突标记 |
| **M3** | 兴趣算法 | 单元测试 | 给定事件序列，评分符合预期 |
| | 时效衰减 | 单元测试 | 30天无活动，decay≈0.37 |
| | 批量重算 | 性能测试 | 1000实体<5分钟 |
| | 孤儿检测 | 集成测试 | 90天无引用自动标记orphan |
| **M4** | 实体CRUD | API测试 | Postman集合100%通过 |
| | 动态查询 | 模糊测试 | 随机查询条件不报错 |
| | Feed推荐 | 人工评估 | 10次推荐，7次以上相关性可接受 |
| | Agent上下文 | 集成测试 | 响应时间<500ms，结构正确 |
| **M5A** | Obsidian插件 | 手动测试 | iOS/Android上安装、启用正常 |
| | 评分显示 | 截图对比 | Dataview表格显示composite |
| | 快速记录 | 操作测试 | QuickAdd 3步内完成记录 |
| **M5C** | 飞书Bot连通 | 端到端测试 | 消息发送→Bot响应<3秒 |
| | 命令准确性 | 测试用例 | 20条命令，错误率<5% |
| | 卡片渲染 | 视觉检查 | 关键信息飞书APP一屏可见 |

### 5.2 性能指标（SLA）

| 指标 | 目标值 | 测试方法 |
|------|--------|---------|
| 文件同步延迟 | P95 < 30秒 | 监控100次文件操作 |
| API响应时间 | P95 < 200ms | k6负载测试 |
| 评分重算速度 | >100实体/分钟 | 批量任务监控 |
| 全文搜索延迟 | P95 < 500ms | 1000实体，随机关键词 |
| 飞书Bot响应 | < 3秒 | 端到端计时 |
| 数据库查询 | 简单查询<50ms | SQLite EXPLAIN分析 |

### 5.3 可靠性指标

| 指标 | 目标值 | 测试方法 |
|------|--------|---------|
| 数据一致性 | 99.9% | 文件hash与数据库对比 |
| 备份成功率 | 100% | 每日自动备份监控 |
| 系统可用性 | 99%（排除维护窗口） | 心跳监控 |
| 数据丢失风险 | 0（本地文件+Git+备份） | 灾难恢复演练 |

### 5.4 验收测试用例（关键场景）

#### 场景1：完整记录流程
```gherkin
Given 用户在飞书Bot发送 "/q 量子纠缠与意识的关系"
When Bot处理完成
Then 1. 00-Inbox/下创建新文件，包含完整Frontmatter
    And 2. 数据库entities表存在对应记录
    And 3. timeline表记录create事件
    And 4. 用户收到飞书卡片，显示ID和建议标签
    And 5. 用户在Obsidian Mobile可查看该文件
```

#### 场景2：引用建立与评分反馈
```gherkin
Given 实体A（知识层）和实体B（日志层）已存在
When 用户在Obsidian编辑A，添加"[[B]]"引用
Then 1. 文件保存后，refs表创建双向记录
    And 2. B的consensus_score增加（被引用加分）
    And 3. A的引用区块自动更新
    And 4. 次日评分重算后，composite分数更新
```

#### 场景3：Agent上下文准备
```gherkin
Given 用户询问Agent关于"量子计算"
When Agent调用GET /agent/context?topic=量子计算
Then 1. 返回的relevant_entities按composite_score排序
    And 2. 包含战略焦点和近期日志
    And 3. 建议的knowledge_gaps非空
    And 4. 总token数<4000
```

#### 场景4：孤儿节点处理
```gherkin
Given 实体C创建于90天前，无任何引用
When 定时任务运行孤儿检测
Then 1. C的status标记为orphan
    And 2. 用户收到飞书Bot提醒（可选）
    And 3. C的composite_score应用额外衰减
    And 4. Obsidian中C显示为归档样式
```

---

## 六、部署架构

### 本地部署（推荐）

```yaml
# docker-compose.yml（可选，也可直接用Python）
version: '3.8'
services:
  pcos-api:
    build: .
    ports:
      - "8000:8000"
    volumes:
      - /path/to/vault:/vault:rw
      - ./data:/app/data
    environment:
      - VAULT_PATH=/vault
      - DB_PATH=/vault/_system/pcos.db
      - FEISHU_APP_ID=${FEISHU_APP_ID}
      - FEISHU_APP_SECRET=${FEISHU_APP_SECRET}
    restart: unless-stopped
  
  # 可选：用于飞书Bot Webhook的内网穿透
  ngrok:
    image: ngrok/ngrok
    command: http pcos-api:8000
    environment:
      - NGROK_AUTHTOKEN=${NGROK_TOKEN}
    ports:
      - "4040:4040"  # ngrok管理界面
```

### 目录权限要求

```bash
# 安装脚本检查清单
PCOS_Vault/
├── 读写权限: 所有内容目录
├── 执行权限: _system/（数据库存放）
└── Git初始化: 自动commit钩子

# 自动配置
./install.sh /path/to/vault  # 一键初始化
```

---

## 七、风险与应对

| 风险 | 可能性 | 影响 | 应对 |
|------|--------|------|------|
| SQLite并发性能瓶颈 | 中 | 高 | WAL模式+读写分离（查询副本） |
| 飞书Bot网络依赖 | 高 | 中 | 本地队列，失败重试，离线提醒 |
| 评分算法不准确 | 中 | 高 | 保留手动覆盖，人工反馈调参 |
| Obsidian插件API限制 | 低 | 中 | 优先保证文件层兼容，插件增强 |
| 数据丢失 | 低 | 极高 | Git版本+每日备份+导出脚本 |

---

## 八、版本规划

| 版本 | 功能 | 时间 |
|------|------|------|
| v1.0.0 | MVP：文件同步+评分+API+飞书Bot | 8周后 |
| v1.1.0 | 高级查询+Agent自动标签建议 | +4周 |
| v1.2.0 | 可视化图谱+评分趋势分析 | +4周 |
| v2.0.0 | 多Vault同步+协作功能（可选） | 待定 |
