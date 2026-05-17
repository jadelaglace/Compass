<script setup lang="ts">
import { ref, reactive, computed, watch } from 'vue'

// ─── Types ────────────────────────────────────────────────────────────────

interface DecayConfig {
  interest_hl: number   // half-life in days
  strategy_hl: number
  consensus_hl: number
}

interface ScoreWeights {
  interest: number
  strategy: number
  consensus: number
}

// ─── State ──────────────────────────────────────────────────────────────────

const weights = reactive<ScoreWeights>({
  interest: 0.40,
  strategy: 0.35,
  consensus: 0.25,
})

const decayConfig = reactive<DecayConfig>({
  interest_hl: 30,
  strategy_hl: 60,
  consensus_hl: 90,
})

const simulateDays = ref(30)
const simulateInitScore = ref(8.0)
const activeTab = ref<'weights' | 'decay'>('weights')

// ─── Weight validation ───────────────────────────────────────────────────────

const weightSum = computed(() =>
  Number((weights.interest + weights.strategy + weights.consensus).toFixed(3))
)
const weightValid = computed(() => Math.abs(weightSum.value - 1.0) < 0.001)

// ─── Decay simulation ───────────────────────────────────────────────────────

interface DecayPoint { days: number; interest: number; strategy: number; consensus: number; composite: number }

function calcDecayed(init: number, days: number, halfLife: number): number {
  return init * Math.pow(0.5, days / halfLife)
}

const decayChart = computed<DecayPoint[]>(() => {
  const points: DecayPoint[] = []
  for (let d = 0; d <= simulateDays.value; d += Math.max(1, Math.floor(simulateDays.value / 20))) {
    const intScore = calcDecayed(simulateInitScore.value, d, decayConfig.interest_hl)
    const strScore = calcDecayed(simulateInitScore.value, d, decayConfig.strategy_hl)
    const conScore = calcDecayed(simulateInitScore.value, d, decayConfig.consensus_hl)
    const composite = intScore * weights.interest + strScore * weights.strategy + conScore * weights.consensus
    points.push({ days: d, interest: intScore, strategy: strScore, consensus: conScore, composite })
  }
  return points
})

const lastPoint = computed(() => decayChart.value[decayChart.value.length - 1])

// ─── Preview table (static) ─────────────────────────────────────────────────

const previewDays = [0, 7, 14, 30, 60, 90, 180, 365]

const previewTable = computed(() =>
  previewDays.map(d => ({
    days: d,
    interest: calcDecayed(8.0, d, decayConfig.interest_hl),
    strategy: calcDecayed(8.0, d, decayConfig.strategy_hl),
    consensus: calcDecayed(8.0, d, decayConfig.consensus_hl),
  }))
)

// ─── Normalise weights on blur ───────────────────────────────────────────────

function normaliseWeights() {
  const total = weights.interest + weights.strategy + weights.consensus
  if (total === 0) return
  weights.interest = Number((weights.interest / total).toFixed(3))
  weights.strategy = Number((weights.strategy / total).toFixed(3))
  weights.consensus = Number((consensusWeight.value / total).toFixed(3))
}

// We need a separate ref for consensus to avoid computed/setter conflict
const consensusWeight = ref(0.25)
watch(consensusWeight, (v) => {
  weights.consensus = v
})

// ─── SVG chart ──────────────────────────────────────────────────────────────

const chartPadding = { top: 16, right: 16, bottom: 32, left: 40 }
const chartW = 540
const chartH = 180
const innerW = chartW - chartPadding.left - chartPadding.right
const innerH = chartH - chartPadding.top - chartPadding.bottom

const xScale = computed(() => (days: number) =>
  chartPadding.left + (days / simulateDays.value) * innerW
)
const yScale = computed(() => (v: number) =>
  chartPadding.top + (1 - v / simulateInitScore.value) * innerH
)

