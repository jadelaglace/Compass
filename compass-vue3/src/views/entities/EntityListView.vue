<script setup lang="ts">
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useEntityStore, type EntityType, type SortKey } from '@/stores/entity'

const store = useEntityStore()
const router = useRouter()

onMounted(() => store.fetchEntities())

const typeOptions: { label: string; value: EntityType }[] = [
  { label: '全部', value: 'all' },
  { label: '概念', value: 'concept' },
  { label: '项目', value: 'project' },
  { label: '人物', value: 'person' },
  { label: '文章', value: 'article' },
]

const sortOptions: { label: string; value: SortKey }[] = [
  { label: '最近更新', value: 'updated' },
  { label: '创建时间', value: 'created' },
  { label: '评分', value: 'score' },
  { label: '访问量', value: 'access_count' },
]
</script>

<template>
  <div class="view">
    <h1>📚 知识库</h1>

    <div class="toolbar">
      <div class="filters">
        <button
          v-for="opt in typeOptions"
          :key="opt.value"
          :class="['filter-btn', { active: store.typeFilter === opt.value }]"
          @click="store.setFilter(opt.value)"
        >
          {{ opt.label }}
        </button>
      </div>
      <select
        class="sort-select"
        :value="store.sortKey"
        @change="store.setSort(($event.target as HTMLSelectElement).value as SortKey)"
      >
        <option v-for="opt in sortOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
      </select>
    </div>

    <div v-if="store.loading" class="loading">加载中...</div>
    <div v-else-if="store.entities.length === 0" class="empty">暂无数据</div>
    <template v-else>
      <div class="entity-grid">
        <div
          v-for="entity in store.sorted"
          :key="entity.id"
          class="entity-card"
          @click="router.push(`/entities/${entity.id}`)"
        >
          <div class="entity-header">
            <span class="entity-type">{{ entity.entity_type }}</span>
            <span class="entity-score">{{ (entity.score * 100).toFixed(0) }}分</span>
          </div>
          <h3 class="entity-title">{{ entity.title }}</h3>
          <p class="entity-content">{{ entity.content?.slice(0, 80) }}...</p>
          <div class="entity-tags">
            <span v-for="tag in (entity.tags || []).slice(0, 3)" :key="tag" class="tag">{{ tag }}</span>
          </div>
          <div class="entity-footer">
            <span>{{ entity.access_count }} 次访问</span>
            <span>{{ new Date(entity.updated_at).toLocaleDateString('zh-CN') }}</span>
          </div>
        </div>
      </div>

      <div class="pagination">
        <button :disabled="store.page <= 1" @click="store.setPage(store.page - 1)">上一页</button>
        <span>{{ store.page }} / {{ store.totalPages || 1 }}</span>
        <button :disabled="store.page >= store.totalPages" @click="store.setPage(store.page + 1)">下一页</button>
      </div>
    </template>
  </div>
</template>

<style scoped>
.view {
  max-width: 960px;
}

h1 {
  margin-bottom: var(--space-5);
  color: var(--text-primary);
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
}

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
  gap: var(--space-3);
  flex-wrap: wrap;
}

.filters {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.filter-btn {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-full);
  background: var(--bg-primary);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.filter-btn:hover {
  border-color: var(--color-brand);
  color: var(--color-brand);
}

.filter-btn.active {
  background: var(--color-brand);
  border-color: var(--color-brand);
  color: var(--text-inverse);
}

.sort-select {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: var(--text-sm);
  cursor: pointer;
}

.loading, .empty {
  text-align: center;
  padding: var(--space-16);
  color: var(--text-muted);
}

.entity-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-4);
}

.entity-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.entity-card:hover {
  border-color: var(--color-brand);
  transform: translateY(-2px);
}

.entity-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-2);
}

.entity-type {
  font-size: var(--text-xs);
  color: var(--text-muted);
  text-transform: capitalize;
}

.entity-score {
  font-size: var(--text-xs);
  color: var(--color-brand);
  font-weight: var(--weight-semibold);
}

.entity-title {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin-bottom: var(--space-2);
}

.entity-content {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  margin-bottom: var(--space-3);
  line-height: 1.5;
}

.entity-tags {
  display: flex;
  gap: var(--space-1);
  flex-wrap: wrap;
  margin-bottom: var(--space-3);
}

.tag {
  font-size: var(--text-xs);
  padding: 2px 8px;
  background: var(--color-brand-bg);
  color: var(--color-brand);
  border-radius: var(--radius-full);
}

.entity-footer {
  display: flex;
  justify-content: space-between;
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-4);
  margin-top: var(--space-6);
}

.pagination button {
  padding: var(--space-1) var(--space-4);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  background: var(--bg-primary);
  color: var(--text-primary);
  font-size: var(--text-sm);
  cursor: pointer;
}

.pagination button:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.pagination span {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}
</style>