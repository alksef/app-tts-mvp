<template>
  <div class="audio-output-settings">
    <!-- Speaker block -->
    <div class="output-block" :class="{ disabled: ttsProvider === 'system' }">
      <h3>Динамики (аудиовывод)</h3>

      <div class="setting-row device-row">
        <label>Устройство:</label>
        <select
          v-model="speakerDevice"
          :disabled="!speakerEnabled || controlsDisabled"
          @change="onSpeakerDeviceChange"
          class="device-select"
        >
          <option value="">(по умолчанию)</option>
          <option v-for="device in outputDevices" :key="device.id" :value="device.id">
            {{ device.name }}
          </option>
        </select>
      </div>

      <div class="setting-row volume-row">
        <label>Громкость:</label>
        <input
          type="range"
          v-model.number="speakerVolume"
          min="0"
          max="100"
          :disabled="!speakerEnabled || controlsDisabled"
          class="volume-slider"
          @input="onSpeakerVolumeChange"
        />
        <span class="volume-value">{{ speakerVolume }}%</span>
      </div>

      <div class="button-row">
        <button
          @click="setSpeakerEnabled(true)"
          :class="['toggle-btn', { active: speakerEnabled }]"
          :disabled="controlsDisabled"
        >
          🔊 Вкл
        </button>
        <button
          @click="setSpeakerEnabled(false)"
          :class="['toggle-btn', { active: !speakerEnabled }]"
          :disabled="controlsDisabled"
        >
          🔇 Выкл
        </button>
      </div>
    </div>

    <!-- Virtual mic block -->
    <div class="output-block" :class="{ disabled: ttsProvider === 'system' }">
      <h3>Виртуальный микрофон</h3>

      <div class="setting-row device-row">
        <label>Устройство:</label>
        <select
          v-model="virtualMicDevice"
          :disabled="controlsDisabled"
          @change="onVirtualMicDeviceChange"
          class="device-select"
        >
          <option value="">(не выбрано)</option>
          <option v-for="device in virtualMicDevices" :key="device.id" :value="device.id">
            {{ device.name }}
          </option>
        </select>
      </div>

      <div class="setting-row volume-row">
        <label>Громкость:</label>
        <input
          type="range"
          v-model.number="virtualMicVolume"
          min="0"
          max="100"
          :disabled="!virtualMicDevice || controlsDisabled"
          class="volume-slider"
          @input="onVirtualMicVolumeChange"
        />
        <span class="volume-value">{{ virtualMicVolume }}%</span>
      </div>

      <div class="button-row">
        <button
          @click="enableVirtualMic"
          :class="['toggle-btn', { active: !!virtualMicDevice }]"
          :disabled="controlsDisabled"
        >
          🎤 Вкл
        </button>
        <button
          @click="disableVirtualMic"
          :class="['toggle-btn', { active: !virtualMicDevice }]"
          :disabled="controlsDisabled"
        >
          🎤 Выкл
        </button>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

interface Device {
  id: string
  name: string
  is_default: boolean
}

interface AudioSettings {
  speaker_device: string | null
  speaker_enabled: boolean
  speaker_volume: number
  virtual_mic_device: string | null
  virtual_mic_volume: number
}

const outputDevices = ref<Device[]>([])
const virtualMicDevices = ref<Device[]>([])

const speakerDevice = ref<string>('')
const speakerEnabled = ref(true)
const speakerVolume = ref(100)

const virtualMicDevice = ref<string>('')
const virtualMicVolume = ref(100)

const ttsProvider = ref<string>('system')

// Computed property to check if controls should be disabled
const controlsDisabled = computed(() => ttsProvider.value === 'system')

async function loadDevices() {
  try {
    const [outputs, virtuals] = await Promise.all([
      invoke<Device[]>('get_output_devices'),
      invoke<Device[]>('get_virtual_mic_devices')
    ])
    outputDevices.value = outputs
    virtualMicDevices.value = virtuals
  } catch (e) {
    console.error('Failed to load devices:', e)
  }
}

async function loadSettings() {
  try {
    const settings = await invoke<AudioSettings>('get_audio_settings')
    speakerDevice.value = settings.speaker_device || ''
    speakerEnabled.value = settings.speaker_enabled
    speakerVolume.value = settings.speaker_volume
    virtualMicDevice.value = settings.virtual_mic_device || ''
    virtualMicVolume.value = settings.virtual_mic_volume
  } catch (e) {
    console.error('Failed to load audio settings:', e)
  }
}

