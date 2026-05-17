import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import apiClient from '@/api/client'

export type EntityType = 'all' | 'concept' | 'project' | 'person' | 'article'
export type SortKey = 'updated' | 'created' | 'score' | 'access_count'

export interface Entity {
  id: string
  title: string
  content: string
  entity_type: string
  status: string
  maturity: number
  score: number
  tags: string[]
  access_count: number
  created_at: string
  updated_at: string
}

export const useEntityStore = defineStore('entity', () => {
  const entities = ref<Entity[]>([])
  const typeFilter = ref<EntityType>('all')
  const sortKey = ref<SortKey>('updated')
  const page = ref(1)
  const pageSize = ref(20)
  const loading = ref(false)

  const fetchEntities = async () => {
    loading.value = true
    try {
      const params: Record<string, string> = {}
      if (typeFilter.value !== 'all') params.entity_type = typeFilter.value
      params.sort = sortKey.value
      params.page = String(page.value)
      params.page_size = String(pageSize.value)
      const res = await apiClient.get('/entities', { params })
      entities.value = res.data.entities || res.data.items || []
    } catch (err) {
      console.error('[entity store] fetchEntities failed:', err)
    } finally {
      loading.value = false
    }
  }

  const fetchEntity = async (id: string): Promise<Entity | null> => {
    try {
      const res = await apiClient.get(`/entities/${id}`)
      return res.data
    } catch (err) {
      console.error('[entity store] fetchEntity failed:', err)
      return null
    }
  }

  const createEntity = async (data: Partial<Entity>) => {
    const res = await apiClient.post('/entities', data)
    return res.data
  }

  const updateEntity = async (id: string, data: Partial<Entity>) => {
    const res = await apiClient.put(`/entities/${id}`, data)
    return res.data
  }

  const deleteEntity = async (id: string) => {
    await apiClient.delete(`/entities/${id}`)
  }

  const sorted = computed(() => {
    const list = [...entities.value]
    if (sortKey.value === 'score') return list.sort((a, b) => b.score - a.score)
    if (sortKey.value === 'access_count') return list.sort((a, b) => b.access_count - a.access_count)
    if (sortKey.value === 'created') return list.sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
    return list.sort((a, b) => new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime())
  })

  const total = computed(() => entities.value.length)
  const totalPages = computed(() => Math.ceil(total.value / pageSize.value))

  const setFilter = (t: EntityType) => { typeFilter.value = t; page.value = 1 }
  const setSort = (s: SortKey) => { sortKey.value = s; page.value = 1 }
  const setPage = (p: number) => { page.value = p }

  return {
    entities, typeFilter, sortKey, page, pageSize, loading,
    sorted, total, totalPages,
    fetchEntities, fetchEntity, createEntity, updateEntity, deleteEntity,
    setFilter, setSort, setPage,
  }
})