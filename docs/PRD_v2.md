# Compass 产品需求文档 v2.0

**PRD 2.0 — 全量升级版**

| 字段 | 内容 |
|------|------|
| 产品名 | Compass（罗盘） |
| 版本 | v2.0 |
| 日期 | 2026-04-17 |
| 状态 | Phase 1 完成，Phase 2 规划中 |

---

## 一、版本说明

### v1.0 MVP（Phase 1）完成状态

| 模块 | 功能 | 状态 |
|------|------|------|
| **Vault 结构** | 目录规范 + 模板体系 | ✅ 已完成 |
| **文件监听** | watchdog 实时同步 | ✅ 已完成 |
| **引用解析** | `[[id]]` 双向链接 | ✅ 已完成 |
| **评分引擎** | interest / strategy / consensus + 衰减 | ✅ 已完成 |
| **FastAPI** | CRUD / query / feed / agent/context | ✅ 已完成 |
| **OpenClaw Skill** | Agent 访问 Compass 的统一入口 | ✅ 已完成 |

### v2.0 升级背景

v1.0 MVP 完成了核心闭环：**记录 → 关联 → 评分 → 浮现**。

v2.0 的目标：**补全 Phase 1 砍掉的功能，按难度排序，分阶段实现 Phase 2 及后续计划。**

砍掉的功能来源：
- **PCOS v1.0 PRD**（规划B版本）— 完整功能套件，Phase 1 只取了骨架
- **PKM Universe PRD v1.0**（规划A版本）— 向量语义层、可视化层、Agent协议层

---

## 二、被砍功能全量列表

### 2.1 难度分级说明

| 难度 | 定义 | 典型特征 |
|------|------|---------|
| 🟢 **Easy** | 纯配置/脚本，0-1天 | Obsidian 插件配置、脚本、简单 cron |
| 🟡 **Medium** | 独立模块，1-3天 | 单个 API 端点、独立服务、简单后台任务 |
| 🔴 **Hard** | 核心模块，1-2周 | 评分引擎重构、向量索引、协议集成 |
| 🔴🔴 **Very Hard** | 系统级，2-4周+ | 向量语义层、MCP Server、可视化图谱 |

---

### 2.2 🟢 Easy — 零成本可上线（1天内）

| # | 功能 | 来源 | 描述 | 实现方式 |
|---|------|------|------|---------|
| E1 | **孤儿节点检测** | PCOS M3 | 90天无引用自动标记 orphan | APScheduler cron job |
| E2 | **Obsidian 插件配置** | PCOS M5A | QuickAdd + Dataview + Templater + Periodic Notes 一键配置脚本 | YAML 配置文件 + install.sh |
| E3 | **Dataview 评分展示模板** | PCOS M5A | 查询库：Top N、按分类筛选、评分分布 | Dataview JS 查询模板 |
| E4 | **自动 Git commit** | PKM | 每日自动 commit 备份 | cron + git hooks |
| E5 | **手动评分覆盖界面** | PCOS M3 | 用户直接修改 Frontmatter 手动评分，API 层支持覆盖 | Frontmatter 规范扩展 |

---

### 2.3 🟡 Medium — 独立模块（1-3天）

| # | 功能 | 来源 | 描述 | 实现方式 |
|---|------|------|------|---------|
| M1 | **孤儿节点提醒** | PCOS M3 | 检测到孤儿节点后，通过飞书 Bot 推送提醒用户 | 定时任务 + 飞书卡片 |
| M2 | **飞书 Bot `/l` 引用命令** | PCOS M5C | 创建 A→B 引用链接 | `compass link <from> <to>` |
| M3 | **飞书 Bot `/t` 今日日志命令** | PCOS M5C | 直接进入 ShortTerm 日志层 | `compass log --type short` |
| M4 | **飞书 Bot 引用区块生成** | PCOS M2 | 文件末尾自动生成 outgoing/incoming 引用区块 | 文件监听触发，追加到 Markdown |
| M5 | **评分历史 API** | PCOS M3 | `GET /entities/{id}/score/history` 返回评分变化曲线 | SQLite score_history 表查询 |
| M6 | **冲突检测与标记** | PCOS M2 | 文件 hash 不一致时生成 `.conflict.md` | 文件监听 + hash 比对 |
| M7 | **全量同步工具** | PCOS M2 | 手动重建整个数据库索引 | `compass sync --full` CLI |
| M8 | **Obsidian QuickAdd 模板库** | PCOS M5A | 预置：quick_log / concept / case / seed 等模板 | 模板文件 + install.sh |
| M9 | **Agent `/agent/suggest_links` 接口** | PCOS M4 | 建议潜在链接 | content similarity + graph proximity |
| M10 | **Agent `/agent/action` 批量操作接口** | PCOS M4 | 批量创建/链接/评分操作 | POST /agent/action，支持事务 |

