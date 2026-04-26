# Compass PRD v2.2 — 完整规格升级说明书

**版本**: 2.2  
**日期**: 2026-04-27  
**状态**: 开发就绪  
**分支**: `docs/prd-v2.2`  

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
| v2.1 | 2026-04-20 | 整合A/B版本完整规格，填补v2.0技术缺口 |
| **v2.2** | 2026-04-27 | 恢复阉割功能：前端TS锁定（Vue3+TS）、Feishu Bot完整命令集（12条）、MCP完整工具集（15 Tool）、FAISS搜索、定时任务、备份策略、PWA离线 |

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
      summary: 列出所有实体（分页+过滤）
      description: |
        无需 query 参数即可返回 vault 中所有已索引实体的列表，按综合分数降序排列。
        **对应 Issue #44**：解决 `GET /entities/search?q=` 需要 query 字符串才能查询的问题。
      parameters:
        - name: type
          in: query
          schema:
            type: string
            enum: [knowledge, case, log, insight]
          description: 过滤实体类型
        - name: min_score
          in: query
          schema:
            type: number
            default: 0
          description: 最低综合分数过滤（默认 0，即返回所有实体）
        - name: tags
          in: query
          schema:
            type: array
            items:
              type: string
          description: 标签过滤（AND 逻辑）
        - name: limit
          in: query
          schema:
            type: integer
            default: 20
            maximum: 100
          description: 每页条数
        - name: offset
          in: query
          schema:
            type: integer
            default: 0
          description: 偏移量
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


---

## 6. Feishu Bot 完整命令集

### 6.1 命令总表

| 命令 | 功能 | 示例 |
|------|------|------|
| `/q <内容>` | 快速记录到 Inbox | `/q 量子纠缠与意识的关系值得探索` |
| `/r <关键词>` | 搜索相关实体（返回卡片） | `/r 量子纠缠` |
| `/s <id>` | 查看实体详情和评分 | `/s know-000001` |
| `/l <from> <to>` | 创建双向引用 | `/l know-000001 know-000002` |
| `/f` | 获取今日战略焦点（Top 5 strategic feed） | `/f` |
| `/t <内容>` | 快速创建日志条目 | `/t 今天完成了架构设计` |
| `/score <id> <维度> <分值>` | 手动调整评分 | `/score know-000001 interest 85` |
| `/tag <id> <标签>` | 添加标签 | `/tag know-000001 #物理 #意识系` |
| `/feed <mode>` | 获取信息流 | `/feed explore` 或 `/feed consolidate` |
| `/graph <id>` | 查看实体邻居图（简化版） | `/graph know-000001` |
| `/help` | 显示命令帮助 | `/help` |
| `/sync` | 触发强制全量同步 | `/sync` |

### 6.2 功能定义

#### 6.2.1 `/q` — 快速记录

**输入：** `/q <内容>`
**返回卡片（success，green）：**
```json
{
  "header": "已记录到 Inbox",
  "content": "<内容前100字>",
  "metadata": {
    "id": "<实体ID>",
    "time": "<时间戳>",
    "suggested_tags": ["#飞书导入", "#待分类"]
  },
  "actions": [
    {"text": "查看", "action": "open_entity", "entity_id": "<id>"},
    {"text": "关联知识", "action": "link_prompt"},
    {"text": "标记战略", "action": "score_prompt"}
  ]
}
```

#### 6.2.2 `/r` — 搜索

**输入：** `/r <关键词>`
**返回卡片（blue）：** Top 5 结果，每条显示标题、评分、标签、摘要片段

#### 6.2.3 `/s` — 详情

**输入：** `/s <id>`
**返回卡片（grey）：** 标题、评分雷达字段、标签、内容摘要、快捷操作按钮

#### 6.2.4 `/l` — 创建引用

**输入：** `/l <from_id> <to_id>`
**约束：** 两个 ID 均需存在，返回成功/错误提示

#### 6.2.5 `/f` — 战略焦点

**输入：** `/f`
**调用：** `GET /feed/strategic?limit=5`，返回 Top 5 战略高分实体

#### 6.2.6 `/t` — 创建日志

**输入：** `/t <内容>`
**调用：** `POST /entities`，type=log，subtype=short_term

#### 6.2.7 `/score` — 评分调整

**输入：** `/score <id> <维度> <分值>`
**维度：** `interest` | `strategy` | `consensus`
**分值：** 0-100
**调用：** `PATCH /entities/<id>/score`

