<template>
  <div class="tts-settings">
    <div class="settings-header">
      <div class="tabs">
        <button
          class="tab"
          :class="{ active: activeTab === 'openai' }"
          @click="activeTab = 'openai'"
        >
          OpenAI TTS
        </button>
        <button
          class="tab"
          :class="{ active: activeTab === 'system' }"
          @click="activeTab = 'system'"
        >
          Системный
        </button>
        <button
          class="tab"
          :class="{ active: activeTab === 'localhost' }"
          @click="activeTab = 'localhost'"
        >
          Local host
        </button>
      </div>
    </div>

    <!-- Системный TTS -->
    <div class="tab-content" v-if="activeTab === 'system'">
      <div class="warning-banner">
        <span class="warning-icon">⚠️</span>
        <div class="warning-content">
          <span class="warning-title">Ограничения системного TTS</span>
          <div class="warning-list">
            <span class="warning-text">• Из-за особенностей реализации Windows SAPI возможны кратковременные фризы UI при воспроизведении.</span>
            <span class="warning-text">• Выбор устройства вывода недоступен — воспроизведение всегда идёт на устройстве по умолчанию.</span>
          </div>
        </div>
      </div>

      <div class="setting-row">
        <span class="label">Голос</span>
        <select class="voice-select" v-model="selectedVoice" @change="handleVoiceChange">
          <option v-for="voice in availableVoices" :key="voice.id" :value="voice.id">
            {{ voice.name }}
          </option>
        </select>
      </div>

      <div class="setting-row">
        <span class="label">Скорость</span>
        <div class="slider-control">
          <input
            type="range"
            class="slider-input"
            v-model.number="rate"
            min="-10"
            max="10"
            step="1"
            @input="handleRateChange"
          />
          <span class="slider-value">{{ rateValue }}</span>
        </div>
      </div>

      <div class="setting-row">
        <span class="label">Высота тона</span>
        <div class="slider-control">
          <input
            type="range"
            class="slider-input"
            v-model.number="pitch"
            min="-10"
            max="10"
            step="1"
            @input="handlePitchChange"
          />
          <span class="slider-value">{{ pitchValue }}</span>
        </div>
      </div>

      <div class="setting-row">
        <span class="label">Громкость</span>
        <div class="slider-control">
          <input
            type="range"
            class="slider-input"
            v-model.number="volume"
            min="0"
            max="100"
            step="1"
            @input="handleVolumeChange"
          />
          <span class="slider-value">{{ volume }}%</span>
        </div>
      </div>
    </div>

    <!-- OpenAI TTS -->
    <div class="tab-content openai-tab" v-else-if="activeTab === 'openai'">
      <!-- Первая строка: токен + прокси -->
      <div class="setting-row">
        <span class="label">API ключ</span>
        <div class="token-input-wrapper">
          <input
            :type="openaiApiKeyVisible ? 'text' : 'password'"
            class="text-input"
            placeholder="sk-..."
            v-model="openaiApiKey"
            @blur="saveApiKey"
          />
          <button
            class="btn-toggle-visibility"
            @click="openaiApiKeyVisible = !openaiApiKeyVisible"
            :title="openaiApiKeyVisible ? 'Скрыть' : 'Показать'"
          >
            {{ openaiApiKeyVisible ? '👁️' : '👁️‍🗨️' }}
          </button>
        </div>
      </div>

      <div class="setting-row">
        <span class="label">Прокси</span>
        <input
          type="text"
          class="text-input proxy-host"
          placeholder="localhost"
          v-model="openaiProxyHost"
          title="Прокси только для запросов в openai. Если задан хост и порт, то он используется"
        />
        <input
          type="number"
          class="text-input proxy-port"
          placeholder="8080"
          v-model.number="openaiProxyPort"
          @blur="saveProxy"
          title="Прокси только для запросов в openai. Если задан хост и порт, то он используется"
        />
      </div>

      <!-- Вторая строка: модель + голос -->
      <div class="setting-row">
        <span class="label">Модель</span>
        <select class="select-input" v-model="openaiModel" disabled>
          <option value="gpt-4o-mini-tts">gpt-4o-mini-tts</option>
        </select>
        <span class="label voice-label">Голос</span>
        <select class="select-input voice-select" v-model="openaiVoice" @change="saveVoice">
          <option v-for="voice in openaiVoices" :key="voice.id" :value="voice.id">
            {{ voice.name }}
          </option>
        </select>
      </div>

      <!-- Ползунок скорости -->
      <div class="setting-row">
        <span class="label">Скорость</span>
        <div class="slider-control">
          <input
            type="range"
            class="slider-input"
            v-model.number="openaiSpeed"
            min="0.25"
            max="4.0"
            step="0.25"
            @input="saveSpeed"
          />
          <span class="slider-value">{{ speedValue }}</span>
        </div>
      </div>

      <!-- Инструкция -->
      <div class="setting-row instruction-row">
        <span class="label">Инструкция</span>
        <textarea
          class="textarea-input"
          placeholder="Опишите как должен звучать голос (эмоции, тон, акцент и т.д.)"
          v-model="openaiInstructions"
          @blur="saveInstructions"
          rows="3"
        ></textarea>
      </div>
    </div>

    <!-- Localhost TTS -->
    <div class="tab-content localhost-tab" v-else-if="activeTab === 'localhost'">
      <!-- Порт -->
      <div class="setting-row">
        <span class="label">Порт</span>
        <input
          type="number"
          class="text-input"
          placeholder="8080"
          v-model="localhostPort"
          @blur="saveLocalhostPort"
        />
      </div>

      <!-- Токен -->
      <div class="setting-row">
        <span class="label">Токен</span>
        <div class="token-input-wrapper">
          <input
            :type="localhostTokenVisible ? 'text' : 'password'"
            class="text-input token-input"
            placeholder="опционально"
            v-model="localhostToken"
            @blur="saveLocalhostToken"
          />
          <button
            class="btn-toggle-visibility"
            @click="localhostTokenVisible = !localhostTokenVisible"
            :title="localhostTokenVisible ? 'Скрыть' : 'Показать'"
          >
            {{ localhostTokenVisible ? '👁️' : '👁️‍🗨️' }}
          </button>
        </div>
      </div>

      <!-- Голоса -->
      <div class="setting-row">
        <span class="label">Голос</span>
        <select
          class="voice-select"
          v-model="localhostVoice"
          @change="saveLocalhostVoice"
        >
          <option value="">Не выбран</option>
          <option v-for="voice in localhostVoices" :key="voice.code" :value="voice.code">
            {{ voice.name }}
          </option>
        </select>
        <button
          class="btn-refresh"
          @click="refreshLocalhostVoices"
          :disabled="localhostRefreshing"
          title="Обновить список голосов"
        >
          <span v-if="localhostRefreshing">↻</span>
          <span v-else>↻</span>
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useKeyboardStore } from '../stores/keyboard';

