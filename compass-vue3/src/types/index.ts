export interface Entity {
  id: string
  title: string
  content: string
  entity_type: string
  status: string
  maturity: number
  score: number
  tags: string[]
  created_at: string
  updated_at: string
}

export interface TimelineEvent {
  id: string
  entity_id: string
  event_type: string
  timestamp: string
  data: Record<string, unknown>
}

export interface Insight {
  id: string
  entity_id: string
  content: string
  maturity: string
  created_at: string
}

export interface GraphNode {
  id: string
  title: string
  entity_type: string
  score: number
}

export interface GraphEdge {
  source: string
  target: string
  strength: number
}

export interface SearchResult {
  entity: Entity
  score: number
  highlights: string[]
}

export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  page_size: number
}

export interface ApiError {
  detail: string
}
