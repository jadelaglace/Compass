# Compass API — 自然语言测试地图

> 基于 dev 分支（fca038b），覆盖所有 35 个端点

---

## 准备工作

```bash
BASE="http://localhost:8001"
AUTH_HEADER=""

# 辅助函数
ping() { curl -s "$1" | python3 -m json.tool 2>/dev/null || echo "$1"; }

# 创建测试实体（用于后续测试）
 ENTITY1=$(curl -s -X POST "$BASE/entities" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-entity-1",
    "title": "Rust Web Performance Guide",
    "category": "Knowledge",
    "vault_path": "Knowledge/rust-web.md",
    "interest": 7.5,
    "strategy": 8.0,
    "consensus": 6.0,
    "content": "高性能 Rust Web 开发指南，内容涉及 [[test-entity-2]] 和 [[test-entity-3]]。推荐阅读 see [[test-entity-4]]。"
  }' | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['id'])")

 ENTITY2=$(curl -s -X POST "$BASE/entities" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-entity-2",
    "title": "Async Rust Deep Dive",
    "category": "Knowledge",
    "vault_path": "Knowledge/async-rust.md",
    "interest": 6.0,
    "strategy": 7.0
  }' | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['id'])")

 ENTITY3=$(curl -s -X POST "$BASE/entities" \
  -H "Content-Type: application/json" \
  -d '{
    "id": "test-entity-3",
    "title": "Tokio Runtime Internals",
    "category": "Direction",
    "vault_path": "Direction/tokio.md",
    "interest": 8.5,
    "strategy": 9.0,
    "consensus": 7.5
  }' | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['id'])")
```

---

## 测试矩阵

### Group A — Entity CRUD

**A1. 创建实体**
```
POST /entities
body: {
  "id": "test-a1",
  "title": "Test Entity A1",
  "category": "Inbox",
  "vault_path": "Inbox/test-a1.md",
  "interest": 5.0,
  "strategy": 5.0,
  "consensus": 0.0
}
预期: 201, 返回 id/title/category/final_score/tags/created_at/updated_at
验证: tags 包含自动提取标签（如 #test #entity）
```

**A2. 创建实体（带内容，含 [[wikilink]] 引用）**
```
POST /entities
body: {
  "id": "test-a2",
  "title": "Entity With References",
  "category": "Knowledge",
  "vault_path": "Knowledge/refs.md",
  "interest": 6.0,
  "strategy": 6.0,
  "content": "这篇文章涉及 [[test-entity-1]] 和 [[test-entity-3]]，see [[test-entity-2]]"
}
预期: 201, references 表中自动插入 3 条边
验证: GET /entities/test-a2 返回 outgoing_refs 包含 3 个 target
```

**A3. 自动标签提取验证**
```
POST /entities
body: {
  "id": "test-a3",
  "title": "Vue3 Composition API Design Patterns",
  "category": "Knowledge",
  "vault_path": "Knowledge/vue3-patterns.md"
}
预期: tags 包含 #vue3, #composition, #design（stopwords 过滤正确）
```

**A4. 获取单个实体**
```
GET /entities/{entity1}
预期: 200, 包含 outgoing_refs/incoming_refs/tags 字段
验证: outgoing_refs 中有 test-entity-2/3（来自 A2 的 wikilink）
```

**A5. 获取不存在的实体**
```
GET /entities/nonexistent-id
预期: 404, detail="Entity not found"
```

**A6. 列表查询（分页 + 过滤）**
```
GET /entities?type=knowledge&min_score=5.0&limit=10&offset=0
预期: 200, items 数组, total 数字, has_more 布尔
验证: items[0] 包含 id/title/entity_type/category/vault_path/final_score/tags
```

**A7. 列表查询（标签过滤 AND 逻辑）**
```
GET /entities?tags=#vue3&limit=20
预期: 只返回包含 #vue3 标签的实体
```

**A8. 列表查询（非法 type）**
```
GET /entities?type=invalid
预期: 422, detail="Invalid entity_type"
```

**A9. FTS 搜索**
```
GET /entities/search?q=rust&limit=5
预期: 200, results 数组包含匹配 Rust 的实体
验证: 每条 result 有 id/title/category/final_score
```

