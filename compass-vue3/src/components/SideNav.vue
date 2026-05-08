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
  background: #1a1a2e;
  color: #fff;
  display: flex;
  flex-direction: column;
  transition: width 0.3s ease;
  flex-shrink: 0;
}

.side-nav.collapsed {
  width: 56px;
}

.nav-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 16px;
  font-size: 18px;
  font-weight: bold;
  border-bottom: 1px solid #2a2a4a;
}

.logo {
  font-size: 22px;
}

.logo-text {
  white-space: nowrap;
}

.nav-list {
  flex: 1;
  list-style: none;
  padding: 8px;
  margin: 0;
  overflow-y: auto;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 8px;
  color: #a0a0c0;
  text-decoration: none;
  border-radius: 6px;
  transition: background 0.15s, color 0.15s;
  margin-bottom: 2px;
}

.nav-item:hover {
  background: #2a2a4a;
  color: #fff;
}

.nav-item.router-link-active {
  background: #3a3a6a;
  color: #fff;
}

.nav-icon {
  font-size: 18px;
  flex-shrink: 0;
}

.nav-label {
  white-space: nowrap;
  font-size: 14px;
}

.toggle-btn {
  margin: 12px;
  padding: 8px;
  background: #2a2a4a;
  border: none;
  color: #a0a0c0;
  border-radius: 6px;
  cursor: pointer;
  font-size: 14px;
}

.toggle-btn:hover {
  background: #3a3a6a;
  color: #fff;
}
</style>
