import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Entity } from '@/types'

export type EntityType = 'all' | 'concept' | 'project' | 'person' | 'article'

const MOCK_ENTITIES: Entity[] = [
  { id: '1', title: 'Compass 架构设计', content: 'Rust core + Python FastAPI glue 两层架构', entity_type: 'concept', status: 'active', maturity: 5, score: 0.95, tags: ['架构', 'Rust', 'Python'], created_at: '2026-04-06T10:00:00Z', updated_at: '2026-05-01T10:00:00Z' },
  { id: '2', title: 'Phase 4 前端规划', content: 'Vue3 + TypeScript + D3.js 可视化', entity_type: 'project', status: 'active', maturity: 3, score: 0.88, tags: ['Vue3', 'TypeScript', 'D3.js'], created_at: '2026-04-10T10:00:00Z', updated_at: '2026-05-08T10:00:00Z' },
  { id: '3', title: 'FTS5 全文搜索优化', content: 'BM25 + FTS5 混合搜索算法实现', entity_type: 'concept', status: 'active', maturity: 4, score: 0.82, tags: ['FTS5', 'SQLite', '搜索'], created_at: '2026-04-12T10:00:00Z', updated_at: '2026-04-28T10:00:00Z' },
  { id: '4', title: 'Decay 衰减算法', content: '基于 half-life 的指数衰减评分模型', entity_type: 'concept', status: 'active', maturity: 4, score: 0.76, tags: ['算法', '衰减', '评分'], created_at: '2026-04-14T10:00:00Z', updated_at: '2026-04-30T10:00:00Z' },
  { id: '5', title: 'OpenClaw Skill 架构', content: '意图检测 + 两层调用 + render 翻译', entity_type: 'concept', status: 'active', maturity: 5, score: 0.71, tags: ['OpenClaw', 'Agent', 'Skill'], created_at: '2026-04-18T10:00:00Z', updated_at: '2026-05-05T10:00:00Z' },
  { id: '6', title: 'D3.js 力导向图', content: '基于 D3 force simulation 的知识图谱可视化', entity_type: 'article', status: 'active', maturity: 2, score: 0.65, tags: ['D3', '可视化', '图谱'], created_at: '2026-05-01T10:00:00Z', updated_at: '2026-05-08T10:00:00Z' },
]

export const useFeedStore = defineStore('feed', () => {
  const entities = ref<Entity[]>(MOCK_ENTITIES)
  const filter = ref<EntityType>('all')
  const loading = ref(false)

  const filtered = computed(() => {
    if (filter.value === 'all') return entities.value
    return entities.value.filter(e => e.entity_type === filter.value)
  })

  const setFilter = (t: EntityType) => { filter.value = t }

  return { entities, filter, loading, filtered, setFilter }
})
