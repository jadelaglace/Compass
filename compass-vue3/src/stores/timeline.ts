import { defineStore } from 'pinia'
import { ref } from 'vue'

export type EventType = 'all' | 'access' | 'score_change' | 'tag_add' | 'insight_mature'

export interface TimelineEvent {
  id: string
  entity_id: string
  entity_title: string
  event_type: EventType
  timestamp: string
  description: string
}

const MOCK_EVENTS: TimelineEvent[] = [
  { id: '1', entity_id: '1', entity_title: 'Compass 架构设计', event_type: 'access', timestamp: '2026-05-08T22:00:00Z', description: '查看了实体' },
  { id: '2', entity_id: '2', entity_title: 'Phase 4 前端规划', event_type: 'score_change', timestamp: '2026-05-08T20:00:00Z', description: '评分从 0.82 → 0.88' },
  { id: '3', entity_id: '3', entity_title: 'FTS5 全文搜索优化', event_type: 'tag_add', timestamp: '2026-05-07T18:00:00Z', description: '新增标签: 搜索' },
  { id: '4', entity_id: '4', entity_title: 'Decay 衰减算法', event_type: 'insight_mature', timestamp: '2026-05-07T12:00:00Z', description: 'Insight 已成熟：触发实体升级' },
  { id: '5', entity_id: '1', entity_title: 'Compass 架构设计', event_type: 'access', timestamp: '2026-05-06T10:00:00Z', description: '查看了实体' },
  { id: '6', entity_id: '5', entity_title: 'OpenClaw Skill 架构', event_type: 'score_change', timestamp: '2026-05-05T08:00:00Z', description: '评分从 0.65 → 0.71' },
]

export const useTimelineStore = defineStore('timeline', () => {
  const events = ref<TimelineEvent[]>(MOCK_EVENTS)
  const filter = ref<EventType>('all')
  const loading = ref(false)

  const setFilter = (t: EventType) => { filter.value = t }

  return { events, filter, loading, setFilter }
})
