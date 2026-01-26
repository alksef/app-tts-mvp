<template>
  <div class="key-display">
    <h3 class="display-title">Перехваченные клавиши</h3>

    <div class="latest-key-container" v-if="store.hasLatestKey">
      <div class="latest-key">
        <span class="key-label">Последняя:</span>
        <span class="key-value">{{ store.latestKey?.key_name }}</span>
      </div>
    </div>

    <div class="no-key" v-else>
      <p>Пока нет перехваченных клавиш</p>
      <p class="hint">Нажмите Tab+Esc для включения режима блокировки</p>
    </div>

    <div class="keys-list" v-if="store.interceptedKeys.length > 0">
      <div class="list-header">
        <span>Последние клавиши ({{ store.interceptedKeys.length }})</span>
        <button class="clear-button" @click="handleClear">Очистить</button>
      </div>
      <div class="keys-container">
        <div
          class="key-item"
          v-for="(key, index) in displayKeys"
          :key="index"
        >
          <span class="key-name">{{ key.key_name }}</span>
          <span class="key-time">{{ formatTime(key.timestamp) }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import { useKeyboardStore, type KeyEvent } from '../stores/keyboard';

const store = useKeyboardStore();

// Display last 10 keys (most recent first)
const displayKeys = computed(() => {
  return [...store.interceptedKeys].reverse().slice(0, 10);
});

const formatTime = (timestamp: number): string => {
  const date = new Date(timestamp * 1000);
  return date.toLocaleTimeString();
};

const handleClear = async () => {
  await store.clearKeys();
};

onMounted(async () => {
  await store.initialize();
});

onUnmounted(() => {
  store.destroy();
});
</script>

<style scoped>
.key-display {
  padding: 2rem;
  background: white;
  border-radius: 12px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  max-width: 600px;
  margin: 2rem auto;
}

.display-title {
  font-size: 1.25rem;
  font-weight: 700;
  color: #1f2937;
  margin: 0 0 1.5rem 0;
}

.latest-key-container {
  margin-bottom: 2rem;
  padding: 1.5rem;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  border-radius: 8px;
}

.latest-key {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.key-label {
  font-size: 0.875rem;
  font-weight: 600;
  color: rgba(255, 255, 255, 0.8);
  text-transform: uppercase;
}

.key-value {
  font-size: 1.5rem;
  font-weight: 700;
  color: white;
  padding: 0.5rem 1rem;
  background: rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  backdrop-filter: blur(10px);
}

.no-key {
  text-align: center;
  padding: 2rem;
  color: #6b7280;
}

.no-key p {
  margin: 0.5rem 0;
}

.hint {
  font-size: 0.875rem;
  color: #9ca3af;
}

.keys-list {
  margin-top: 1.5rem;
}

.list-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
  padding-bottom: 0.75rem;
  border-bottom: 2px solid #f3f4f6;
}

.list-header span {
  font-size: 0.875rem;
  font-weight: 600;
  color: #6b7280;
}

.clear-button {
  padding: 0.375rem 0.75rem;
  background-color: #ef4444;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: background-color 0.2s ease;
}

.clear-button:hover {
  background-color: #dc2626;
}

.keys-container {
  max-height: 300px;
  overflow-y: auto;
}

.key-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.75rem 1rem;
  background: #f9fafb;
  border-radius: 6px;
  margin-bottom: 0.5rem;
  transition: background-color 0.2s ease;
}

.key-item:hover {
  background: #f3f4f6;
}

.key-name {
  font-weight: 600;
  color: #1f2937;
}

.key-time {
  font-size: 0.875rem;
  color: #6b7280;
}

/* Scrollbar styling */
.keys-container::-webkit-scrollbar {
  width: 6px;
}

.keys-container::-webkit-scrollbar-track {
  background: #f3f4f6;
  border-radius: 3px;
}

.keys-container::-webkit-scrollbar-thumb {
  background: #d1d5db;
  border-radius: 3px;
}

.keys-container::-webkit-scrollbar-thumb:hover {
  background: #9ca3af;
}
</style>
