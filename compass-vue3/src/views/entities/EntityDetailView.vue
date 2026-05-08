<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { marked } from 'marked'
import type { Entity } from '@/stores/entity'
import { useEntityStore } from '@/stores/entity'

marked.setOptions({})

const route = useRoute()
const router = useRouter()
const entityStore = useEntityStore()

const entity = ref<Entity | null>(null)
const loading = ref(true)

onMounted(() => {
  const id = route.params.id as string
  const found = entityStore.entities.find(e => e.id === id)
  setTimeout(() => {
    entity.value = found ?? null
    loading.value = false
  }, 300)
})

const renderedContent = computed(() => {
  if (!entity.value) return ''
  let content = entity.value.content
  // Highlight [[id]] WikiLinks
  content = content.replace(/\[\[([^\]]+)\]\]/g, (_, id) =>
    `<a href="/entities/${id}" class="wikilink">[[${id}]]</a>`
  )
  return marked(content)
})

function confirmDelete() {
  if (confirm(`确定删除「${entity.value?.title}」？`)) {
    entityStore.entities = entityStore.entities.filter(e => e.id !== entity.value?.id)
    router.push({ name: 'entities' })
  }
}
</script>

<template>
  <div class="view">
    <div v-if="loading" class="loading">加载中...</div>
    <div v-else-if="!entity" class="not-found">
      <p>实体不存在</p>
      <router-link to="/entities">← 返回列表</router-link>
    </div>
    <div v-else class="detail">
      <div class="detail-header">
        <div class="header-top">
          <span class="type-badge">{{ entity.entity_type }}</span>
          <div class="header-actions">
            <button class="btn-edit" @click="router.push({ name: 'entities' })">✏️ 编辑</button>
            <button class="btn-delete" @click="confirmDelete">🗑️ 删除</button>
          </div>
        </div>
        <h1 class="entity-title">{{ entity.title }}</h1>
        <div class="entity-meta">
          <span>⭐ {{ (entity.score * 100).toFixed(0) }}</span>
          <span>成熟度: {{ entity.maturity }}/5</span>
          <span>访问: {{ entity.access_count }}</span>
          <span>更新: {{ new Date(entity.updated_at).toLocaleDateString('zh-CN') }}</span>
        </div>
      </div>

      <div class="tags">
        <span v-for="tag in entity.tags" :key="tag" class="tag">{{ tag }}</span>
      </div>

      <div class="content-body" v-html="renderedContent"></div>

      <div class="score-panel">
        <h3>📊 评分详情</h3>
        <div class="score-bars">
          <div class="score-item">
            <span class="score-label">综合评分</span>
            <div class="bar"><div class="fill" :style="{ width: `${entity.score * 100}%` }"></div></div>
            <span class="score-val">{{ (entity.score * 100).toFixed(0) }}</span>
          </div>
          <div class="score-item">
            <span class="score-label">成熟度</span>
            <div class="bar"><div class="fill maturity" :style="{ width: `${entity.maturity * 20}%` }"></div></div>
            <span class="score-val">{{ entity.maturity }}/5</span>
          </div>
          <div class="score-item">
            <span class="score-label">活跃度</span>
            <div class="bar"><div class="fill activity" :style="{ width: `${Math.min(entity.access_count, 100)}%` }"></div></div>
            <span class="score-val">{{ entity.access_count }}</span>
          </div>
        </div>
      </div>

      <div class="references">
        <h3>🔗 关联实体</h3>
        <p class="empty-hint">关联实体将在 Phase 5 实现</p>
      </div>

      <div class="timeline-section">
        <h3>📜 活动时间线</h3>
        <div class="timeline-list">
          <div class="timeline-item">
            <span class="t-icon">✏️</span>
            <div class="t-content">
              <span class="t-time">{{ new Date(entity.updated_at).toLocaleString('zh-CN') }}</span>
              <span class="t-desc">内容更新</span>
            </div>
          </div>
          <div class="timeline-item">
            <span class="t-icon">👁️</span>
            <div class="t-content">
              <span class="t-time">{{ new Date(entity.created_at).toLocaleString('zh-CN') }}</span>
              <span class="t-desc">创建实体</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.view { max-width: 800px; }

