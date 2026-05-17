<script setup lang="ts">
import { onMounted } from 'vue'
import { useFeedStore, type EntityType } from '@/stores/feed'
import EntityCard from '@/components/feed/EntityCard.vue'

const feedStore = useFeedStore()

onMounted(() => feedStore.fetchFeed())

const filters: { label: string; value: EntityType }[] = [
  { label: '全部', value: 'all' },
  { label: '概念', value: 'concept' },
  { label: '项目', value: 'project' },
  { label: '人物', value: 'person' },
  { label: '文章', value: 'article' },
]
</script>

<template>
  <div class="view">
    <div class="view-header">
      <h1>✨ 今日推荐</h1>
      <div class="filter-bar">
        <button
          v-for="f in filters"
          :key="f.value"
          class="filter-chip"
          :class="{ active: feedStore.filter === f.value }"
          @click="feedStore.setFilter(f.value)"
        >
          {{ f.label }}
        </button>
      </div>
    </div>

    <div v-if="feedStore.loading" class="loading">加载中...</div>
    <div v-else-if="feedStore.filtered.length === 0" class="empty">暂无内容</div>
    <div v-else class="card-grid">
      <EntityCard v-for="entity in feedStore.filtered" :key="entity.id" :entity="entity" />
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 960px;
}

.view-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
  flex-wrap: wrap;
  gap: var(--space-3);
}

h1 {
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.filter-bar {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.filter-chip {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-full);
  background: var(--bg-primary);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.filter-chip:hover {
  border-color: var(--color-brand);
  color: var(--color-brand);
}

.filter-chip.active {
  background: var(--color-brand);
  border-color: var(--color-brand);
  color: var(--text-inverse);
}

.card-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-4);
}

.loading, .empty {
  text-align: center;
  padding: var(--space-16);
  color: var(--text-muted);
  font-size: var(--text-md);
}
</style>
