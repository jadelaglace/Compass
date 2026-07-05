# Compass 整体分析结论（原始需求 + 现状审视 综合版）

> 版本：v1.0 ｜ 日期：2026-07-05
> 依据：归档原始文档（`archive/`）+ 现状代码（`compass-api` / `compass-core`）+ `docs/PRD_v2.1.md`
> 性质：审查结论，非实施规格。实施规格见后续 `PRD_v3.0`（待定）。

---

## 一、最初真正想要的东西（从归档还原）

### 1.1 一句话与一个引擎

手写原始文字（`archive/README.md`）：

> "这是一个以'我'为核心、高度结构化且动态演进的**个人知识宇宙**。"

**核心引擎 = 动态相关度评分系统**，原始比喻是"**引力场**"——整个知识库的"最高准则"，决定每个知识元素的"价值"与"位置"。三维：

1. 我最感兴趣的（**现在**）— 当下热情
2. 我最该关心的（**未来**）— 战略布局
3. 共识（**过去**）— 已验证的基石

运作：综合打分 + 动态更新 + 历史追踪 → "认知演化图谱"。

### 1.2 三大界

- **架构层**（宇宙地图）：基石系列→学科系列→分支系列，金字塔，受评分调控。
- **内容层**（原子与标本）：知识原子（理论）+ 案例标本（实践），相互引用成闭环。
- **日志感悟层**（认知轨迹）：日志（长期/中期/短期/实时时间切片）+ 感悟（必须自己写的思想结晶）。

全局：统一标签系统 + 双向引用。

### 1.3 第一次提问点明的三条底线

- **数据主权**：Obsidian vs Notion vs MySQL 的抉择。
- **Agent 友好**：接口对 agent 友好；"接入 agent 的话，UI 就不用太考虑了"。
- **多端**：手机/浏览器方便看。

### 1.4 架构设计原案的四条关键决策（真实意志）

1. **数据主权优先**——选 Obsidian，确保 50 年后还能打开。
2. **Agent 优先设计**——未来主要交互是自然语言对话。
3. **评分是灵魂**——三维评分是区别于普通笔记软件的核心竞争力，**保留手动调整权（AI 建议，人类决策）**。
4. **渐进式复杂**——从 Markdown+标签起步，逐步加评分/图谱/Agent。

### 1.5 规划A 与 规划B 的三点共识（关键）

- 评分**写进 Markdown frontmatter**（规划A 的 `relevance_score:` 块；规划B 的 `scores:` 块）。
- **Obsidian 内可见评分**：规划A "DataviewJS 展示分数"；规划B "Obsidian 插件开发 + Dataview 查询模板，表格展示 composite_score"。
- 后端都用 **Python/FastAPI**（归档中**无任何 Rust**——"纯 Rust"是后发的新偏好）。

---

## 二、从原始需求到现在：三处关键漂移

### 漂移 ①：评分从 frontmatter 搬进 SQLite，Obsidian 看不到了

- **原始**：分数是 Markdown 文件的一部分（frontmatter），Obsidian/Dataview 直接读，Git 还能 diff 分数变化。
- **现状**：`compass-api/src/db/schema.sql` 把分数全放进独立 `scores` 表，frontmatter 无分数。**结果：在 Obsidian 里完全看不到"引力场"**——而引力场本该是最高准则。这逼出了漂移②③。

### 漂移 ②：放弃 Obsidian 插件，Obsidian 降级成"纯文件管理器"

- **原始**：规划A/B 都要在 Obsidian 里做插件 + Dataview 展示评分（"浮现"的主舞台）。
- **现状**：`docs/PRD_v2.1.md` §13 明确"**不开发 Obsidian 插件**，Obsidian 纯作 Vault 文件管理器"。

### 漂移 ③：另起一套 Vue3+TS+D3 Web UI（Phase 4，84h）

- **原始**：Web/PWA 只是"轻量查询、评分调整"的**辅助端**；主交互是 Obsidian + Agent。
- **现状**：Phase 4 规划完整 SPA（列表/详情/搜索/图谱/时间线/评分面板），**与 Obsidian 自身 UI 大面积重复**，且与"少量 JS"的新偏好冲突。

> 三处漂移是连环的：分数离开 frontmatter → Obsidian 看不到 → 砍掉 Obsidian 插件 → 只能另建 Web UI。**根因是漂移①。**

### 两处实现漂移（与原始规格不符）

- **衰减模型**：原始规划A 4.1.1 是"**只衰减 interest_now**，0.98^天，地板 50%"；规划B 是单一 `temporal_decay` 乘子；PRD v2.1 §9.2 也写"只衰减 interest"。但 `compass-core/src/scoring.rs` 实际**对三个维度都做半衰期衰减**（30/365/60 天）——与所有规格都不一致。
- **权重**：原始默认 0.40/0.35/0.25（规划A `DEFAULT_WEIGHTS`、规划B `scores.composite`、Rust 都对）。但 `compass-api/src/api/decay.py` 预览用了 0.4/0.4/0.2（误抄 frontmatter 单条覆盖示例当默认）——**预览分数与实际算分不一致**。

---

## 三、结合技术审查的综合判断

### 3.1 架构形态错了

- 现状："Python 主力（~2000 行）+ Rust 点缀（~250 行，每次算分 spawn 新进程）+ 重 JS 前端"。
- Rust 既没承担主力，又带来漂移负担：`parse_refs` 是死代码（Python `_extract_refs_with_strength` 重写）；`/agent/context` 漏 `await` 是坏的；FileWatcher 没接进 `main.py`；飞书 Bot 没写；Rust 二进制 `bin/compass_core` 不存在导致 `config.py` 启动即 `RuntimeError`。

