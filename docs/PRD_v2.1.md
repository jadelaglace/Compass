# Compass PRD v2.1 — 完整规格升级说明书

**版本**: 2.1  
**日期**: 2026-04-20  
**状态**: 开发就绪  
**分支**: `docs/prd-v2.1`  

---

## 文档说明

本文档基于以下三份原始文档整合升级：

1. **规划A版本**: `archive/规划A版本/PKM_Universe_PRD_v1.0.md` — 完整技术规格
2. **规划B版本**: `archive/规划B版本/PCOS_v1.0_PRD.md` — 8周MVP详细分解
3. **v2.0版本**: `docs/PRD_v2.0.md` — Phase 2功能分档规划

**v2.1核心任务**: 将v2.0的"功能清单"升级为"完整可执行规格"。

---

## 1. 版本历史与变更记录

### 1.1 版本演进

| 版本 | 日期 | 核心内容 |
|------|------|----------|
| v1.0-A | 2026-04-03 | PKM Universe 完整技术规格 |
| v1.0-B | 2026-04-03 | PCOS 8周MVP详细分解 |
| v2.0 | 2026-04-17 | Phase 2功能分档（Easy/Medium/Hard/Very Hard） |
| **v2.1** | 2026-04-20 | 整合A/B版本完整规格，填补v2.0技术缺口 |

### 1.2 v2.0 → v2.1 关键增补

| 缺失项（v2.0） | 增补内容（v2.1） | 重要性 |
|----------------|------------------|--------|
| SQLite Schema | 完整DDL含触发器/索引/FTS5 | P0 |
| API OpenAPI Spec | YAML格式完整接口定义 | P0 |
| MCP Server实现 | `@mcp.tool()`完整代码 | P0 |
| Case系统模型 | `ApplicationContext`/`OutcomeInfo` | P1 |
| Insight系统模型 | 成熟度演化算法 | P1 |
| 界面原型 | 引力场/评分面板ASCII图 | P1 |
| Gherkin场景 | 4个端到端测试场景 | P1 |
| 部署架构 | docker-compose.yml | P2 |
| 命名规范 | ID/Tag/路径格式表 | P2 |
| 风险分析 | 完整风险矩阵 | P2 |

---

## 2. 数据模型完整规范

### 2.1 核心实体关系图

```
┌──────────────────┐       ┌──────────────────┐       ┌──────────────────┐
│   Knowledge      │◄─────►│      Tag         │◄─────►│   Category       │
│   (知识原子)      │  M:N  │   (标签)          │  M:N  │   (分类/架构层)   │
└────────┬─────────┘       └──────────────────┘       └──────────────────┘
         │
         │ 1:N
         ▼
┌──────────────────┐       ┌──────────────────┐
│     Case         │◄─────►│   Reference      │
│   (案例标本)      │  双向  │   (双向引用)      │
└────────┬─────────┘       └──────────────────┘
         │
         │ 1:N
         ▼
┌──────────────────┐
│  ScoreHistory    │
│ (评分历史追踪)    │
└──────────────────┘
```

### 2.2 Knowledge（知识原子）

```python
class KnowledgeBase(BaseModel):
    """知识原子基础模型 - 对应Markdown文件"""

    # 标识
    id: str = Field(..., pattern=r"^know-[0-9]{6}$")
    title: str = Field(..., min_length=1, max_length=200)
    slug: str = Field(..., pattern=r"^[a-z0-9-]+$")  # URL友好标识

    # 内容
    content: str = Field(..., description="Markdown正文")
    brain_map: Optional[str] = Field(None, description="Mermaid脑图代码")
    summary: Optional[str] = Field(None, max_length=500, description="AI生成摘要")

    # 分类与标签
    category_path: List[str] = Field(..., description=["架构层", "学科系列", "数学"])
    tags: List[str] = Field(default=[], description=["#数学", "#意识系列"])

    # 评分系统 (核心)
    relevance_score: RelevanceScore

    # 关联
    linked_cases: List[str] = Field(default=[], description=["case-001", "case-002"])
    linked_knowledge: List[str] = Field(default=[], description=["know-002"])

    # 元数据
    source: Optional[SourceInfo] = None
    created_at: datetime
    updated_at: datetime
    accessed_at: Optional[datetime] = None  # 最后访问时间
    access_count: int = Field(default=0, ge=0)

    # 状态
    status: Literal["seed", "sprout", "mature", "archived"] = "seed"
```

### 2.3 RelevanceScore（三维评分模型）

