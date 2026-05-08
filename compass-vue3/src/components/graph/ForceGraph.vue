<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import * as d3 from 'd3'
import { useGraphStore } from '@/stores/graph'

const graphStore = useGraphStore()
const svgRef = ref<SVGSVGElement | null>(null)
const containerRef = ref<HTMLDivElement | null>(null)

interface SimNode extends d3.SimulationNodeDatum {
  id: string
  title: string
  entity_type: string
  score: number
}

let simulation: d3.Simulation<SimNode, d3.SimulationLinkDatum<SimNode>> | null = null

const entityTypeColors: Record<string, string> = {
  concept: '#4f46e5',
  project: '#10b981',
  person: '#f59e0b',
  article: '#3b82f6',
  default: '#6b7280',
}

function getColor(type: string): string {
  return entityTypeColors[type] ?? entityTypeColors.default
}

function initGraph() {
  if (!svgRef.value || !containerRef.value) return

  const width = containerRef.value.clientWidth
  const height = containerRef.value.clientHeight

  d3.select(svgRef.value).selectAll('*').remove()

  const svg = d3.select(svgRef.value)
    .attr('width', width)
    .attr('height', height)
    .attr('viewBox', [0, 0, width, height])

  const g = svg.append('g')
  svg.call(
    d3.zoom<SVGSVGElement, unknown>()
      .scaleExtent([0.2, 4])
      .on('zoom', (event) => {
        g.attr('transform', event.transform)
      })
  )

  const nodes: SimNode[] = graphStore.nodes.map(n => ({ ...n }))
  const edges: d3.SimulationLinkDatum<SimNode>[] = graphStore.edges.map(e => ({ ...e }))

  const link = g.append('g')
    .selectAll('line')
    .data(edges)
    .join('line')
    .attr('stroke', '#d1d5db')
    .attr('stroke-width', (d: d3.SimulationLinkDatum<SimNode>) => ((d as { strength?: number }).strength ?? 0.5) * 3)
    .attr('stroke-opacity', 0.6)

  const node = g.append('g')
    .selectAll<SVGGElement, SimNode>('g')
    .data(nodes)
    .join('g')
    .style('cursor', 'pointer')
    .call(
      d3.drag<SVGGElement, SimNode>()
        .on('start', (event, d) => {
          if (!event.active) simulation!.alphaTarget(0.3).restart()
          d.fx = d.x
          d.fy = d.y
        })
        .on('drag', (event, d) => {
          d.fx = event.x
          d.fy = event.y
        })
        .on('end', (event, d) => {
          if (!event.active) simulation!.alphaTarget(0)
          d.fx = null
          d.fy = null
        })
    )

  node.append('circle')
    .attr('r', d => 10 + d.score * 20)
    .attr('fill', d => getColor(d.entity_type))
    .attr('fill-opacity', 0.85)
    .attr('stroke', '#fff')
    .attr('stroke-width', 2)

  node.append('text')
    .text(d => d.title)
    .attr('font-size', 12)
    .attr('text-anchor', 'middle')
    .attr('dy', d => -(14 + d.score * 20))
    .attr('fill', 'var(--text-primary)')

  node
    .on('mouseover', function(_, d) {
      d3.select(this).select('circle').attr('stroke-width', 3)
      graphStore.hoverNode(d.id)
    })
    .on('mouseout', function() {
      d3.select(this).select('circle').attr('stroke-width', 2)
      graphStore.hoverNode(null)
    })
    .on('click', (_, d) => {
      graphStore.selectNode(d.id)
    })

  const tooltip = d3.select(containerRef.value)
    .append('div')
    .style('position', 'absolute')
    .style('background', 'var(--bg-primary)')
    .style('border', '1px solid var(--border-color)')
    .style('border-radius', 'var(--radius-md)')
    .style('padding', '8px 12px')
    .style('font-size', '13px')
    .style('pointer-events', 'none')
    .style('opacity', 0)
    .style('z-index', 10)
    .style('box-shadow', 'var(--shadow-md)')

  node
    .on('mouseover.tooltip', (event: MouseEvent, d: SimNode) => {
      tooltip
        .style('opacity', 1)
        .html(`<strong>${d.title}</strong><br/>${d.entity_type} · score: ${(d.score * 100).toFixed(0)}`)
        .style('left', `${event.offsetX + 12}px`)
        .style('top', `${event.offsetY - 10}px`)
    })
    .on('mousemove.tooltip', (event: MouseEvent) => {
      tooltip.style('left', `${event.offsetX + 12}px`).style('top', `${event.offsetY - 10}px`)
    })
    .on('mouseout.tooltip', () => { tooltip.style('opacity', 0) })

  simulation = d3.forceSimulation(nodes)
    .force('link', d3.forceLink<SimNode, d3.SimulationLinkDatum<SimNode>>(edges).id(d => d.id).distance(120))
    .force('charge', d3.forceManyBody().strength(-300))
    .force('center', d3.forceCenter(width / 2, height / 2))
    .force('collision', d3.forceCollide<SimNode>().radius(d => 30 + d.score * 20))
    .on('tick', () => {
      link
        .attr('x1', d => (d.source as SimNode).x ?? 0)
        .attr('y1', d => (d.source as SimNode).y ?? 0)
        .attr('x2', d => (d.target as SimNode).x ?? 0)
        .attr('y2', d => (d.target as SimNode).y ?? 0)

      node.attr('transform', d => `translate(${d.x ?? 0},${d.y ?? 0})`)
    })

  const ro = new ResizeObserver(() => {
    if (!containerRef.value || !simulation) return
    const w = containerRef.value.clientWidth
    const h = containerRef.value.clientHeight
    svg.attr('viewBox', [0, 0, w, h])
    simulation.force('center', d3.forceCenter(w / 2, h / 2))
    simulation.alpha(0.3).restart()
  })
  ro.observe(containerRef.value)
}

onMounted(initGraph)
onUnmounted(() => { simulation?.stop() })
</script>

<template>
  <div ref="containerRef" class="force-graph">
    <svg ref="svgRef" class="graph-svg"></svg>
  </div>
</template>

<style scoped>
.force-graph {
  position: relative;
  width: 100%;
  height: 500px;
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  overflow: hidden;
}
.graph-svg { width: 100%; height: 100%; }
</style>
