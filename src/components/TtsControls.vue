<template>
  <div class="tts-controls">
    <div class="controls-row">
      <!-- Play/Stop buttons -->
      <button class="btn-play" @click="handlePlay" :disabled="isPlaying || !hasText">
        {{ isPlaying ? '♪ Проигрывается...' : '▶ Воспроизвести' }}
      </button>
      <button class="btn-stop" @click="handleStop" :disabled="!isPlaying">
        ■ Стоп
      </button>
      <button class="btn-clear" @click="handleClear" :disabled="!hasText">
        Очистить
      </button>
    </div>

    <div class="controls-row">
      <!-- Continuous mode toggle -->
      <span class="label">Непрерывный</span>
      <button class="toggle" :class="{ active: continuousPlay }" @click="toggleContinuous">
        <span class="slider"></span>
      </button>
    </div>

    <div class="controls-row">
      <!-- TTS provider selector -->
      <span class="label">Голос</span>
      <select class="provider-select" v-model="selectedProvider" @change="handleProviderChange">
        <option value="system">Система</option>
        <option value="openai">OpenAI</option>
      </select>
    </div>

    <!-- OpenAI API key input (только когда выбран OpenAI) -->
    <div class="controls-row" v-if="selectedProvider === 'openai'">
      <span class="label">API ключ</span>
      <input
        type="password"
        class="api-key-input"
        placeholder="sk-..."
        v-model="openaiKey"
        @blur="saveApiKey"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useKeyboardStore } from '../stores/keyboard';

const store = useKeyboardStore();
const openaiKey = ref('');
let isProcessingSpeech = ref(false); // Флаг для предотвращения повторного срабатывания

// Use store values instead of local state
const isPlaying = computed(() => store.isPlaying);
const continuousPlay = computed(() => store.continuousPlay);
const selectedProvider = computed({
  get: () => store.ttsProvider,
  set: (val: 'system' | 'openai') => {
    // Provider will be updated via handleProviderChange
  }
});

const hasText = computed(() => {
  return store.interceptedText.length > 0;
});

const fetchStatus = async () => {
  try {
    const status = await invoke<{ is_speaking: boolean; provider: string; continuous_play: boolean }>('get_tts_status');
    // Store will be updated via events, just sync OpenAI key here if needed
  } catch (error) {
    console.error('Failed to fetch TTS status:', error);
  }
};

const handlePlay = async () => {
  const text = store.interceptedText;
  if (!text) return;

  try {
    // В режиме continuous play произносим текст до первого знака препинания
    if (continuousPlay.value) {
      const match = text.match(/[.?!]/);
      if (match && match.index !== undefined) {
        const idx = match.index + 1;
        const toSpeak = text.slice(0, idx);
        const remaining = text.slice(idx);
        // Сразу обновляем поле - оставляем только остаток
        store.interceptedText = remaining;
        // Non-blocking call - adds to queue
        store.speakTextWithHistory(toSpeak);
      } else {
        // Нет знака препинания - сразу очищаем поле
        store.interceptedText = '';
        // Non-blocking call - adds to queue
        store.speakTextWithHistory(text);
      }
    } else {
      // Обычный режим - сразу очищаем поле
      store.interceptedText = '';
      // Non-blocking call - adds to queue
      store.speakTextWithHistory(text);
    }
  } catch (error) {
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

const handleProviderChange = async () => {
  try {
    await invoke('set_tts_provider', { provider: store.ttsProvider });
    // Store will be updated via event
  } catch (error) {
    console.error('Failed to set TTS provider:', error);
  }
};

const loadOpenAIKey = async () => {
  try {
    const config = await invoke<{ api_key: string | null }>('get_openai_config');
    openaiKey.value = config.api_key || '';
  } catch (error) {
    console.error('Failed to load OpenAI key:', error);
  }
};

const saveApiKey = async () => {
  try {
    await invoke('set_openai_key', { key: openaiKey.value });
  } catch (error) {
    console.error('Failed to save API key:', error);
  }
};

// Автовоспроизведение при вводе знаков препинания в continuous mode
watch(() => store.interceptedText, (newText) => {
  if (!continuousPlay.value || !newText || isPlaying.value || isProcessingSpeech.value) return;

  // Проверяем, есть ли в тексте знаки препинания
  const match = newText.match(/[.?!]/);
  if (match && match.index !== undefined) {
    const idx = match.index + 1;
    const toSpeak = newText.slice(0, idx);
    const remaining = newText.slice(idx);

    // Устанавливаем флаг, чтобы предотвратить повторное срабатывание
    isProcessingSpeech.value = true;

    // Сразу обновляем поле - оставляем только остаток
    store.interceptedText = remaining;

    // Non-blocking call - adds to queue
    store.speakTextWithHistory(toSpeak);

    // Небольшая задержка перед сбросом флага
    setTimeout(() => {
      isProcessingSpeech.value = false;
    }, 100);
  }
});

onMounted(() => {
  fetchStatus();
  loadOpenAIKey();

  // Listen for config changes to reload OpenAI key
  listen('tts_config_changed', () => {
    loadOpenAIKey();
  });
});
</script>

<style scoped>
.tts-controls {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 1.5rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  max-width: 600px;
  margin: 1rem auto;
}

.controls-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.controls-row:last-child {
  margin-bottom: 0;
}

.label {
  font-size: 0.875rem;
  color: #374151;
  min-width: 80px;
  font-weight: 500;
}

.btn-play, .btn-stop, .btn-clear {
  padding: 0.5rem 1rem;
  border-radius: 8px;
  border: none;
  font-weight: 600;
  cursor: pointer;
  transition: all 0.2s ease;
  font-size: 0.875rem;
}

.btn-play {
  background: #3b82f6;
  color: white;
  flex: 1;
}

.btn-play:hover:not(:disabled) {
  background: #2563eb;
}

.btn-play:disabled {
  background: #9ca3af;
  cursor: not-allowed;
}

.btn-stop {
  background: #ef4444;
  color: white;
  flex: 1;
}

.btn-stop:hover:not(:disabled) {
  background: #dc2626;
}

.btn-stop:disabled {
  background: #fca5a5;
  cursor: not-allowed;
}

.btn-clear {
  background: #6b7280;
  color: white;
  flex: 1;
}

.btn-clear:hover:not(:disabled) {
  background: #4b5563;
}

.btn-clear:disabled {
  background: #d1d5db;
  cursor: not-allowed;
}

.toggle {
  width: 36px;
  height: 20px;
  background: #d1d5db;
  border-radius: 10px;
  position: relative;
  cursor: pointer;
  transition: background 0.2s ease;
  border: none;
  flex-shrink: 0;
}

.toggle:hover {
  background: #9ca3af;
}

.toggle.active {
  background: #667eea;
}

.toggle.active:hover {
  background: #5a67d8;
}

.slider {
  width: 16px;
  height: 16px;
  background: white;
  border-radius: 50%;
  position: absolute;
  top: 2px;
  left: 2px;
  transition: transform 0.2s ease;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.toggle.active .slider {
  transform: translateX(16px);
}

.provider-select {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  background: white;
  cursor: pointer;
}

.provider-select:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.api-key-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
}

.api-key-input:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

@media (max-width: 640px) {
  .tts-controls {
    padding: 1rem;
  }

  .controls-row {
    flex-wrap: wrap;
  }

  .label {
    min-width: 70px;
  }

  .btn-play, .btn-stop, .btn-clear {
    font-size: 0.75rem;
    padding: 0.4rem 0.75rem;
  }
}
</style>
