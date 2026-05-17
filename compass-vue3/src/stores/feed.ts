import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import apiClient from '@/api/client'
import type { Entity } from '@/types'

export type EntityType = 'all' | 'concept' | 'project' | 'person' | 'article'

export const useFeedStore = defineStore('feed', () => {
  const entities = ref<Entity[]>([])
  const filter = ref<EntityType>('all')
  const loading = ref(false)

  const fetchFeed = async () => {
    loading.value = true
    try {
      const params: Record<string, string> = { sort: 'score', page_size: '20' }
      if (filter.value !== 'all') params.entity_type = filter.value
      const res = await apiClient.get('/entities', { params })
      entities.value = res.data.entities || res.data.items || []
    } catch (err) {
      console.error('[feed store] fetchFeed failed:', err)
    } finally {
      loading.value = false
    }
  }

  const filtered = computed(() => {
    if (filter.value === 'all') return entities.value
    return entities.value.filter(e => e.entity_type === filter.value)
  })

  const setFilter = (t: EntityType) => { filter.value = t }

  return { entities, filter, loading, filtered, setFilter, fetchFeed }
})