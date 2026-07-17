# Compass 开发计划 (PLAN)

> 版本：v1.0 ｜ 日期：2026-07-05
> 文档链：[`PRD_v3.0.md`](PRD_v3.0.md) → [`ARCHITECTURE.md`](ARCHITECTURE.md) → [`TEST_CASES.md`](TEST_CASES.md) → 本计划 → [`GITHUB_WORKFLOW.md`](GITHUB_WORKFLOW.md)
> 原则：渐进式复杂，每个 Phase 有可验收闭环。

---

## 0. 路线图

| Phase | 名称 | 周期 | 状态 | 验收闭环 |
|-------|------|------|------|----------|
| 1 | 核心闭环 | 2-3 周 | ✅ 完成 | Obsidian 新建笔记→引擎算分写回 frontmatter→Dataview 排序可见 |
| 2 | 浮现与可视化 | 2 周 | ✅ 完成（Web 已冻结） | `/feed` 浮现正确；保留既有 Web 引力场兼容 |
| 3 | Agent/Skill 对接 | 1-2 周 | ✅ 完成 | skill action→Compass API→vault；本地 E2E 覆盖 action + render + FileWatcher |
| 4 | 智能增强 | 按需 | ✅ 完成 | 可解释建议/周报 + 可配置 Skill 连接 + 实时有效分 |
| 5 | 架构收敛与打磨 | 按需 | 待开发 | 按 `ARCHITECTURE.md` 收敛边界，再完成模板、备份与同步 |

**总周期：5-8 周。**

---

## 1. Phase 1 · 核心闭环（详细任务分解）

> 目标：建立"写笔记→引擎算分→写回 frontmatter→Obsidian 可见"的最小闭环。
> 技术栈：Rust 单二进制（axum + rusqlite + notify + serde_yaml + tokio + tower-http）。

### 任务分解

| ID | 任务 | 产出 | 依赖 | 工时 | 验收 |
|----|------|------|------|------|------|
| T1.1 | 项目骨架 | Cargo workspace + crate 依赖 + `compass.toml` 配置加载 | — | 4h | `cargo build` 通过；`/health` 返回 ok |
| T1.2 | frontmatter 读写模块 | 解析 YAML → 改 `score:` 块 → 原子写回 + 文件锁 | T1.1 | 8h | 读 .md frontmatter；改 score 写回不破坏正文；Obsidian 自动重载 |
| T1.3 | 评分引擎 | composite 公式 + 触发器表；历史持久化衰减已由 T4.9 取代 | T1.1 | 8h | 单元测试：权重 0.4/0.35/0.25；触发器 boost；时效仅在读取时计算 |
| T1.4 | SQLite 索引层 | `entities/score_history/timeline/entities_fts` + 可重建 | T1.1 | 6h | 从 vault 全量重建索引；FTS5 可查 |
| T1.5 | FileWatcher | `notify` 监听 vault → 解析 → 索引 + 重算评分 → 写回 frontmatter | T1.2,T1.3,T1.4 | 10h | Obsidian 新建/改笔记 → 30s 内索引+算分+写回 |
| T1.6 | 基础 API | `GET /feed` `/entities/top` `/entities/{id}` `/search` `PATCH /score` `/access` `POST /entities` | T1.4 | 8h | curl 各端点返回正确 JSON；score/access 写回 frontmatter |
| T1.7 | 验收测试 | 端到端：新建笔记→算分→写回→Dataview 排序 | T1.5,T1.6 | 4h | 闭环跑通；Dataview 查询按 composite 排序 |
| T1.8 | 文档与样例 | Templater 模板（带 score 骨架）+ README 更新 | T1.7 | 2h | 新建笔记模板含完整 frontmatter |

**Phase 1 合计：~50h（2-3 周）。**

### Phase 1 关键不变量（验收必须满足）

1. **frontmatter 是权威**：`score.composite` 由引擎计算并写回；SQLite 仅缓存，删库可从 vault 重建。
2. **历史衰减已废止**：T1.3 的持久化 interest 衰减已由 T4.9 替代；三维基础分不因时间改写，知识时效只在读取时形成有效分。
3. **权重默认 0.40/0.35/0.25**：不得出现 0.4/0.4/0.2（修正 v2.x bug）。
4. **写回不破坏正文**：只改 `score:` 块，Mermaid/正文/其他 frontmatter 字段不变。
5. **单二进制**：无 Python、无 subprocess；`cargo build --release` 产出一个可执行文件。

---

## 2. Phase 2 · 浮现与可视化

