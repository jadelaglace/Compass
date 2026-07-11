# Compass Phase 4 准备规格

> 状态：准备完成，尚未开始运行时实现
> 依据：`docs/PRD_v3.0.md`、Phase 1-3 已合并实现、GitHub Issue #205
> 目标：在不偏离“纯 Rust + Obsidian 为根 + Agent 优先”的前提下，冻结 Phase 4 的边界、协议和验收方式。

## 1. 结论先行

Phase 4 先做**可解释的候选建议和结构化汇总**，不做自动决策。

- Compass 负责读取 Vault、建立可重建索引、计算确定性候选、校验建议、生成报告数据。
- Agent/skill 负责调用已有 LLM、组织自然语言、渲染结果和交给飞书 ws 发送。
- Obsidian 负责编辑、标签、双向链接、搜索和最终人工确认。
- Compass 只有在明确的 accept 操作之后，才允许对 Markdown 做最小范围写回。

因此，Phase 4 不引入 Rust LLM SDK、Embedding 服务、Python 胶水、Feishu transport 或新的 Web SPA。

## 2. 不可变约束

| 能力 | 责任方 | Phase 4 规则 |
|---|---|---|
| Markdown/frontmatter | Obsidian + Vault | 根数据；SQLite 可删除并从 Vault 重建 |
| 标签和双向链接 | Obsidian | Compass 只提供建议；接受后才做最小写回 |
| 候选计算和排序 | Compass Rust | 第一版只用 FTS5、词项/标签重叠、已有 wiki-link 和评分 |
| LLM 推理 | Agent（已有） | Compass 不保存 provider key，不直接请求模型 |
| JSON 到人话/卡片 | compass skill（已有） | Phase 4 只扩展 action/renderer，不把渲染塞进 Rust |
| 飞书消息 | 飞书 ws（已有） | Compass 不发送消息、不实现 WebSocket |
| Web | Compass 静态页 | Phase 4 默认不新增 JS；需要展示时复用现有薄页 |

## 3. 功能边界

### 3.1 自动标签建议

“自动”只表示自动产生候选，不表示自动修改笔记。

- Rust 基线候选：标题、正文、category、已有标签共现和 FTS 词项。
- Agent 可提交 LLM 候选，但必须标记 `source=agent` 或具体 provider。
- 已存在的标签不重复建议；建议必须经过人工 accept 才能写入。
- reject、过期、重复 accept 都有明确状态；accept 必须幂等。

### 3.2 关联推荐

第一版是**可解释的相关候选**，不是语义搜索。

- 使用 FTS/词项重叠、标签重叠、已有 wiki-link 的局部图距离和 composite 作为排序信号。
- 排除当前实体、已直接链接实体和 archived 实体。
- 每个候选返回理由和各信号分解；不自动创建 `[[link]]`。
- 接受关联时只修改目标实体明确允许的正文/metadata，并校验内容版本。

### 3.3 认知周报

Compass 生成确定性的结构化数据，Agent/skill 再决定如何叙述和发送。

- 输入必须包含明确的时间范围和时区。
- 输出至少包含：评分上升/下降 Top 5、访问/复习统计、新增实体、建议 accept/reject 统计和数据缺失提示。
- 首次运行、无事件、重复请求必须有稳定结果；不在 Compass 内发送飞书。
- 周报默认按需计算；预计算只能是可选缓存，不能改变权威数据。

## 4. 计划中的协议

以下是实现前冻结的方向，具体字段变更须通过独立 issue/PR，不在本准备 PR 中提前实现。

### 4.1 标签建议

计划接口：

```text
POST /entities/{id}/tag-suggestions
POST /tag-suggestions/{suggestion_id}/accept
POST /tag-suggestions/{suggestion_id}/reject
```

建议对象至少包含：

```json
{
  "suggestion_id": "sug-...",
  "entity_id": "know-...",
  "tag": "决策科学",
  "confidence": 0.82,
  "reason": "标题与正文词项重叠",
  "source": "rust_lexical",
  "algorithm_version": "tags-v1",
  "content_hash": "...",
  "status": "pending"
}
```

### 4.2 关联推荐

计划接口：

```text
GET /entities/{id}/related?limit=10
```