const keyboardStore = useKeyboardStore();

interface TtsStatus {
  is_speaking: boolean;
  provider: string;
  continuous_play: boolean;
  has_openai_key: boolean;
}

interface Voice {
  id: string;
  name: string;
}

interface OpenAIVoice {
  id: string;
  name: string;
}

interface LocalhostVoice {
  code: string;
  name: string;
}

const activeTab = ref<'system' | 'openai' | 'localhost'>('openai');
const selectedProvider = ref<'system' | 'openai' | 'localhost'>('system');
const availableVoices = ref<Voice[]>([]);
const selectedVoice = ref('');
const rate = ref(0);
const pitch = ref(0);
const volume = ref(100);

// OpenAI state - separate refs for inputs to avoid reactivity issues
const openaiApiKey = ref('');
const openaiApiKeyVisible = ref(false);
const openaiProxyHost = ref('');
const openaiProxyPort = ref<number | null>(null);
const openaiModel = ref('gpt-4o-mini-tts');
const openaiVoice = ref('alloy');
const openaiSpeed = ref(1.0);
const openaiInstructions = ref('');
const openaiVoices = ref<OpenAIVoice[]>([]);

// Localhost state
const localhostPort = ref<number | null>(null);
const localhostToken = ref('');
const localhostTokenVisible = ref(false);
const localhostVoice = ref('');
const localhostVoices = ref<LocalhostVoice[]>([]);
const localhostRefreshing = ref(false);

// Track if config has been loaded
let openaiConfigLoaded = false;
let localhostConfigLoaded = false;

