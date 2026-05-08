import { defineStore } from 'pinia'
import { ref } from 'vue'

export type Maturity = 'draft' | 'validated' | 'mature'

export interface Insight {
  id: string
  entity_id: string
  entity_title: string
  content: string
  maturity: Maturity
  created_at: string
}

const INITIAL: Insight[] = [
  { id: '1', entity_id: '1', entity_title: 'Compass 架构设计', content: 'Rust core 处理高性能计算，Python FastAPI 处理 HTTP 层，职责分离清晰。', maturity: 'mature', created_at: '2026-04-20T10:00:00Z' },
  { id: '2', entity_id: '2', entity_title: 'Phase 4 前端规划', content: 'Vue3 + TypeScript 组合是前端最优解，配套 Pinia + D3.js 可覆盖所有需求。', maturity: 'validated', created_at: '2026-04-25T10:00:00Z' },
  { id: '3', entity_id: '3', entity_title: 'FTS5 搜索优化', content: 'BM25 配合 FTS5 的 phrase query 可实现高精度全文检索。', maturity: 'draft', created_at: '2026-05-01T10:00:00Z' },
  { id: '4', entity_id: '4', entity_title: 'Decay 算法选择', content: '指数衰减比线性衰减更符合知识遗忘规律，half-life=30d 较为合理。', maturity: 'validated', created_at: '2026-05-03T10:00:00Z' },
]

let nextId = 5

export const useInsightsStore = defineStore('insights', () => {
  const insights = ref<Insight[]>([...INITIAL])

  const addInsight = (data: Omit<Insight, 'id' | 'created_at'>) => {
    insights.value.push({ ...data, id: String(nextId++), created_at: new Date().toISOString() })
  }

  const updateInsight = (id: string, data: Partial<Omit<Insight, 'id' | 'created_at'>>) => {
    const idx = insights.value.findIndex(i => i.id === id)
    if (idx !== -1) insights.value[idx] = { ...insights.value[idx], ...data }
  }

  const deleteInsight = (id: string) => {
    insights.value = insights.value.filter(i => i.id !== id)
  }

  return { insights, addInsight, updateInsight, deleteInsight }
})