```python
class RelevanceScore(BaseModel):
    """三维评分模型"""
    interest_now: float = Field(..., ge=0, le=100, description="当前兴趣度")
    should_care_future: float = Field(..., ge=0, le=100, description="未来战略值")
    consensus_past: float = Field(..., ge=0, le=100, description="共识基础度")

    # 计算属性
    total: float = Field(..., ge=0, le=100, description="加权综合分")

    # 历史追踪
    history: List[ScoreRecord] = Field(default=[], max_length=100)

    # 权重配置 (可覆盖全局默认)
    weights: Optional[ScoreWeights] = None

class ScoreRecord(BaseModel):
    """单次评分记录"""
    timestamp: datetime
    dimension: Literal["interest_now", "should_care_future", "consensus_past", "total"]
    old_value: float
    new_value: float
    reason: str = Field(..., max_length=200)
    trigger: Literal["manual", "auto_decay", "access_boost", "application_boost", "agent_suggestion"]

class ScoreWeights(BaseModel):
    """评分权重"""
    interest_now: float = 0.4
    should_care_future: float = 0.35
    consensus_past: float = 0.25
```

### 2.4 Case（案例标本）

```python
class Case(BaseModel):
    """实践案例模型"""

    id: str = Field(..., pattern=r"^case-[0-9]{6}$")
    title: str

    # 应用场景
    context: ApplicationContext

    # 内容
    content: str
    outcome: OutcomeInfo
    reflection: Optional[str] = None  # 事后复盘

    # 关联
    applied_knowledge: List[str] = Field(..., description="应用了哪些知识")
    tags: List[str] = []

    # 时间
    happened_at: datetime
    created_at: datetime

class ApplicationContext(BaseModel):
    """应用场景"""
    domain: str  # 领域：工作/学习/生活
    project: Optional[str] = None
    stakeholders: List[str] = []
    constraints: List[str] = []

class OutcomeInfo(BaseModel):
    """结果信息"""
    result: str  # 结果描述
    lessons: str  # 经验教训
    success_rating: int = Field(..., ge=1, le=10)  # 成功度评分
```

### 2.5 Log & Insight（日志与感悟）

```python
class LogEntry(BaseModel):
    """日志条目 - 时间切片"""

    id: str = Field(..., pattern=r"^log-[0-9]{8}-[0-9]{4}$")  # 日期+序号
    type: Literal["realtime", "short_term", "medium_term", "long_term"]

    content: str
    mood: Optional[str] = None
    energy_level: Optional[int] = Field(None, ge=1, le=10)

    # 关联
    related_knowledge: List[str] = []
    related_cases: List[str] = []

    timestamp: datetime
    location: Optional[str] = None

class Insight(BaseModel):
    """感悟/思想结晶"""

    id: str = Field(..., pattern=r"^ins-[0-9]{6}$")
    title: str
    content: str

    # 成熟度
    maturity: Literal["spark", "framework", "mature"] = "spark"

    # 衍生
    derived_from: List[str] = []  # 源自哪些知识/日志
    evolved_into: Optional[str] = None  # 演化为哪个成熟知识

    created_at: datetime
    refined_at: Optional[datetime] = None
```

### 2.6 Reference（双向引用）

```python
class Reference(BaseModel):
    """双向引用关系"""
    
    source_id: str  # 引用方
    target_id: str  # 被引用方
    ref_type: Literal["cites", "applies", "inspired", "parent", "child"] = "cites"
    strength: float = Field(default=1.0, ge=0.0, le=1.0)
    context: Optional[str] = None  # 引用上下文摘要
    created_at: datetime = Field(default_factory=datetime.now)
```

---

## 4. SQLite Schema（完整DDL）

### 4.1 主表：entities（知识实体）

