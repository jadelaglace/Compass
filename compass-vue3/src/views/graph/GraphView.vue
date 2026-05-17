<script setup lang="ts">
import { onMounted } from 'vue'
import ForceGraph from '@/components/graph/ForceGraph.vue'
import { useGraphStore } from '@/stores/graph'

const graphStore = useGraphStore()
onMounted(() => graphStore.fetchGraph())
</script>

<template>
  <div class="view">
    <div class="view-header">
      <h1>🔗 知识图谱</h1>
      <div class="legend">
        <span class="legend-item"><span class="dot" style="background:#4f46e5"></span>概念</span>
        <span class="legend-item"><span class="dot" style="background:#10b981"></span>项目</span>
        <span class="legend-item"><span class="dot" style="background:#f59e0b"></span>人物</span>
        <span class="legend-item"><span class="dot" style="background:#3b82f6"></span>文章</span>
      </div>
    </div>
    <ForceGraph />
    <p v-if="graphStore.selectedNodeId" class="selection-info">
      选中节点: <strong>{{ graphStore.nodes.find(n => n.id === graphStore.selectedNodeId)?.title }}</strong>
    </p>
  </div>
</template>

<style scoped>
.view {
  max-width: 1100px;
}

.view-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
  flex-wrap: wrap;
  gap: var(--space-3);
}

h1 {
  color: var(--text-primary);
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
}

.legend {
  display: flex;
  gap: var(--space-4);
  flex-wrap: wrap;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.dot {
  width: 10px;
  height: 10px;
  border-radius: 50%;
  flex-shrink: 0;
}

.selection-info {
  margin-top: var(--space-3);
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
</style>
