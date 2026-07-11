# Compass Phase 1-4 测试报告

> 日期：2026-07-11
> 范围：Phase 1-4 最终收尾
> 环境：Windows 10 / Rust release / Python unittest
> 原则：所有写入型黑盒测试均使用临时 Vault，不修改真实 Vault

---

## 0. 结论

Phase 1-4 验收通过，可以进入 Phase 5。

| 验收层 | 结果 |
|---|---|
| Rust 单元与模块 E2E | 149 passed / 0 failed / 0 ignored |
| Skill HTTP E2E | 17 passed / 0 failed |
| Skill renderer | 21 passed / 0 failed |
| Phase 4 HTTP 边界 | 通过 |
| 混合 TCP 负载 | 通过，服务最终健康检查为 200 |
| 真实 Vault 副作用 | 已清理 |

## 1. 最终修复

### 1.1 运行时稳定性

初版报告把一次临时批量探测中的服务失活直接定性为 axum 死锁，但未保存可复跑脚本、退出码、线程栈或完整连接生命周期，因此不足以证明服务端死锁。

收尾处理：

- 将混合 TCP 负载固化到 `skills/compass/test_e2e.py`。
- 每个成功响应使用上下文管理器关闭。
- 每个 `HTTPError` 在读取响应体后显式 `close()`。
- 使用临时 Vault 和独立端口，不与真实服务或真实数据混用。
- 覆盖 health、中文 search、feed、agent context、422 错误响应和 access 写回，累计超过原报告的 20-40 请求阈值。
- 压测结束后断言服务进程仍存活，并再次检查 `GET /health == 200`。

结果：回归通过；测试期间观察到正常 `TIME_WAIT`，未观察到原报告所称的 `CLOSE_WAIT` 堆积。现有证据不支持“已确认服务端死锁”的结论。

### 1.2 Feed 非法模式

`GET /feed?mode=invalid` 现在返回 422，不再静默回退到 explore。合法值仅为：

- `explore`
- `consolidate`
- `strategic`

### 1.3 中文搜索

英文查询继续使用 FTS5 MATCH 和 rank。包含 CJK 字符的查询使用参数化索引内容子串兜底：

- 支持两字中文查询，例如“评分”。
- 多个空白分隔词保持 AND 语义。
- snippet 限制长度，避免返回整篇正文。
- 不修改 FTS schema，因此无需迁移或重建数据库。

说明：FTS5 默认 tokenizer 已是 `unicode61`，仅显式改成 `unicode61` 不能解决中文分词问题。

### 1.4 Phase 4 请求契约

补齐并验证：

- Agent context 空 task / 缺 task：422。
- Tag agent candidate 缺必填字段：422。
- Tag candidates 超过 20 项：422。
- 不存在的实体或 suggestion：404。
- accept 重复调用：幂等。
- reject 已 accepted suggestion：409。
- Related 返回 reasons，并排除自身。
- Weekly 缺时区、非法时区、非法日期、反向区间：422。
- Weekly 同参数调用结果确定。

同时修复了一个实际缺陷：原 `Option<Json<TagSuggestionsRequest>>` 会把非法 agent candidate 的反序列化失败吞掉并回退到 lexical 候选；现在请求体必须通过 JSON schema。

## 2. Phase 验收地图

| Phase | 验收内容 | 状态 |
|---|---|---|
| 1 | frontmatter、评分、SQLite、FileWatcher、基础 API | 通过 |
| 2 | 衰减、Feed 三模式、Graph | 通过 |
| 3 | skill action、HTTP API、render | 通过 |
| 4 | tags、related、accept/reject、weekly、content hash | 通过 |

当前 router 注册 18 条实际 HTTP 路径。此前“14 个端点”的统计把多组 accept/reject 路径合并计算，已不作为验收口径。

## 3. 测试命令

```powershell
cargo test --release --manifest-path compass-core/Cargo.toml
python -m unittest -q skills/compass/test_compass.py
python -m unittest -q skills/compass/test_e2e.py
cargo fmt --manifest-path compass-core/Cargo.toml -- --check
cargo clippy --manifest-path compass-core/Cargo.toml --all-targets -- -D warnings
git diff --check
```

## 4. 数据清理

初版黑盒探测写入真实 Vault 的以下测试文件已确认并删除：

- `vault/Knowledge/know000004.md`
- `vault/Knowledge/know000005.md`
- `vault/Knowledge/know000007.md`
- `vault/Insights/ins000001.md`

`know000006.md` 在清理时不存在。后续 E2E 使用临时目录，并在 teardown 中自动删除。

## 5. 剩余风险

- CJK 子串兜底为线性扫描索引内容，适合个人 Vault；若数据量显著增长，再评估 trigram 或中文 tokenizer。
- 当前稳定性回归覆盖短时混合负载，不替代数小时 soak test。
- SQLite 仍由单一 mutex 串行访问；当前回归未发现死锁，但高并发吞吐不是本阶段目标。

这些风险不阻塞 Phase 4 收尾。
