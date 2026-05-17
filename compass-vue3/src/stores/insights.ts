import { defineStore } from 'pinia'
import { ref } from 'vue'
import apiClient from '@/api/client'

export type Maturity = 'seed' | 'sprout' | 'bud' | 'bloom' | 'ripe'

export interface Insight {
  id: string
  entity_id: string
  title: string
  content: string
  maturity: Maturity
  created_at: string
  updated_at?: string
}

export const useInsightsStore = defineStore('insights', () => {
  const insights = ref<Insight[]>([])
  const loading = ref(false)

  const fetchInsights = async () => {
    loading.value = true
    try {
      const res = await apiClient.get('/insights')
      insights.value = res.data.items || res.data.insights || []
    } catch (err) {
      console.error('[insights store] fetchInsights failed:', err)
    } finally {
      loading.value = false
    }
  }

  const createInsight = async (data: Partial<Insight>) => {
    const res = await apiClient.post('/insights', data)
    return res.data
  }

  const updateInsight = async (id: string, data: Partial<Insight>) => {
    const res = await apiClient.put(`/insights/${id}`, data)
    return res.data
  }

  const deleteInsight = async (id: string) => {
    await apiClient.delete(`/insights/${id}`)
  }

  const fetchScoreHistory = async (entityId: string) => {
    const res = await apiClient.get(`/entities/${entityId}/score-history`)
    return res.data.history || []
  }

  return { insights, loading, fetchInsights, createInsight, updateInsight, deleteInsight, fetchScoreHistory }
})