const rateValue = computed(() => {
  const val = rate.value;
  if (val === 0) return 'Норм';
  if (val < 0) return `${val} (медленно)`;
  return `+${val} (быстро)`;
});

const pitchValue = computed(() => {
  const val = pitch.value;
  if (val === 0) return 'Норм';
  if (val < 0) return `${val} (ниже)`;
  return `+${val} (выше)`;
});

const speedValue = computed(() => {
  return openaiSpeed.value.toFixed(2);
});

const fetchStatus = async () => {
  try {
    const status = await invoke<TtsStatus>('get_tts_status');
    selectedProvider.value = status.provider as 'system' | 'openai';
    // Синхронизируем activeTab с текущим провайдером
    activeTab.value = status.provider as 'system' | 'openai';
  } catch (error) {
    console.error('Failed to fetch TTS status:', error);
  }
};

const fetchVoices = async () => {
  console.log('[TtsSettings] Fetching voices...');
  try {
    const voices = await invoke<Voice[]>('get_system_voices');
    console.log('[TtsSettings] Got voices:', voices);
    availableVoices.value = voices;
    if (voices.length > 0 && !selectedVoice.value) {
      console.log('[TtsSettings] Selecting first voice:', voices[0].id);
      selectedVoice.value = voices[0].id;
    }
  } catch (error) {
    console.error('[TtsSettings] Failed to fetch voices:', error);
  }
};

// OpenAI functions
const loadOpenAIConfig = async () => {
  // Don't reload if already loaded (prevents overwriting user input)
  if (openaiConfigLoaded) return;

  try {
    const config = await invoke<{
      api_key: string | null;
      proxy_host: string | null;
      proxy_port: number | null;
      model: string;
      voice: string;
      speed: number;
      instructions: string | null;
    }>('get_openai_config');
    openaiApiKey.value = config.api_key || '';
    openaiProxyHost.value = config.proxy_host || '';
    openaiProxyPort.value = config.proxy_port;
    openaiModel.value = config.model;
    openaiVoice.value = config.voice;
    openaiSpeed.value = config.speed;
    openaiInstructions.value = config.instructions || '';
    openaiConfigLoaded = true;
  } catch (error) {
    console.error('Failed to load OpenAI config:', error);
    // Set defaults on error
    openaiProxyHost.value = '';
    openaiConfigLoaded = true;
  }
};

const loadOpenAIVoices = async () => {
  try {
    const voices = await invoke<OpenAIVoice[]>('get_openai_voices');
    openaiVoices.value = voices;
  } catch (error) {
    console.error('Failed to load OpenAI voices:', error);
  }
};

const saveApiKey = async () => {
  try {
    await invoke('set_openai_key', { key: openaiApiKey.value });
  } catch (error) {
    console.error('Failed to save API key:', error);
  }
};

const saveProxy = async () => {
  try {
    await invoke('set_openai_proxy', {
      host: openaiProxyHost.value || null,
      port: openaiProxyPort.value || null
    });
  } catch (error) {
    console.error('Failed to save proxy:', error);
  }
};

const saveVoice = async () => {
  try {
    await invoke('set_openai_voice', { voice: openaiVoice.value });
  } catch (error) {
    console.error('Failed to save voice:', error);
  }
};

const saveSpeed = async () => {
  try {
    await invoke('set_openai_speed', { speed: openaiSpeed.value });
  } catch (error) {
    console.error('Failed to save speed:', error);
  }
};

const saveInstructions = async () => {
  try {
    await invoke('set_openai_instructions', { instructions: openaiInstructions.value });
  } catch (error) {
    console.error('Failed to save instructions:', error);
  }
};

// Localhost functions
const loadLocalhostConfig = async () => {
  // Don't reload if already loaded (prevents overwriting user input)
  if (localhostConfigLoaded) return;

  try {
    const config = await invoke<{
      port: string | null;
      token: string | null;
      voice: string | null;
      connected: boolean;
    }>('get_localhost_config');
    localhostPort.value = config.port ? parseInt(config.port, 10) : null;
    localhostToken.value = config.token || '';
    localhostVoice.value = config.voice || '';
    localhostConfigLoaded = true;
    return config.port ? parseInt(config.port, 10) : null;
  } catch (error) {
    console.error('Failed to load Localhost config:', error);
    // Set defaults on error
    localhostConfigLoaded = true;
    return null;
  }
};