```sql
-- 数据库版本：v2.1.0
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;

-- 1. 实体主表（统一存储所有类型实体）
CREATE TABLE entities (
    id TEXT PRIMARY KEY,           -- 格式: know-000001, case-000001, log-20260420-0001
    file_path TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    content_hash TEXT NOT NULL,    -- SHA256前16位
    entity_type TEXT NOT NULL CHECK(entity_type IN ('knowledge', 'case', 'log', 'insight')),
    layer TEXT CHECK(layer IN ('direction', 'knowledge', 'case', 'log', 'insight')),
    subtype TEXT,
    status TEXT DEFAULT 'active' CHECK(status IN ('active', 'archived', 'orphan')),
    
    -- 评分字段（缓存）
    score_interest REAL DEFAULT 50 CHECK(score_interest BETWEEN 0 AND 100),
    score_strategy REAL DEFAULT 50 CHECK(score_strategy BETWEEN 0 AND 100),
    score_consensus REAL DEFAULT 50 CHECK(score_consensus BETWEEN 0 AND 100),
    score_temporal_decay REAL DEFAULT 1.0,
    score_composite REAL GENERATED ALWAYS AS (
        (score_interest * 0.4 + score_strategy * 0.35 + score_consensus * 0.25) * score_temporal_decay
    ) STORED,
    score_calculated_at TIMESTAMP,
    score_history_json TEXT DEFAULT '[]',
    
    -- 元数据
    source_type TEXT,
    source_title TEXT,
    source_url TEXT,
    
    -- 时间戳
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    accessed_at TIMESTAMP,
    access_count INTEGER DEFAULT 0,
    
    -- 全文搜索关联
    fts_doc_id INTEGER
);

-- 索引优化
CREATE INDEX idx_entities_type ON entities(entity_type, status);
CREATE INDEX idx_entities_layer ON entities(layer, status);
CREATE INDEX idx_entities_modified ON entities(modified_at);
CREATE INDEX idx_entities_score ON entities(score_composite DESC);
CREATE INDEX idx_entities_accessed ON entities(accessed_at DESC);
```

### 4.2 评分历史表

```sql
-- 2. 评分历史表
CREATE TABLE score_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT NOT NULL,
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    dimension TEXT NOT NULL CHECK(dimension IN ('interest', 'strategy', 'consensus', 'composite')),
    old_value REAL NOT NULL,
    new_value REAL NOT NULL,
    reason TEXT,
    trigger_type TEXT CHECK(trigger_type IN ('manual', 'auto_decay', 'access_boost', 'application_boost', 'agent_suggestion')),
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

CREATE INDEX idx_score_history_entity ON score_history(entity_id, timestamp DESC);
CREATE INDEX idx_score_history_time ON score_history(timestamp);
```

### 4.3 引用关系表

```sql
-- 3. 引用关系表（有向图，双向维护）
CREATE TABLE refs (
    source_id TEXT,
    target_id TEXT,
    ref_type TEXT CHECK(ref_type IN ('cites', 'applies', 'inspired', 'parent', 'child')),
    strength REAL DEFAULT 1.0 CHECK(strength BETWEEN 0 AND 1),
    context TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (source_id, target_id, ref_type),
    FOREIGN KEY (source_id) REFERENCES entities(id) ON DELETE CASCADE,
    FOREIGN KEY (target_id) REFERENCES entities(id) ON DELETE CASCADE
);

CREATE INDEX idx_refs_source ON refs(source_id);
CREATE INDEX idx_refs_target ON refs(target_id);
```

### 4.4 标签系统表

```sql
-- 4. 标签本体表
CREATE TABLE tags (
    id TEXT PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,     -- 如 "#时间系/物理/相对论"
    category TEXT CHECK(category IN ('foundation', 'discipline', 'weight', 'source', 'temporal')),
    description TEXT,
    parent_path TEXT,
    usage_count INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_path) REFERENCES tags(path)
);

CREATE TABLE entity_tags (
    entity_id TEXT,
    tag_path TEXT,
    confidence REAL DEFAULT 1.0 CHECK(confidence BETWEEN 0 AND 1),
    PRIMARY KEY (entity_id, tag_path),
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_path) REFERENCES tags(path) ON DELETE CASCADE
);

CREATE INDEX idx_tags_category ON tags(category);
CREATE INDEX idx_tags_usage ON tags(usage_count DESC);
```

### 4.5 时间轴与向量索引表

```sql
-- 5. 时间轴事件表
CREATE TABLE timeline (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    entity_id TEXT,
    event_type TEXT CHECK(event_type IN ('create', 'read', 'update', 'cite', 'reflect', 'agent_query')),
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    intensity REAL DEFAULT 1.0,
    source TEXT,                   -- obsidian|feishu|agent|system
    metadata_json TEXT,
    FOREIGN KEY (entity_id) REFERENCES entities(id)
);

CREATE INDEX idx_timeline_entity ON timeline(entity_id, timestamp DESC);
CREATE INDEX idx_timeline_time ON timeline(timestamp DESC);

-- 6. 向量索引表 (FAISS辅助)
CREATE TABLE embeddings (
    entity_id TEXT PRIMARY KEY,
    vector BLOB NOT NULL,          -- 序列化的向量
    model_name TEXT NOT NULL,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
);

-- 7. 全文搜索虚拟表 (FTS5)
CREATE VIRTUAL TABLE entities_fts USING fts5(
    title,
    content UNINDEXED,  -- 正文不参与索引，通过程序处理
    content='entities',
    content_rowid='rowid'
);

-- 触发器：保持FTS索引同步
CREATE TRIGGER entities_ai AFTER INSERT ON entities BEGIN
    INSERT INTO entities_fts(rowid, title, content) 
    VALUES (new.rowid, new.title, '');
END;

CREATE TRIGGER entities_ad AFTER DELETE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, title, content) 
    VALUES ('delete', old.rowid, old.title, '');
END;

CREATE TRIGGER entities_au AFTER UPDATE ON entities BEGIN
    INSERT INTO entities_fts(entities_fts, rowid, title, content) 
    VALUES ('delete', old.rowid, old.title, '');
    INSERT INTO entities_fts(rowid, title, content) 
    VALUES (new.rowid, new.title, '');
END;
```

