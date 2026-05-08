# Compass · 罗盘

> 个人认知操作系统 · Personal Compass OS

---

## 产品定位

**Compass（罗盘）** —— 让高价值内容自然浮现，让过时内容优雅衰减。

不是"第二大脑"，不是知识仓库。是**认知操作系统**：帮你记住、排序、浮现最重要的知识。

---

## 技术架构

```
┌─────────────────────────────────────────────────────┐
│                    接入层                              │
│  飞书Bot · Obsidian · Agent API (OpenClaw Skill)    │
└────────────────────┬────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────┐
│              compass-api (Python/FastAPI)             │
│  REST API · CRUD · Graph · Search · Fetch · Decay    │
└────────────────────┬────────────────────────────────┘
                     │ FFI (ctypes)
┌────────────────────▼────────────────────────────────┐
│            compass-core (Rust)                        │
│  评分引擎 · 衰减算法 · FTS5 · 引用解析                  │
└─────────────────────────────────────────────────────┘
                     │
┌────────────────────▼────────────────────────────────┐
│              数据层                                   │
│  Obsidian Vault (Markdown) + SQLite (索引/元数据)     │
└─────────────────────────────────────────────────────┘
```

---

## 快速导航

| 文档 | 说明 |
|------|------|
| [docs/PRD_v2.1.md](docs/PRD_v2.1.md) | **产品需求文档 v2.3** — Phase 1-5 完整规划与状态 |
| [docs/TDD.md](docs/TDD.md) | 测试驱动开发规范与测试地图 |
| [OPENCLAW.md](OPENCLAW.md) | 架构决策 · 技术选型 · License |
| [SKILL.md](SKILL.md) | OpenClaw Agent Skill 接口定义 |
| [archive/](archive/) | 历史文档归档 |

---

## 开发生态

### 技术栈

| 层级 | 技术 |
|------|------|
| API | Python 3.11 · FastAPI · Pydantic v2 |
| 核心引擎 | Rust (ctypes FFI) |
| 数据库 | SQLite + FTS5 |
| 索引存储 | Obsidian Vault (Markdown) |
| Agent集成 | OpenClaw Skill (自然语言接口) |
| 飞书 | Webhook 消息管道 |

### 目录结构

```
Compass/
├── compass-core/          # Rust 核心引擎
│   ├── src/
│   │   ├── scoring.rs     # 三维评分引擎
│   │   ├── decay.rs       # 半衰期衰减
│   │   ├── search.rs      # FTS5 搜索
│   │   └── reference.rs   # 引用解析
│   └── Cargo.toml
├── compass-api/           # Python FastAPI 层
│   └── src/
│       ├── api/           # 各模块端点
│       ├── core/          # Rust 客户端封装
│       ├── db/            # SQLite + FTS5
│       └── services/      # FileWatcher 等
├── docs/                  # PRD / TDD / 架构文档
└── archive/               # 历史版本归档
```

---

## 开发阶段

### ✅ Phase 1 — MVP (完成)

| 模块 | 功能 | 状态 |
|------|------|------|
| Vault 结构 | 目录规范 + Frontmatter | ✅ |
| 文件监听 | watchdog 实时同步 | ✅ |
| 引用解析 | `[[id]]` 双向链接 | ✅ |
| 三维评分 | interest / strategy / consensus | ✅ |
| FastAPI | CRUD / query / feed / agent | ✅ |
| OpenClaw Skill | 自然语言接口 | ✅ |

### ✅ Phase 2 — 后端扩展 (完成)

| 模块 | 功能 | PR |
|------|------|-----|
| Schema Foundation | entity_type / status / maturity / taggings | #74 |
| 实体列表 | 分页 + 过滤 + 搜索 | #72 |
| 邻居查询 | Graph BFS / depth / min_strength | #75 #78 |
| URL 抓取 | fetch → clean → vault 保存 | #76 #79 |
| 混合搜索 | BM25 + FTS5 全文检索 | #77 |
| Timeline | 访问记录 + 评分历史 | #81 |
| Insights | 感悟 CRUD + maturity 状态机 | #82 |
| Insight 演化 | 感悟成熟 → 知识升级 | #83 |
| 引用强度 | 共同邻居推断强度 | #84 |
| 双向引用 | 反向边自动维护 | #85 |
| Decay 配置 | 半衰期个性化 | #68 #69 #70 |
| 导出接口 | JSON / Markdown 导出 | #119 |

### ✅ Phase 3 — 自动能力增强 (完成)

| 模块 | 功能 | PR |
|------|------|-----|
| 自动标签 | 创建时自动提取标签 | #107 |
| 标签推荐 | 智能标签推荐 + 批量更新 | #111 |
| Maturity 状态机 | 实体成熟度三级演化 | #108 |
| 演化规则引擎 | 可配置演化规则 | #114 |
| 关联推荐 | 相似度混合打分 | #109 |
| 自动关联 | 双向引用边自动创建 | #114 |

### 🎨 Phase 4 — 前端与可视化 (待开发)

