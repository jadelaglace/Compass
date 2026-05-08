<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { Chart, LineController, CategoryScale, LinearScale, PointElement, LineElement, Filler, Tooltip, Legend } from 'chart.js'

Chart.register(LineController, CategoryScale, LinearScale, PointElement, LineElement, Filler, Tooltip, Legend)

const chartRef = ref<HTMLCanvasElement | null>(null)

onMounted(() => {
  if (!chartRef.value) return
  const ctx = chartRef.value.getContext('2d')!

  const gradient = ctx.createLinearGradient(0, 0, 0, 200)
  gradient.addColorStop(0, 'rgba(79, 70, 229, 0.3)')
  gradient.addColorStop(1, 'rgba(79, 70, 229, 0)')

  new Chart(ctx, {
    type: 'line',
    data: {
      labels: ['5/1', '5/2', '5/3', '5/4', '5/5', '5/6', '5/7', '5/8'],
      datasets: [{
        label: '质量评分',
        data: [0.71, 0.73, 0.72, 0.76, 0.78, 0.80, 0.79, 0.82],
        borderColor: '#4f46e5',
        backgroundColor: gradient,
        borderWidth: 2,
        pointRadius: 4,
        pointBackgroundColor: '#4f46e5',
        fill: true,
        tension: 0.4,
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      scales: {
        y: {
          min: 0,
          max: 1,
          grid: { color: 'rgba(0,0,0,0.06)' },
          ticks: {
            color: '#9ca3af',
            font: { size: 11 },
            callback: (v: number | string) => `${Number(v).toFixed(1)}`
          }
        },
        x: {
          grid: { display: false },
          ticks: { color: '#9ca3af', font: { size: 11 } }
        }
      },
      plugins: {
        legend: { display: false },
        tooltip: {
          callbacks: {
            label: (ctx: unknown) => ` 评分: ${(ctx as { raw: number }).raw.toFixed(2)}`
          }
        }
      }
    }
  })
})
</script>

<template>
  <div class="chart-card">
    <h3 class="chart-title">📈 评分历史</h3>
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
.chart-wrap { height: 200px; }
</style>