const loadLocalhostVoices = async () => {
  try {
    const voices = await invoke<LocalhostVoice[]>('get_localhost_voices');
    localhostVoices.value = voices;
  } catch (error) {
    console.error('Failed to load Localhost voices:', error);
  }
};

const saveLocalhostPort = async () => {
  try {
    await invoke('set_localhost_port', { port: localhostPort.value ?? 0 });
    // Очищаем список голосов в UI после изменения порта
    localhostVoices.value = [];
  } catch (error) {
    console.error('Failed to save port:', error);
  }
};

const saveLocalhostToken = async () => {
  try {
    await invoke('set_localhost_token', { token: localhostToken.value });
  } catch (error) {
    console.error('Failed to save token:', error);
  }
};

const saveLocalhostVoice = async () => {
  try {
    await invoke('set_localhost_voice', { voice: localhostVoice.value || null });
  } catch (error) {
    console.error('Failed to save voice:', error);
  }
};

const refreshLocalhostVoices = async () => {
  localhostRefreshing.value = true;
  try {
    const voices = await invoke<LocalhostVoice[]>('refresh_localhost_voices');
    localhostVoices.value = voices;
  } catch (error) {
    const errorMsg = error instanceof Error ? error.message : String(error);
    console.error('Failed to refresh voices:', error);
    keyboardStore.showToast(errorMsg, 'error');
    localhostVoices.value = [];
  } finally {
    localhostRefreshing.value = false;
  }
};

// System TTS handlers
const handleVoiceChange = async () => {
  try {
    await invoke('set_system_voice', { voiceId: selectedVoice.value });
  } catch (error) {
    console.error('Failed to set voice:', error);
  }
};

const handleRateChange = async () => {
  try {
    await invoke('set_tts_rate', { rate: rate.value });
  } catch (error) {
    console.error('Failed to set rate:', error);
  }
};

const handlePitchChange = async () => {
  try {
    await invoke('set_tts_pitch', { pitch: pitch.value });
  } catch (error) {
    console.error('Failed to set pitch:', error);
  }
};

const handleVolumeChange = async () => {
  try {
    await invoke('set_tts_volume', { volume: volume.value });
  } catch (error) {
    console.error('Failed to set volume:', error);
  }
};

// Store the unlisten function
let unlistenPromise: Promise<() => void> | null = null;

onMounted(() => {
  fetchStatus();
  fetchVoices();
  loadOpenAIConfig();
  loadOpenAIVoices();
  loadLocalhostConfig();
  loadLocalhostVoices();

  // Listen for provider changes from backend
  unlistenPromise = listen<string>('tts_provider_changed', (event) => {
    const provider = event.payload as 'system' | 'openai' | 'localhost';
    selectedProvider.value = provider;
    activeTab.value = provider;
  });
});

onUnmounted(() => {
  if (unlistenPromise) {
    unlistenPromise.then(fn => fn());
  }
});

// При переключении вкладки меняем провайдер TTS
watch(activeTab, (newTab) => {
  // Only set provider if it's different from current backend value
  if (newTab !== selectedProvider.value) {
    invoke('set_tts_provider', { provider: newTab }).then(() => {
      selectedProvider.value = newTab;
    }).catch((error) => {
      console.error('Failed to set TTS provider:', error);
    });
  }

  // При переключении на вкладку OpenAI загружаем данные
  if (newTab === 'openai') {
    loadOpenAIConfig();
    loadOpenAIVoices();
  }

  // При переключении на вкладку Localhost загружаем данные
  if (newTab === 'localhost') {
    // Сначала загружаем конфиг, затем голоса
    loadLocalhostConfig().then((port) => {
      if (port) {
        // Если порт установлен, обновляем голоса с сервера
        refreshLocalhostVoices();
      } else {
        // Если порт не установлен, загружаем кеш
        loadLocalhostVoices();
      }
    });
  }
});
</script>

