<script setup lang="ts">
import { useRouter } from 'vue-router'
import type { Entity } from '@/stores/entity'

const props = defineProps<{ entity: Entity }>()
const router = useRouter()

const typeColors: Record<string, { bg: string; color: string }> = {
  concept: { bg: '#eef0ff', color: '#4f46e5' },
  project: { bg: '#ecfdf5', color: '#059669' },
  person: { bg: '#fffbeb', color: '#d97706' },
  article: { bg: '#eff6ff', color: '#2563eb' },
}

function getTypeStyle(type: string): { bg: string; color: string } {
  return typeColors[type] ?? { bg: '#f3f4f6', color: '#6b7280' }
}
</script>

<template>
  <div class="entity-card" @click="router.push({ name: 'entity', params: { id: props.entity.id } })">
    <div class="card-top">
      <span class="type-badge" :style="getTypeStyle(props.entity.entity_type)">
        {{ props.entity.entity_type }}
      </span>
      <span class="score">⭐ {{ (props.entity.score * 100).toFixed(0) }}</span>
    </div>
    <h3 class="card-title">{{ props.entity.title }}</h3>
    <p class="card-excerpt">{{ props.entity.content }}</p>
    <div class="tags">
      <span v-for="tag in props.entity.tags" :key="tag" class="tag">{{ tag }}</span>
    </div>
    <div class="card-meta">
      <span class="meta-item">👁️ {{ props.entity.access_count }}</span>
      <span class="meta-item">{{ new Date(props.entity.updated_at).toLocaleDateString('zh-CN') }}</span>
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
  gap: var(--space-2);
  cursor: pointer;
  transition: box-shadow var(--transition-fast), border-color var(--transition-fast);
}

.entity-card:hover {
  box-shadow: var(--shadow-md);
  border-color: var(--color-brand);
}

.card-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.type-badge {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  padding: 2px 8px;
  border-radius: var(--radius-full);
}

.score { font-size: var(--text-sm); color: var(--text-secondary); }

.card-title {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin: 0;
  line-height: var(--leading-tight);
}

.card-excerpt {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  margin: 0;
  line-height: var(--leading-normal);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-1);
  margin-top: var(--space-1);
}

.tag {
  font-size: var(--text-xs);
  padding: 2px 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
}

.card-meta {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-top: auto;
  padding-top: var(--space-2);
  border-top: 1px solid var(--border-color);
}

.meta-item {
  font-size: var(--text-xs);
  color: var(--text-muted);
}
</style>
