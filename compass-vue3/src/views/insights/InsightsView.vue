<script setup lang="ts">
import { ref } from 'vue'
import { useInsightsStore, type Insight, type Maturity } from '@/stores/insights'
import InsightCard from '@/components/insights/InsightCard.vue'
import InsightForm from '@/components/insights/InsightForm.vue'
import RadarChart from '@/components/insights/RadarChart.vue'
import ScoreHistoryChart from '@/components/insights/ScoreHistoryChart.vue'

const store = useInsightsStore()
const showModal = ref(false)
const editingInsight = ref<Insight | null>(null)

function openNew() {
  editingInsight.value = null
  showModal.value = true
}

function openEdit(id: string) {
  editingInsight.value = store.insights.find(i => i.id === id) ?? null
  showModal.value = true
}

function handleDelete(id: string) {
  if (confirm('确定删除这条洞察？')) {
    store.deleteInsight(id)
  }
}

function handleSave(data: { content: string; maturity: Maturity; entity_title: string; entity_id: string }) {
  if (editingInsight.value) {
    store.updateInsight(editingInsight.value.id, data)
  } else {
    store.addInsight(data)
  }
}

const stats = [
  { label: '总实体数', value: '48', icon: '📚' },
  { label: '平均评分', value: '0.74', icon: '⭐' },
  { label: '本周访问', value: '127', icon: '👁️' },
  { label: '成熟洞察', value: '12', icon: '💡' },
]
</script>

<template>
  <div class="view">
    <div class="view-header">
      <h1>💡 洞察</h1>
      <button class="btn-new" @click="openNew">+ 新建洞察</button>
    </div>

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

    <div v-if="store.insights.length === 0" class="empty">
      <p>暂无洞察</p>
    </div>
    <div v-else class="insights-grid">
      <InsightCard
        v-for="insight in store.insights"
        :key="insight.id"
        :insight="insight"
        @edit="openEdit"
        @delete="handleDelete"
      />
    </div>

    <InsightForm
      :show="showModal"
      :insight="editingInsight"
      @close="showModal = false"
      @save="handleSave"
    />
  </div>
</template>

<style scoped>
.view {
  max-width: 960px;
}

.view-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-5);
}

h1 {
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.btn-new {
  padding: var(--space-2) var(--space-4);
  background: var(--color-brand);
  color: var(--text-inverse);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.btn-new:hover { background: var(--color-brand-dark); }

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

.stat-icon { font-size: 28px; }

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
  margin-bottom: var(--space-6);
}

.insights-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--space-4);
}

.empty {
  text-align: center;
  padding: var(--space-8);
  color: var(--text-muted);
}
</style>
