import { defineStore } from 'pinia'
import { ref } from 'vue'
import type { GraphNode, GraphEdge } from '@/types'

export const useGraphStore = defineStore('graph', () => {
  const nodes = ref<GraphNode[]>([
    { id: '1', title: 'Compass架构', entity_type: 'concept', score: 0.95 },
    { id: '2', title: 'Rust核心', entity_type: 'concept', score: 0.88 },
    { id: '3', title: 'FastAPI层', entity_type: 'concept', score: 0.82 },
    { id: '4', title: 'Phase 2 Graph API', entity_type: 'project', score: 0.76 },
    { id: '5', title: 'Phase 3 自动标签', entity_type: 'project', score: 0.71 },
  ])

  const edges = ref<GraphEdge[]>([
    { source: '1', target: '2', strength: 0.9 },
    { source: '1', target: '3', strength: 0.85 },
    { source: '2', target: '4', strength: 0.7 },
    { source: '3', target: '4', strength: 0.65 },
    { source: '4', target: '5', strength: 0.6 },
  ])

  const selectedNodeId = ref<string | null>(null)
  const hoveredNodeId = ref<string | null>(null)

  const selectNode = (id: string | null) => {
    selectedNodeId.value = id
  }

  const hoverNode = (id: string | null) => {
    hoveredNodeId.value = id
  }

  return { nodes, edges, selectedNodeId, hoveredNodeId, selectNode, hoverNode }
})