---

### 2.4 🔴 Hard — 核心模块（1-2周）

| # | 功能 | 来源 | 描述 | 实现方式 |
|---|------|------|------|---------|
| H1 | **高级评分触发器** | PKM | application_boost（理论被实践）、access_boost（深度阅读）、手动标记重点 | 扩展 timeline 事件类型 + 评分调整逻辑 |
| H2 | **共识维度增强** | PCOS/PKM | 入度中心性 + 跨层引用加成 + 标签通用性加成 | 扩展 `calculate_consensus()` 算法 |
| H3 | **飞书 Bot 富媒体卡片** | PCOS M5C | 搜索结果、评分详情用飞书卡片渲染（而非纯文本） | 飞书 Card API |
| H4 | **评分面板 Obsidian 插件** | PCOS M5A | 在 Obsidian 内直接看/改评分，不用飞书 | Obsidian Plugin SDK |
| H5 | **评分演化数据 API** | PKM | `GET /graph/evolution?days=90` 返回 Top N 知识评分变化轨迹 | 时序数据聚合 API |
| H6 | **URL fetch → 清洗 → 存储 Pipeline** | PCOS M4 | Agent 调用 `POST /fetch` 获取 URL 正文，自动清洗入库 | 集成 firecrawl/exa 或自建 scraper |
| H7 | **战略焦点增强 API** | PCOS M3/PKM | `GET /feed/strategic` 返回高 strategy 实体列表 + 趋势分析 | 扩展 feed 接口 + 趋势计算 |
| H8 | **间隔重复提醒系统** | PKM | 知识被标记为"待复习"后，按遗忘曲线提醒 | SQLite 复习记录表 + cron |

---

### 2.5 🔴🔴 Very Hard — 系统级（2-4周+）

| # | 功能 | 来源 | 描述 | 实现方式 |
|---|------|------|------|---------|
| VH1 | **FAISS 向量语义索引** | PKM | sentence-transformers 生成向量 + FAISS 索引，支持语义相似度召回 | Python: sentence-transformers + faiss-cpu |
| VH2 | **混合搜索 API** | PKM | FTS5 全文 + 向量语义混合加权检索 | POST /search/hybrid，支持 semantic_weight 参数 |
| VH3 | **MCP Server** | PKM | Model Context Protocol 服务端实现，支持 Claude/Cursor 直接调用 | `mcp-sdk` Python 实现 |
| VH4 | **D3.js 引力场图谱** | PKM | Vue3 + D3.js 力导向图，节点大小=评分，颜色=分类 | 独立 PWA 项目 or 嵌入式 Web Component |
| VH5 | **PWA Web UI** | PKM | 浏览器访问的 Web 应用，替代/增强 Obsidian 移动端 | Vue3 + Vite + Service Worker |
| VH6 | **标签关系图谱 API** | PKM | `GET /graph/tags` 返回标签共现关系，用于 D3.js 可视化 | SQLite tag_relations 表 + 算法 |
| VH7 | **认知摘要生成 API** | PKM | 总结最近 N 天评分变化最大、新增高价值、被遗忘的重要知识 | `/agent/insight_summary` |
| VH8 | **Case 应用案例系统** | PKM | 记录"理论→实践→反思"闭环，独立于 Knowledge 的案例表 | SQLite cases 表 + API |
| VH9 | **Insight 感悟进化系统** | PKM | 感悟分 spark/framework/mature 三个成熟度，可演化为正式知识 | SQLite insights 表 + 演化算法 |
| VH10 | **Brain Map Mermaid 渲染** | PKM | 知识文件内 Mermaid 脑图，API 返回渲染数据 | mermaid.js + 前端/插件渲染 |
| VH11 | **多 Vault 同步** | PKM | 支持多个 Vault 挂载，跨 Vault 引用和评分 | 路径别名 + Vault 注册表 |

---

## 三、Compass v2.0 功能全景图