每个结果至少包含 `id`、`title`、`composite`、`score`、`reasons` 和 `content_hash`。`score` 是候选排序分，不得伪装成语义相似度。

### 4.3 周报

计划接口：

```text
GET /reports/weekly?from=2026-07-06&to=2026-07-12&tz=Asia/Shanghai
```

响应必须带 `from`、`to`、`tz`、`generated_at`、`data_quality` 和各统计分组。`generated_at` 不能参与业务排序，便于同一时间范围重复生成和测试。

## 5. 数据和写回策略

### 5.1 SQLite

建议新增的 suggestion、tag、link 缓存均属于可重建数据；不能成为 Markdown 的第二权威。

- 增表前先建立 schema version/migration 机制，已有 `.compass/index.db` 必须可升级。
- 建议记录保存 `content_hash`、`algorithm_version`、`source`、`status` 和时间戳。
- Vault 全量 rebuild 后，实体、标签和链接缓存应能恢复；历史事件是否可恢复必须在协议中明确。
- 不把完整 LLM prompt 或敏感正文永久写入 SQLite。

### 5.2 Markdown 写回

建议接口是唯一允许写回的入口，且必须满足：

1. accept 前校验实体仍存在且 `content_hash` 未变化；过期建议返回 `409`。
2. 只合并指定标签或链接，不重排无关 frontmatter，不改正文其他内容。
3. 规范化标签为 YAML 字符串数组，值不带 `#`；比较重复时大小写不敏感，但保留首次写入的原文。
4. 接受操作幂等；reject 不修改 Vault。
5. 复用锁、原子写和 watcher 重建，并补充并发/换行/BOM 回归测试。

## 6. 任务拆解和验收

| ID | 任务 | 依赖 | 读/写边界 | 最小验收 |
|---|---|---|---|---|
| T4.0 | 协议、标签格式、事件和 schema migration | 无 | 只读文档/fixture | 固定 JSON fixture、迁移可重复执行 |
| T4.1 | 事件与标签/链接索引基础 | T4.0 | 读 Vault；写可重建 SQLite | rebuild 后索引可恢复，删除不破坏启动 |
| T4.2 | 通用 metadata patch | T4.0 | accept 时最小写 Markdown | 正文、score、无关字段保持不变；stale 返回 409 |
| T4.3 | 标签候选和 accept/reject | T4.1,T4.2 | 建议只读；accept 写标签 | 固定输入结果稳定，重复 accept 幂等，reject 无文件变化 |
| T4.4 | 关联推荐 | T4.1 | 只读；不自动写 link | 排除自身/已有链接/archived，并返回可解释理由 |
| T4.5 | 周报聚合 | T4.1 | 读历史和事件；默认不写 | 固定时区下重复请求一致，覆盖空数据和缺失数据 |
| T4.6 | skill action/render 与 E2E | T4.3,T4.4,T4.5 | 外部 Agent/skill 只调 JSON API | action → Rust API → Vault/SQLite → render 全链路通过 |
| T4.7 | 安全门禁 | Issue #206 | 影响 HTTP 暴露面 | 默认 localhost；非本机访问需显式配置/认证 |

执行顺序固定为 `T4.0 → T4.1 → T4.2 → (T4.3/T4.4/T4.5) → T4.6`；T4.7 在任何新增写接口合并前完成。

## 7. 明确不做

- 不在 Rust 内嵌 LLM、Embedding、向量数据库或外部模型服务。
- 不自动覆盖用户标签，不自动创建双向链接，不自动移动/删除笔记。
- 不开发 Obsidian 插件、完整详情页、搜索页、时间线 UI 或 Phase 4 SPA。
- 不把 Feishu ws、消息发送、Agent 意图解析重新实现到 Compass。
- 不把“词项相关”宣传为“语义相似”；语义能力若需要，走已有 Agent 并保留来源标记。

## 8. 完成定义

Phase 4 只有同时满足以下条件才算完成：

- 所有写操作均由显式 accept 触发，且通过内容版本校验。
- 建议、推荐、周报有固定 fixture 和 Rust 单元/E2E 测试。
- skill renderer 能处理成功、空结果、过期和拒绝结果。
- `cargo test`、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings` 和 skill 测试通过。
- 本地部署仍是一个 Rust 二进制；无新增 Python/subprocess/JS 构建链。
