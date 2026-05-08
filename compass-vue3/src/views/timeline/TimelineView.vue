<script setup lang="ts">
import { computed } from 'vue'
import { useTimelineStore, type EventType } from '@/stores/timeline'
import TimelineItem from '@/components/timeline/TimelineItem.vue'

const timelineStore = useTimelineStore()

const filters: { label: string; value: EventType }[] = [
  { label: '全部', value: 'all' },
  { label: '访问', value: 'access' },
  { label: '评分', value: 'score_change' },
  { label: '标签', value: 'tag_add' },
  { label: '洞察', value: 'insight_mature' },
]

const filteredEvents = computed(() => {
  if (timelineStore.filter === 'all') return timelineStore.events
  return timelineStore.events.filter(e => e.event_type === timelineStore.filter)
})
</script>

<template>
  <div class="view">
    <div class="view-header">
      <h1>📜 时间线</h1>
      <div class="filter-bar">
        <button
          v-for="f in filters"
          :key="f.value"
          class="filter-chip"
          :class="{ active: timelineStore.filter === f.value }"
          @click="timelineStore.setFilter(f.value)"
        >
          {{ f.label }}
        </button>
      </div>
    </div>

    <div class="timeline-list">
      <TimelineItem
        v-for="event in filteredEvents"
        :key="event.id"
        :event="event"
      />
    </div>
  </div>
</template>

<style scoped>
.view {
  max-width: 720px;
}

.view-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-6);
  flex-wrap: wrap;
  gap: var(--space-3);
}

h1 {
  font-size: var(--text-2xl);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
}

.filter-bar {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.filter-chip {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-full);
  background: var(--bg-primary);
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.filter-chip:hover {
  border-color: var(--color-brand);
  color: var(--color-brand);
}

.filter-chip.active {
  background: var(--color-brand);
  border-color: var(--color-brand);
  color: var(--text-inverse);
}

.timeline-list {
  padding-left: var(--space-2);
}
</style>
