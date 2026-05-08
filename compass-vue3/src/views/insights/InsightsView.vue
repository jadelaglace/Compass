<script setup lang="ts">
import RadarChart from '@/components/insights/RadarChart.vue'
import ScoreHistoryChart from '@/components/insights/ScoreHistoryChart.vue'

const stats = [
  { label: '总实体数', value: '48', icon: '📚' },
  { label: '平均评分', value: '0.74', icon: '⭐' },
  { label: '本周访问', value: '127', icon: '👁️' },
  { label: '成熟洞察', value: '12', icon: '💡' },
]
</script>

<template>
  <div class="view">
    <h1>💡 洞察</h1>

    <div class="stats-grid">
      <div v-for="s in stats" :key="s.label" class="stat-card">
        <span class="stat-icon">{{ s.icon }}</span>
        <div class="stat-body">
          <span class="stat-value">{{ s.value }}</span>
          <span class="stat-label">{{ s.label }}</span>
        </div>
      </div>
    </div>

    <div class="charts-grid">
      <RadarChart />
      <ScoreHistoryChart />
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 960px;
}

h1 {
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin-bottom: var(--space-5);
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--space-4);
  margin-bottom: var(--space-6);
}

.stat-card {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.stat-icon {
  font-size: 28px;
}

.stat-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.stat-value {
  font-size: var(--text-xl);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  line-height: 1;
}

.stat-label {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.charts-grid {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
  gap: var(--space-4);
}
</style>
