# PKM / Compass 架构方案判断

**时间：** 2026-04-05
**决策者：** 本源龙虾 ✨

---

## 产品名称

**Compass** —— 个人认知操作系统（Personal Compass OS）

定位：认知罗盘，而非第二大脑。让高价值内容自然浮现，让过时内容优雅衰减。

**中文名：** 罗盘
**英文名：** Compass
**Slogan：** "让知识找到方向" / "Knowledge, with direction"

---

## License 策略

**Compass Open License (COL)** + 商业闭源扩展

**免费授权（MIT兼容）：**
- 个人使用
- 非商业目的研究
- 10人以内的非商业团队

**商业授权（付费）：**
- 11人以上团队
- SaaS / 托管服务
- 商业产品集成（需书面授权）

**品牌保护：**
- 可标注"Powered by Compass"
- 不得自立门户使用Compass品牌

**核心壁垒不靠License，靠：**
1. 飞书Bot——独特入口
2. Agent工作流集成——AI辅助认知能力
3. 评分系统——数据资产

**热度引爆路径：** 个人用户 → 开发者社区 → 模板/插件生态 → 企业版付费

---

## 核心判断

**以 PCOS（规划B）为基础骨架，取规划A明显优势项补充，Phase 2预埋扩展接口。**

原则：高级功能 / 非最紧急功能 全部留接口，先跑MVP。

---

## 架构决策

### 取各方案明显优势

| 优势项 | 来源 | 决策 |
|--------|------|------|
| 飞书Bot（核心入口） | B | ✅ Phase 1直接上 |
| 文件监听 + 引用解析 | B | ✅ Phase 1直接上 |
| 三维评分引擎 | A/B均有 | ✅ Phase 1直接上 |
| **向量语义检索** | A | ⚙️ Phase 2，预留接口 |
| **D3.js 力导向图谱** | A | ⚙️ Phase 3，预留接口 |
| **PWA / 自建Web UI** | A | ⚙️ Phase 3，预留接口 |
| MCP 协议 | A | ⚙️ Phase 2，视生态演进决定 |

---

## 最终架构分层

```
Phase 1 (MVP · 8周)
┌─────────────────────────────────────────────────────────┐
│  接入层                                                        │
│  ┌──────────────┐  ┌──────────────────┐  ┌───────────────┐  │
│  │ 飞书Bot      │  │ Obsidian 桌面    │  │ Agent API     │  │
│  │ (快速记录)    │  │ (编辑/查阅)      │  │ (上下文查询)   │  │
│  └──────┬───────┘  └────────┬─────────┘  └───────┬───────┘  │
│         │                    │                     │          │
│  ┌──────▼────────────────────────────────────────────────▼───┐ │
│  │              FastAPI  服务层（统一网关）                      │ │
│  │   /entities  /scores  /query  /agent/context  /feed       │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  核心引擎                                                   │ │
│  │  ├─ 文件监听（watchdog）  ├─ 引用解析  ├─ 评分引擎          │ │
│  │  ├─ FTS5 全文检索         ├─ Timeline 事件系统              │ │
│  └────────────────────────────────────────────────────────────┘ │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │  数据层                                                     │ │
│  │  ├─ Obsidian Vault（Markdown，根数据）                      │ │
│  │  └─ SQLite（索引、元数据、评分历史）⚙️向量扩展接口预埋         │ │
│  └────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────┘

Phase 2（语义层 · 独立验证）
┌──────────────────────────────────────────────────────────────┐
│  + sentence-transformers 本地推理                            │
│  + FAISS 向量索引（⚙️接口已预埋，Phase 1 已预留插入点）           │
│  + /search/semantic API                                       │
│  + MCP Server（⚙️接口已预埋）                                   │
└──────────────────────────────────────────────────────────────┘

Phase 3（可视化层 · 可选）
┌──────────────────────────────────────────────────────────────┐
│  + Vue3 + D3.js 力导向图谱（⚙️接口已预埋）                      │
│  + PWA / Web UI（⚙️接口已预埋）                                │
│  + 移动端专属优化                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## Phase 1 功能范围（MVP）

### 必须交付（P0）

| 模块 | 功能 | 说明 | 状态 |
|------|------|------|------|
| **Vault结构** | 目录规范 + 模板体系 | Inbox/Direction/Knowledge/Logs/Insights | ✅ |
| **文件监听** | watchdog 实时同步 | 文件增删改 → SQLite 索引自动更新 | ✅ |
| **引用解析** | `[[id]]` 双向链接 | outgoing + incoming 自动维护 | ✅ |
| **评分引擎** | interest / strategy / consensus | 30天半衰期衰减，支持手动覆盖 | ✅ |
| **飞书Bot** | `/q` `/r` `/s` `/f` 命令 | 快速记录、搜索、评分查看、战略焦点 | 🔴 |
| **URL抓取** | `/fetch` 接口 | Agent 调用获取正文（Phase 1 不过清洗） | 🔴 |
| **Agent API** | `/agent/context` | 上下文准备、建议链接、批量操作 | 🔴 |

> **截至 2026-04-07：** P0 前四项已完成（PR #13 合并），FileWatcher + Vault Service + 引用解析全部打通。飞书Bot、URL抓取、Agent API 为待开发状态。

### 预留接口（P1，Phase 1 只留插口）

| 扩展点 | Phase 1 行为 | Phase 2 实现 |
|--------|-------------|-------------|
| **向量语义检索** | FTS5 全文检索 | 叠加 semantic_search API |
| **MCP 协议** | REST /agent/* 端点 | MCP Server 适配层 |
| **前端图谱** | 飞书卡片 + Obsidian 内链 | Vue3 + D3.js 独立部署 |
| **多端同步** | Git 版本控制 | WebDAV / Syncthing 方案 |
| **数据清洗** | Agent/人类保证干净 | 独立清洗 pipeline |
| **URL抓取全自动** | Agent 手动调用 /fetch | 全自动 fetch → clean → save |

---

## 接口演进路线（REST → MCP）

### OpenClaw Agent Phase 1 接入方式

```
飞书消息
    ↓