#### 6.2.8 `/tag` — 添加标签

**输入：** `/tag <id> <标签1> [标签2...]`
**调用：** `PATCH /entities/<id>`，追加 tags

#### 6.2.9 `/feed` — 信息流

**输入：** `/feed <mode>`
**mode：** `explore` | `consolidate` | `strategic`

#### 6.2.10 `/graph` — 邻居图

**输入：** `/graph <id>`
**调用：** `GET /graph/neighbors/<id>?depth=1`，返回邻居数量+标题列表

#### 6.2.11 `/sync` — 强制同步

**输入：** `/sync`
**调用：** `POST /sync/force`，返回同步条数+错误列表

### 6.3 飞书卡片模板规范

```json
{
  "msg_type": "interactive",
  "card": {
    "header": {
      "title": {"tag": "plain_text", "content": "<标题>"},
      "template": "<color>"
    },
    "elements": [...]
  }
}
```

**template_color 规范：**
- searching / info：`blue`
- created / success：`green`
- warning：`orange`
- error：`red`
- neutral：`grey`

---

## 7. MCP Server 完整工具集

### 7.1 工具总表（15 Tool）

| Tool Name | 对应 REST API | 用途 |
|-----------|--------------|------|
| `compass_entities_list` | `GET /entities` | 列出实体 |
| `compass_entity_get` | `GET /entities/{id}` | 获取详情 |
| `compass_entity_create` | `POST /entities` | 创建实体 |
| `compass_entity_update` | `PATCH /entities/{id}` | 更新实体 |
| `compass_entity_delete` | `DELETE /entities/{id}` | 删除实体 |
| `compass_neighbors` | `GET /graph/neighbors/{id}` | 邻居查询 |
| `compass_search` | `POST /search` | 语义+评分混合搜索 |
| `compass_feed` | `GET /feed/{mode}` | 信息流 |
| `compass_score_get` | `GET /entities/{id}/score` | 获取评分 |
| `compass_score_update` | `PATCH /entities/{id}/score` | 调整评分 |
| `compass_score_history` | `GET /entities/{id}/score/history` | 评分历史 |
| `compass_refs_create` | `POST /entities/{id}/refs` | 创建引用 |
| `compass_fetch_save` | `POST /fetch/save` | URL抓取并保存 |
| `compass_timeline` | `GET /entities/{id}/timeline` | 时间线 |
| `compass_insight_suggest` | `GET /insights/suggest` | 感悟建议 |

### 7.2 核心工具 Input Schema

#### `compass_search`

```json
{
  "name": "compass_search",
  "description": "语义+评分混合搜索。结合FAISS向量相似度和RelevanceScore权重。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": {"type": "string"},
      "semantic_weight": {"type": "number", "default": 0.6},
      "filters": {
        "type": "object",
        "properties": {
          "tags": {"type": "array", "items": {"type": "string"}},
          "entity_types": {"type": "array", "items": {"type": "string"}},
          "date_range": {
            "type": "object",
            "properties": {"start": {"type": "string"}, "end": {"type": "string"}}
          }
        }
      },
      "limit": {"type": "integer", "default": 10, "maximum": 50}
    },
    "required": ["query"]
  }
}
```

#### `compass_neighbors`

```json
{
  "name": "compass_neighbors",
  "description": "获取实体的邻居节点（图谱查询）。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_id": {"type": "string"},
      "depth": {"type": "integer", "default": 1, "maximum": 3},
      "min_strength": {"type": "number", "default": 0.0},
      "edge_types": {
        "type": "array",
        "items": {"type": "string", "enum": ["cites", "applies", "inspired", "parent", "child"]}
      }
    },
    "required": ["entity_id"]
  }
}
```

#### `compass_fetch_save`

```json
{
  "name": "compass_fetch_save",
  "description": "抓取URL内容，清洗后保存到Vault。完整pipeline：fetch → clean → save。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "url": {"type": "string", "format": "uri"},
      "tags": {"type": "array", "items": {"type": "string"}},
      "auto_tag": {"type": "boolean", "default": true},
      "entity_type": {"type": "string", "enum": ["knowledge", "case", "log"], "default": "knowledge"}
    },
    "required": ["url"]
  }
}
```

#### `compass_score_update`

```json
{
  "name": "compass_score_update",
  "description": "调整实体评分维度，记录历史变更。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "entity_id": {"type": "string"},
      "dimension": {"type": "string", "enum": ["interest", "strategy", "consensus"]},
      "value": {"type": "number", "minimum": 0, "maximum": 100},
      "reason": {"type": "string", "maxLength": 200}
    },
    "required": ["entity_id", "dimension", "value", "reason"]
  }
}
```

