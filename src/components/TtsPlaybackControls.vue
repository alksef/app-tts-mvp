<template>
  <div class="tts-playback-controls">
    <h3 class="panel-title">Управление</h3>

    <div class="controls-row">
      <div class="playback-buttons">
        <button class="btn-play" @click="handlePlay" :disabled="isPlaying || !hasText" title="Воспроизвести">
          ▶
        </button>
        <button class="btn-stop" @click="handleStop" :disabled="!isPlaying" title="Остановить">
          ■
        </button>
        <button class="btn-clear" @click="handleClear" :disabled="!hasText" title="Очистить">
          🗑
        </button>
      </div>

      <div class="continuous-toggle">
        <span class="toggle-label">Непрерывный</span>
        <button class="toggle" :class="{ active: continuousPlay }" @click="toggleContinuous">
          <span class="slider"></span>
        </button>
      </div>
    </div>

    <div class="status-indicator" :class="{ playing: isPlaying }">
      {{ isPlaying ? '♪ Проигрывается...' : '' }}
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch, nextTick } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useKeyboardStore } from '../stores/keyboard';

const store = useKeyboardStore();
const isProcessingSpeech = ref(false);

// Use store values instead of local state
const isPlaying = computed(() => store.isPlaying);
const continuousPlay = computed(() => store.continuousPlay);

const hasText = computed(() => {
  return store.interceptedText.length > 0;
});

const handlePlay = async () => {
  const text = store.interceptedText;
  if (!text) return;

  try {
    if (continuousPlay.value) {
      const match = text.match(/[.?!]/);
      if (match && match.index !== undefined) {
        const idx = match.index + 1;
        const toSpeak = text.slice(0, idx);
        const remaining = text.slice(idx);
        // Use nextTick to clear field after current render cycle
        await nextTick();
        store.interceptedText = remaining;
        // Non-blocking call - adds to queue
        store.speakTextWithHistory(toSpeak);
      } else {
        // Use nextTick to clear field after current render cycle
        await nextTick();
        store.interceptedText = '';
        // Non-blocking call - adds to queue
        store.speakTextWithHistory(text);
      }
    } else {
      // Use nextTick to clear field after current render cycle
      await nextTick();
      store.interceptedText = '';
      // Non-blocking call - adds to queue
      store.speakTextWithHistory(text);
    }
  } catch (error) {
    console.error('Error in handlePlay:', error);
    // При ошибке возвращаем текст обратно в начало поля
    store.interceptedText = text + store.interceptedText;
  }
};

const handleStop = async () => {
  try {
    await invoke('stop_speech');
    isPlaying.value = false;
  } catch (error) {
    console.error('Failed to stop speech:', error);
  }
};

const handleClear = async () => {
  store.interceptedText = '';
};

const toggleContinuous = async () => {
  const newValue = !store.continuousPlay;
  try {
    await invoke('set_continuous_play', { enabled: newValue });
    // Store will be updated via event
  } catch (error) {
    console.error('Failed to set continuous play:', error);
  }
};

// Автовоспроизведение при вводе знаков препинания в continuous mode
watch(() => store.interceptedText, (newText) => {
  if (!continuousPlay.value || !newText || isPlaying.value || isProcessingSpeech.value) {
    return;
  }

  const match = newText.match(/[.?!]/);
  if (match && match.index !== undefined) {
    const idx = match.index + 1;
    const toSpeak = newText.slice(0, idx);
    const remaining = newText.slice(idx);

    isProcessingSpeech.value = true;

    // Use nextTick to update field after current render cycle
    nextTick(() => {
      store.interceptedText = remaining;
    });

    // Non-blocking call - adds to queue
    store.speakTextWithHistory(toSpeak);

    // Small delay before resetting flag
    setTimeout(() => {
      isProcessingSpeech.value = false;
    }, 100);
  }
});
</script>

<style scoped>
.tts-playback-controls {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 0.875rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.panel-title {
  margin: 0 0 0.75rem 0;
  font-size: 0.875rem;
  font-weight: 600;
  color: #374151;
}

.controls-row {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.continuous-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  background: #f9fafb;
  border-radius: 6px;
}

.toggle-label {
  font-size: 0.75rem;
  color: #374151;
  font-weight: 500;
  white-space: nowrap;
}

.toggle {
  width: 36px;
  height: 20px;
  background: #d1d5db;
  border-radius: 10px;
  padding: 0 2px;
  display: flex;
  align-items: center;
  cursor: pointer;
  transition: background 0.2s ease;
  flex-shrink: 0;
  border: none;
}

.toggle.active {
  background: #22c55e;
  justify-content: flex-end;
}

.slider {
  width: 16px;
  height: 16px;
  background: white;
  border-radius: 50%;
}

.playback-buttons {
  display: flex;
  gap: 0.5rem;
}

.btn-play, .btn-stop, .btn-clear {
  width: 36px;
  height: 36px;
  border-radius: 6px;
  border: none;
  cursor: pointer;
  transition: all 0.2s ease;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  font-size: 1rem;
  line-height: 1;
}

.btn-play {
  background: #3b82f6;
  color: white;
}

.btn-play:hover:not(:disabled) {
  background: #2563eb;
  transform: scale(1.1);
}

.btn-play:disabled {
  background: #9ca3af;
  cursor: not-allowed;
  opacity: 0.5;
}

.btn-stop {
  background: #ef4444;
  color: white;
}

.btn-stop:hover:not(:disabled) {
  background: #dc2626;
  transform: scale(1.1);
}

.btn-stop:disabled {
  background: #fca5a5;
  cursor: not-allowed;
  opacity: 0.5;
}

.btn-clear {
  background: #6b7280;
  color: white;
}

.btn-clear:hover:not(:disabled) {
  background: #4b5563;
  transform: scale(1.1);
}

.btn-clear:disabled {
  background: #d1d5db;
  cursor: not-allowed;
  opacity: 0.5;
}

.status-indicator {
  padding: 0.5rem 0.75rem;
  background: #f3f4f6;
  border-radius: 6px;
  text-align: center;
  font-size: 0.75rem;
  color: #6b7280;
  font-weight: 500;
  transition: all 0.3s ease;
}

.status-indicator.playing {
  background: linear-gradient(135deg, #dbeafe, #bfdbfe);
  color: #2563eb;
  animation: pulse 1.5s ease-in-out infinite;
}

@keyframes pulse {
  0%, 100% {
    opacity: 1;
  }
  50% {
    opacity: 0.7;
  }
}
</style>