| 模块 | 功能 | 依赖 |
|------|------|------|
| Vue3 骨架 | TypeScript + Vite + Pinia | — |
| 实体列表页 | 分页 + 过滤 + 搜索 | P4-UI-1 |
| 实体详情页 | Markdown 渲染 + 引用 | P4-UI-1 |
| 评分面板 | 三维雷达图 + 历史曲线 | P4-UI-1 |
| 图谱可视化 | D3.js Force-Directed | P2-Graph-1 |
| Feed 信息流 | explore / consolidate / strategic | P2-Search-1 |
| 搜索页面 | 语义搜索 + 高亮 | P2-Search-1 |
| 时间线页面 | 访问 + 评分历史 | P2-Timeline-2 |
| Insight 页面 | 成熟度状态机 | P2-Insight-2 |
| 用户设置 | 权重 + Decay 配置 | P4-UI-1 |
| PWA | SW + Manifest + 离线队列 | P4-UI-1 |
| MCP Server | 3 Tool → 15 Tool | P2-Search-1 |

**Phase 4 工时：84h（4-6 周）**

### 🚀 Phase 5 — 部署与工程化 (待开发)

| 模块 | 功能 | 依赖 |
|------|------|------|
| docker-compose | 一键部署 | P4-UI-1 |
| Dockerfile | 容器化 | P5-Deploy-1 |
| 监控面板 | 健康检查 + 指标 | P2-Search-1 |
| 数据迁移 | 导入/导出工具 | P2-Entity-1 |

**Phase 5 工时：35h（2-3 周）**

### 🎨 Phase 4 — 前端与可视化 (待开发)

| 模块 | 功能 | 依赖 |
|------|------|------|
| Vue3 骨架 | TypeScript + Vite + Pinia | — |
| 实体列表页 | 分页 + 过滤 + 搜索 | P4-UI-1 |
| 实体详情页 | Markdown 渲染 + 引用 | P4-UI-1 |
| 评分面板 | 三维雷达图 + 历史曲线 | P4-UI-1 |
| 图谱可视化 | D3.js Force-Directed | P2-Graph-1 |
| Feed 信息流 | explore / consolidate / strategic | P2-Search-1 |
| 搜索页面 | 语义搜索 + 高亮 | P2-Search-1 |
| 时间线页面 | 访问 + 评分历史 | P2-Timeline-2 |
| Insight 页面 | 成熟度状态机 | P2-Insight-2 |
| 用户设置 | 权重 + Decay 配置 | P4-UI-1 |
| PWA | SW + Manifest + 离线队列 | P4-UI-1 |
| MCP Server | 3 Tool → 15 Tool | P2-Search-1 |

**Phase 4 工时：84h（4-6 周）**

### 🚀 Phase 5 — 部署与工程化 (待开发)

| 模块 | 功能 | 依赖 |
|------|------|------|
| docker-compose | 一键部署 | P4-UI-1 |
| Dockerfile | 容器化 | P5-Deploy-1 |
| 监控面板 | 健康检查 + 指标 | P2-Search-1 |
| 数据迁移 | 导入/导出工具 | P2-Entity-1 |

**Phase 5 工时：35h（2-3 周）**

---

## API 端点一览

```
GET    /entities                  实体列表（分页/过滤）
POST   /entities                  创建实体
GET    /entities/{id}             获取实体
PUT    /entities/{id}             更新实体
DELETE /entities/{id}             删除实体（级联清理）

PATCH  /entities/{id}/access      访问记录（触发 decay）
PATCH  /entities/{id}/score       手动评分

GET    /entities/{id}/timeline    时间线（访问/评分历史）
GET    /entities/{id}/score/history  评分趋势

GET    /graph/neighbors/{id}       邻居查询
GET    /graph/path/{from}/{to}    最短路径

POST   /search                    混合搜索（BM25 + FTS5）
POST   /fetch                     URL 抓取

GET    /insights                  Insight 列表
POST   /insights                  创建 Insight
PATCH  /insights/{id}/maturity     成熟度演化

PATCH  /decay/config              半衰期配置
GET    /decay/preview             衰减预览
GET    /decay/simulate            衰减模拟

GET    /feed                      个性化推送
GET    /feed/strategic            战略焦点
```

---

## 贡献指南

### 分支模型

```
长期分支：
  main     — 稳定可发布状态，始终等于最新正式版
  dev      — 下一版本开发集成分支，所有 PR 的目标分支

临时分支（从 dev 创建）：
  feat/{issue-id}-{desc}
  fix/{issue-id}-{desc}
  docs/{issue-id}-{desc}   从 main 创建，PR → dev

合并路径： feat/fix/docs → dev → main
```

### 流程规范

```
1. 创建 Issue（描述完整，包含验收标准）
2. 从目标分支拉干净分支：
   git checkout dev && git pull origin dev
   git checkout -b feat/42-new-feature
3. 开发 + 测试
4. 提交 PR（包含：解决什么问题、怎么验证）
5. Code Review（至少 1 人 Approve）
6. PR → dev → main（功能/修复）
7. 文档更新走单独 PR，基于 main 创建
```

### PR 规范

- **Title**: `{type}: {简短描述}`
- **Description** 必须包含：问题描述、验证方式、影响范围
- **最小 PR 原则**：一个 PR 只解决一个问题
- **Review 通过前禁止 self-merge**

---

## License

**Compass Open License (COL)**
- 免费：个人 / 10人以内非商业团队
- 付费：企业 / SaaS / 商业集成

详见 [OPENCLAW.md](OPENCLAW.md)
