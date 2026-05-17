import { defineStore } from 'pinia'
import { ref } from 'vue'
import apiClient from '@/api/client'

export type EventType = 'all' | 'access' | 'score_change' | 'tag_add' | 'insight_mature'

export interface TimelineEvent {
  id: string
  entity_id: string
  entity_title: string
  event_type: EventType
  timestamp: string
  description: string
}

export const useTimelineStore = defineStore('timeline', () => {
  const events = ref<TimelineEvent[]>([])
  const filter = ref<EventType>('all')
  const loading = ref(false)

  const fetchTimeline = async (entityId?: string) => {
    loading.value = true
    try {
      if (entityId) {
        const res = await apiClient.get(`/entities/${entityId}/timeline`)
        events.value = res.data.events || []
      } else {
        // 全局 timeline：取最近更新的实体，批量拉 timeline
        const entRes = await apiClient.get('/entities', { params: { sort: 'updated', page_size: '20' } })
        const entities = entRes.data.entities || []
        const allEvents: TimelineEvent[] = []
        for (const ent of entities.slice(0, 10)) {
          try {
            const evRes = await apiClient.get(`/entities/${ent.id}/timeline`)
            for (const ev of (evRes.data.events || [])) {
              allEvents.push({ ...ev, entity_title: ent.title })
            }
          } catch {}
        }
        events.value = allEvents.sort((a, b) => new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime())
      }
    } catch (err) {
      console.error('[timeline store] fetchTimeline failed:', err)
    } finally {
      loading.value = false
    }
  }

  const setFilter = (t: EventType) => { filter.value = t }

  return { events, filter, loading, fetchTimeline, setFilter }
})