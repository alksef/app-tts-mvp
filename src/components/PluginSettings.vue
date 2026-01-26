<template>
  <div class="plugin-settings">
    <div class="output-block">
      <h3>Плагины</h3>

      <div v-if="loading" class="loading">Загрузка...</div>

      <div v-else-if="plugins.length === 0" class="empty-state">
        <p>Нет загруженных плагинов</p>
        <p class="hint">Поместите .dll файлы в папку plugins рядом с приложением</p>
      </div>

      <div v-else class="plugins-list">
        <PluginCard
          v-for="plugin in plugins"
          :key="plugin.name"
          :plugin="plugin"
          @toggle="handleToggle"
          @save="handleSaveConfig"
          @check-status="handleCheckStatus"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import PluginCard from './PluginCard.vue'

interface PluginInfo {
  name: string
  version: number
  enabled: boolean
  status: number
  config_schema: any
  last_error: string | null
}

const plugins = ref<PluginInfo[]>([])
const loading = ref(true)

async function loadPlugins() {
  try {
    const result = await invoke<PluginInfo[]>('get_plugins')
    plugins.value = result
  } catch (e) {
    console.error('Failed to load plugins:', e)
  } finally {
    loading.value = false
  }
}

async function handleToggle(name: string, enabled: boolean) {
  try {
    await invoke('toggle_plugin', { name, enabled })
    // Plugins will be updated via event
  } catch (e) {
    console.error('Failed to toggle plugin:', e)
  }
}

async function handleSaveConfig(name: string, config: any) {
  try {
    await invoke('set_plugin_config', { name, config })
    // Plugins will be updated via event
  } catch (e) {
    console.error('Failed to save config:', e)
  }
}

async function handleCheckStatus(name: string) {
  try {
    await invoke('check_plugin_status', { name })
    // Reload plugins after checking status
    await loadPlugins()
  } catch (e) {
    console.error('Failed to check status:', e)
  }
}

onMounted(async () => {
  await loadPlugins()

  // Listen for plugins changed events from backend
  const unlisten = await listen<PluginInfo[]>('plugins_changed', (event) => {
    plugins.value = event.payload
  })

  // Cleanup on unmount
  onUnmounted(() => {
    unlisten()
  })
})
</script>

<style scoped>
.plugin-settings {
  display: flex;
  flex-direction: column;
}

.output-block {
  background: rgba(255, 255, 255, 0.95);
  border-radius: 8px;
  padding: 0.75rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.output-block h3 {
  margin: 0 0 0.75rem 0;
  font-size: 0.85rem;
  color: #374151;
  font-weight: 600;
}

.loading,
.empty-state {
  text-align: center;
  padding: 1.5rem;
  background: rgba(255, 255, 255, 0.5);
  border-radius: 8px;
  color: #6b7280;
  font-size: 0.8rem;
}

.hint {
  font-size: 0.7rem;
  opacity: 0.7;
  margin-top: 0.25rem;
}

.plugins-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}
</style>