const pathInterest = computed(() => {
  const pts = decayChart.value
  if (!pts.length) return ''
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${xScale.value(p.days)} ${yScale.value(p.interest)}`).join(' ')
})
const pathStrategy = computed(() => {
  const pts = decayChart.value
  if (!pts.length) return ''
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${xScale.value(p.days)} ${yScale.value(p.strategy)}`).join(' ')
})
const pathConsensus = computed(() => {
  const pts = decayChart.value
  if (!pts.length) return ''
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${xScale.value(p.days)} ${yScale.value(p.consensus)}`).join(' ')
})
const pathComposite = computed(() => {
  const pts = decayChart.value
  if (!pts.length) return ''
  return pts.map((p, i) => `${i === 0 ? 'M' : 'L'} ${xScale.value(p.days)} ${yScale.value(p.composite)}`).join(' ')
})

// Y-axis ticks
const yTicks = computed(() => {
  const max = simulateInitScore.value
  const step = max / 4
  return Array.from({ length: 5 }, (_, i) => Math.round(step * i * 10) / 10)
})

function saveWeights() {
  console.log('[Settings] Save weights:', { ...weights })
  // TODO: call PATCH /config
}

function saveDecay() {
  console.log('[Settings] Save decay:', { ...decayConfig })
  // TODO: call PATCH /decay/config
}
</script>

<template>
  <div class="settings-view">
    <header class="page-header">
      <h1>⚙️ 系统设置</h1>
      <p class="subtitle">配置评分权重与衰减参数</p>
    </header>

    <!-- Tab switcher -->
    <div class="tab-bar">
      <button :class="['tab', { active: activeTab === 'weights' }]" @click="activeTab = 'weights'">
        天平权重
      </button>
      <button :class="['tab', { active: activeTab === 'decay' }]" @click="activeTab = 'decay'">
        衰减配置
      </button>
    </div>

    <!-- ── Weights tab ───────────────────────────────────────────────── -->
    <section v-if="activeTab === 'weights'" class="section">
      <h2>评分权重分配</h2>
      <p class="section-desc">三个维度的相对重要性，之和必须等于 1.0</p>

      <div class="weight-sliders">
        <div class="slider-row">
          <label>🎯 兴趣分（Interest）</label>
          <input
            type="range"
            v-model.number="weights.interest"
            min="0" max="1" step="0.01"
            @blur="normaliseWeights"
          />
          <span class="value-badge">{{ (weights.interest * 100).toFixed(0) }}%</span>
        </div>

        <div class="slider-row">
          <label>📡 战略分（Strategy）</label>
          <input
            type="range"
            v-model.number="weights.strategy"
            min="0" max="1" step="0.01"
            @blur="normaliseWeights"
          />
          <span class="value-badge">{{ (weights.strategy * 100).toFixed(0) }}%</span>
        </div>

        <div class="slider-row">
          <label>🤝 共识分（Consensus）</label>
          <input
            type="range"
            v-model.number="weights.consensus"
            min="0" max="1" step="0.01"
            @blur="normaliseWeights"
          />
          <span class="value-badge">{{ (weights.consensus * 100).toFixed(0) }}%</span>
        </div>
      </div>

      <div class="weight-sum" :class="{ valid: weightValid, invalid: !weightValid }">
        权重之和：{{ (weightSum * 100).toFixed(1) }}%
        <span v-if="!weightValid">（必须等于 100%）</span>
      </div>

      <!-- Visual bar -->
      <div class="weight-bar-wrap">
        <div class="weight-bar">
          <div class="seg interest" :style="{ flex: weights.interest }" />
          <div class="seg strategy" :style="{ flex: weights.strategy }" />
          <div class="seg consensus" :style="{ flex: weights.consensus }" />
        </div>
        <div class="bar-labels">
          <span>Interest {{ (weights.interest * 100).toFixed(0) }}%</span>
          <span>Strategy {{ (weights.strategy * 100).toFixed(0) }}%</span>
          <span>Consensus {{ (weights.consensus * 100).toFixed(0) }}%</span>
        </div>
      </div>

      <button class="btn-primary" :disabled="!weightValid" @click="saveWeights">
        保存权重配置
      </button>
    </section>

    <!-- ── Decay tab ─────────────────────────────────────────────────── -->
    <section v-if="activeTab === 'decay'" class="section">
      <h2>衰减半衰期配置</h2>
      <p class="section-desc">每个维度多少天衰减到原始分数的一半</p>

      <div class="decay-inputs">
        <div class="decay-row">
          <label>🎯 Interest 半衰期</label>
          <div class="input-group">
            <input type="number" v-model.number="decayConfig.interest_hl" min="1" max="3650" />
            <span class="unit">天</span>
          </div>
        </div>

        <div class="decay-row">
          <label>📡 Strategy 半衰期</label>
          <div class="input-group">
            <input type="number" v-model.number="decayConfig.strategy_hl" min="1" max="3650" />
            <span class="unit">天</span>
          </div>
        </div>

        <div class="decay-row">
          <label>🤝 Consensus 半衰期</label>
          <div class="input-group">
            <input type="number" v-model.number="decayConfig.consensus_hl" min="1" max="3650" />
            <span class="unit">天</span>
          </div>
        </div>
      </div>

      <!-- Decay preview table -->
      <h3 class="sub-title">衰减预览表（初始分 = 8.0）</h3>
      <div class="preview-table-wrap">
        <table class="preview-table">
          <thead>
            <tr>
              <th>天数</th>
              <th>Interest ↓</th>
              <th>Strategy ↓</th>
              <th>Consensus ↓</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="row in previewTable" :key="row.days">
              <td class="days-cell">{{ row.days === 0 ? '今日' : `${row.days}天` }}</td>
              <td>{{ row.interest.toFixed(2) }}</td>
              <td>{{ row.strategy.toFixed(2) }}</td>
              <td>{{ row.consensus.toFixed(2) }}</td>
            </tr>
          </tbody>
        </table>
      </div>

      <!-- Interactive simulation -->
      <h3 class="sub-title">衰减模拟器</h3>
      <div class="sim-controls">
        <label>
          初始分数
          <input type="number" v-model.number="simulateInitScore" min="0.1" max="10" step="0.1" />
        </label>
        <label>
          模拟天数
          <input type="range" v-model.number="simulateDays" min="7" max="365" step="1" />
          <span>{{ simulateDays }}天</span>
        </label>
      </div>

      <!-- SVG Chart -->
      <div class="chart-wrap">
        <svg :width="chartW" :height="chartH" class="decay-chart">
          <!-- Grid lines -->
          <line
            v-for="tick in yTicks"
            :key="tick"
            :x1="chartPadding.left"
            :y1="yScale(tick)"
            :x2="chartW - chartPadding.right"
            :y2="yScale(tick)"
            stroke="var(--border-subtle)"
            stroke-width="1"
            stroke-dasharray="4 4"
          />
          <!-- Y axis labels -->
          <text
            v-for="tick in yTicks"
            :key="tick"
            :x="chartPadding.left - 6"
            :y="yScale(tick) + 4"
            text-anchor="end"
            class="axis-label"
          >{{ tick }}</text>
          <!-- X axis label -->
          <text :x="chartW / 2" :y="chartH - 4" text-anchor="middle" class="axis-label">天数</text>

          <!-- Lines -->
          <path :d="pathInterest" fill="none" stroke="var(--color-interest)" stroke-width="2" />
          <path :d="pathStrategy" fill="none" stroke="var(--color-strategy)" stroke-width="2" />
          <path :d="pathConsensus" fill="none" stroke="var(--color-consensus)" stroke-width="2" />
          <path :d="pathComposite" fill="none" stroke="var(--color-composite)" stroke-width="2.5" stroke-dasharray="6 3" />

          <!-- End dot -->
          <circle
            v-if="lastPoint"
            :cx="xScale(lastPoint.days)"
            :cy="yScale(lastPoint.composite)"
            r="4"
            fill="var(--color-composite)"
          />
        </svg>

        <!-- Legend -->
        <div class="chart-legend">
          <span class="legend-item interest">● Interest</span>
          <span class="legend-item strategy">● Strategy</span>
          <span class="legend-item consensus">● Consensus</span>
          <span class="legend-item composite">◆ Composite</span>
        </div>
      </div>

      <!-- Endpoint readout -->
      <div class="endpoint-readout" v-if="lastPoint">
        <span>第 {{ lastPoint.days }} 天综合分：<strong>{{ lastPoint.composite.toFixed(3) }}</strong></span>
        <span>Interest {{ lastPoint.interest.toFixed(2) }} | Strategy {{ lastPoint.strategy.toFixed(2) }} | Consensus {{ lastPoint.consensus.toFixed(2) }}</span>
      </div>

      <button class="btn-primary" @click="saveDecay">
        保存衰减配置
      </button>
    </section>
  </div>
</template>

<style scoped>
.settings-view {
  max-width: 680px;
  padding: var(--space-6) var(--space-4);
}

.page-header {
  margin-bottom: var(--space-6);
}

.page-header h1 {
  font-size: var(--text-2xl);
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.subtitle {
  color: var(--text-secondary);
  font-size: var(--text-sm);
}

/* Tab bar */
.tab-bar {
  display: flex;
  gap: var(--space-2);
  border-bottom: 1px solid var(--border-subtle);
  margin-bottom: var(--space-6);
}

.tab {
  padding: var(--space-2) var(--space-4);
  border: none;
  background: none;
  color: var(--text-secondary);
  font-size: var(--text-sm);
  cursor: pointer;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  transition: color 0.15s, border-color 0.15s;
}

.tab:hover { color: var(--text-primary); }
.tab.active {
  color: var(--color-primary, var(--accent));
  border-bottom-color: var(--color-primary, var(--accent));
  font-weight: var(--weight-semibold);
}

/* Section */
.section h2 {
  font-size: var(--text-lg);
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.section-desc {
  color: var(--text-secondary);
  font-size: var(--text-sm);
  margin-bottom: var(--space-5);
}

/* Weights */
.weight-sliders {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  margin-bottom: var(--space-4);
}

.slider-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.slider-row label {
  width: 180px;
  font-size: var(--text-sm);
  color: var(--text-primary);
  flex-shrink: 0;
}

.slider-row input[type="range"] {
  flex: 1;
  accent-color: var(--color-primary, var(--accent));
}

.value-badge {
  width: 48px;
  text-align: right;
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--color-primary, var(--accent));
}

.weight-sum {
  font-size: var(--text-sm);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  margin-bottom: var(--space-4);
}

.weight-sum.valid { background: color-mix(in srgb, var(--color-success, #22c55e) 15%, transparent); color: var(--color-success, #22c55e); }
.weight-sum.invalid { background: color-mix(in srgb, var(--color-error, #ef4444) 15%, transparent); color: var(--color-error, #ef4444); }

/* Weight bar */
.weight-bar-wrap { margin-bottom: var(--space-5); }

.weight-bar {
  display: flex;
  height: 24px;
  border-radius: var(--radius-md);
  overflow: hidden;
  border: 1px solid var(--border-subtle);
}

.weight-bar .seg { transition: flex 0.2s; }
.weight-bar .seg.interest { background: var(--color-interest, #8b5cf6); }
.weight-bar .seg.strategy { background: var(--color-strategy, #06b6d4); }
.weight-bar .seg.consensus { background: var(--color-consensus, #f59e0b); }

.bar-labels {
  display: flex;
  justify-content: space-between;
  margin-top: var(--space-1);
  font-size: var(--text-xs);
  color: var(--text-secondary);
}

/* Decay inputs */
.decay-inputs {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  margin-bottom: var(--space-5);
}

.decay-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.decay-row label {
  width: 180px;
  font-size: var(--text-sm);
  flex-shrink: 0;
}

.input-group {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.input-group input {
  width: 80px;
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-primary);
  font-size: var(--text-sm);
}

.unit { color: var(--text-secondary); font-size: var(--text-sm); }

/* Preview table */
.sub-title {
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  color: var(--text-secondary);
  margin-bottom: var(--space-2);
  margin-top: var(--space-5);
}

.preview-table-wrap { overflow-x: auto; margin-bottom: var(--space-4); }

.preview-table {
  width: 100%;
  border-collapse: collapse;
  font-size: var(--text-sm);
}

.preview-table th,
.preview-table td {
  padding: var(--space-1) var(--space-3);
  text-align: right;
  border-bottom: 1px solid var(--border-subtle);
}

.preview-table th { color: var(--text-secondary); font-weight: var(--weight-normal); }
.days-cell { text-align: left; font-weight: var(--weight-medium); }

/* Simulation controls */
.sim-controls {
  display: flex;
  gap: var(--space-5);
  align-items: center;
  margin-bottom: var(--space-4);
}

.sim-controls label {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: var(--text-sm);
  color: var(--text-secondary);
}

.sim-controls input[type="number"] {
  width: 64px;
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-sm);
  background: var(--bg-secondary);
  color: var(--text-primary);
}

.sim-controls input[type="range"] {
  width: 120px;
  accent-color: var(--accent);
}

/* Chart */
.chart-wrap { margin-bottom: var(--space-4); }

.decay-chart { display: block; background: var(--bg-secondary); border-radius: var(--radius-md); }

.axis-label {
  font-size: 11px;
  fill: var(--text-secondary);
}

.chart-legend {
  display: flex;
  gap: var(--space-4);
  margin-top: var(--space-2);
  flex-wrap: wrap;
}

.legend-item {
  font-size: var(--text-xs);
  display: flex;
  align-items: center;
  gap: var(--space-1);
}

.legend-item.interest { color: var(--color-interest, #8b5cf6); }
.legend-item.strategy { color: var(--color-strategy, #06b6d4); }
.legend-item.consensus { color: var(--color-consensus, #f59e0b); }
.legend-item.composite { color: var(--color-composite, #ec4899); }

.endpoint-readout {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
  padding: var(--space-3);
  background: var(--bg-secondary);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  margin-bottom: var(--space-4);
}

.endpoint-readout strong { color: var(--color-composite, #ec4899); font-size: var(--text-lg); }

/* Button */
.btn-primary {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-5);
  background: var(--color-primary, var(--accent));
  color: #fff;
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  font-weight: var(--weight-semibold);
  cursor: pointer;
  transition: opacity 0.15s;
}

.btn-primary:hover:not(:disabled) { opacity: 0.85; }
.btn-primary:disabled { opacity: 0.4; cursor: not-allowed; }
</style>