```
┌─────────────────────────────────────────────────────────────────┐
│                      Compass v2.0 功能全景                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ████████████████████ Phase 1 已完成 ████████████████████        │
│                                                                 │
│  ✅ Vault 结构 + Frontmatter 规范                               │
│  ✅ 文件监听（watchdog）                                         │
│  ✅ 引用解析（[[id]] 双向链接）                                   │
│  ✅ 三维评分引擎（interest/strategy/consensus + 衰减）            │
│  ✅ FastAPI CRUD / query / feed / agent/context                 │
│  ✅ OpenClaw Skill                                              │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ████████████████████ Phase 2 ████████████████████               │
│  ██ 语义增强层 ██                                                │
│                                                                 │
│  🟢 E1   孤儿节点检测（cron）                                     │
│  🟢 E2   Obsidian 插件一键配置                                    │
│  🟢 E3   Dataview 评分展示模板                                   │
│  🟢 E4   自动 Git commit                                         │
│  🟡 M1   孤儿节点飞书提醒                                         │
│  🟡 M2   飞书 /l 引用命令                                         │
│  🟡 M3   飞书 /t 今日日志命令                                      │
│  🟡 M4   引用区块自动生成                                         │
│  🟡 M5   评分历史 API                                            │
│  🟡 M6   冲突检测与标记                                          │
│  🟡 M7   全量同步工具                                            │
│  🟡 M8   Obsidian QuickAdd 模板库                                │
│  🟡 M9   /agent/suggest_links 接口                               │
│  🟡 M10  /agent/action 批量操作接口                               │
│  🔴 H1   高级评分触发器                                           │
│  🔴 H2   共识维度增强（跨层引用+标签通用性）                         │
│  🔴 H3   飞书 Bot 富媒体卡片                                      │
│  🔴 H4   Obsidian 评分面板插件                                    │
│  🔴 H5   评分演化数据 API                                         │
│  🔴 H6   URL fetch→清洗→存储 Pipeline                           │
│  🔴 H7   战略焦点增强 API                                        │
│  🔴 H8   间隔重复提醒系统                                         │
│  🔴🔴 VH1 FAISS 向量语义索引                                      │
│  🔴🔴 VH2 混合搜索 API（全文+语义）                                │
│  🔴🔴 VH3 MCP Server                                            │
│                                                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ████████████████████ Phase 3 ████████████████████               │
│  ██ 可视化层 ██                                                  │
│                                                                 │
│  🔴🔴 VH4   D3.js 引力场图谱                                     │
│  🔴🔴 VH5   PWA Web UI                                          │
│  🔴🔴 VH6   标签关系图谱 API                                       │
│  🔴🔴 VH7   认知摘要生成 API                                       │
│  🔴🔴 VH8   Case 应用案例系统                                      │
│  🔴🔴 VH9   Insight 感悟进化系统                                   │
│  🔴🔴 VH10  Brain Map Mermaid 渲染                               │
│  🔴🔴 VH11  多 Vault 同步                                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 四、Phase 2 详细开发计划

### 4.1 Sprint 划分

```
Phase 2 总工期：约 8-10 周