---

## 8. FAISS 语义搜索完整规格

### 8.1 向量模型配置

| 配置项 | 值 |
|--------|-----|
| **模型** | `sentence-transformers/all-MiniLM-L6-v2` |
| **向量维度** | 384 |
| **索引类型** | `IndexFlatIP`（内积，精确检索）|
| **归一化** | L2 normalize |
| **批量大小** | 32 |

### 8.2 混合搜索算法

```
final_score = semantic_weight * semantic_similarity + score_weight * relevance_score

semantic_weight 默认 0.6，score_weight 默认 0.4
```

**检索流程：**
1. query → sentence-transformers → 向量 v（384维，L2归一化）
2. FAISS.IndexFlatIP 检索 Top-K（K = limit * 3，过滤后取前 limit）
3. 计算每个候选的 semantic_similarity
4. 应用 score_weight 加权
5. 按 final_score 降序返回

### 8.3 向量更新策略

| 事件 | 方式 |
|------|------|
| 实体创建 | 即时写入 FAISS |
| 实体内容更新 | 异步，5分钟后（先删后插） |
| 实体删除 | 即时从 FAISS 删除 |
| 全量重建 | 手动触发，遍历所有实体批量写入 |

### 8.4 向量存储

- **FAISS 索引**：`vault/_system/embeddings.faiss`
- **ID 映射**：FAISS 内部 offset → entity_id
- **元数据缓存**：`vault/_system/embeddings.meta.json`

---

## 9. 定时任务完整定义

### 9.1 任务总表

| 任务 | Cron 表达式 | 功能 |
|------|-----------|------|
| **衰减计算** | `0 2 * * *`（每日02:00） | 对7天未访问实体执行 decay |
| **孤儿检测** | `0 4 1 * *`（每月1日04:00） | 标记90天无引用实体为 orphan |
| **周报生成** | `0 9 * * 1`（每周一09:00） | 生成上周认知活动报告推送飞书 |
| **全量索引重建** | `0 3 * * 0`（每周日03:00） | 重建 FAISS + FTS5 索引 |
| **评分重算** | `0 5 * * 0`（每周日05:00） | 全量 recalculate composite_score |
| **Git 自动提交** | `0 23 * * *`（每日23:00） | watchdog 变更检测 + git add/commit |

### 9.2 衰减计算逻辑

```
FOR each entity WHERE accessed_at < now - 7d:
    days_inactive = (now - accessed_at).days
    decay_factor = 0.98 ^ days_inactive
    new_interest = MAX(original_interest * decay_factor, original_interest * 0.5)
    UPDATE scores SET interest = new_interest
    INSERT INTO score_history (trigger_type='auto_decay')
```

**特殊规则：**
- `last_boosted_at` 有值且距今<3天：跳过（boost 保护期）
- `status = archived`：跳过
- `layer = direction`：Decay 减半

### 9.3 孤儿检测逻辑

```
FOR each entity WHERE status='active' AND modified_at < now - 90d
    AND NOT EXISTS (SELECT 1 FROM refs WHERE source_id=entity.id OR target_id=entity.id):
    UPDATE entities SET status = 'orphan'
    INSERT INTO timeline (event_type='mark_orphan', source='system')
```

---

## 10. 备份策略完整定义

### 10.1 备份层级

| 层级 | 频率 | 方式 | 保留 |
|------|------|------|------|
| **Git 提交** | 每日 + watch 变更 | `git add . && git commit` | 永久 |
| **数据库快照** | 每日02:30 | SQLite `.dump` + gzip | 30天滚动 |
| **FAISS 索引** | 每周日04:00 | 文件复制 | 12周滚动 |
| **完整快照** | 每月1日03:00 | `tar.gz` vault 全量 | 12个月滚动 |

### 10.2 Git 自动提交规范

**触发：** watchdog 文件系统事件（创建/修改/删除/重命名）
**最小提交间隔：** 5分钟
**最大缓冲：** 20个变更或30分钟（先到先触发）
**Commit Message：**
```
[Compass] <日期> <操作类型>
<file_path>: <操作>
Generated by Compass backup system
```

### 10.3 灾难恢复

1. **数据损坏检测**：启动时校验 entities 行数 == FAISS 向量数
2. **FTS 损坏**：触发 REINDEX
3. **FAISS 损坏**：从最近快照恢复
4. **Vault 文件损坏**：从 Git 历史恢复

