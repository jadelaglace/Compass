<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import apiClient from '@/api/client'
import type { SearchResult } from '@/types'

const route = useRoute()
const router = useRouter()
const query = ref((route.query.q as string) ?? '')
const loading = ref(false)

const results = ref<SearchResult[]>([])
const hasSearched = ref(false)

watch(() => route.query.q, (q) => {
  if (q) {
    query.value = q as string
    performSearch(q as string)
  }
}, { immediate: true })

async function performSearch(q: string) {
  if (!q.trim()) return
  loading.value = true
  hasSearched.value = true
  try {
    const res = await apiClient.post('/search', { query: q, limit: 20 })
    results.value = res.data.results || []
  } catch (err) {
    console.error('[search] failed:', err)
    results.value = []
  } finally {
    loading.value = false
  }
}

function handleSearch() {
  if (query.value.trim()) {
    router.push({ name: 'search', query: { q: query.value.trim() } })
  }
}
</script>

<template>
  <div class="view">
    <h1>🔍 搜索</h1>

    <div class="search-bar">
      <input
        v-model="query"
        type="text"
        placeholder="输入关键词..."
        class="search-input"
        @keydown.enter="handleSearch"
      />
      <button class="btn-primary" @click="handleSearch">搜索</button>
    </div>

    <div v-if="loading" class="loading">
      <div class="spinner"></div>
      搜索中...
    </div>

    <div v-else-if="!hasSearched" class="hint">
      输入关键词开始搜索
    </div>

    <div v-else-if="results.length === 0" class="empty">
      未找到相关结果
    </div>

    <div v-else class="results">
      <div class="results-count">{{ results.length }} 个结果</div>
      <div
        v-for="result in results"
        :key="result.entity.id"
        class="result-item"
      >
        <div class="result-header">
          <span class="result-type-badge">{{ result.entity.entity_type }}</span>
          <span class="result-score">⭐ {{ (result.score * 100).toFixed(0) }}</span>
        </div>
        <h3 class="result-title">{{ result.entity.title }}</h3>
        <p class="result-snippet" v-html="result.highlights[0]"></p>
        <div class="tags">
          <span v-for="tag in result.entity.tags" :key="tag" class="tag">{{ tag }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 720px;
}

h1 {
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin-bottom: var(--space-4);
}

.search-bar {
  display: flex;
  gap: var(--space-3);
  margin-bottom: var(--space-6);
}

.search-input {
  flex: 1;
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  background: var(--bg-primary);
  color: var(--text-primary);
  outline: none;
  transition: border-color var(--transition-fast);
}

.search-input:focus { border-color: var(--color-brand); }

.btn-primary {
  padding: var(--space-2) var(--space-5);
  background: var(--color-brand);
  color: var(--text-inverse);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-base);
  font-weight: var(--weight-medium);
  cursor: pointer;
  transition: background var(--transition-fast);
}

.btn-primary:hover { background: var(--color-brand-dark); }

.loading {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-8);
  color: var(--text-secondary);
  justify-content: center;
}

.spinner {
  width: 20px;
  height: 20px;
  border: 2px solid var(--border-color);
  border-top-color: var(--color-brand);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin { to { transform: rotate(360deg); } }

.hint, .empty {
  text-align: center;
  padding: var(--space-12);
  color: var(--text-muted);
}

.results-count {
  font-size: var(--text-sm);
  color: var(--text-muted);
  margin-bottom: var(--space-4);
}

.result-item {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  margin-bottom: var(--space-3);
  transition: box-shadow var(--transition-fast);
}

.result-item:hover { box-shadow: var(--shadow-sm); }

.result-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-2);
}

.result-type-badge {
  font-size: var(--text-xs);
  padding: 2px 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
  text-transform: capitalize;
}

.result-score { font-size: var(--text-sm); color: var(--text-secondary); }

.result-title {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin: 0 0 var(--space-2);
}

.result-snippet {
  font-size: var(--text-sm);
  color: var(--text-secondary);
  margin: 0 0 var(--space-2);
  line-height: var(--leading-relaxed);
}

:deep(mark) {
  background: #fef9c3;
  color: #854d0e;
  border-radius: 2px;
  padding: 0 2px;
}

.tags { display: flex; gap: var(--space-1); flex-wrap: wrap; }

.tag {
  font-size: var(--text-xs);
  padding: 2px 8px;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  border-radius: var(--radius-full);
}
</style>
