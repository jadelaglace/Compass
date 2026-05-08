<script setup lang="ts">
import { useAppStore } from '@/stores/app'
import SideNav from '@/components/SideNav.vue'
import TopBar from '@/components/TopBar.vue'

const appStore = useAppStore()
</script>

<template>
  <div class="app-layout" :class="{ 'sidebar-collapsed': appStore.sidebarCollapsed }">
    <SideNav />
    <div class="main-area">
      <TopBar />
      <main class="content">
        <router-view v-slot="{ Component }">
          <transition name="fade-slide" mode="out-in">
            <component :is="Component" />
          </transition>
        </router-view>
      </main>
    </div>
  </div>
</template>

<style scoped>
.app-layout {
  display: flex;
  height: 100vh;
  overflow: hidden;
}

.main-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.content {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-6);
  background: var(--bg-secondary);
}

.fade-slide-enter-active,
.fade-slide-leave-active {
  transition: opacity var(--transition-normal), transform var(--transition-normal);
}

.fade-slide-enter-from {
  opacity: 0;
  transform: translateX(8px);
}

.fade-slide-leave-to {
  opacity: 0;
  transform: translateX(-8px);
}
</style>