---

## 11. PWA 离线能力完整定义

### 11.1 Service Worker 策略

| 资源类型 | 缓存策略 | 过期 |
|---------|---------|------|
| **静态资源**（JS/CSS/图片） | Cache-First | 永久（版本控制）|
| **API 响应**（实体详情） | Stale-While-Revalidate | 5分钟 |
| **搜索结果** | Network-First | - |
| **Feed 数据** | Stale-While-Revalidate | 10分钟 |

### 11.2 离线功能范围

| 功能 | 离线支持 |
|------|---------|
| 查看已访问实体 | ✅ |
| 评分调整 | ⚠️（队列化，恢复后同步）|
| 实体搜索（全文） | ❌ |
| 新建实体 | ❌ |
| 引用创建 | ⚠️（队列化，恢复后同步）|

### 11.3 离线队列

**存储：** IndexedDB（`compass_offline_queue`）
**同步时机：** 网络恢复后自动同步，按时间戳顺序执行

---

## 12. Docker Compose 部署完整定义

### 12.1 服务组件

```yaml
version: '3.8'
services:
  compass-core:
    image: compass-core:latest
    volumes:
      - vault-data:/vault
    command: ["--mode", "server", "--port", "9000"]
    restart: unless-stopped

  compass-api:
    image: compass-api:latest
    ports:
      - "8000:8000"
    volumes:
      - vault-data:/vault:ro
      - compass-db:/data
    environment:
      - VAULT_PATH=/vault
      - DB_PATH=/data/compass.db
      - RUST_CORE_URL=http://compass-core:9000
    depends_on:
      - compass-core
    restart: unless-stopped

  compass-web:
    image: compass-web:latest
    ports:
      - "3000:80"
    environment:
      - API_URL=http://compass-api:8000
    depends_on:
      - compass-api
    restart: unless-stopped

volumes:
  vault-data:
  compass-db:

networks:
  compass-net:
    driver: bridge
```

### 12.2 环境变量

| 变量 | 示例 | 说明 |
|------|------|------|
| `VAULT_PATH` | `/vault` | Vault 根目录 |
| `DB_PATH` | `/data/compass.db` | SQLite 路径 |
| `RUST_CORE_URL` | `http://compass-core:9000` | Rust 服务地址 |
| `FEISHU_APP_ID` | `cli_xxx` | 飞书应用 ID |
| `FEISHU_APP_SECRET` | `xxx` | 飞书应用密钥 |
| `EMBEDDING_MODEL` | `all-MiniLM-L6-v2` | 向量模型 |

---

## 13. Obsidian 插件说明（已取消）

> **决定：不开发 Obsidian 插件。**  
> OpenClaw Skill 已承担 Agent 交互入口角色，Obsidian 纯作为 Vault 文件管理器。
>
> **用户操作路径：**
> - **飞书 Bot**：快速记录 / 搜索 / 评分（/q /r /s 等）
> - **Obsidian Desktop**：深度编辑 / 阅读 / 脑图（纯文件操作）
> - **OpenClaw Agent**：上下文查询 / 知识生成（通过 MCP 或 Skill）

---

## 14. Phase 规划（v2.2 重构）

### 14.1 Phase 1 ✅ 已完成（v0.1.0）

- Rust Core（评分引擎 + 引用解析 + Decay）
- Python FastAPI（CRUD + Search + Feed）
- OpenClaw Skill Phase 1（7 action + render）
- SQLite Schema + FTS5
- FileWatcher 监听

### 14.2 Phase 2：后端能力扩展

**目标：** 完善后端 API，为 Phase 3 前端和 MCP 调用打好基础。