Sprint 1（Week 1-2）🟢 Easy 收尾
Sprint 2（Week 3-4）🟡 Medium 核心增强
Sprint 3（Week 5-6）🔴 Hard 评分与 Agent 增强
Sprint 4（Week 7-8）🔴🔴 Very Hard 向量语义层
Sprint 5（Week 9-10）收尾 + 集成测试
```

### 4.2 Sprint 1（Week 1-2）🟢 Easy 收尾

**目标：Phase 1 遗留的零成本功能全部上线**

| 任务 | 功能 | 预计工时 | 验收标准 |
|------|------|---------|---------|
| E1-a | 孤儿节点检测 cron job | 2h | 90天无引用实体自动标记 status=orphan |
| E1-b | 孤儿节点飞书提醒（M1） | 2h | 检测到孤儿 → 飞书卡片推送 |
| E2 | Obsidian 插件一键配置脚本 | 4h | 运行脚本自动安装/配置 QuickAdd/Dataview/Templater |
| E3 | Dataview 评分展示模板库 | 3h | 10个常用查询模板（Top10/分类/评分分布等） |
| E4 | 自动 Git commit hook | 2h | 每日 23:30 自动 git add + commit |
| E5 | 手动评分覆盖支持 | 2h | Frontmatter 手动设置 scores覆盖API默认值 |

**产出：**
- E1-E5 全部上线
- Phase 1 功能完整性达到 95%

---

### 4.3 Sprint 2（Week 3-4）🟡 Medium 核心增强

**目标：飞书 Bot 完整功能 + 引用系统增强**

| 任务 | 功能 | 预计工时 | 验收标准 |
|------|------|---------|---------|
| M2 | 飞书 `/l <from> <to>` 引用命令 | 4h | 创建双向引用，同步更新 refs 表 |
| M3 | 飞书 `/t <content>` 今日日志命令 | 3h | 内容直接进 Logs/ShortTerm 层 |
| M4 | 引用区块自动生成 | 6h | 文件保存时末尾追加 outgoing/incoming 区块 |
| M5 | 评分历史 API | 4h | GET /entities/{id}/score/history 返回完整轨迹 |
| M6 | 冲突检测与标记 | 5h | hash 不一致 → 生成 .conflict.md |
| M7 | 全量同步工具 | 3h | `compass sync --full` 完整重建索引 |
| M8 | Obsidian QuickAdd 模板库 | 4h | 预置 5 种模板文件，自动安装 |

**产出：**
- 飞书 Bot 具备完整命令集（/q /r /s /f /l /t）
- 引用系统完整闭环

---

### 4.4 Sprint 3（Week 5-6）🔴 Hard 评分与 Agent 增强

**目标：评分引擎完整 + Agent 接口增强**

| 任务 | 功能 | 预计工时 | 验收标准 |
|------|------|---------|---------|
| H1 | 高级评分触发器 | 8h | application_boost / access_boost / manual_boost 三种触发 |
| H2 | 共识维度增强 | 6h | 入度中心性 + 跨层引用 + 标签通用性 三路加成 |
| H3 | 飞书 Bot 富媒体卡片 | 8h | 搜索结果/评分详情用飞书卡片 API 渲染 |
| H4 | Obsidian 评分面板插件 | 12h | Obsidian 内看/改评分，实时同步 SQLite |
| H5 | 评分演化数据 API | 6h | GET /graph/evolution 返回 90 天评分变化轨迹 |
| H6 | URL fetch→清洗→存储 Pipeline | 10h | POST /fetch 返回清洗后正文，POST /entities 自动入库 |
| H7 | 战略焦点增强 API | 5h | GET /feed/strategic 返回 Top10 + 趋势分析 |
| H8 | 间隔重复提醒系统 | 8h | 基于遗忘曲线生成复习提醒，推送飞书 |

**产出：**
- 评分引擎完整版（与 PCOS PRD M3 对齐）
- Agent API 完整（context / suggest_links / action / insight_summary）
- 飞书 Bot 富媒体化

---

### 4.5 Sprint 4（Week 7-8）🔴🔴 Very Hard 向量语义层

**目标：Phase 2 核心差异化能力上线**

| 任务 | 功能 | 预计工时 | 验收标准 |
|------|------|---------|---------|
| VH1 | FAISS 向量语义索引 | 16h | sentence-transformers 生成向量，FAISS 建索引 |
| VH2 | 混合搜索 API | 10h | POST /search/hybrid，semantic_weight 可调 |
| VH3 | MCP Server | 16h | 实现 knowledge_query / knowledge_create / score_suggest 三个核心工具 |

**技术方案（VH1+VH2）：**
```
用户查询
    ↓
query → sentence-transformers → 向量
    ↓
FAISS ANN 检索 top_k
    ↓
与 FTS5 结果混合加权
    ↓
返回排序结果
```

**技术方案（VH3）：**
```
Claude/Cursor (MCP Client)
    ↓ MCP Protocol
compass-mcp-server
    ↓
compass-api (FastAPI)
    ↓
compass-core (Rust)
```

**Note：** VH1-VH3 可并行开发。VH1 和 VH2 可串行（VH2 依赖 VH1）。

**产出：**
- `/search/semantic` 语义搜索能力
- `/search/hybrid` 混合搜索能力
- MCP Server 支持 Claude/Cursor 直接访问 Compass

---

### 4.6 Sprint 5（Week 9-10）收尾 + 集成测试

**目标：Phase 2 完整验收**

| 任务 | 内容 | 预计工时 |
|------|------|---------|
| 端到端测试 | 飞书 Bot → API → Rust Core → SQLite 全链路 | 8h |
| 性能测试 | 1000 实体查询 P95 < 200ms，向量检索 P95 < 500ms | 6h |
| MCP 联调 | Claude Desktop + Compass MCP Server 实际调用 | 6h |
| 文档更新 | API 文档更新 + README 更新 | 4h |
| Phase 2 Release | tag v2.0，Release Note | 2h |

---

## 五、功能优先级排序（按难度 × 价值）

### 5.1 推荐执行顺序

```
第一梯队（立即做，价值高 + 难度低）
  E1 孤儿节点检测
  E2 Obsidian 插件配置脚本
  E3 Dataview 评分模板
  M2 飞书 /l 引用命令
  M3 飞书 /t 今日日志

