<script setup lang="ts">
import { ref } from 'vue'

const query = ref('')
</script>

<template>
  <div class="view">
    <h1>🔍 搜索</h1>
    <div class="search-bar">
      <input v-model="query" type="text" placeholder="输入关键词..." class="search-input" />
      <button class="btn-primary">搜索</button>
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 720px;
}

h1 {
  margin-bottom: var(--space-4);
  color: var(--text-primary);
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
}

.search-bar {
  display: flex;
  gap: var(--space-3);
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

.search-input:focus {
  border-color: var(--color-brand);
}

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

.btn-primary:hover {
  background: var(--color-brand-dark);
}
</style>