| ID | 任务 | 依赖 | 工时 |
|----|------|------|------|
| T2.1 | 历史衰减调度（已由 T4.9 废止） | P1 | 4h |
| T2.2 | Feed 三模式（explore/consolidate/strategic） | P1 | 6h |
| T2.3 | `/graph` 引力场数据端点（节点+边+评分） | P1 | 6h |
| T2.4 | Web 静态页（引力场 + Feed，已完成并冻结） | T2.3 | 12h |
| T2.5 | 验收：固定时间下实时有效分 + 节点大小=有效分 | T2.4 | 4h |

**Phase 2 合计：~32h（2 周）。**

---

## 3. Phase 3 · Agent/Skill 对接

> 飞书 ws / Agent / compass skill 均已有。本 Phase 只适配 skill 调 Compass API。

| ID | 任务 | 依赖 | 工时 |
|----|------|------|------|
| T3.1 | 适配 `skills/compass/compass` 脚本（端口 8080、端点路径对齐 §7） | P1 | 4h |
| T3.2 | 更新 `SKILL.md`（启动命令改 Rust 二进制、衰减描述修正） | T3.1 | 2h |
| T3.3 | 全链路验收：飞书→Agent→skill→compass→vault | T3.2 | 6h |

**Phase 3 合计：~12h（1-2 周）。**

---

## 4. Phase 4 · 智能增强（T4.0-T4.9 已完成）

Phase 4 的实施入口是 [`docs/PHASE4_PREP.md`](PHASE4_PREP.md)。先冻结协议和责任边界，再进入运行时开发：

| ID | 任务 | 依赖 | 状态 | 验收摘要 |
|----|------|------|------|----------|
| T4.0 | 协议、标签格式、事件与 schema migration | — | 已完成（PR #211） | 固定 JSON fixture；迁移可重复 |
| T4.1 | 事件与标签/链接可重建索引 | T4.0 | 已完成（PR #214） | rebuild 后索引恢复 |
| T4.2 | 带 content hash 的 metadata patch | T4.0,T4.7 | 已完成（PR #218） | stale 返回 409；只改目标字段 |
| T4.3 | 标签候选 + accept/reject | T4.1,T4.2,T4.7 | 已完成（PR #221） | 候选只读；accept 幂等；reject 无写入 |
| T4.4 | 关联推荐 | T4.1,T4.2,T4.7 | 已完成（PR #222） | 推荐只读；accept/reject 幂等；排除既有链接 |
| T4.5 | 认知周报聚合 | T4.1 | 已完成 | 固定时区可重复；覆盖空/缺失数据 |
| T4.6 | skill action/render 与 E2E | T4.3-T4.5,T4.7 | 已完成 | action → API → Vault/SQLite → render |
| T4.7 | HTTP 暴露面安全门禁 | Issue #206 | 已完成（PR #208） | 默认 localhost；非本机需显式配置 |
| T4.8 | Skill 服务连接合同 | T4.7 | 已完成（PR #233） | `COMPASS_API_URL` 覆盖默认地址；可选 Bearer token 透传；E2E 覆盖自定义地址与鉴权 |
| T4.9 | 实时有效分与知识时效 | T4.8 | 已完成（PR #233） | 基础分不因时间改写；无定时衰减；所有查询/渲染使用实时 `effective_composite` |

T4.9 取代历史的持久化 interest 衰减：`score` 中的三维基础分只由显式评分、访问和触发器改变；时间与知识时效仅在读取时形成有效分，不写回 Vault 或 SQLite。具体字段、非目标和完成定义见 [`docs/PHASE4_PREP.md`](PHASE4_PREP.md)。

---

## 5. Phase 5 · 架构收敛与打磨

**预估合计：~49h。** 架构主线（P5.1–P5.6）与工具链（P5.7–P5.9）相互独立，可并行推进。

