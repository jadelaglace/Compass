import { defineStore } from 'pinia'
import { ref } from 'vue'
import apiClient from '@/api/client'
import type { GraphNode, GraphEdge } from '@/types'

export const useGraphStore = defineStore('graph', () => {
  const nodes = ref<GraphNode[]>([])
  const edges = ref<GraphEdge[]>([])
  const loading = ref(false)
  const selectedNodeId = ref<string | null>(null)
  const hoveredNodeId = ref<string | null>(null)

  const fetchGraph = async (centerId?: string) => {
    loading.value = true
    try {
      if (centerId) {
        // 中心节点邻居图
        const res = await apiClient.get(`/graph/neighbors/${centerId}`, { params: { depth: 2 } })
        nodes.value = res.data.nodes || []
        edges.value = res.data.links || []
      } else {
        // 全局图：取实体列表构建邻居子图
        const entRes = await apiClient.get('/entities', { params: { sort: 'updated', page_size: '30' } })
        const entities = entRes.data.entities || []
        nodes.value = entities.map((e: any) => ({
          id: e.id,
          title: e.title,
          entity_type: e.entity_type,
          score: e.score,
        }))
        edges.value = []
        for (const ent of entities.slice(0, 15)) {
          try {
            const nRes = await apiClient.get(`/graph/neighbors/${ent.id}`, { params: { depth: 1, limit: 5 } })
            for (const nb of (nRes.data.neighbors || [])) {
              edges.value.push({
                source: ent.id,
                target: nb.id,
                strength: nb.strength || 0.5,
              })
            }
          } catch {}
        }
      }
    } catch (err) {
      console.error('[graph store] fetchGraph failed:', err)
    } finally {
      loading.value = false
    }
  }

  const selectNode = (id: string | null) => { selectedNodeId.value = id }
  const hoverNode = (id: string | null) => { hoveredNodeId.value = id }

  return { nodes, edges, loading, selectedNodeId, hoveredNodeId, fetchGraph, selectNode, hoverNode }
})