---

## 5. API 接口规范（OpenAPI）

### 5.1 核心接口概览

```yaml
openapi: 3.0.0
info:
  title: Compass API
  version: 2.1.0
  description: 个人认知罗盘系统API

servers:
  - url: http://localhost:8000/api/v1
    description: 本地开发服务器

paths:
  /health:
    get:
      summary: 健康检查
      responses:
        200:
          description: 系统正常运行
          content:
            application/json:
              schema:
                type: object
                properties:
                  status:
                    type: string
                    example: ok
                  version:
                    type: string
                    example: 2.1.0

  /entities:
    get:
      summary: 查询实体列表
      parameters:
        - name: type
          in: query
          schema:
            type: string
            enum: [knowledge, case, log, insight]
        - name: min_score
          in: query
          schema:
            type: number
            default: 50
        - name: tags
          in: query
          schema:
            type: array
            items:
              type: string
        - name: limit
          in: query
          schema:
            type: integer
            default: 20
        - name: offset
          in: query
          schema:
            type: integer
            default: 0
      responses:
        200:
          description: 实体列表
          content:
            application/json:
              schema:
                type: object
                properties:
                  items:
                    type: array
                    items:
                      $ref: '#/components/schemas/Entity'
                  total:
                    type: integer
                  has_more:
                    type: boolean

    post:
      summary: 创建实体
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/EntityCreate'
      responses:
        201:
          description: 创建成功
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Entity'
          headers:
            Location:
              description: 新资源URL
              schema:
                type: string

  /entities/{id}:
    get:
      summary: 获取实体详情
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: with_refs
          in: query
          schema:
            type: boolean
            default: true
      responses:
        200:
          description: 实体详情
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/EntityDetail'

    put:
      summary: 更新实体
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      requestBody:
        required: true
        content:
          application/json:
            schema:
              $ref: '#/components/schemas/EntityUpdate'
      responses:
        200:
          description: 更新成功
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Entity'

    delete:
      summary: 删除实体（软删除）
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: hard
          in: query
          schema:
            type: boolean
            default: false
      responses:
        204:
          description: 删除成功

  /entities/{id}/score:
    patch:
      summary: 调整评分
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
      requestBody:
        required: true
        content:
          application/json:
            schema:
              oneOf:
                - type: object
                  properties:
                    dimension:
                      type: string
                      enum: [interest, strategy, consensus]
                    value:
                      type: number
                    reason:
                      type: string
                - type: object
                  properties:
                    adjustments:
                      type: array
                      items:
                        type: object
                        properties:
                          dimension:
                            type: string
                          delta:
                            type: number
                    reason:
                      type: string
      responses:
        200:
          description: 评分更新成功
          content:
            application/json:
              schema:
                $ref: '#/components/schemas/Score'

  /entities/{id}/score/history:
    get:
      summary: 获取评分历史
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: days
          in: query
          schema:
            type: integer
            default: 90
      responses:
        200:
          description: 评分历史列表
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/ScoreRecord'

  /query:
    post:
      summary: 动态查询（Agent主要接口）
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                layers:
                  type: array
                  items:
                    type: string
                    enum: [knowledge, case, log, insight]
                tags:
                  type: array
                  items:
                    type: string
                score_min:
                  type: number
                  default: 50
                fulltext:
                  type: string
                semantic_query:
                  type: string
                temporal_range:
                  type: array
                  items:
                    type: string
                  example: ["2026-01-01", "2026-04-20"]
                sort_by:
                  type: string
                  enum: [relevance, updated, created, accessed]
                  default: relevance
                limit:
                  type: integer
                  default: 20
                offset:
                  type: integer
                  default: 0
      responses:
        200:
          description: 查询结果
          content:
            application/json:
              schema:
                type: object
                properties:
                  items:
                    type: array
                    items:
                      $ref: '#/components/schemas/Entity'
                  total:
                    type: integer
                  facets:
                    type: object
                    properties:
                      score_distribution:
                        type: object
                      top_tags:
                        type: array

  /feed/{mode}:
    get:
      summary: 个性化信息流
      parameters:
        - name: mode
          in: path
          required: true
          schema:
            type: string
            enum: [explore, consolidate, strategic]
        - name: limit
          in: query
          schema:
            type: integer
            default: 10
      responses:
        200:
          description: 信息流列表
          content:
            application/json:
              schema:
                type: array
                items:
                  $ref: '#/components/schemas/Entity'

  /graph/neighbors/{id}:
    get:
      summary: 获取邻近节点
      parameters:
        - name: id
          in: path
          required: true
          schema:
            type: string
        - name: depth
          in: query
          schema:
            type: integer
            default: 1
        - name: min_strength
          in: query
          schema:
            type: number
            default: 0.5
      responses:
        200:
          description: 邻近节点图
          content:
            application/json:
              schema:
                type: object
                properties:
                  nodes:
                    type: array
                    items:
                      $ref: '#/components/schemas/GraphNode'
                  edges:
                    type: array
                    items:
                      $ref: '#/components/schemas/GraphEdge'

  /search:
    post:
      summary: 高级语义搜索
      requestBody:
        required: true
        content:
          application/json:
            schema:
              type: object
              properties:
                query:
                  type: string
                semantic_weight:
                  type: number
                  default: 0.6
                score_weight:
                  type: number
                  default: 0.4
                filters:
                  type: object
                  properties:
                    tags:
                      type: array
                      items:
                        type: string
                    date_range:
                      type: object
                      properties:
                        start:
                          type: string
                        end:
                          type: string
      responses:
        200:
          description: 搜索结果
          content:
            application/json:
              schema:
                type: array
                items:
                  type: object
                  properties:
                    entity:
                      $ref: '#/components/schemas/Entity'
                    match_score:
                      type: number
                    highlights:
                      type: array
                      items:
                        type: string

  /sync/force:
    post:
      summary: 强制全量同步
      responses:
        200:
          description: 同步完成
          content:
            application/json:
              schema:
                type: object
                properties:
                  synced_count:
                    type: integer
                  errors:
                    type: array
                    items:
                      type: object

components:
  schemas:
    Entity:
      type: object
      properties:
        id:
          type: string
        title:
          type: string
        entity_type:
          type: string
        score:
          $ref: '#/components/schemas/Score'
        tags:
          type: array
          items:
            type: string
        created_at:
          type: string
          format: date-time
        
    EntityCreate:
      type: object
      required: [title, entity_type]
      properties:
        title:
          type: string
        entity_type:
          type: string
        content:
          type: string
        tags:
          type: array
          items:
            type: string
        initial_scores:
          type: object
          properties:
            interest:
              type: number
            strategy:
              type: number
            consensus:
              type: number
            
    EntityUpdate:
      type: object
      properties:
        title:
          type: string
        content:
          type: string
        tags:
          type: array
          items:
            type: string
            
    EntityDetail:
      allOf:
        - $ref: '#/components/schemas/Entity'
        - type: object
          properties:
            content:
              type: string
            refs_outgoing:
              type: array
              items:
                $ref: '#/components/schemas/Reference'
            refs_incoming:
              type: array
              items:
                $ref: '#/components/schemas/Reference'
                
    Score:
      type: object
      properties:
        interest:
          type: number
        strategy:
          type: number
        consensus:
          type: number
        composite:
          type: number
        temporal_decay:
          type: number
        calculated_at:
          type: string
          format: date-time
          
    ScoreRecord:
      type: object
      properties:
        timestamp:
          type: string
          format: date-time
        dimension:
          type: string
        old_value:
          type: number
        new_value:
          type: number
        reason:
          type: string
        trigger:
          type: string
          
    Reference:
      type: object
      properties:
        target_id:
          type: string
        target_title:
          type: string
        ref_type:
          type: string
        strength:
          type: number
        context:
          type: string
          
    GraphNode:
      type: object
      properties:
        id:
          type: string
        label:
          type: string
        type:
          type: string
        score:
          type: number
          
    GraphEdge:
      type: object
      properties:
        source:
          type: string
        target:
          type: string
        type:
          type: string
        strength:
          type: number
```

---