第二梯队（紧随其后，价值高 + 难度中）
  H3 飞书富媒体卡片
  H6 URL fetch Pipeline
  H7 战略焦点增强
  M4 引用区块自动生成
  M9 /agent/suggest_links

第三梯队（Phase 2 核心差异化）
  VH1 FAISS 向量索引
  VH2 混合搜索 API
  VH3 MCP Server
  H4 Obsidian 评分面板插件
  H5 评分演化 API

第四梯队（Phase 3）
  VH4-VH11 可视化层 + 高级系统
```

### 5.2 PCOS/PKM 功能对应表

| PCOS/PKM 原功能 | 对应 Compass v2.0 任务 | Phase | 难度 |
|----------------|----------------------|-------|------|
| PCOS M3 孤儿节点检测 | E1 | P2S1 | 🟢 |
| PCOS M5A QuickAdd 配置 | E2 | P2S1 | 🟢 |
| PCOS M5A Dataview 模板 | E3 | P2S1 | 🟢 |
| PCOS M5C /l 引用命令 | M2 | P2S2 | 🟡 |
| PCOS M5C /t 今日日志 | M3 | P2S2 | 🟡 |
| PCOS M2 引用区块生成 | M4 | P2S2 | 🟡 |
| PCOS M3 评分历史 | M5 | P2S2 | 🟡 |
| PCOS M2 冲突检测 | M6 | P2S2 | 🟡 |
| PCOS M2 全量同步 | M7 | P2S2 | 🟡 |
| PCOS M5A QuickAdd 模板 | M8 | P2S2 | 🟡 |
| PCOS M4 suggest_links | M9 | P2S2 | 🟡 |
| PCOS M4 /agent/action | M10 | P2S2 | 🟡 |
| PKM 高级评分触发器 | H1 | P2S3 | 🔴 |
| PCOS M3 共识维度增强 | H2 | P2S3 | 🔴 |
| PCOS M5C 富媒体卡片 | H3 | P2S3 | 🔴 |
| PCOS M5A 评分面板插件 | H4 | P2S3 | 🔴 |
| PKM 评分演化图谱 | H5 | P2S3 | 🔴 |
| PCOS M4 URL fetch | H6 | P2S3 | 🔴 |
| PCOS M3 战略焦点增强 | H7 | P2S3 | 🔴 |
| PKM 间隔重复提醒 | H8 | P2S3 | 🔴 |
| PKM FAISS 向量索引 | VH1 | P2S4 | 🔴🔴 |
| PKM 混合搜索 | VH2 | P2S4 | 🔴🔴 |
| PKM MCP Server | VH3 | P2S4 | 🔴🔴 |
| PKM D3.js 引力场 | VH4 | P3 | 🔴🔴 |
| PKM PWA Web UI | VH5 | P3 | 🔴🔴 |
| PKM 标签图谱 API | VH6 | P3 | 🔴🔴 |
| PKM 认知摘要 | VH7 | P3 | 🔴🔴 |
| PKM Case 系统 | VH8 | P3 | 🔴🔴 |
| PKM Insight 系统 | VH9 | P3 | 🔴🔴 |
| PKM Brain Map | VH10 | P3 | 🔴🔴 |
| PKM 多 Vault 同步 | VH11 | P3 | 🔴🔴 |

---

## 六、技术债务与接口预留

### 6.1 Phase 2 必须遵守的接口预留

Phase 1 已预埋以下接口，Phase 2 必须实现：

```python
# 向量语义扩展（Phase 2 必须实现）
@app.post("/search/hybrid")
async def hybrid_search(
    query: str,
    semantic_weight: float = 0.5,  # Phase 2 实时值，Phase 1 硬编码为 0
    limit: int = 20
):
    """FTS5 + 向量混合搜索"""
    pass

# MCP 扩展（Phase 2 必须实现）
class MCPLayer:
    async def knowledge_query(self, query: str, top_k: int = 5):
        """转发到 /agent/context"""
        pass

