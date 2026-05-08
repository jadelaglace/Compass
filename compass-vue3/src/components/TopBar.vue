<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'

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
      <span class="user-info">👤 dbb</span>
    </div>
  </header>
</template>

<style scoped>
.top-bar {
  height: 56px;
  background: #fff;
  border-bottom: 1px solid #e0e0e0;
  display: flex;
  align-items: center;
  padding: 0 24px;
  gap: 16px;
  flex-shrink: 0;
}

.search-box {
  flex: 1;
  max-width: 480px;
}

.search-input {
  width: 100%;
  padding: 8px 16px;
  border: 1px solid #e0e0e0;
  border-radius: 20px;
  font-size: 14px;
  outline: none;
  transition: border-color 0.15s;
}

.search-input:focus {
  border-color: #3a3a6a;
}

.top-bar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.user-info {
  font-size: 14px;
  color: #666;
}
</style>
