<template>
  <div class="plugin-card" :class="{ disabled: !plugin.enabled }">
    <div class="plugin-header">
      <div class="plugin-name-row">
        <button
          class="toggle-switch"
          :class="{ active: plugin.enabled }"
          @click="$emit('toggle', plugin.name, !plugin.enabled)"
        >
          <span class="toggle-slider"></span>
        </button>
        <span class="plugin-name">{{ plugin.name }}</span>
        <span
          class="status-indicator"
          :class="statusClass"
          :title="statusText"
        >
          <span class="status-dot"></span>
        </span>
      </div>
      <span class="plugin-version">v{{ plugin.version }}</span>
    </div>

    <div v-if="plugin.last_error" class="error-message">
      {{ plugin.last_error }}
    </div>

    <div v-if="plugin.config_schema && plugin.config_schema.properties" class="config-form">
      <div
        v-for="(field, key) in plugin.config_schema.properties"
        :key="key"
        class="config-field"
      >
        <label class="field-label">{{ field.title || key }}</label>

        <input
          v-if="field.type === 'string' && !field.secret"
          type="text"
          class="field-input"
          :value="getConfigValue(key)"
          :placeholder="field.description || ''"
          @input="updateConfig(key, ($event.target as HTMLInputElement).value)"
        />

        <input
          v-else-if="field.type === 'string' && field.secret"
          type="password"
          class="field-input"
          :value="getConfigValue(key)"
          :placeholder="field.description || ''"
          @input="updateConfig(key, ($event.target as HTMLInputElement).value)"
        />

        <p v-if="field.description" class="field-description">{{ field.description }}</p>
      </div>
    </div>

    <button
      v-if="plugin.config_schema && plugin.config_schema.properties"
      class="save-button"
      @click="handleSave"
    >
      Сохранить
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'

interface PluginInfo {
  name: string
  version: number
  enabled: boolean
  status: number
  config_schema: any
  config: any
  last_error: string | null
}

const props = defineProps<{
  plugin: PluginInfo
}>()

const emit = defineEmits<{
  toggle: [name: string, enabled: boolean]
  save: [name: string, config: any]
  'check-status': [name: string]
}>()

const localConfig = ref<Record<string, any>>({})

// Инициализируем localConfig из plugin.config (сохраненные значения) или defaults
watch(() => props.plugin.config_schema, (schema) => {
  if (schema && schema.properties) {
    // Copy values from saved config
    if (props.plugin.config && typeof props.plugin.config === 'object') {
      Object.keys(props.plugin.config).forEach(key => {
        localConfig.value[key] = props.plugin.config[key]
      })
    }
    // Initialize missing keys from schema defaults or empty string
    Object.keys(schema.properties).forEach(key => {
      if (localConfig.value[key] === undefined) {
        const field = schema.properties[key]
        localConfig.value[key] = field.default || ''
      }
    })
  }
}, { immediate: true })

// Also update localConfig when plugin.config changes (e.g., after save)
watch(() => props.plugin.config, (config) => {
  if (config && typeof config === 'object' && props.plugin.config_schema?.properties) {
    Object.keys(config).forEach(key => {
      localConfig.value[key] = config[key]
    })
  }
})

const statusClass = computed(() => {
  switch (props.plugin.status) {
    case 0: return 'status-ok'
    case 1: return 'status-warning'
    case 2:
    case 3:
    case 4: return 'status-error'
    default: return 'status-unknown'
  }
})

const statusText = computed(() => {
  switch (props.plugin.status) {
    case 0: return 'Работает'
    case 1: return 'Не настроен'
    case 2: return 'Ошибка подключения'
    case 3: return 'Ошибка авторизации'
    case 4: return 'Неизвестная ошибка'
    default: return 'Неизвестный статус'
  }
})

function getConfigValue(key: string): string {
  return localConfig.value[key] || ''
}

function updateConfig(key: string, value: string) {
  localConfig.value[key] = value
}

function handleSave() {
  emit('save', props.plugin.name, localConfig.value)
}
</script>

<style scoped>
.plugin-card {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 8px;
  padding: 0.75rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  transition: opacity 0.2s;
}

.plugin-card.disabled {
  opacity: 0.5;
}

.plugin-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.5rem;
}

.plugin-name-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex: 1;
}

.plugin-name {
  font-weight: 500;
  font-size: 0.85rem;
  color: #374151;
}

.plugin-version {
  font-size: 0.7rem;
  color: #6b7280;
}

.status-indicator {
  display: flex;
  align-items: center;
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.status-ok .status-dot {
  background: #22c55e;
}

.status-warning .status-dot {
  background: #eab308;
}

.status-error .status-dot {
  background: #ef4444;
}

.status-unknown .status-dot {
  background: #9ca3af;
}

.error-message {
  background: #fef2f2;
  color: #991b1b;
  border: 1px solid #fecaca;
  padding: 0.5rem;
  border-radius: 6px;
  font-size: 0.75rem;
  margin-bottom: 0.5rem;
}

.config-form {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.config-field {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.field-label {
  font-size: 0.75rem;
  color: #6b7280;
  font-weight: 500;
}

.field-input {
  padding: 0.4rem 0.5rem;
  border: 1px solid #d1d5db;
  border-radius: 6px;
  background: white;
  color: #374151;
  font-size: 0.75rem;
}

.field-input:focus {
  outline: none;
  border-color: #3b82f6;
}

.field-description {
  font-size: 0.7rem;
  color: #6b7280;
  margin: 0;
}

.save-button {
  margin-top: 0.5rem;
  padding: 0.4rem 0.75rem;
  background: #3b82f6;
  color: white;
  border: none;
  border-radius: 6px;
  font-size: 0.75rem;
  cursor: pointer;
  align-self: flex-start;
  transition: background 0.2s;
}

.save-button:hover {
  background: #2563eb;
}

.toggle-switch {
  position: relative;
  width: 32px;
  height: 18px;
  background: #d1d5db;
  border-radius: 9px;
  cursor: pointer;
  transition: background 0.2s;
  border: none;
  padding: 0;
}

.toggle-switch.active {
  background: #3b82f6;
}

.toggle-slider {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  background: white;
  border-radius: 50%;
  transition: transform 0.2s;
}

.toggle-switch.active .toggle-slider {
  transform: translateX(14px);
}
</style>
