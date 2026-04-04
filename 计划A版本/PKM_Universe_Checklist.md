
# PKM Universe 开发启动清单

## Phase 1: 基础架构 (Week 1-2)

### Day 1-2: 项目初始化
- [ ] 创建Git仓库，设置Python 3.11+环境
- [ ] 搭建FastAPI基础结构（路由、模型、依赖注入）
- [ ] 配置开发环境：black, ruff, mypy, pytest
- [ ] 创建基础目录结构：
  ```
  pkm-universe/
  ├── api/              # FastAPI应用
  ├── core/             # 评分引擎、解析器
  ├── models/           # Pydantic模型
  ├── db/               # SQLite操作
  ├── search/           # 向量检索
  ├── mcp/              # MCP协议实现
  ├── jobs/             # 定时任务
  └── tests/            # 测试用例
  ```

### Day 3-4: 数据模型实现
- [ ] 实现Knowledge Pydantic模型（含验证）
- [ ] 实现RelevanceScore模型（含历史记录）
- [ ] 实现Case和Log模型
- [ ] 创建SQLite Schema（执行SQL脚本）
- [ ] 实现数据库连接池和基础CRUD

### Day 5-7: Markdown解析引擎
- [ ] 集成python-frontmatter库
- [ ] 实现YAML Frontmatter解析和验证
- [ ] 实现Mermaid脑图提取
- [ ] 实现双向链接解析（[[id|title]]格式）
- [ ] 编写解析器单元测试

### Day 8-10: 文件监控同步
- [ ] 集成watchdog库
- [ ] 实现Vault目录监控
- [ ] 实现文件变更事件处理（增删改）
- [ ] 实现数据库与文件的同步逻辑
- [ ] 处理冲突（文件vs数据库最新）

### Day 11-14: 基础API完成
- [ ] GET /api/v1/knowledge（列表+过滤）
- [ ] POST /api/v1/knowledge（创建）
- [ ] GET /api/v1/knowledge/{id}（详情）
- [ ] PUT /api/v1/knowledge/{id}（更新）
- [ ] DELETE /api/v1/knowledge/{id}（软删除）
- [ ] API文档（OpenAPI自动生成）
- [ ] 编写API集成测试

## Phase 2: 评分引擎 (Week 3-4)

### Week 3: 核心算法
- [ ] 实现ScoringAlgorithm.calculate_total()
- [ ] 实现apply_decay()衰减算法
- [ ] 实现boost_for_access()访问提升
- [ ] 实现评分历史记录存储
- [ ] 编写算法单元测试（边界值、异常值）

### Week 4: 服务与接口
- [ ] 实现ScoreEngineService类
- [ ] PATCH /api/v1/knowledge/{id}/score接口
- [ ] 实现批量评分调整
- [ ] 集成APScheduler定时任务
- [ ] 实现每日衰减任务
- [ ] 实现评分变更Git提交钩子

## Phase 3: 智能检索 (Week 5-6)

### Week 5: 向量引擎
- [ ] 安装sentence-transformers
- [ ] 下载all-MiniLM-L6-v2模型（本地）
- [ ] 实现文本向量化服务
- [ ] 集成FAISS索引
- [ ] 实现向量存储和检索

### Week 6: 混合搜索
- [ ] 实现语义相似度计算
- [ ] 设计混合排序算法（语义60%+评分40%）
- [ ] POST /api/v1/search接口
- [ ] 实现搜索结果高亮
- [ ] 性能优化（索引<100ms）

## Phase 4: Agent集成 (Week 7-8)

### Week 7: MCP协议
- [ ] 研究MCP SDK文档
- [ ] 实现MCPServer基础类
- [ ] 实现knowledge_query工具
- [ ] 实现knowledge_create工具
- [ ] 本地测试MCP连接

### Week 8: 智能功能
- [ ] 实现score_suggest智能建议
- [ ] 实现get_insight_summary
- [ ] 实现自动标签推荐（简单关键词提取）
- [ ] 与Claude Desktop联调
- [ ] 与Cursor联调

## Phase 5: 前端界面 (Week 9-10)

### Week 9: 基础界面
- [ ] Vue3 + Vite项目搭建
- [ ] 配置Tailwind CSS
- [ ] 实现API客户端封装
- [ ] 实现知识列表视图（表格+卡片）
- [ ] 实现Markdown渲染组件

### Week 10: 可视化
- [ ] 集成D3.js
- [ ] 实现引力场力导向图
- [ ] 实现评分调整三滑块组件
- [ ] 实现时间轴折线图
- [ ] 配置PWA（manifest + service worker）

## Phase 6: 优化与发布 (Week 11-12)

### Week 11: 质量保障
- [ ] 性能测试（1000条知识负载）
- [ ] 编写用户文档
- [ ] 编写API文档
- [ ] 实现数据导入（Obsidian导出格式）
- [ ] 实现数据导出（Markdown+JSON）

### Week 12: 发布准备
- [ ] 创建GitHub Release
- [ ] 编写README（中英文）
- [ ] 制作演示视频/GIF
- [ ] 发布到Product Hunt/Hacker News
- [ ] 收集反馈，规划v1.1

## 关键检查点

### Checkpoint 1 (Day 14)
**目标**: 基础API可用，可通过curl创建和查询知识
**验证**: 
```bash
curl -X POST http://localhost:8000/api/v1/knowledge   -H "Content-Type: application/json"   -d '{"title":"测试","content":"内容"}'
```

### Checkpoint 2 (Day 28)
**目标**: 评分引擎运行，衰减任务自动执行
**验证**: 查看数据库score_history表有衰减记录

### Checkpoint 3 (Day 42)
**目标**: Agent可查询知识库
**验证**: Claude Desktop可调用knowledge_query工具

### Checkpoint 4 (Day 56)
**目标**: 完整UI可用，可视化正常
**验证**: 浏览器访问localhost:3000，可看到引力场图

### Checkpoint 5 (Day 84)
**目标**: v1.0发布
**验证**: GitHub Release创建，文档完整

## 风险预案

### 如果进度落后
1. **优先保障**: Phase 1+2（基础+评分），这是核心差异化
2. **可延后**: Phase 5前端（先用Obsidian原生）
3. **可简化**: Phase 4 Agent功能（先实现基础query）

### 如果遇到技术难题
1. **SQLite性能**: 切换到PostgreSQL（保持SQLAlchemy抽象）
2. **向量检索**: 使用chromaDB替代FAISS（更简单的API）
3. **MCP协议**: 先用简单HTTP API暴露给Agent

## 每日开发节奏建议

```
09:00-09:30  回顾昨日，规划今日
09:30-12:00  核心开发（深度工作）
12:00-14:00  午餐+休息
14:00-15:00  代码审查+测试
15:00-17:00  核心开发
17:00-18:00  文档+提交+复盘
```

## 工具链

- **编辑器**: VSCode / Cursor
- **API测试**: Postman / HTTPie / curl
- **数据库**: DB Browser for SQLite
- **Git**: GitHub Desktop / CLI
- **文档**: Markdown + Mermaid
- **沟通**: 单人项目，用GitHub Issues跟踪任务

---

**祝开发顺利！**
