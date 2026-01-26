<template>
  <div class="tts-history-panel">
    <div class="panel-header">
      <h3 class="panel-title">История TTS</h3>
      <button @click="handleClearHistory" class="clear-btn" title="Очистить завершённые">
        🗑️
      </button>
    </div>

    <div class="history-list">
      <div
        v-for="message in keyboardStore.ttsHistory"
        :key="message.id"
        class="history-item"
        :class="{
          'playing': message.status === 'playing',
          'queued': message.status === 'queued',
          'completed': message.status === 'completed',
          'locked': message.locked
        }"
      >
        <!-- Status icon and content -->
        <div class="item-main">
          <span class="status-icon">
            <span v-if="message.status === 'playing'">🔊</span>
            <span v-else-if="message.status === 'queued'">⏳</span>
            <span v-else>✓</span>
          </span>

          <div class="item-content">
            <div class="item-text">{{ message.text }}</div>
            <div class="item-meta">
              <span class="item-time">{{ keyboardStore.formatTimestamp(message.timestamp) }}</span>
              <span v-if="message.status === 'playing'" class="item-status">Воспроизводится</span>
              <span v-else-if="message.status === 'queued'" class="item-status">В очереди</span>
              <span v-if="message.locked" class="item-locked" title="Заблокировано от удаления">🔒</span>
            </div>
          </div>
        </div>

        <!-- Action buttons -->
        <div class="item-actions">
          <button
            @click="handleRepeat(message.id)"
            class="action-btn repeat-btn"
            title="Повторить"
            :disabled="message.status === 'playing'"
          >
            🔁
          </button>
          <button
            @click="handleToggleLock(message.id)"
            class="action-btn lock-btn"
            :class="{ active: message.locked }"
            :title="message.locked ? 'Разблокировать' : 'Заблокировать от удаления'"
          >
            {{ message.locked ? '🔒' : '🔓' }}
          </button>
          <button
            @click="handleDelete(message.id)"
            class="action-btn delete-btn"
            title="Удалить"
            :disabled="message.status === 'playing' || message.locked"
          >
            ✕
          </button>
        </div>
      </div>

      <div v-if="keyboardStore.ttsHistory.length === 0" class="empty-state">
        <p>История пуста</p>
        <small>Отправьте текст на воспроизведение</small>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { onMounted } from 'vue';
import { useKeyboardStore, type TtsMessage } from '../stores/keyboard';

const keyboardStore = useKeyboardStore();

const handleClearHistory = async () => {
  await keyboardStore.clearTtsHistory();
};

const handleRepeat = async (id: string) => {
  await keyboardStore.repeatTtsMessage(id);
};

const handleToggleLock = async (id: string) => {
  await keyboardStore.toggleTtsMessageLocked(id);
};

const handleDelete = async (id: string) => {
  await keyboardStore.deleteTtsMessage(id);
};

onMounted(async () => {
  await keyboardStore.fetchTtsHistory();
  // Setup event listeners for real-time updates
  keyboardStore.setupTtsEventListeners();
});
</script>

<style scoped>
.tts-history-panel {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 1rem;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  display: flex;
  flex-direction: column;
  height: 100%;
  max-height: 600px;
}

.panel-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 1rem;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid #e5e7eb;
}

.panel-title {
  margin: 0;
  font-size: 1.125rem;
  font-weight: 600;
  color: #374151;
}

.clear-btn {
  background: none;
  border: none;
  font-size: 1.25rem;
  cursor: pointer;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  transition: background 0.2s;
}

.clear-btn:hover {
  background: #fee2e2;
}

.history-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding-right: 0.5rem;
}

.history-list::-webkit-scrollbar {
  width: 6px;
}

.history-list::-webkit-scrollbar-track {
  background: #f1f5f9;
  border-radius: 3px;
}

.history-list::-webkit-scrollbar-thumb {
  background: #cbd5e1;
  border-radius: 3px;
}

.history-list::-webkit-scrollbar-thumb:hover {
  background: #94a3b8;
}

.history-item {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.75rem;
  background: #f9fafb;
  border-radius: 8px;
  border: 1px solid #e5e7eb;
  transition: all 0.2s;
}

.history-item.playing {
  background: linear-gradient(135deg, #dbeafe, #bfdbfe);
  border-color: #3b82f6;
  border-left: 4px solid #3b82f6;
  box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.2);
  animation: pulse-blue 1.5s ease-in-out infinite;
}

@keyframes pulse-blue {
  0%, 100% {
    box-shadow: 0 0 0 2px rgba(59, 130, 246, 0.2);
  }
  50% {
    box-shadow: 0 0 0 4px rgba(59, 130, 246, 0.4), 0 0 12px rgba(59, 130, 246, 0.3);
  }
}

.history-item.playing .status-icon {
  animation: bounce 0.6s ease-in-out infinite;
}

@keyframes bounce {
  0%, 100% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.2);
  }
}

.history-item.queued {
  background: linear-gradient(135deg, #fef3c7, #fde68a);
  border-color: #f59e0b;
}

.history-item.completed {
  background: #f9fafb;
  border-color: #e5e7eb;
}

.history-item.locked {
  border-left: 3px solid #8b5cf6;
}

.item-main {
  flex: 1;
  display: flex;
  gap: 0.5rem;
  min-width: 0;
}

.status-icon {
  font-size: 1rem;
  line-height: 1.5;
  flex-shrink: 0;
}

.item-content {
  flex: 1;
  min-width: 0;
}

.item-text {
  font-size: 0.875rem;
  color: #1f2937;
  word-break: break-word;
  line-height: 1.4;
  margin-bottom: 0.25rem;
}

.item-meta {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.75rem;
  color: #6b7280;
  flex-wrap: wrap;
}

.item-status {
  font-weight: 500;
}

.playing .item-status {
  color: #2563eb;
  font-weight: 600;
  text-transform: uppercase;
  font-size: 0.7rem;
  letter-spacing: 0.05em;
}

.queued .item-status {
  color: #f59e0b;
}

.item-locked {
  color: #8b5cf6;
}

.item-actions {
  display: flex;
  flex-direction: column;
  gap: 2px;
  flex-shrink: 0;
}

.action-btn {
  background: white;
  border: 1px solid #d1d5db;
  border-radius: 3px;
  padding: 0;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.2s;
  min-width: 15px;
  width: 15px;
  height: 15px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.action-btn:hover:not(:disabled) {
  background: #f3f4f6;
  border-color: #9ca3af;
}

.action-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.repeat-btn:hover:not(:disabled) {
  background: #dbeafe;
  border-color: #3b82f6;
}

.lock-btn.active {
  background: #ede9fe;
  border-color: #8b5cf6;
}

.delete-btn:hover:not(:disabled) {
  background: #fee2e2;
  border-color: #ef4444;
}

.empty-state {
  text-align: center;
  padding: 2rem;
  color: #9ca3af;
}

.empty-state p {
  margin: 0 0 0.5rem 0;
  font-size: 1rem;
}

.empty-state small {
  font-size: 0.875rem;
}
</style>