async function loadTtsProvider() {
  try {
    const status = await invoke<{ provider: string }>('get_tts_status')
    ttsProvider.value = status.provider
  } catch (e) {
    console.error('Failed to load TTS provider:', e)
  }
}

async function onSpeakerDeviceChange() {
  try {
    await invoke('set_speaker_device', {
      deviceId: speakerDevice.value || null
    })
  } catch (e) {
    console.error('Failed to set speaker device:', e)
  }
}

async function setSpeakerEnabled(enabled: boolean) {
  try {
    await invoke('set_speaker_enabled', { enabled })
    speakerEnabled.value = enabled
  } catch (e) {
    console.error('Failed to set speaker enabled:', e)
  }
}

async function onSpeakerVolumeChange() {
  try {
    await invoke('set_speaker_volume', { volume: speakerVolume.value })
  } catch (e) {
    console.error('Failed to set speaker volume:', e)
  }
}

async function onVirtualMicDeviceChange() {
  try {
    await invoke('set_virtual_mic_device', {
      deviceId: virtualMicDevice.value || null
    })
  } catch (e) {
    console.error('Failed to set virtual mic device:', e)
  }
}

async function enableVirtualMic() {
  try {
    await invoke('enable_virtual_mic')
    // Reload settings to get the device ID
    await loadSettings()
  } catch (e) {
    console.error('Failed to enable virtual mic:', e)
  }
}

async function disableVirtualMic() {
  try {
    await invoke('disable_virtual_mic')
    virtualMicDevice.value = ''
  } catch (e) {
    console.error('Failed to disable virtual mic:', e)
  }
}

async function onVirtualMicVolumeChange() {
  try {
    await invoke('set_virtual_mic_volume', { volume: virtualMicVolume.value })
  } catch (e) {
    console.error('Failed to set virtual mic volume:', e)
  }
}

onMounted(() => {
  loadDevices()
  loadSettings()
  loadTtsProvider()

  // Listen for provider changes
  listen<string>('tts_provider_changed', (event) => {
    ttsProvider.value = event.payload
  })
})
</script>

<style scoped>
.audio-output-settings {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.output-block {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 8px;
  padding: 0.75rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transition: opacity 0.2s;
}

.output-block.disabled {
  opacity: 0.5;
  pointer-events: none;
}

.output-block h3 {
  margin: 0 0 0.75rem 0;
  font-size: 0.85rem;
  color: #374151;
  font-weight: 600;
}

.setting-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.setting-row:last-child {
  margin-bottom: 0;
}

.device-row {
  flex-direction: column;
  align-items: stretch;
  gap: 0.4rem;
}

.setting-row label {
  min-width: 60px;
  color: #6b7280;
  font-size: 0.75rem;
  font-weight: 500;
}

.device-row label {
  min-width: auto;
}

.device-row .device-select {
  flex: auto;
  width: 100%;
}

.device-select {
  flex: 1;
  padding: 0.4rem;
  border-radius: 6px;
  border: 1px solid #d1d5db;
  background: white;
  color: #374151;
  font-size: 0.75rem;
}

.device-select:disabled {
  opacity: 0.5;
}

.volume-row {
  position: relative;
}

.volume-slider {
  flex: 1;
  height: 4px;
  border-radius: 2px;
  background: #e5e7eb;
  outline: none;
  -webkit-appearance: none;
}

.volume-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #3b82f6;
  cursor: pointer;
}

.volume-slider:disabled::-webkit-slider-thumb {
  opacity: 0.5;
}

.volume-value {
  min-width: 35px;
  text-align: right;
  color: #6b7280;
  font-size: 0.75rem;
}

.button-row {
  display: flex;
  gap: 0.4rem;
  margin-top: 0.5rem;
}

.toggle-btn {
  flex: 1;
  padding: 0.4rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  background: white;
  color: #6b7280;
  font-size: 0.75rem;
  cursor: pointer;
  transition: all 0.2s;
}

.toggle-btn:hover:not(:disabled) {
  background: #f3f4f6;
}

.toggle-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.toggle-btn.active {
  background: #3b82f6;
  color: white;
  border-color: #3b82f6;
}
</style>