| 任务 ID | 任务名称 | 分支 | 优先级 | 依赖 | 工时 |
|---------|----------|------|--------|------|------|
| P2-Entity-1 | `GET /entities` 列表+分页 | `feat/p2-entity-list` | P0 | - | 4h |
| P2-Graph-1 | `GET /graph/neighbors/{id}` 基础邻居 | `feat/p2-graph-1-neighbors-basic` | P0 | - | 4h |
| P2-Graph-2 | 深度邻居（depth=N） | `feat/p2-graph-2-neighbors-depth` | P0 | P2-Graph-1 | 3h |
| P2-Graph-3 | 强度过滤（min_strength=X） | `feat/p2-graph-3-neighbors-filter` | P1 | P2-Graph-1 | 3h |
| P2-Graph-4 | 最短路径查询 | `feat/p2-graph-4-path` | P1 | P2-Graph-1 | 6h |
| P2-Fetch-1 | `POST /fetch` URL 抓取 | `feat/p2-fetch-1-url-fetch` | P0 | - | 4h |
| P2-Fetch-2 | `POST /fetch/clean` 内容清洗 | `feat/p2-fetch-2-clean` | P0 | P2-Fetch-1 | 6h |
| P2-Fetch-3 | `POST /fetch/save` 写入 Vault | `feat/p2-fetch-3-save` | P0 | P2-Fetch-2 | 4h |
| P2-Search-1 | `POST /search` FAISS 语义搜索 | `feat/p2-search-1-semantic` | P1 | - | 8h |
| P2-Search-2 | 混合搜索权重调优 | `feat/p2-search-2-params` | P1 | P2-Search-1 | 3h |
| P2-MCP-1 | MCP Server 基础（3 Tool） | `feat/p2-mcp-1-basic` | P1 | - | 6h |
| P2-MCP-2 | MCP Server 扩展至 15 Tool | `feat/p2-mcp-2-full` | P1 | P2-MCP-1 | 8h |
| P2-Timeline-1 | `PATCH /entities/{id}/access` | `feat/p2-timeline-1` | P1 | P2-Entity-1 | 2h |
| P2-Timeline-2 | `GET /entities/{id}/timeline` | `feat/p2-timeline-2` | P1 | P2-Timeline-1 | 3h |
| P2-History-1 | score_history 写入 | `feat/p2-history-1` | P1 | P2-Entity-1 | 2h |
| P2-History-2 | 评分历史趋势 API | `feat/p2-history-2` | P1 | P2-History-1 | 3h |
| P2-Insight-1 | Insight CRUD + maturity 状态机 | `feat/p2-insight-1` | P2 | P2-Entity-1 | 4h |
| P2-Insight-2 | Insight 成熟度演化触发器 | `feat/p2-insight-2` | P2 | P2-Graph-1 + P2-Insight-1 | 6h |
| P2-Insight-3 | Insight → Knowledge 导出 | `feat/p2-insight-3` | P2 | P2-Insight-2 | 3h |
| P2-Ref-1 | 引用强度自动计算 | `feat/p2-ref-1` | P2 | P2-Graph-1 | 6h |
| P2-Ref-2 | 双向引用自动维护 | `feat/p2-ref-2` | P2 | P2-Graph-1 | 3h |
| P2-Decay-1 | `PATCH /entities/{id}/decay-config` | `feat/p2-decay-1` | P2 | P2-Entity-1 | 3h |
| P2-Decay-2 | Decay 预览（90天曲线） | `feat/p2-decay-2` | P2 | P2-Decay-1 | 4h |
| P2-Backup-1 | Git 自动提交 | `feat/p2-backup-1` | P2 | - | 4h |
| P2-Backup-2 | DB 定时快照 + 滚动清理 | `feat/p2-backup-2` | P2 | P2-Backup-1 | 3h |
| P2-Backup-3 | 灾难恢复流程 | `feat/p2-backup-3` | P2 | P2-Backup-2 | 3h |
| P2-Scheduler-1 | APScheduler 框架 | `feat/p2-scheduler-1` | P2 | P2-Backup-1 | 3h |
| P2-Scheduler-2 | 衰减 + 孤儿检测任务 | `feat/p2-scheduler-2` | P2 | P2-Scheduler-1 | 4h |
| P2-Scheduler-3 | 周报生成任务 | `feat/p2-scheduler-3` | P2 | P2-Scheduler-1 | 4h |
| P2-Scheduler-4 | 索引重建任务 | `feat/p2-scheduler-4` | P2 | P2-Search-1 + P2-Scheduler-1 | 3h |

**Phase 2 总工时：130h（6-8 周）**

### 14.3 Phase 3：前端（Vue3 + TypeScript）

**目标：** Web UI 呈现 Phase 1/2 后端能力。

> **⚠️ 技术栈锁定：Vue3 + TypeScript + Vite + D3.js（TS 不可替换）**

