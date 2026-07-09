# T2.1 衰减调度 - 独立 Review

> 审阅对象：`compass-core/src/scheduler.rs`（新）+ `main.rs`（接线）
> 依据：PRD §5.2 / PLAN T2.1
> 模式：批判性自审。

## 验收对照

| 验收项 | 证据 | 结论 |
|--------|------|------|
| tokio 定时每日 02:00 | `start()` + `next_run_time` | ✅ |
| interest 衰减 | `compute_decay` + `decay_interest` | ✅ |
| 只衰 interest | compute_decay 只改 interest | ✅ |
| 跳过 archived | `test_decay_skips_archived` | ✅ |
| boost 保护期 <3天 | `test_decay_skips_boost_protection` | ✅ |
| direction 衰减减半 | `test_decay_direction_layer_halved` | ✅ |
| 地板 50% | `test_decay_floor_enforced` | ✅ |
| 写回 frontmatter | `test_decay_writes_back_frontmatter` | ✅ |
| 记录 score_history | `test_decay_records_history` | ✅ |
| run_once 多实体 | `test_run_once_multiple_entities` | ✅ |
| 手动触发可测试 | `run_once` pub | ✅ |

## 测试（9 个新）
1. test_decay_reduces_interest - 30天衰减 80->43.6
2. test_decay_skips_archived - archived 跳过
3. test_decay_skips_boost_protection - 1天<3天保护期
4. test_decay_direction_layer_halved - direction 0.98^15
5. test_decay_floor_enforced - 200天到地板50
6. test_decay_writes_back_frontmatter - 写回验证
7. test_decay_records_history - Decay 历史记录
8. test_run_once_multiple_entities - 3实体统计
9. test_next_run_time - 02:00 调度计算

## 设计
- `compute_decay`（纯计算）+ `process_one`（IO+db）分离，避免 Db 非 Sync 问题
- `run_once` 内单次 lock 遍历，避免死锁
- `start` spawn 异步循环，计算到下次 02:00 间隔

## 结论
可合并。