<template>
  <div class="status-indicator">
    <div class="indicator-container">
      <div
        class="status-dot"
        :class="{ 'active': store.blockingEnabled }"
      ></div>
      <div class="status-info">
        <h2 class="status-title">Перехват клавиатуры</h2>
        <p class="shortcut-label">Win + Esc</p>
      </div>
    </div>

    <button
      class="toggle-button"
      @click="handleToggle"
      :disabled="isToggling"
    >
      {{ isToggling ? 'Переключение...' : 'Включить / Выключить' }}
    </button>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { useKeyboardStore } from '../stores/keyboard';

const store = useKeyboardStore();
const isToggling = ref(false);

const handleToggle = async () => {
  isToggling.value = true;
  await store.toggleBlocking();
  isToggling.value = false;
};

onMounted(async () => {
  await store.initialize();
});

onUnmounted(() => {
  store.destroy();
});
</script>

<style scoped>
.status-indicator {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.75rem 1rem;
  background: rgba(255, 255, 255, 0.95);
  border-radius: 8px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.indicator-container {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.status-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background-color: #22c55e;
  transition: background-color 0.3s ease;
  box-shadow: 0 2px 8px rgba(34, 197, 94, 0.3);
  flex-shrink: 0;
}

.status-dot.active {
  background-color: #ef4444;
  box-shadow: 0 2px 8px rgba(239, 68, 68, 0.3);
}

.status-info {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.status-title {
  font-size: 0.75rem;
  font-weight: 700;
  color: #374151;
  margin: 0;
  white-space: nowrap;
}

.shortcut-label {
  font-size: 0.7rem;
  font-weight: 500;
  color: #6b7280;
  margin: 0;
}

.status-text {
  display: none;
}

.status-hint {
  display: none;
}

.toggle-button {
  width: 100%;
  padding: 0.5rem 0.75rem;
  background-color: #3b82f6;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 0.75rem;
  font-weight: 600;
  cursor: pointer;
  transition: background-color 0.2s ease;
  text-align: center;
}

.toggle-button:hover:not(:disabled) {
  background-color: #2563eb;
}

.toggle-button:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}
</style>
