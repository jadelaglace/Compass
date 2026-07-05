---
id: proj-compass-v3
title: Compass V3 — 评分引力场重做
layer: direction
category: [Projects, Compass]
tags:
  - compass
  - architecture
  - rust
score:
  interest: 9.5
  strategy: 9.5
  consensus: 7.0
  composite: 8.9
  weights: {interest: 0.4, strategy: 0.35, consensus: 0.25}
  updated_at: 2026-07-05T10:00:00+08:00
  last_boosted_at: 2026-07-05T10:00:00+08:00
  access_count: 1
status: active
source: {type: internal}
created_at: 2026-04-06T00:00:00+08:00
updated_at: 2026-07-05T10:00:00+08:00
---

# Compass V3 — 评分引力场重做

> 旧方向（V2："Rust core + Python glue"）已废弃。V3 回到原始意志：单一 Rust 二进制 + Obsidian 当 UI + 评分写回 frontmatter。

## 核心转向

- **评分引力场**：三维 `interest/strategy/consensus`，权重 0.40/0.35/0.25，只衰 interest（0.98^天，50% 地板）。
- **分数写回 frontmatter**：让引力场在 Obsidian/Dataview 可见（V2 把分数搬进 SQLite 是根本错误）。
- **纯 Rust 单二进制**：axum + rusqlite + notify + serde_yaml，无 Python 胶水、无 subprocess。
- **三方分工**：Obsidian 管编辑/链接/标签/图谱/搜索；Compass 只做评分→衰减→浮现；Web 极薄（HTMX+D3）只做引力场/Feed。
- **接入层已有**：飞书 ws → Agent → compass skill → Compass HTTP API。Compass 只提供 API。

## 文档索引

- 规格：[[PRD_v3.0]]（`docs/PRD_v3.0.md`）
- 计划：[[PLAN]]（`docs/PLAN.md`）
- 审查结论：[[REVIEW_整体分析结论]]（`docs/REVIEW_整体分析结论.md`）
- 已废弃：`docs/PRD_v2.1.md`

## 当前阶段

Phase 1 · 核心闭环（见 `docs/PLAN.md`）：Rust 骨架 + frontmatter 读写 + 评分引擎 + FileWatcher。

> 本文件本身采用 V3 frontmatter 格式，是 V3 数据模型的第一个实例笔记——引擎就绪后会被自动索引与评分。
