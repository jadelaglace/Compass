<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import ThemeToggle from '@/components/ThemeToggle.vue'

const router = useRouter()
const searchQuery = ref('')

const handleSearch = () => {
  if (searchQuery.value.trim()) {
    router.push({ name: 'search', query: { q: searchQuery.value.trim() } })
    searchQuery.value = ''
  }
}
</script>

<template>
  <header class="top-bar">
    <div class="search-box">
      <input
        v-model="searchQuery"
        type="text"
        placeholder="搜索知识库..."
        class="search-input"
        @keydown.enter="handleSearch"
      />
    </div>
    <div class="top-bar-right">
      <ThemeToggle />
      <span class="user-info">👤 dbb</span>
    </div>
  </header>
</template>

<style scoped>
.top-bar {
  height: 56px;
  background: var(--bg-primary);
  border-bottom: 1px solid var(--border-color);
  display: flex;
  align-items: center;
  padding: 0 var(--space-6);
  gap: var(--space-4);
  flex-shrink: 0;
}

.search-box {
  flex: 1;
  max-width: 480px;
}

.search-input {
  width: 100%;
  padding: var(--space-2) var(--space-4);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-full);
  font-size: var(--text-base);
  background: var(--bg-secondary);
  color: var(--text-primary);
  outline: none;
  transition: border-color var(--transition-fast), background var(--transition-fast);
}

.search-input::placeholder {
  color: var(--text-muted);
}

.search-input:focus {
  border-color: var(--color-brand);
  background: var(--bg-primary);
}

.top-bar-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.user-info {
  font-size: var(--text-sm);
  color: var(--text-secondary);
}
</style>