### 3.2 与"纯 Rust + 少量 JS"目标结构性冲突

- 归档里全是 Python，"纯 Rust"是后发偏好——意味着现有 Python 实现要么留（违背新目标）要么重写（不叫重构叫重写）。

### 3.3 PRD 范围臃肿

- 130h(Phase2)+84h(Phase4) 大量重建 Obsidian 已有能力（双向链接/FTS 搜索/标签/图谱/时间线/Markdown 渲染），稀释了唯一差异化——评分→衰减→浮现闭环。

---

## 四、最终建议：推倒重来，回到原始意志

**结论：推倒重来更合适。** 现有主力代码在错误的语言/架构里，可复用的只有**语言无关的规格与数据模型**——而归档里的原始设计反而比 v2.2 更接近正确答案。

### 4.1 重做要守住的"原始三条底线"

1. **评分是灵魂，且必须在 Obsidian 内可见**——分数写回 frontmatter（恢复漂移①），用 Dataview/轻插件在 Obsidian 里展示引力场。
2. **Obsidian 当主 UI，Agent 当主交互，Web 只做辅助**——放弃 Phase 4 完整 SPA。Web 端只做"引力场视图/Feed 排行"的极薄页面（Obsidian 图谱不会按自定义评分给节点定大小，这才是增量）。
3. **数据主权**：Markdown + frontmatter 是根数据，SQLite 只是索引/缓存。

### 4.2 重做的技术形态（满足"纯 Rust + 少量 JS"）

- **单一 Rust 二进制**：axum/actix HTTP + rusqlite(WAL/FTS5) + 评分/衰减引擎 + Feed，全在一个 crate。丢掉 Python 胶水层与 subprocess-Rust。
- **衰减模型回到原始规格**：只衰减 interest，0.98^天，50% 地板；权重 0.40/0.35/0.25；分数写回 frontmatter（Rust 直接读写 Markdown+YAML）。
- **引用/标签/图谱/搜索/编辑/渲染全部交给 Obsidian**——不自建。
- **Web**：HTMX 或 ~100 行 vanilla JS，只做引力场 + Feed。
- **飞书/Agent**：作为 Rust server 的 webhook 路由，按需加，不单开 Python 服务。

### 4.3 重做要砍掉的（来自 v2.2 的臃肿）

- 独立 Vue3+TS+D3+Pinia+PWA 前端（84h）。
- 自建 FTS 搜索 UI、自建图谱、自建时间线 UI、自建 Markdown 详情页（全交 Obsidian）。
- 飞书 Bot 的 slash 命令→自然语言复杂意图检测层（先回归简单）。

### 4.4 可 salvage 的资产

- **归档原始设计文档**——数据模型、三大界、评分算法，比 v2.2 更纯粹，是最大资产。
- **`schema.sql` 的表结构思路**（语言无关，Rust 版 rusqlite 可复用，但需补"分数回写 frontmatter"）。
- **OpenClaw Skill 的 action/render 约定**。
- **Rust `scoring.rs` 的公式参考**（~15 行算术，但衰减模型要按原始规格改）。

---

## 五、一句话总结

> 你最初要的是一个**以三维评分为引力场、分数在 Obsidian 内可见、Agent 为主交互、数据主权在 Markdown** 的个人知识宇宙；现在的实现把分数搬进了 SQLite（Obsidian 看不到）、砍了 Obsidian 插件、又另起一套和 Obsidian 重复的重 JS Web UI，主力还在你不想要的 Python 里、Rust 反而成了带 bug 的点缀。**建议推倒重来**：用单一 Rust 二进制 + 分数回写 frontmatter + Obsidian 当 UI + 极薄 Web 只做引力场/Feed，回到归档里的原始意志，砍掉一切 Obsidian 已有能力。

---

## 附：关键论断的核验证据

| 论断 | 证据来源 |
|------|----------|
| 评分原始写在 frontmatter | `archive/规划A版本/PKM_Universe_PRD_v1.0.md` §3.3；`archive/规划B版本/PCOS_v1.0_PRD.md` frontmatter `scores:` 块 |
| Obsidian 内展示评分（原始） | `archive/规划A版本/...架构设计原案.md` Phase 2 "DataviewJS 展示分数"；`archive/规划B版本/PCOS_v1.0_PRD.md` Week6 "Obsidian 插件开发 + Dataview 查询模板" |
| 现状分数只在 SQLite | `compass-api/src/db/schema.sql` `scores` 表；frontmatter 无 score 字段 |
| 现状放弃 Obsidian 插件 | `docs/PRD_v2.1.md` §13 "不开发 Obsidian 插件" |
| 衰减模型漂移 | 原始规划A §4.1.1（只衰减 interest，0.98^天，50% 地板）vs `compass-core/src/scoring.rs`（三维度半衰期） |
| 权重漂移 | 默认 0.40/0.35/0.25（规划A `DEFAULT_WEIGHTS`、规划B `scores.composite`、Rust）vs `compass-api/src/api/decay.py` 预览 0.4/0.4/0.2 |
| Rust 是点缀且有 bug | `compass-core/src/*` ~250 行；`rust_client.py` subprocess-per-call；`agent.py:37` 漏 await；`parse_refs` 死代码 |
| 关键件未接上 | `main.py` 不含 filewatcher/feishu；`bin/compass_core` 不存在；`config.py` 启动校验会 RuntimeError |
