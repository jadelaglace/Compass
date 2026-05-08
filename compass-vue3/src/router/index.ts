import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  {
    path: '/',
    redirect: '/entities',
  },
  {
    path: '/entities',
    name: 'entities',
    component: () => import('@/views/entities/EntityListView.vue'),
  },
  {
    path: '/entities/:id',
    name: 'entity',
    component: () => import('@/views/entities/EntityDetailView.vue'),
  },
  {
    path: '/graph',
    name: 'graph',
    component: () => import('@/views/graph/GraphView.vue'),
  },
  {
    path: '/search',
    name: 'search',
    component: () => import('@/views/search/SearchView.vue'),
  },
  {
    path: '/feed',
    name: 'feed',
    component: () => import('@/views/feed/FeedView.vue'),
  },
  {
    path: '/timeline',
    name: 'timeline',
    component: () => import('@/views/timeline/TimelineView.vue'),
  },
  {
    path: '/insights',
    name: 'insights',
    component: () => import('@/views/insights/InsightsView.vue'),
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/settings/SettingsView.vue'),
  },
  {
    path: '/:pathMatch(.*)*',
    name: 'not-found',
    component: () => import('@/views/NotFoundView.vue'),
  },
]

const router = createRouter({
  history: createWebHistory(),
  routes,
})

export default router
