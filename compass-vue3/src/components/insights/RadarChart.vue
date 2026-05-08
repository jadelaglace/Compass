<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Chart, RadarController, RadialLinearScale, PointElement, LineElement, Filler, Tooltip, Filler as ChartFiller } from 'chart.js'

Chart.register(RadarController, RadialLinearScale, PointElement, LineElement, Filler, Tooltip, ChartFiller)

const chartRef = ref<HTMLCanvasElement | null>(null)

onMounted(() => {
  if (!chartRef.value) return
  const ctx = chartRef.value.getContext('2d')!
  new Chart(ctx, {
    type: 'radar',
    data: {
      labels: ['访问频率', '内容质量', '标签密度', '成熟度'],
      datasets: [{
        label: '综合评分',
        data: [0.88, 0.72, 0.65, 0.80],
        backgroundColor: 'rgba(79, 70, 229, 0.15)',
        borderColor: '#4f46e5',
        borderWidth: 2,
        pointBackgroundColor: '#4f46e5',
        pointRadius: 4,
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: true,
      scales: {
        r: {
          min: 0,
          max: 1,
          ticks: { stepSize: 0.25, display: false },
          grid: { color: 'rgba(0,0,0,0.08)' },
          pointLabels: { font: { size: 12 }, color: '#4b5563' },
          angleLines: { color: 'rgba(0,0,0,0.06)' }
        }
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          callbacks: {
            label: (ctx: unknown) => ` ${((ctx as { raw: number }).raw * 100).toFixed(0)}%`
          }
        }
      }
    }
  })
})
</script>

<template>
  <div class="chart-card">
    <h3 class="chart-title">📡 综合能力雷达</h3>
    <div class="chart-wrap">
      <canvas ref="chartRef"></canvas>
    </div>
  </div>
</template>

<style scoped>
.chart-card {
  background: var(--bg-primary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-lg);
  padding: var(--space-5);
}
.chart-title {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  color: var(--text-primary);
  margin: 0 0 var(--space-4);
}
.chart-wrap { max-width: 320px; margin: 0 auto; }
</style>
