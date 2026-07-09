# T2.5 Phase 2 验收 - Review

## 验收（PRD §10）
1. **GET /feed 浮现正确**：三模式排序验证 ✅ (test_p2_feed_three_modes_e2e)
2. **Web 引力场节点大小=评分**：/graph 返回 composite 字段 ✅ (test_p2_graph_node_size_equals_score)
3. **30天衰减曲线合理**：单调递减 + 100天接近地板 ✅ (test_p2_decay_curve_30_days)

## 测试（3 个新）
- test_p2_decay_curve_30_days: 5实体(10/20/30/60/100天)衰减曲线单调递减，100天接近地板45
- test_p2_feed_three_modes_e2e: explore/consolidate/strategic 三模式排序正确
- test_p2_graph_node_size_equals_score: /graph 节点含 composite，高分>低分

## 115 测试通过（原 112 + 新 3）
## Phase 2 全部完成