<script setup lang="ts">
// 404 view
</script>

<template>
  <div class="not-found">
    <h1>404</h1>
    <p>页面不存在</p>
    <router-link to="/entities" class="back-link">← 返回知识库</router-link>
  </div>
</template>

<style scoped>
.not-found {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 60vh;
  gap: var(--space-3);
}

h1 {
  font-size: 64px;
  font-weight: var(--weight-bold);
  color: var(--color-gray-300);
}

p {
  color: var(--text-secondary);
  font-size: var(--text-lg);
}

.back-link {
  color: var(--color-brand);
  text-decoration: none;
  font-size: var(--text-base);
}

.back-link:hover {
  text-decoration: underline;
}
</style>