| 任务 ID | 任务名称 | 分支 | 优先级 | 依赖 | 工时 |
|---------|----------|------|--------|------|------|
| P3-UI-1 | Vue3 + TypeScript + Vite 骨架 | `feat/p3-ui-1-skeleton` | P0 | P2-Search-1 | 8h |
| P3-UI-2 | 实体列表页（分页+过滤+搜索） | `feat/p3-ui-2-entity-list` | P0 | P3-UI-1 | 6h |
| P3-UI-3 | 实体详情页（Markdown渲染+引用） | `feat/p3-ui-3-entity-detail` | P0 | P3-UI-1 | 8h |
| P3-UI-4 | 评分面板（三维雷达图+历史曲线） | `feat/p3-ui-4-score-panel` | P1 | P3-UI-1 | 6h |
| P3-UI-5 | 图谱可视化（D3.js Force-Directed） | `feat/p3-ui-5-graph-viz` | P1 | P2-Graph-1 | 12h |
| P3-UI-6 | Feed 信息流页面 | `feat/p3-ui-6-feed` | P1 | P2-Search-1 | 4h |
| P3-UI-7 | 搜索页面（语义搜索+高亮） | `feat/p3-ui-7-search` | P1 | P2-Search-1 | 6h |
| P3-UI-8 | 时间线页面 | `feat/p3-ui-8-timeline` | P2 | P2-Timeline-2 | 4h |
| P3-UI-9 | Insight 页面（成熟度状态机） | `feat/p3-ui-9-insight` | P2 | P2-Insight-2 | 6h |
| P3-UI-10 | 用户设置页（权重+Decay配置） | `feat/p3-ui-10-settings` | P2 | P3-UI-1 | 4h |
| P3-PWA-1 | PWA 配置（SW + Manifest） | `feat/p3-pwa-1` | P2 | P3-UI-1 | 6h |
| P3-PWA-2 | 离线缓存策略 | `feat/p3-pwa-2` | P2 | P3-PWA-1 | 6h |
| P3-PWA-3 | 离线队列（评分/引用） | `feat/p3-pwa-3` | P2 | P3-PWA-2 | 4h |

**Phase 3 总工时：80h（4-6 周）**

### 14.4 Phase 4：部署与工程化

| 任务 ID | 任务名称 | 分支 | 优先级 | 依赖 | 工时 |
|---------|----------|------|--------|------|------|
| P4-Deploy-1 | docker-compose.yml | `feat/p4-deploy-1` | P0 | P3-UI-1 | 4h |
| P4-Deploy-2 | Dockerfile | `feat/p4-deploy-2` | P0 | P4-Deploy-1 | 2h |
| P4-Deploy-3 | 一键部署脚本 | `feat/p4-deploy-3` | P1 | P4-Deploy-1 | 3h |
| P4-Deploy-4 | 环境变量规范 | `feat/p4-deploy-4` | P1 | P4-Deploy-1 | 1h |
| P4-Monitor-1 | 健康检查增强 | `feat/p4-monitor-1` | P1 | P2-Search-1 | 3h |
| P4-Monitor-2 | 监控面板 | `feat/p4-monitor-2` | P2 | P4-Monitor-1 | 6h |
| P4-Migrate-1 | 数据迁移工具 | `feat/p4-migrate-1` | P3 | P2-Entity-1 | 12h |
| P4-Migrate-2 | 导出工具 | `feat/p4-migrate-2` | P3 | P2-Entity-1 | 4h |

**Phase 4 总工时：35h（2-3 周）**

### 14.5 完整路线图

```
Phase 1 ✅ (v0.1.0)
  └─ 核心引擎完成

Phase 2 🔧 (130h，6-8 周)
  ├─ Graph API（P2-Graph-1~4）
  ├─ Fetch Pipeline（P2-Fetch-1~3）
  ├─ 语义搜索（P2-Search-1~2）
  ├─ MCP Server（P2-MCP-1~2）→ 15 Tool
  ├─ Timeline + History（P2-Timeline, P2-History）
  ├─ Insight 引擎（P2-Insight-1~3）
  ├─ 引用智能（P2-Ref-1~2）
  ├─ Decay 调优器（P2-Decay-1~2）
  ├─ 备份系统（P2-Backup-1~3）
  └─ 定时任务（P2-Scheduler-1~4）

Phase 3 🎨 (80h，4-6 周)
  ├─ Vue3 + TypeScript 前端骨架
  ├─ 实体管理页面
  ├─ 评分面板 + 图谱可视化
  ├─ Feed + 搜索
  └─ PWA 离线能力

Phase 4 🚀 (35h，2-3 周)
  ├─ docker-compose 部署
  ├─ 监控告警
  └─ 数据迁移工具

总工时：245h | 建议周期：12-16 周
```

---

*文档版本：v2.2 | 更新日期：2026-04-27*