**A10. Top 实体**
```
GET /entities/top?limit=5
预期: 200, results 按 final_score 降序排列
验证: 所有返回实体 score >= 未返回实体
```

**A11. 更新实体（PUT 全量更新）**
```
PUT /entities/{entity1}
body: {
  "id": "{entity1}",
  "title": "Rust Web Performance Guide v2",
  "category": "Knowledge",
  "vault_path": "Knowledge/rust-web-v2.md",
  "interest": 9.0,
  "strategy": 9.0,
  "consensus": 7.0,
  "content": "更新版本，涉及 [[test-entity-2]]"
}
预期: 200, title 更新为 v2, score 重新计算
验证: GET /entities/{entity1} title 包含 "v2"
```

**A12. 删除实体**
```
DELETE /entities/test-a1
预期: 200, 返回 {"deleted": "test-a1"}
验证: GET /entities/test-a1 → 404
```

**A13. 删除不存在的实体**
```
DELETE /entities/nonexistent-id
预期: 404
```

---

### Group B — Access & Score

**B1. 记录访问（第一次）**
```
PATCH /entities/{entity1}/access
预期: 200, access_count=1, decay_updated=true
验证: 返回的 access_count > 0
```

**B2. 访问防抖（5分钟内重复访问）**
```
立即再次 PATCH /entities/{entity1}/access
预期: 200, decay_updated=false, access_count 不增加
```

**B3. 等待 6 秒后访问（防抖过期）**
```
sleep 6 && PATCH /entities/{entity1}/access
预期: 200, access_count 增加 1, decay_updated=true
```

**B4. 分数更新**
```
POST /scores/update
body: {
  "entity_id": "{entity1}",
  "interest": 9.0,
  "strategy": 8.5,
  "consensus": 7.0,
  "manual_override": true
}
预期: 200, 返回新的 final_score/decay_factor/days_elapsed
验证: final_score > 原来的值
```

**B5. 获取分数历史**
```
GET /entities/{entity1}/score/history?dimension=composite&days=90
预期: 200, records 数组, trend 字符串, change_pct 数字
```

**B6. 分数历史（无效维度）**
```
GET /entities/{entity1}/score/history?dimension=invalid
预期: 422 或使用默认值 composite
```

---

### Group C — Timeline & Events

**C1. 全局时间线（时间窗口）**
```
GET /entities/timeline?start=2026-01-01T00:00:00Z&end=2026-12-31T23:59:59Z&limit=50
预期: 200, items 数组, total 数字, has_more 布尔
验证: items[0] 包含 entity_id/title/event_type/created_at
```

**C2. 全局时间线（按事件类型过滤）**
```
GET /entities/timeline?start=2026-01-01T00:00:00Z&event_type=created&limit=20
预期: 200, 只返回 created 事件
```

**C3. 单实体时间线**
```
GET /entities/{entity1}/timeline?limit=20
预期: 200, entity_id 字段匹配, items 包含 event_type/trigger/created_at
```

**C4. 时间线（无效 datetime 格式）**
```
GET /entities/timeline?start=not-a-date
预期: 400, detail="Invalid start datetime format"
```

**C5. 时间线（end 早于 start）**
```
GET /entities/timeline?start=2026-12-31T00:00:00Z&end=2026-01-01T00:00:00Z
预期: 400, detail="end must be after start"
```

---

### Group D — Graph / References

**D1. 邻居查询（depth=1）**
```
GET /graph/neighbors/{entity1}?depth=1
预期: 200, nodes 数组, edges 数组, total_neighbors 数字
验证: entity1 不在 nodes 中（排除自身）, edges 包含方向信息
```

**D2. 邻居查询（depth=2 BFS）**
```
GET /graph/neighbors/{entity2}?depth=2&min_strength=0.0
预期: 200, 包含 2 度邻居（entity2 → entity1 → entity3）
```

**D3. 邻居查询（min_strength 过滤）**
```
GET /graph/neighbors/{entity1}?min_strength=0.9
预期: 200, edges 全是 strength >= 0.9
```

**D4. 邻居查询（不存在的实体）**
```
GET /graph/neighbors/nonexistent?depth=1
预期: 404, detail="Entity not found"
```

**D5. 最短路径**
```
GET /graph/path?from={entity2}&to={entity3}
预期: 200, path 数组, distance 数字, edges 数组
验证: path[0]=entity2, path[-1]=entity3
```

