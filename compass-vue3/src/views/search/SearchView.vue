<script setup lang="ts">
import { ref } from 'vue'

const query = ref('')
// TODO: wire to API
</script>

<template>
  <div class="view">
    <h1>🔍 搜索</h1>
    <div class="search-bar">
      <input v-model="query" type="text" placeholder="输入关键词..." />
      <button>搜索</button>
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 720px;
}
h1 {
  margin-bottom: 16px;
}
.search-bar {
  display: flex;
  gap: 8px;
}
input {
  flex: 1;
  padding: 8px 12px;
  border: 1px solid #e0e0e0;
  border-radius: 6px;
  font-size: 14px;
}
button {
  padding: 8px 20px;
  background: #1a1a2e;
  color: #fff;
  border: none;
  border-radius: 6px;
  cursor: pointer;
}
</style>