| ID | 任务 | 依赖 | 工时 | 验收 |
|----|------|------|------|------|
| P5.1 | ✅ 覆盖缺口审查与刻画测试（#234 / PR #235） | `ARCHITECTURE.md`、`TEST_CASES.md` | 4h | 已逐一核查 TC-D/TC-V/TC-I/TC-Q/TC-H/TC-K/TC-A；补齐可在当前架构稳定刻画的测试，既有回归全通，剩余缺口已明确归属后续步骤 |
| P5.2 | ✅ Domain/DTO 分界与模块骨架 | P5.1 | 6h | 已建立 `domain/`、`application/`、`infrastructure/`、`transport/` 子目录与 `mod.rs`；领域模型与规则、HTTP DTO/路由已迁入相应边界，`pub(crate)` 可见性约束和无 Axum/SQL 行类型回归检查生效 |
| P5.3a | ✅ Vault 适配层隔离 | P5.2 | 6h | frontmatter 解析、扫描与原子写入已封装在 `infrastructure/vault_adapter.rs`；HTTP 与 rebuild 通过 `VaultPort` 调用；`db.rs` 不再扫描或解析 Markdown |
| P5.3b | ✅ 索引服务提取 | P5.3a | 4h | `application/index_service.rs` 统一 rebuild 与 watcher 的解析→投影路径；watcher 仅做 notify 适配与去抖，不再直接操作 SQLite；全量替换在短事务中完成，TC-I02/TC-A04 回归生效 |
| P5.4 | ✅ SQLite 仓储与锁范围收敛 | P5.3b | 6h | `EntityRow` 及 SQL 语句为 `sqlite_repository.rs` 私有；数据库锁不覆盖文件 I/O、排序或 `.await` |
| P5.5 | ✅ 查询、实体、建议应用服务 | P5.4 | 8h | `QueryService`、`EntityService`、`SuggestionService` 承载用例编排；HTTP handler 仅做 DTO 适配、调用与序列化 |
| P5.6 | ✅ 架构收尾与回归验收 | P5.5 | 4h | 已删除 HTTP 遗留编排与临时 dead-code 豁免；Rust、HTTP/Skill E2E、rebuild 幂等回归全通；文档与实际结构一致 |
| P5.7 | ✅ Dataview 查询模板库 | —（独立） | 3h | `docs/dataview-queries.md` 已覆盖 Top 高分、待复习、战略焦点、按层/分类/标签聚合、内容更新、孤儿与评分差异场景；模板直接读取稳定的 frontmatter 基础分 |
| P5.8 | ✅ Git 自动提交备份 | —（独立） | 4h | 已提供独立 Vault Git 仓库的 Markdown/稳定 Obsidian 配置白名单提交、审计日志、无变更不建空提交、预暂存保护与 Windows 每日任务安装脚本；Git 不参与 Compass API、索引、评分或恢复决策 |
| P5.9 | ✅ 跨端同步（Syncthing/WebDAV） | P5.8 | 4h | 已提供 Syncthing 首选与冲突保留型 WebDAV 操作规范；冲突副本不入索引、watcher 触发重建；双临时 Syncthing 实例的离线并发冲突与 Compass 启动重建已验证 |

> P5.1–P5.6 以 [`ARCHITECTURE.md`](ARCHITECTURE.md) 和 [`TEST_CASES.md`](TEST_CASES.md) 为设计、测试与验收基线；未经 PRD 更新不得改变公开 HTTP、Skill 或 Vault 契约。

> **Web UI 冻结策略：**保留现有 `web/` 静态页面、`/graph` API 和 Rust 静态服务，以维持已有访问方式；不删除、不新增功能、不进行 SPA 化或视觉重构。后续若重新投入，目标是将其剥离为可选的独立包/服务，而不是继续扩展 Compass 核心。

---

## 6. 当前状态与下一步

Phase 1、Phase 2、Phase 3 已完成。Phase 3 的本地验收从 `skills/compass` 开始，覆盖：

`skill action → Rust HTTP API → frontmatter/SQLite → FileWatcher → skill render`

证据：`cargo test --release`（141 个 Rust 测试）、`skills/compass/test_e2e.py`（17 个 HTTP/skill E2E）和 `skills/compass/test_compass.py`（23 个 Skill 单测）。完整验收见 [`docs/PHASE1_4_TEST_REPORT.md`](PHASE1_4_TEST_REPORT.md)。

Phase 4 的 T4.0-T4.9 已完成。skill 已覆盖标签候选、关联推荐、accept/reject、周报、服务认证与实时有效分，并通过 action → Rust API → Vault/SQLite → render 的本地 E2E。

P5.1–P5.9 已完成：`domain/` 承载纯模型与规则，`application/` 承载查询、实体、建议和索引用例，`infrastructure/` 承载 SQLite 与 Vault 适配器，`transport/http.rs` 仅保留 Axum DTO、路由、认证和错误序列化。rebuild 与 watcher 共享 `IndexService`，查询服务显式接收请求时间；P5.8 的外置 Git 任务记录 Vault 已有写入结果，P5.9 的外置同步工具仅复制 Vault 内容并保留冲突副本，不改变任何公开 HTTP、Skill、Vault 与冻结 Web 契约。Phase 5 已完成。