<style scoped>
.tts-settings {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 12px;
  padding: 1rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.settings-header {
  margin-bottom: 1rem;
}

.tabs {
  display: flex;
  gap: 0.25rem;
  background: #f3f4f6;
  padding: 0.25rem;
  border-radius: 8px;
}

.tab {
  flex: 1;
  padding: 0.5rem 0.75rem;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #6b7280;
  font-size: 0.875rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.tab:hover {
  color: #374151;
}

.tab.active {
  background: white;
  color: #3b82f6;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.tab-content {
  padding: 0.5rem 0;
}

.warning-banner {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.875rem 1rem;
  margin-bottom: 1rem;
  background: linear-gradient(135deg, #fef3c7 0%, #fde68a 100%);
  border: 1px solid #fbbf24;
  border-radius: 8px;
  box-shadow: 0 1px 3px rgba(251, 191, 36, 0.2);
}

.warning-icon {
  font-size: 1.25rem;
  line-height: 1;
  flex-shrink: 0;
}

.warning-content {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  flex: 1;
}

.warning-list {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.warning-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: #92400e;
}

.warning-text {
  font-size: 0.8125rem;
  color: #78350f;
  line-height: 1.4;
}

.openai-tab {
  padding: 0.5rem 0;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.label {
  font-size: 0.875rem;
  color: #374151;
  font-weight: 500;
  min-width: 100px;
}

.voice-select {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  background: white;
  cursor: pointer;
}

.voice-select:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.text-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  font-family: inherit;
}

.text-input:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.proxy-host {
  flex: 2;
}

.proxy-port {
  flex: 1;
  min-width: 80px;
}

.select-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  background: white;
  cursor: pointer;
}

.select-input:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.voice-label {
  min-width: 60px;
  margin-left: 0.5rem;
}

.openai .voice-select {
  flex: 2;
}

.textarea-input {
  flex: 1;
  padding: 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  font-size: 0.875rem;
  font-family: inherit;
  resize: vertical;
  min-height: 80px;
}

.textarea-input:focus {
  outline: none;
  border-color: #3b82f6;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.instruction-row {
  align-items: flex-start;
}

.slider-control {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.slider-input {
  flex: 1;
  height: 6px;
  border-radius: 3px;
  appearance: none;
  background: #e5e7eb;
  outline: none;
  cursor: pointer;
}

.slider-input::-webkit-slider-thumb {
  appearance: none;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #3b82f6;
  cursor: pointer;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.slider-input::-webkit-slider-thumb:hover {
  background: #2563eb;
  transform: scale(1.1);
}

.slider-input::-moz-range-thumb {
  width: 16px;
  height: 16px;
  border: none;
  border-radius: 50%;
  background: #3b82f6;
  cursor: pointer;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.2);
}

.slider-input::-moz-range-thumb:hover {
  background: #2563eb;
  transform: scale(1.1);
}

.slider-value {
  min-width: 80px;
  text-align: right;
  font-size: 0.875rem;
  color: #6b7280;
  font-weight: 500;
}

.localhost-tab {
  padding: 0.5rem 0;
}

.btn-refresh {
  padding: 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-radius: 8px;
  background: white;
  cursor: pointer;
  font-size: 1rem;
  transition: all 0.2s ease;
}

.btn-refresh:hover:not(:disabled) {
  border-color: #3b82f6;
  background: #f0f9ff;
}

.btn-refresh:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.token-input {
  flex: 1;
}

.token-input-wrapper {
  display: flex;
  gap: 0;
  flex: 1;
}

.token-input-wrapper .text-input,
.token-input-wrapper .token-input {
  flex: 1;
  border-top-right-radius: 0;
  border-bottom-right-radius: 0;
  border-right: none;
}

.token-input-wrapper .text-input:focus,
.token-input-wrapper .token-input:focus {
  border-right: none;
  box-shadow: 0 0 0 3px rgba(59, 130, 246, 0.1);
}

.btn-toggle-visibility {
  padding: 0.5rem 0.75rem;
  border: 1px solid #d1d5db;
  border-left: none;
  border-radius: 0 8px 8px 0;
  background: white;
  cursor: pointer;
  font-size: 1rem;
  transition: all 0.2s ease;
  flex-shrink: 0;
}

.btn-toggle-visibility:hover {
  border-color: #3b82f6;
  border-left: 1px solid #3b82f6;
  background: #f0f9ff;
}

/* Подсвечиваем кнопку при фокусе на соседнем input */
.token-input-wrapper .text-input:focus + .btn-toggle-visibility,
.token-input-wrapper .token-input:focus + .btn-toggle-visibility {
  border-color: #3b82f6;
  border-left: 1px solid #3b82f6;
  background: #f0f9ff;
}
</style>
