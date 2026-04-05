# Compass · 罗盘

> 个人认知操作系统 · Personal Compass OS

---

## 产品定位

**Compass（罗盘）** —— 让高价值内容自然浮现，让过时内容优雅衰减。

不是"第二大脑"，不是知识仓库。是**认知操作系统**：帮你记住、排序、浮现最重要的知识。

---

## 核心价值

**三维评分引擎** × **飞书Bot** × **Agent原生**

---

## 快速导航

| 文档 | 说明 |
|------|------|
| [docs/PRD.md](docs/PRD.md) | **产品需求文档** — 产品/CEO 输出，定义做什么、为什么、不做什么 |
| [OPENCLAW.md](OPENCLAW.md) | **架构决策** — 技术选型、Phase 规划、竞品分析 |
| [archive/](archive/) | 历史文档 — 规划A/B 版本归档 |

---

## 开发阶段

```
Phase 1 (MVP · 8周)
├── Vault 结构设计
├── 三维评分引擎
├── 飞书Bot
├── Obsidian 移动端接入
└── Agent API

Phase 2（向量语义层）
└── FAISS 向量索引 + 语义搜索

Phase 3（可视化层）
└── Web UI + D3.js 图谱
```

---

## 贡献指南

**所有参与者必须遵守完整的 Issue/PR 流程。**

### 流程规范

```
1. 创建 Issue（描述要完整，包含验收标准）
2. 创建分支：git checkout -b feat/{issue-id}-{short-description}
3. 开发 + 测试
4. 提交 PR（包含：解决什么问题、怎么验证）
5. Code Review（至少 1 人 Approve）
6. Merge to master
```

### 分支命名

| 类型 | 格式 | 示例 |
|------|------|------|
| 功能 | `feat/{issue-id}-{description}` | `feat/2-TDD-phase1` |
| 修复 | `fix/{issue-id}-{description}` | `fix/42-vault-path-bug` |
| 文档 | `docs/{issue-id}-{description}` | `docs/5-contribution-guide` |

### PR 规范

- **Title**: `{type}: {简短描述}` （如 `feat: implement scoring engine decay`）
- **Description** 必须包含：
  - 解决了什么问题
  - 怎么验证（测试说明 / 截图）
  - 影响范围
- **最小 PR 原则**：一个 PR 只解决一个问题
- **Review 通过前禁止 self-merge**

### Code Review 要求

- 审核者需明确 Approve 或 Request Changes
- 结构性变更（架构、数据库 Schema）必须 CTO Review
- 文档更新同样需要 Review（不允许 self-merge）

### ⚠️ 重要

> **此要求适用于所有变更类型——代码、文档、配置、CI/CD。本规则本身更新也必须走 Issue/PR 流程。**

---

## License

**Compass Open License (COL)**
- 免费：个人 / 10人以内非商业团队
- 付费：企业 / SaaS / 商业集成

详见 [OPENCLAW.md](OPENCLAW.md)
