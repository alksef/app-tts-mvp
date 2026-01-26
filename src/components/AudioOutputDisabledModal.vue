<template>
  <div v-if="show" class="modal-overlay" @click="onOverlayClick">
    <div class="modal-content" @click.stop>
      <div class="warning-icon">⚠️</div>
      <h2>ПРЕДУПРЕЖДЕНИЕ</h2>
      <p>Аудиовывод и виртуальный микрофон выключены</p>
      <p class="secondary">TTS не будет воспроизводиться.</p>
      <p class="secondary">Включите хотя бы один вывод.</p>
      <button @click="onOk" class="ok-button">Ок</button>
    </div>
  </div>
</template>

<script setup lang="ts">
const props = defineProps<{
  show: boolean
}>()

const emit = defineEmits<{
  close: []
}>()

function onOk() {
  emit('close')
}

function onOverlayClick() {
  // Don't close on overlay click - user must click Ok
  // This ensures they acknowledge the warning
}
</script>

<style scoped>
.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.7);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-content {
  background: var(--bg-secondary);
  padding: 2rem;
  border-radius: 12px;
  text-align: center;
  min-width: 320px;
  max-width: 400px;
}

.warning-icon {
  font-size: 3rem;
  margin-bottom: 1rem;
}

h2 {
  margin: 0 0 1rem 0;
  color: var(--color-warning);
  font-size: 1.2rem;
}

p {
  margin: 0.5rem 0;
  color: var(--text-primary);
}

p.secondary {
  color: var(--text-muted);
  font-size: 0.9rem;
}

.ok-button {
  margin-top: 1.5rem;
  padding: 0.75rem 2rem;
  background: var(--color-primary);
  color: white;
  border: none;
  border-radius: 8px;
  cursor: pointer;
  font-size: 1rem;
}

.ok-button:hover {
  opacity: 0.9;
}
</style>
