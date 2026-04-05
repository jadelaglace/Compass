# Compass 产品需求文档

| 字段 | 内容 |
|------|------|
| 产品名 | Compass（罗盘） |
| 版本 | v1.0 MVP |
| 日期 | 2026-04-05 |
| 负责人 | 贾玉冰 |
| 状态 | 规划完成，待开发 |

---

## 1. 产品概述

### 1.1 产品定义

**Compass（罗盘）** 是个人认知操作系统（Personal Compass OS）。

核心理念：知识不是库存，是有方向、有权重的流动网络。高价值内容自然浮现，过时内容优雅衰减。

**不是"第二大脑"、不是知识仓库。是一套让知识找到方向的操作系统。**

### 1.2 目标用户

| 画像 | 描述 |
|------|------|
| 知识工作者 | 每天处理大量信息，需要快速分类、优先级排序 |
| 终身学习者 | 持续积累阅读/研究，需要长期记忆管理 |
| AI Harness 用户 | 已经在用 Agent，希望知识管理和 Agent 工作流打通 |

### 1.3 核心问题解决

| 痛点 | 现有方案 | Compass 解法 |
|------|---------|-------------|
| 知识太多，找不到重点 | 标签/文件夹分类 | 三维评分自动排序 |
| 重要内容被淹没 | 人工维护置顶 | 评分衰减自动浮现 |
| 移动端记录麻烦 | 打开 App 写笔记 | 飞书 Bot 直接说 |
| 知识和 Agent 割裂 | 复制粘贴给 Agent | Agent 直接查询知识库 |

---

## 2. 产品功能

### 2.1 MVP 功能清单（Phase 1）

#### 2.1.1 Vault 结构

Obsidian 本地 Markdown 存储，目录规范：

```
vault/
├── Inbox/          # 快速记录入口，新内容先到这里
├── Direction/      # 战略方向、目标、原则（高 strategy 分）
├── Knowledge/      # 知识积累、可复用的信息
├── Logs/          # 过程日志、会议记录、临时内容
└── Insights/       # 洞察、想法、原创输出
```

**每个文件必须包含 YAML Front-matter：**
```yaml
---
id: unique-id
created: 2026-04-05
scores:
  interest: 7      # 1-10，个人兴趣度
  strategy: 9       # 1-10，对战略的相关度
  consensus: 6     # 1-10，被引用/共识程度
last_boosted:      # 最近一次被引用/讨论的时间
---
```

#### 2.1.2 三维评分引擎

**三个维度：**

| 维度 | 含义 | 衰减规则 |
|------|------|---------|
| **interest** | 这篇内容对我有多吸引 | 30天半衰期，interest 自然衰减 |
| **strategy** | 对我的战略/方向有多重要 | 低衰减，strategy 是锚点 |
| **consensus** | 有多少其他内容引用它 | 被引用时 boost，被遗忘时衰减 |

**计算公式：**
```
final_score = interest × 0.4 + strategy × 0.35 + consensus × 0.25
score = final_score × decay_factor
```

**评分来源：**
- 人工：用户直接给某个维度打分
- 自动：被引用时 boost，被时间遗忘时衰减
- Timeline：事件触发（阅读、讨论、写作）时激活评分

#### 2.1.3 引用解析系统

```markdown
# A 文件
这是 [[b1]] 的核心观点...

# B 文件  
ID: b1
这是一段被多处引用的内容...
```

- 每次保存文件，解析所有 `[[id]]` 引用
- 更新双向链接：outgoing（我引用谁）+ incoming（谁引用我）
- SQLite 记录：`entity_id`, `outgoing_refs[]`, `incoming_refs[]`

#### 2.1.4 飞书 Bot

| 命令 | 功能 | 示例 |
|------|------|------|
| `/q [内容]` | 快速记录到 Inbox | `/q 今天看了篇文章讲...` |
| `/r [query]` | 搜索并返回评分最高结果 | `/r 竞品分析` |
| `/s [id] [维度] [分数]` | 手动调整评分 | `/s b1 interest 9` |
| `/f` | 查看战略焦点（高 strategy 分内容） | `/f` |

**Bot 工作流：**
```
用户发送消息
      ↓
  LLM 理解意图（是记录？搜索？打分？）
      ↓
  REST API 调用 FastAPI 服务层
      ↓
  处理后返回结果（Markdown 格式）
```