OpenClaw Agent（现成 Bot，LLM 驱动）
    ↓ Tool Call（OpenClaw Skill）
compass-api（FastAPI REST）
    ↓ subprocess JSON-RPC
Rust Core（compass-core）
```

**OpenClaw Skill 是 Agent 访问 Compass 的唯一渠道，不直接调 REST。**

| 阶段 | 接口形式 | Agent 接入方式 |
|------|---------|---------------|
| **Phase 1** | FastAPI REST | OpenClaw Skill（封装 HTTP） |
| **Phase 2** | REST + MCP Server | OpenClaw Skill → MCP Tools 迁移 |
| **稳定期** | MCP Server 为主 | REST 可选关闭 |

---

## 接口预留规范（Phase 1 必须遵守）

### 1. 向量语义扩展接口

```python
# Phase 1: 只实现基础检索
@app.post("/search/fulltext")
async def fulltext_search(query: str, limit: int = 20):
    """FTS5 全文搜索"""
    pass

# Phase 2: 扩展为混合检索
@app.post("/search/hybrid")
async def hybrid_search(
    query: str,
    semantic_weight: float = 0.5,  # Phase 2 参数，Phase 1 忽略
    limit: int = 20
):
    """FTS5 + 向量混合搜索，semantic_weight 暂时硬编码为 0"""
    # Phase 1: fallback to fulltext
    # Phase 2: real hybrid with FAISS
    pass
```

### 2. 前端扩展接口

```python
# Phase 1: Graph 数据已通过 REST API 暴露
@app.get("/graph/neighbors/{entity_id}")
async def get_neighbors(entity_id: str, depth: int = 1):
    """邻居查询，Phase 3 前端直接调用此 API"""
    pass

@app.get("/graph/evolution")
async def get_evolution(days: int = 90):
    """评分演化数据，Phase 3 D3.js 直接消费"""
    pass
```

### 3. MCP 协议扩展接口

```python
# Phase 1: MCP 适配层
class MCPLayer:
    async def knowledge_query(self, query: str, top_k: int = 5):
        """转发到 /agent/context"""
        pass