# 清洗 Pipeline（Phase 2 必须实现）
@app.post("/clean")
async def clean_content(raw: str):
    """Phase 2 实现，Phase 1 抛出 NotImplementedError"""
    raise NotImplementedError("Phase 2 pipeline")
```

### 6.2 技术债务清单

| 债务项 | 来源 | 优先级 | 修复方式 |
|--------|------|--------|---------|
| 飞书富媒体（仅文本） | Phase 1 简化 | P1 | 升级为 Card API |
| URL fetch 手动 | Phase 1 砍掉 | P1 | 实现 H6 |
| 无评分历史追踪 | Phase 1 简化 | P1 | 实现 M5 |
| 引用区块需手动维护 | Phase 1 砍掉 | P2 | 实现 M4 |

---

## 七、Phase 2 验收标准

### 7.1 功能验收矩阵

| 模块 | 测试项 | 通过标准 |
|------|--------|---------|
| E1 | 孤儿节点检测 | 90天无引用实体自动标记 orphan |
| E2 | 插件配置 | 运行脚本，Obsidian 内可看到所有插件 |
| E3 | Dataview 查询 | 表格显示 composite_score，与 API 查询一致 |
| M2 | /l 命令 | 创建 A→B 后，refs 表双向记录存在 |
| M3 | /t 命令 | 飞书发送 → Logs/ShortTerm/ 下文件创建 |
| H3 | 富媒体卡片 | 飞书搜索结果以卡片渲染，非纯文本 |
| H6 | URL Pipeline | POST /fetch → 返回正文 → POST /entities → 文件入库 |
| VH1 | FAISS 索引 | 1000 实体建索引 < 5 分钟，语义查询 < 500ms |
| VH2 | 混合搜索 | 查询"深度学习框架"，同时召回标题含"TensorFlow"和语义相关但标题不含的内容 |
| VH3 | MCP Server | Claude Desktop 可通过 MCP 调用 knowledge_query |

### 7.2 性能指标

| 指标 | 目标 | 测试方式 |
|------|------|---------|
| API 响应 P95 | < 200ms | k6 负载测试 |
| 向量检索 P95 | < 500ms | 1000 实体，100 次随机查询 |
| 飞书 Bot 响应 | < 3s | 端到端计时 |
| 全量索引重建 | < 5 分钟 | 1000 实体计时 |
| 混合搜索准确性 | Top 10 中 7 个相关 | 人工评估 |

---

## 八、竞品对比（v2.0 阶段）

| 产品 | 评分系统 | 飞书入口 | 向量语义 | Agent 原生 | 移动端 |
|------|---------|---------|---------|-----------|-------|
| **Compass v2.0** | ✅ 三维动态 | ✅ 飞书 Bot | ✅ FAISS | ✅ MCP | ✅ 飞书 |
| Obsidian | ❌ 无 | ❌ 无 | ⚠️ 插件 | ⚠️ 插件 | ⚠️ 弱 |
| Notion | ❌ 无 | ❌ 无 | ❌ 无 | ⚠️ 封闭 AI | ✅ |
| Roam | ❌ 无 | ❌ 无 | ❌ 无 | ⚠️ 弱 | ❌ 无 |
| Logseq | ❌ 无 | ❌ 无 | ❌ 无 | ❌ 无 | ⚠️ 弱 |
| Khoj | ❌ 无 | ❌ 无 | ✅ | ✅ | ❌ 无 |
| Tana | ❌ 无 | ❌ 无 | ✅ | ✅ | ⚠️ PWA |

**Compass v2.0 是唯一同时具备：三维评分 × 飞书 Bot × 向量语义 × MCP × 移动端的产品。**

---

## 九、版本规划

| 版本 | 功能 | 时间 |
|------|------|------|
| **v1.0 MVP** | Phase 1 完成，核心闭环 | 2026-04-08 ✅ |
| **v2.0** | Phase 2 完成，语义增强 | 预计 2026-06-20 |
| **v3.0** | Phase 3 完成，可视化层 | 预计 2026-08-30 |

---

## 十、维护记录

| 版本 | 日期 | 修改内容 |
|------|------|---------|
| v1.0 MVP | 2026-04-05 | Phase 1 初始规划 |
| v2.0 | 2026-04-17 | 全量列出 PCOS/PKM 砍掉功能，按难度排序，Phase 2 详细计划 |
