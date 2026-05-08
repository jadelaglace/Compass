import { defineStore } from 'pinia'
import { ref, computed } from 'vue'

export type EntityType = 'all' | 'concept' | 'project' | 'person' | 'article'
export type SortKey = 'updated' | 'created' | 'score' | 'access_count'

export interface Entity {
  id: string
  title: string
  content: string
  entity_type: EntityType
  status: string
  maturity: number
  score: number
  tags: string[]
  access_count: number
  created_at: string
  updated_at: string
}

const MOCK_ENTITIES: Entity[] = [
  { id: '1', title: 'Compass 架构设计', content: 'Rust core + Python FastAPI glue 两层架构，职责分离清晰。', entity_type: 'concept', status: 'active', maturity: 5, score: 0.95, tags: ['架构', 'Rust', 'Python'], access_count: 42, created_at: '2026-04-06T10:00:00Z', updated_at: '2026-05-08T10:00:00Z' },
  { id: '2', title: 'Phase 4 前端规划', content: 'Vue3 + TypeScript + D3.js + Pinia，前端工程化最佳实践。', entity_type: 'project', status: 'active', maturity: 3, score: 0.88, tags: ['Vue3', 'TypeScript', 'D3.js'], access_count: 38, created_at: '2026-04-10T10:00:00Z', updated_at: '2026-05-08T09:00:00Z' },
  { id: '3', title: 'FTS5 全文搜索优化', content: 'BM25 + FTS5 混合搜索算法，支持 phrase query 高精度检索。', entity_type: 'concept', status: 'active', maturity: 4, score: 0.82, tags: ['FTS5', 'SQLite', '搜索'], access_count: 29, created_at: '2026-04-12T10:00:00Z', updated_at: '2026-05-07T15:00:00Z' },
  { id: '4', title: 'Decay 衰减算法', content: '基于 half-life 的指数衰减评分模型，模拟知识遗忘规律。', entity_type: 'concept', status: 'active', maturity: 4, score: 0.76, tags: ['算法', '衰减', '评分'], access_count: 21, created_at: '2026-04-14T10:00:00Z', updated_at: '2026-05-06T12:00:00Z' },
  { id: '5', title: 'OpenClaw Skill 架构', content: '意图检测 + 两层调用 + render 翻译，零业务逻辑消息管道。', entity_type: 'concept', status: 'active', maturity: 5, score: 0.71, tags: ['OpenClaw', 'Agent', 'Skill'], access_count: 35, created_at: '2026-04-18T10:00:00Z', updated_at: '2026-05-05T08:00:00Z' },
  { id: '6', title: 'D3.js 力导向图', content: '基于 D3 force simulation 的知识图谱可视化，交互丰富。', entity_type: 'article', status: 'active', maturity: 2, score: 0.65, tags: ['D3', '可视化', '图谱'], access_count: 18, created_at: '2026-05-01T10:00:00Z', updated_at: '2026-05-08T10:00:00Z' },
  { id: '7', title: 'Pinia 状态管理', content: 'Vue3 官方推荐状态管理库，TypeScript 支持友好。', entity_type: 'concept', status: 'active', maturity: 3, score: 0.60, tags: ['Pinia', 'Vue3', '状态管理'], access_count: 15, created_at: '2026-05-02T10:00:00Z', updated_at: '2026-05-04T10:00:00Z' },
  { id: '8', title: 'Vue Router 懒加载', content: '路由级代码分割，优化首屏加载性能。', entity_type: 'concept', status: 'archived', maturity: 4, score: 0.55, tags: ['Vue', 'Router', '性能'], access_count: 12, created_at: '2026-04-20T10:00:00Z', updated_at: '2026-04-30T10:00:00Z' },
]

export const useEntityStore = defineStore('entity', () => {
  const entities = ref<Entity[]>(MOCK_ENTITIES)
  const typeFilter = ref<EntityType>('all')
  const sortKey = ref<SortKey>('updated')
  const page = ref(1)
  const pageSize = ref(6)
  const loading = ref(false)

  const sorted = computed(() => {
    const list = [...entities.value]
    if (typeFilter.value !== 'all') {
      return list.filter(e => e.entity_type === typeFilter.value)
    }
    return list
  })

  const total = computed(() => sorted.value.length)

  const paginated = computed(() => {
    const start = (page.value - 1) * pageSize.value
    const list = [...sorted.value].sort((a, b) => {
      if (sortKey.value === 'score') return b.score - a.score
      if (sortKey.value === 'access_count') return b.access_count - a.access_count
      if (sortKey.value === 'created') return new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      return new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime()
    })
    return list.slice(start, start + pageSize.value)
  })

  const totalPages = computed(() => Math.ceil(total.value / pageSize.value))

  const setFilter = (t: EntityType) => { typeFilter.value = t; page.value = 1 }
  const setSort = (s: SortKey) => { sortKey.value = s; page.value = 1 }
  const setPage = (p: number) => { page.value = p }

  return {
    entities, typeFilter, sortKey, page, pageSize, loading,
    sorted, total, paginated, totalPages,
    setFilter, setSort, setPage,
  }
})
