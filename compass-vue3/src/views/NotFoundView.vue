<script setup lang="ts">
// 404 view
</script>

<template>
  <div class="not-found">
    <h1>404</h1>
    <p>页面不存在</p>
    <router-link to="/entities">← 返回知识库</router-link>
  </div>
</template>

<style scoped>
.not-found {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 60vh;
  gap: 12px;
}
h1 {
  font-size: 64px;
  color: #ccc;
}
a {
  color: #3a3a6a;
  text-decoration: none;
}
</style>