#### 2.1.5 Agent API

供 Agent（Claude Code 等）调用的接口：

| 端点 | 方法 | 功能 |
|------|------|------|
| `/agent/context` | POST | 传入任务描述，返回相关知识片段 |
| `/entities/search` | GET | 关键词搜索，返回相关实体列表 |
| `/entities/{id}` | GET | 获取单个实体详情 |
| `/scores/{id}` | GET | 获取某个实体的评分详情 |

### 2.2 Phase 2 功能（路线图）

| 功能 | 说明 |
|------|------|
| 向量语义搜索 | sentence-transformers + FAISS，语义相近内容也能召回 |
| MCP Server | 支持 Claude Code / Cursor 等 MCP 生态 |
| 全自动清洗 | URL → fetch → 清洗 → 存储，全自动 pipeline |

### 2.3 Phase 3 功能（路线图）

| 功能 | 说明 |
|------|------|
| Web UI | 评分趋势图、知识图谱可视化 |
| D3.js 力导向图 | 可视化知识网络结构 |
| PWA | 移动端 Web 应用 |

---

## 3. 用户体验

### 3.1 核心流程

```
记录 → 评分 → 浮现

1. 记录（飞书Bot / Obsidian）
   用户发送 /q 或直接在 Obsidian 写

2. 评分（三维引擎自动计算）
   文件被引用 → consensus boost
   时间流逝 → interest decay
   战略标记 → strategy 锚定

3. 浮现（Agent / Bot 查询）
   用户问 Agent → Agent 调 /agent/context
   返回高评分相关知识
```

### 3.2 不做的事（Out of Scope）

- ❌ AI 自动摘要生成（Phase 2 之后再说）
- ❌ 多人协作 / 实时同步
- ❌ 自动标签分类
- ❌ 移动端原生 App（用飞书 Bot 替代）
- ❌ 笔记富媒体编辑（Obsidian 负责）

---

## 4. 市场与竞品

### 4.1 赛道定位

**AI-native × Scoring-native × Mobile-native**

现有产品要么有 AI 无评分，要么有评分无移动，要么有移动无 Agent。

Compass 是三者同时具备的唯一产品。

### 4.2 主要竞品

| 竞品 | 弱点 |
|------|------|
| Obsidian | 无原生 AI，插件拼凑 |
| Notion | 数据不归己，AI 封闭 |
| Roam Research | 贵，无移动端 |
| Logseq | 无 AI 能力 |
| Khoj | 只是问答，非管理系统 |
| Tana | 极度早期，极贵 |

### 4.3 核心差异化

**护城河 = 三维评分 × 飞书Bot × Agent原生**

---

## 5. 商业模式

### 5.1 License

**Compass Open License (COL)**

| 类型 | 条件 | 价格 |
|------|------|------|
| 个人免费 | 个人使用 | 免费 |
| 团队免费 | 10人以内非商业 | 免费 |
| 企业授权 | 11人以上 / SaaS / 商业集成 | 付费 |

### 5.2 变现路径

```
个人用户免费用
       ↓
声量起来（模板/插件生态）
       ↓
开发者社区传播
       ↓
企业版授权收费
```

---

## 6. Phase 1 里程碑

```
Week 1-2
  ├── Vault 目录结构设计
  ├── Obsidian 模板体系
  └── SQLite Schema 定义

Week 3-4
  ├── 评分引擎开发
  ├── Timeline 事件系统
  └── 引用解析（双向链接）

Week 5-6
  ├── FastAPI 服务层
  ├── Agent API（/agent/context）
  └── 飞书Bot 开发

Week 7
  └── 集成测试 + 端到端验收

Week 8
  └── 文档 + 备份方案 + MVP发布
```

---

## 7. 附录

### 7.1 关键文档

| 文档 | 路径 |
|------|------|
| 架构决策 | `../OPENCLAW.md` |
| 历史归档 | `../archive/` |

### 7.2 术语表

| 术语 | 定义 |
|------|------|
| Vault | Obsidian 的本地存储根目录 |
| Entity | 知识实体，对应一个 Markdown 文件 |
| Inbox | 快速记录入口，未整理的新内容 |
| Boost | 被引用时评分上升 |
| Decay | 时间流逝时评分自然衰减 |
| Consensus | 被引用次数越多，consensus 越高 |