# Phase 2: 当 MCP 生态成熟时，替换为官方 mcp-sdk 实现
```

---

## 开发节奏

```
Week 1-2: Vault结构 + 文件监听 + SQLite Schema ✅
Week 3-4: 评分引擎 + Timeline事件 + 引用解析 ✅ (PR #13 合并)
Week 5-6: FastAPI + /agent/* 接口 + 飞书Bot 🔴 进行中
Week 7:   集成测试 + 端到端验收
Week 8:   文档 + 备份方案 + MVP发布

Phase 2:  向量语义层（Phase 1 稳定运行30天后启动）
Phase 3:  可视化层（按需）
```

> **当前进度（2026-04-08）：** Week 1-4 已完成，P0 前四项全部 ✅。OpenClaw Skill ✅ 已合入。下一步：飞书 Bot（Issue #18）。

---

## 分支管理流程

```
长期分支：
  master   — 稳定可发布状态，始终等于最新正式版
  develop  — 下一版本开发集成分支，所有功能并入此处

临时分支（从 develop 或 master 创建）：
  feat/{issue}-{short-desc}   功能分支
  fix/{issue}-{short-desc}   修复分支
  docs/{issue}-{short-desc}  文档分支

合并路径：
  feat/fix/docs → develop → master
  （文档分支可从 master 创建，但必须通过 PR 合并）
```

### 铁律

1. **从哪个分支创建，就必须先拉那个分支的 最新** — feat/fix 从 develop 拉最新，docs 从 master 拉最新。违反此条会携带不该合并的 commit，导致 PR 冲突爆炸
2. **develop 是默认开发分支** — 所有新功能先合入 develop，stable 后再进 master
3. **开分支前必须拉最新（以 develop 为例）：**
   ```bash
   # feat/fix 分支
   git checkout develop && git pull origin develop
   git checkout -b feat/18-feishu-bot

   # docs 分支
   git checkout master && git pull origin master
   git checkout -b docs/5-contribution-guide
   ```
4. **PR 必须关联 Issue** — Description 写清楚 Closes #18
5. **Review 通过前禁止 self-merge**
6. **Phase 稳定后** — develop 合并进 master，打版本 tag，进入下一 Phase

---

## 风险与应对

| 风险 | 可能性 | 影响 | 应对 |
|------|--------|------|------|
| 评分算法主观性 | 中 | 高 | Phase 1 保留手动覆盖权，用户反馈调参 |
| 飞书Bot网络依赖 | 高 | 低 | 本地队列 + 失败重试 |
| 扩展接口成技术债 | 低 | 中 | 严格遵守本规范的接口预留要求 |
| Phase 2 向量层拖累Phase 1 | 低 | 高 | Phase 2 独立项目，独立验收 |

---

## Phase 1 接口预埋设计

### 1. 数据清洗接口（Phase 2 实现）

```python
# Phase 1: 只留 interface，不过清洗
@app.post("/clean")
async def clean_content(raw: str) -> CleanedContent:
    """
    接口预埋，Phase 2 实现
    目前由 Agent/人类保证输入干净
    """
    raise NotImplementedError("Phase 2 pipeline")
```

### 2. URL 抓取接口（Phase 1 设计，Agent 调用）

```python
# Phase 1: Agent 决定是否调用，自己判断清洗逻辑
@app.post("/fetch")
async def fetch_url(url: str) -> FetchResult:
    """
    输入：URL
    输出：提取的正文（Markdown / JSON）

    Phase 1: Agent calls firecrawl/exa API directly
    Phase 2: 独立清洗 pipeline，全自动
    """
    raise NotImplementedError("Agent calls firecrawl/exa API directly")

# Agent 调用链（Phase 1）
# 用户: "把这个页面存进来 https://..."
#      → Agent 理解意图
#      → 调用 /fetch → 返回正文
#      → Agent 清洗 + 格式化
#      → 调用 /entities POST → 写入 Vault
```

### 3. 媒体存储策略

**存储原则：Markdown 文件存完整内容，SQLite 只存元数据。**

| 类型 | 存储方式 | Markdown 语法 |
|------|---------|---------------|
| **本地附件** | Vault 内 `assets/` 目录，MD 存相对路径 | `![](assets/img.png)` |
| **外链媒体** | MD 直接存 URL | `![](https://example.com/video.mp4)` |
| **PDF** | 本地附件目录 | `![[file.pdf]]` |

**SQLite 字段：**
- `file_path` — Markdown 文件路径
- `has_attachments` — bool，是否含附件
- `attachment_refs` — JSON 数组，附件路径列表

### 4. 各端媒体处理能力

| 端 | 图片 | PDF | 视频 | 音频 |
|----|:----:|:---:|:----:|:----:|
| **Obsidian 桌面** | ✅ 内嵌渲染，相对路径自动解析 | ✅ `![[]]` 嵌入预览 | ⚠️ 本地文件插件嵌入，外链跳浏览器 | ⚠️ 同视频 |
| **Obsidian 移动端** | ✅ 本地渲染 | ✅ 嵌入预览 | ⚠️ 外链跳转系统播放器 | ⚠️ 外链跳转 |
| **飞书Bot（当前）** | ❌ 纯文本，只发链接 | ❌ 只发链接 | ❌ 只发链接 | ❌ 只发链接 |
| **飞书（人看）** | ⚠️ 链接可点，跳浏览器/本地App | ⚠️ 同左 | ⚠️ 同左 | ⚠️ 同左 |

**飞书 Bot 媒体处理说明：**
- 发 Markdown 格式 → 飞书客户端不渲染富媒体
- Phase 1 直接发链接，不做额外处理
- 如需飞书直接显示图片：发 base64 或用飞书富媒体卡片 API

**Phase 3 前端媒体处理：**
- 解析 Markdown 时将相对路径拼接为绝对路径
- 附件读取 = 文件系统操作，无额外转换成本

---

## 竞品分析

### 赛道定位图

```
                    AI-native
                         │
         Khoj           │         Tana
         (AI对话知识库)  │        (节点+AI)
                         │
Notion ──────────────────┼────────────────── Obsidian
(工具型/云优先)          │          (本地优先/插件生态)
                         │
                         │     思源笔记
                         │     (块引用/中文生态)
                         │
                     传统工具
                    (目录/标签)
```

### 竞品详情

| 产品 | 定位 | 存储 | AI能力 | 评分系统 | 移动端 | License | 致命弱点 |
|------|------|------|--------|----------|--------|---------|----------|
| **Obsidian** | 网络化思考 | 本地MD | 插件(Third Brain等) | ❌ 无 | 官方App | Freemium | 无原生AI，插件拼凑 |
| **Notion** | All-in-one | 云 | Notion AI(闭源) | ❌ 无 | 良 | 订阅制 | 数据不归己，AI封闭 |
| **Roam Research** | 双向链接先驱 | 云 | 基础AI | ❌ 无 | 弱 | $15/月 | 贵，无移动端 |
| **Logseq** | 开源Roam替代 | 本地MD | 无原生AI | ❌ 无 | 弱 | MIT | 无AI能力 |
| **RemNote** | 间隔重复+笔记 | 云 | 基础AI | ❌ 无 | 良 | Freemium | 主打记忆，非认知管理 |
| **Heptabase** | 可视化白板 | 云 | 基础AI | ❌ 无 | ❌ | $15/月 | 移动端无，不适合纯文字 |
| **思源笔记** | 块引用/MD | 本地+云同步 | 基础AI | ❌ 无 | 良 | 开源(AFL) | 中文圈，AI弱 |
| **Tana** | 节点+AI | 云 | 原生AI搜索 | ❌ 无 | PWA | 订阅制 | 极度早期，极贵 |
| **Khoj** | AI对话知识库 | 本地/云 | RAG对话(开源) | ❌ 无 | 弱 | MIT | 只是问答，非管理 |
| **Readwise Reader** | 读取+AI摘要 | 云 | 摘要/高亮 | ❌ 无 | 良 | $8/月 | 只是读，不是管 |

### Compass 的差异化位置

```
         AI-native     │   Scoring-native   │  Mobile-native
                        │                     │
    ┌───────────────────┼─────────────────────┼───────────────┐
    │                   │                     │               │
    │   Khoj (弱移动)   │                     │   飞书Bot     │
    │                   │     ⭐ Compass     │   差异化入口   │
    │   Tana (无移动)   │   (三维评分+移动)    │               │
    │                   │                     │               │
    └───────────────────┴─────────────────────┴───────────────┘
```

### 核心结论

**这个赛道没有真正的竞争对手**——现有产品要么有AI无评分，要么有移动无AI，要么有存储无Agent。

Compass 的护城河是**三维评分引擎 × 飞书Bot × Agent原生**三角组合，没有人同时做这三个。

**最大威胁是 Notion AI**——如果 Notion 做出"知识评分+动态浮现"功能，会直接挤压。但目前 Notion AI 停留在"写作助手"层级，短期内不会做评分系统。

---

_判断原则：MVP先跑通核心价值流，高级功能留好扩展口，不因为"以后可能会有"而增加当前复杂度。_
