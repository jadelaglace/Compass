<script setup lang="ts">
import { useTheme } from '@/composables/useTheme'

const { theme, toggleTheme } = useTheme()
</script>

<template>
  <button class="theme-toggle" :title="theme === 'dark' ? '切换亮色模式' : '切换暗色模式'" @click="toggleTheme">
    <span v-if="theme === 'dark'">🌙</span>
    <span v-else>☀️</span>
  </button>
</template>

<style scoped>
.theme-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  font-size: 16px;
  transition: background var(--transition-fast);
}

.theme-toggle:hover {
  background: var(--bg-hover);
}
</style>