**D6. 最短路径（无路径）**
```
GET /graph/path?from={entity1}&to=isolated-entity
预期: 404, detail="No path found between entities"
```

**D7. 最短路径（同实体）**
```
GET /graph/path?from={entity1}&to={entity1}
预期: 200, distance=0, path=[entity1]
```

**D8. 关联实体推荐（AutoRelate）**
```
GET /entities/{entity1}/related?limit=10
预期: 200, items 数组, count 数字
验证: 每项有 id/title/related_score/tags
```

---

### Group E — Insights

**E1. 创建 Insight**
```
POST /insights
body: {
  "entity_id": "{entity1}",
  "title": "Rust Performance Insight",
  "content": "异步 Runtime 对延迟的影响分析"
}
预期: 201, id 格式 "insight-{entity_id}-{date}", maturity="seedling"
```

**E2. 列表 Insights（按 maturity 过滤）**
```
GET /insights?maturity=seedling&limit=20
预期: 200, items 只包含 seedling maturity
```

**E3. 获取单个 Insight**
```
GET /insights/insight-{entity1}-{今日日期}
预期: 200, 返回完整 insight 对象
```

**E4. 升级 Insight maturity（seedling → sprout）**
```
PATCH /insights/insight-{entity1}-{今日日期}/maturity
预期: 200, maturity 更新为 "sprout"
```

**E5. 升级已Fully Mature Insight（sprout → mature 后再次升级）**
```
再次 PATCH /insights/.../maturity（现在是 sprout）
预期: 422, detail="Already fully mature"
```

**E6. Evolve Entity from Mature Insight**
```
GET /insights/insight-{entity1}-{今日日期}/evolve
预期: 200, evolved=true, entity_maturity 升级
验证: GET /entities/{entity1} maturity 字段已更新
```

**E7. Insight Evolve（insight 未 mature）**
```
POST /insights
body: {"entity_id": "{entity2}", "title": "Immature Insight"}
# 不升级直接 evolve
GET /insights/insight-{entity2}-{今日日期}/evolve
预期: evolved=false, detail="Insight not yet mature"
```

**E8. Export Insights（JSON）**
```
GET /insights/export?format=json
预期: 200, {"format": "json", "total": N, "items": [...]}
```

**E9. Export Insights（Markdown）**
```
GET /insights/export?format=markdown
预期: 200, {"format": "markdown", "content": "# Insights Export\n\n..."}
```

**E10. Export 单个 Insight（JSON/Markdown）**
```
GET /insights/insight-{entity1}-{今日日期}/export?format=json
GET /insights/insight-{entity1}-{今日日期}/export?format=markdown
预期: 两个都返回 200
```

**E11. Export Entity 所有 Insights**
```
GET /insights/entity/{entity1}/export?format=json
预期: 200, items 数组只包含 entity1 的 insights
```

---

### Group F — Decay

**F1. 获取 Decay 配置**
```
GET /decay/{entity1}/config
预期: 200, entity_id/interest_half_life_days/strategy_half_life_days/consensus_half_life_days/current_scores
验证: current_scores 包含 interest/strategy/consensus/final_score
```

**F2. 更新 Decay 半衰期配置**
```
PATCH /decay/{entity1}/config
body: {
  "interest_half_life_days": 60.0,
  "strategy_half_life_days": 180.0
}
预期: 200, 返回更新后的配置, final_score 重新计算
```

**F3. Decay Preview（30天后）**
```
GET /decay/{entity1}/preview?days=30
预期: 200, current_score/future_score/days_elapsed/decayed_components
验证: future_score < current_score
```

**F4. Decay Preview（不存在的实体）**
```
GET /decay/nonexistent/preview?days=30
预期: 404
```

**F5. Decay Simulate（90天轨迹）**
```
GET /decay/{entity1}/simulate?days=90&step_days=7
预期: 200, trajectory 数组, total_decay_pct
验证: trajectory[0].day=0, trajectory 最后.day=84 或 91, total_decay_pct > 0
```

**F6. Decay Simulate（invalid days 超出范围）**
```
GET /decay/{entity1}/simulate?days=5000
预期: 422 或 clamp 到 3650
```

---

### Group G — Tags