.loading, .not-found {
  text-align: center;
  padding: var(--space-12);
  color: var(--text-muted);
}

.detail-header {
  margin-bottom: var(--space-5);
  padding-bottom: var(--space-5);
  border-bottom: 1px solid var(--border-color);
}

.header-top {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-3);
}

.type-badge {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  padding: 2px 10px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
  text-transform: capitalize;
}

.header-actions { display: flex; gap: var(--space-2); }

.btn-edit, .btn-delete {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  background: var(--bg-primary);
  font-size: var(--text-xs);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.btn-edit:hover { border-color: var(--color-brand); color: var(--color-brand); }
.btn-delete:hover { border-color: var(--color-danger); color: var(--color-danger); }

.entity-title {
  font-size: var(--text-2xl);
  font-weight: var(--weight-bold);
  color: var(--text-primary);
  margin: 0 0 var(--space-3);
  line-height: var(--leading-tight);
}

.entity-meta {
  display: flex;
  gap: var(--space-4);
  flex-wrap: wrap;
  font-size: var(--text-sm);
  color: var(--text-muted);
}

.tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
  margin-bottom: var(--space-6);
}

.tag {
  font-size: var(--text-xs);
  padding: 2px 10px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
}

.content-body {
  font-size: var(--text-base);
  line-height: var(--leading-relaxed);
  color: var(--text-primary);
  margin-bottom: var(--space-6);
}

:deep(.content-body h1),
:deep(.content-body h2),
:deep(.content-body h3) {
  margin-top: var(--space-6);
  margin-bottom: var(--space-3);
  color: var(--text-primary);
}

:deep(.content-body p) { margin-bottom: var(--space-4); }

:deep(.content-body code) {
  background: var(--bg-tertiary);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  font-family: var(--font-mono);
  font-size: 0.9em;
}

:deep(.content-body pre) {
  background: var(--bg-tertiary);
  padding: var(--space-4);
  border-radius: var(--radius-md);
  overflow-x: auto;
  margin-bottom: var(--space-4);
}

:deep(.content-body pre code) {
  background: none;
  padding: 0;
}

:deep(.wikilink) {
  color: var(--color-brand);
  text-decoration: none;
  border-bottom: 1px dashed currentColor;
}

.score-panel {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-5);
  margin-bottom: var(--space-6);
}

.score-panel h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  margin: 0 0 var(--space-4);
}

.score-bars { display: flex; flex-direction: column; gap: var(--space-3); }

.score-item {
  display: grid;
  grid-template-columns: 80px 1fr 40px;
  align-items: center;
  gap: var(--space-3);
}

.score-label { font-size: var(--text-sm); color: var(--text-secondary); }

.bar {
  height: 8px;
  background: var(--bg-tertiary);
  border-radius: var(--radius-full);
  overflow: hidden;
}

.fill {
  height: 100%;
  background: var(--color-brand);
  border-radius: var(--radius-full);
  transition: width 0.6s ease;
}

.fill.maturity { background: #10b981; }
.fill.activity { background: #f59e0b; }

.score-val {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  text-align: right;
}

.references {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-5);
  margin-bottom: var(--space-6);
}

.references h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  margin: 0 0 var(--space-3);
}

.empty-hint {
  font-size: var(--text-sm);
  color: var(--text-muted);
  margin: 0;
}

.timeline-section h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  margin-bottom: var(--space-4);
}

.timeline-list { display: flex; flex-direction: column; gap: var(--space-3); }

.timeline-item {
  display: flex;
  gap: var(--space-3);
  align-items: flex-start;
}

.t-icon {
  width: 32px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-tertiary);
  border-radius: 50%;
  font-size: 14px;
  flex-shrink: 0;
}

.t-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-bottom: var(--space-3);
  border-bottom: 1px solid var(--border-color);
  flex: 1;
}

.timeline-item:last-child .t-content { border-bottom: none; }

.t-time { font-size: var(--text-xs); color: var(--text-muted); }
.t-desc { font-size: var(--text-sm); color: var(--text-primary); }
</style>
