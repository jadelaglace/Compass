<script setup lang="ts">
import type { TimelineEvent } from '@/stores/timeline'

const props = defineProps<{ event: TimelineEvent }>()

const eventIcons: Record<string, string> = {
  access: '👁️',
  score_change: '📈',
  tag_add: '🏷️',
  insight_mature: '✨',
}

const eventColors: Record<string, string> = {
  access: '#3b82f6',
  score_change: '#10b981',
  tag_add: '#f59e0b',
  insight_mature: '#8b5cf6',
}
</script>

<template>
  <div class="timeline-item">
    <div class="timeline-icon" :style="{ background: eventColors[props.event.event_type] + '20', color: eventColors[props.event.event_type] }">
      {{ eventIcons[props.event.event_type] ?? '•' }}
    </div>
    <div class="timeline-content">
      <div class="event-header">
        <span class="event-type">{{ props.event.event_type.replace('_', ' ') }}</span>
        <span class="event-time">{{ new Date(props.event.timestamp).toLocaleString('zh-CN') }}</span>
      </div>
      <p class="event-desc">
        <strong>{{ props.event.entity_title }}</strong>
        — {{ props.event.description }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.timeline-item {
  display: flex;
  gap: var(--space-4);
  position: relative;
}

.timeline-item::before {
  content: '';
  position: absolute;
  left: 17px;
  top: 44px;
  bottom: -20px;
  width: 2px;
  background: var(--border-color);
}

.timeline-item:last-child::before {
  display: none;
}

.timeline-icon {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  flex-shrink: 0;
  z-index: 1;
}

.timeline-content {
  flex: 1;
  padding-bottom: var(--space-6);
}

.event-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: var(--space-1);
}

.event-type {
  font-size: var(--text-xs);
  font-weight: var(--weight-medium);
  text-transform: capitalize;
  color: var(--text-muted);
}

.event-time {
  font-size: var(--text-xs);
  color: var(--text-muted);
}

.event-desc {
  font-size: var(--text-sm);
  color: var(--text-primary);
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  padding: var(--space-3);
  margin: 0;
  line-height: var(--leading-normal);
}

.event-desc strong {
  font-weight: var(--weight-semibold);
}
</style>
