<script setup lang="ts">
import type { Insight, Maturity } from '@/stores/insights'

const props = defineProps<{ insight: Insight & { entity_title?: string } }>()
const emit = defineEmits<{
  edit: [id: string]
  delete: [id: string]
}>()

const maturityConfig: Record<Maturity, { label: string; color: string; bg: string }> = {
  seed: { label: '种子', color: '#6b7280', bg: '#f3f4f6' },
  sprout: { label: '萌芽', color: '#10b981', bg: '#ecfdf5' },
  bud: { label: '花苞', color: '#f59e0b', bg: '#fffbeb' },
  bloom: { label: '绽放', color: '#3b82f6', bg: '#eff6ff' },
  ripe: { label: '成熟', color: '#059669', bg: '#ecfdf5' },
}
</script>

<template>
  <div class="insight-card">
    <div class="card-top">
      <span
        class="maturity-badge"
        :style="{ background: maturityConfig[props.insight.maturity].bg, color: maturityConfig[props.insight.maturity].color }"
      >
        {{ maturityConfig[props.insight.maturity].label }}
      </span>
      <div class="actions">
        <button class="action-btn" @click="emit('edit', props.insight.id)" title="编辑">✏️</button>
        <button class="action-btn delete" @click="emit('delete', props.insight.id)" title="删除">🗑️</button>
      </div>
    </div>
    <p class="content">{{ props.insight.content }}</p>
    <div class="card-bottom">
      <span class="entity">📌 {{ props.insight.entity_title }}</span>
      <span class="date">{{ new Date(props.insight.created_at).toLocaleDateString('zh-CN') }}</span>
    </div>
  </div>
</template>

<style scoped>
.insight-card {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  transition: box-shadow var(--transition-fast);
}

.insight-card:hover { box-shadow: var(--shadow-sm); }

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.maturity-badge {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  padding: 2px 8px;
  border-radius: var(--radius-full);
}

.actions { display: flex; gap: var(--space-1); }

.action-btn {
  background: none;
  border: none;
  cursor: pointer;
  padding: 2px 4px;
  font-size: 14px;
  opacity: 0.6;
  transition: opacity var(--transition-fast);
}

.action-btn:hover { opacity: 1; }
.action-btn.delete:hover { opacity: 1; }

.content {
  font-size: var(--text-sm);
  color: var(--text-primary);
  line-height: var(--leading-relaxed);
  margin: 0;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.card-bottom {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.entity {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.date {
  font-size: var(--text-xs);
  color: var(--text-muted);
}
</style>
