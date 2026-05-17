<script setup lang="ts">
import { ref, watch } from 'vue'
import type { Insight, Maturity } from '@/stores/insights'

const props = defineProps<{
  show: boolean
  insight?: Insight | null
}>()

const emit = defineEmits<{
  close: []
  save: [data: { content: string; maturity: Maturity; entity_title: string; entity_id: string }]
}>()

const content = ref('')
const maturity = ref<Maturity>('seed')
const entity_title = ref('')
const entity_id = ref('')

watch(() => props.insight, (i) => {
  if (i) {
    content.value = i.content
    maturity.value = i.maturity
    entity_title.value = (i as any).entity_title || ''
    entity_id.value = i.entity_id
  } else {
    content.value = ''
    maturity.value = 'seed'
    entity_title.value = ''
    entity_id.value = ''
  }
}, { immediate: true })

function handleSave() {
  if (!content.value.trim()) return
  emit('save', {
    content: content.value.trim(),
    maturity: maturity.value,
    entity_title: entity_title.value.trim() || '未命名实体',
    entity_id: entity_id.value.trim() || String(Date.now()),
  })
  emit('close')
}
</script>

<template>
  <Teleport to="body">
    <div v-if="show" class="modal-overlay" @click.self="emit('close')">
      <div class="modal">
        <div class="modal-header">
          <h3>{{ props.insight ? '编辑洞察' : '新建洞察' }}</h3>
          <button class="close-btn" @click="emit('close')">✕</button>
        </div>
        <div class="modal-body">
          <label class="field">
            <span>实体名称</span>
            <input v-model="entity_title" type="text" placeholder="关联实体..." />
          </label>
          <label class="field">
            <span>内容</span>
            <textarea v-model="content" rows="4" placeholder="洞察内容..."></textarea>
          </label>
          <label class="field">
            <span>成熟度</span>
            <select v-model="maturity">
              <option value="seed">种子</option>
              <option value="sprout">萌芽</option>
              <option value="bud">花苞</option>
              <option value="bloom">绽放</option>
              <option value="ripe">成熟</option>
            </select>
          </label>
        </div>
        <div class="modal-footer">
          <button class="btn-cancel" @click="emit('close')">取消</button>
          <button class="btn-save" @click="handleSave">保存</button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.4);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal {
  background: var(--bg-primary);
  border-radius: var(--radius-xl);
  width: 480px;
  max-width: 90vw;
  box-shadow: var(--shadow-lg);
  overflow: hidden;
}

.modal-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-color);
}

.modal-header h3 {
  font-size: var(--text-md);
  font-weight: var(--weight-semibold);
  margin: 0;
}

.close-btn {
  background: none;
  border: none;
  font-size: 16px;
  cursor: pointer;
  color: var(--text-muted);
  padding: 4px;
}

.modal-body {
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.field span {
  font-size: var(--text-sm);
  font-weight: var(--weight-medium);
  color: var(--text-secondary);
}

.field input,
.field textarea,
.field select {
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  background: var(--bg-primary);
  color: var(--text-primary);
  font-family: inherit;
  outline: none;
  transition: border-color var(--transition-fast);
}

.field input:focus,
.field textarea:focus,
.field select:focus {
  border-color: var(--color-brand);
}

.field textarea { resize: vertical; min-height: 80px; }

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
  padding: var(--space-4) var(--space-5);
  border-top: 1px solid var(--border-color);
}

.btn-cancel {
  padding: var(--space-2) var(--space-4);
  background: var(--bg-tertiary);
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  color: var(--text-secondary);
  cursor: pointer;
}

.btn-save {
  padding: var(--space-2) var(--space-4);
  background: var(--color-brand);
  border: none;
  border-radius: var(--radius-md);
  font-size: var(--text-sm);
  color: var(--text-inverse);
  cursor: pointer;
  font-weight: var(--weight-medium);
}
</style>