**G1. 标签推荐（AutoTag-2）**
```
POST /entities/{entity1}/tags/recommend?limit=10
预期: 200, recommendations 数组, 每项有 tag/score
```

**G2. 批量更新标签（PUT replace）**
```
PUT /entities/{entity1}/tags
body: {"tags": ["#rust", "#performance", "#web"]}
预期: 200, 返回 {"entity_id": "...", "tags": ["#rust", "#performance", "#web"]}
验证: GET /entities/{entity1} 的 tags 包含这三个标签
```

**G3. 更新不存在实体的标签**
```
PUT /entities/nonexistent/tags
body: {"tags": ["#test"]}
预期: 404
```

---

### Group H — Fetch & Search

**H1. Fetch URL（正常）**
```
POST /fetch
body: {"url": "https://example.com"}
预期: 200, url/title/raw_content/content_type/status_code/fetched_at
```

**H2. Fetch URL（超时/无效）**
```
POST /fetch
body: {"url": "https://invalid-domain-that-does-not-exist-xyz.com"}
预期: 400 或 408
```

**H3. Clean HTML**
```
POST /fetch/clean
body: {"raw_content": "<h1>Title</h1><p>Content here</p>", "source_url": "https://example.com"}
预期: 200, title/content/summary/tags/source_url
```

**H4. Save Fetched Content**
```
POST /fetch/save
body: {
  "title": "Example Domain",
  "content": "Content from example.com",
  "source_url": "https://example.com",
  "tags": ["#web"],
  "category": "Inbox"
}
预期: 201, entity_id/file_path/title
验证: GET /entities/{entity_id} 能查到
```

**H5. 混合搜索（BM25 + score 加权）**
```
POST /search
body: {
  "query": "rust performance",
  "semantic_weight": 0.6,
  "score_weight": 0.4,
  "limit": 10
}
预期: 200, items 数组, total 数字, query_vector_dim=0
验证: match_score 介于 0-1 之间
```

**H6. 搜索（权重不合法）**
```
POST /search
body: {"query": "rust", "semantic_weight": 0.5, "score_weight": 0.8}
预期: 422, detail="...must sum to 1.0"
```

**H7. 搜索（带标签过滤器）**
```
POST /search
body: {
  "query": "rust",
  "semantic_weight": 0.6,
  "score_weight": 0.4,
  "filters": {"tags": ["#rust"]},
  "limit": 20
}
预期: 200, 所有 items 包含 #rust 标签
```

---

### Group I — Feed & Agent

**I1. 每日 Feed**
```
GET /feed/today?limit=10
预期: 200, top_inbox/recently_updated/strategic 三个数组
```

**I2. Agent Context**
```
POST /agent/context
body: {
  "task": "Find high-performance web frameworks",
  "top_k": 5
}
预期: 200, context 数组（top 5 实体）, suggested_entities, reasoning
```

**I3. Agent Context（空结果）**
```
POST /agent/context
body: {"task": "xyzzyqqqnonexistent", "top_k": 3}
预期: 200, context=[], suggested_entities=[], reasoning="No entities found"
```

---

### Group J — Edge Cases & Error Handling

**J1. Content-Type 错误**
```
POST /entities with Content-Type: text/plain
预期: 422 或 400
```

**J2. 创建实体（缺少必填字段）**
```
POST /entities
body: {"title": "No ID"}
预期: 422, validation error
```

**J3. score_history 对不存在的实体**
```
GET /entities/nonexistent/score/history
预期: 404
```

**J4. 更新不存在的实体分数**
```
POST /scores/update
body: {"entity_id": "nonexistent", "interest": 8.0}
预期: 404
```

**J5. insights export（invalid format）**
```
GET /insights/export?format=xml
预期: 422
```

**J6. Decay config 更新（不存在的实体）**
```
PATCH /decay/nonexistent/config
body: {"interest_half_life_days": 30}
预期: 404
```

**J7. URL 编码路径测试**
```
GET /entities/timeline?start=2026-01-01T00%3A00%3A00Z
预期: 200（特殊字符需要正确 URL 编码）
```

**J8. 分页越界（offset > total）**
```
GET /entities?limit=10&offset=999999
预期: 200, items=[], has_more=false
```

---

## 执行方式

每个测试编号对应一个 curl 命令，执行后对比预期与实际响应。
不符合预期的记录到 Issue 列表。
