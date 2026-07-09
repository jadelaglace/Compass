# T2.3 /graph 引力场数据端点 - Review

## 验收
- GET /graph 返回 { nodes, edges } JSON ✅
- 节点含 id/title/layer/composite ✅
- 边含 source/target（从 [[id]] 提取，只连已存在实体）✅

## 测试（3 个新）
- test_graph_returns_nodes_and_edges: 2节点1边
- test_graph_no_edges_for_orphan_refs: 指向不存在实体不产生边
- test_graph_empty_vault: 空vault返回空

## 112 测试通过（原 109 + 新 3）