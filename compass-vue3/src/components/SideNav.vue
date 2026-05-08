<script setup lang="ts">
import { useAppStore } from '@/stores/app'

const appStore = useAppStore()

const navItems = [
  { name: 'entities', label: '知识库', icon: '📚' },
  { name: 'graph', label: '知识图谱', icon: '🔗' },
  { name: 'search', label: '搜索', icon: '🔍' },
  { name: 'feed', label: '推荐', icon: '✨' },
  { name: 'insights', label: '洞察', icon: '💡' },
  { name: 'settings', label: '设置', icon: '⚙️' },
]
</script>

<template>
  <nav class="side-nav" :class="{ collapsed: appStore.sidebarCollapsed }">
    <div class="nav-header">
      <span class="logo">🧭</span>
      <span v-if="!appStore.sidebarCollapsed" class="logo-text">Compass</span>
    </div>
    <ul class="nav-list">
      <li v-for="item in navItems" :key="item.name">
        <router-link :to="{ name: item.name }" class="nav-item" :title="item.label">
          <span class="nav-icon">{{ item.icon }}</span>
          <span v-if="!appStore.sidebarCollapsed" class="nav-label">{{ item.label }}</span>
        </router-link>
      </li>
    </ul>
    <button class="toggle-btn" @click="appStore.toggleSidebar">
      {{ appStore.sidebarCollapsed ? '→' : '←' }}
    </button>
  </nav>
</template>

<style scoped>
.side-nav {
  width: 200px;
  background: var(--sidebar-bg);
  color: var(--sidebar-text);
  display: flex;
  flex-direction: column;
  transition: width var(--transition-normal);
  flex-shrink: 0;
}

.side-nav.collapsed {
  width: 56px;
}

.nav-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-4);
  font-size: var(--text-lg);
  font-weight: var(--weight-bold);
  border-bottom: 1px solid var(--sidebar-hover-bg);
  white-space: nowrap;
  overflow: hidden;
}

.logo {
  font-size: 22px;
  flex-shrink: 0;
}

.logo-text {
  overflow: hidden;
}

.nav-list {
  flex: 1;
  list-style: none;
  padding: var(--space-2);
  margin: 0;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-2);
  color: var(--sidebar-text);
  text-decoration: none;
  border-radius: var(--radius-md);
  transition: background var(--transition-fast), color var(--transition-fast);
  margin-bottom: 2px;
  white-space: nowrap;
}

.nav-item:hover {
  background: var(--sidebar-hover-bg);
  text-decoration: none;
}

.nav-item.router-link-active {
  background: var(--sidebar-active-bg);
}

.nav-icon {
  font-size: 18px;
  flex-shrink: 0;
}

.nav-label {
  font-size: var(--text-sm);
  overflow: hidden;
}

.toggle-btn {
  margin: var(--space-3);
  padding: var(--space-2);
  background: var(--sidebar-hover-bg);
  border: none;
  color: var(--sidebar-text);
  border-radius: var(--radius-md);
  cursor: pointer;
  font-size: var(--text-sm);
  transition: background var(--transition-fast);
}

.toggle-btn:hover {
  background: var(--sidebar-active-bg);
}
</style>
