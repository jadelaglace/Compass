<script setup lang="ts">
import type { Entity } from '@/types'

const props = defineProps<{ entity: Entity }>()
</script>

<template>
  <div class="entity-card">
    <div class="card-header">
      <span class="type-badge" :data-type="props.entity.entity_type">
        {{ props.entity.entity_type }}
      </span>
      <span class="score" title="质量评分">⭐ {{ (props.entity.score * 100).toFixed(0) }}</span>
    </div>
    <h3 class="card-title">{{ props.entity.title }}</h3>
    <div class="tags">
      <span v-for="tag in props.entity.tags" :key="tag" class="tag">{{ tag }}</span>
    </div>
    <div class="card-footer">
      <span class="date">{{ new Date(props.entity.updated_at).toLocaleDateString('zh-CN') }}</span>
      <button class="btn-view">查看 →</button>
    </div>
  </div>
</template>

<style scoped>
.entity-card {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  transition: box-shadow var(--transition-fast), border-color var(--transition-fast);
}

.entity-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--border-color-strong);
}

.card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.type-badge {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  padding: 2px 8px;
  border-radius: var(--radius-full);
  text-transform: capitalize;
}

.type-badge[data-type="concept"] { background: #eef0ff; color: #4f46e5; }
.type-badge[data-type="project"] { background: #ecfdf5; color: #059669; }
.type-badge[data-type="person"] { background: #fffbeb; color: #d97706; }
.type-badge[data-type="article"] { background: #eff6ff; color: #2563eb; }

.score {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.card-title {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  line-height: var(--leading-tight);
  margin: 0;
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
}

.tag {
  font-size: var(--text-xs);
  padding: 2px 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
}

.card-footer {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: auto;
}

.date {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.btn-view {
  font-size: var(--text-sm);
  color: var(--color-brand);
  background: none;
  border: none;
  cursor: pointer;
  padding: 0;
  font-weight: var(--weight-medium);
}

.btn-view:hover {
  text-decoration: underline;
}
</style>
