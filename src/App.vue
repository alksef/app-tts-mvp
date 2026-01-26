<template>
  <div class="app-container">
    <main class="app-main">
      <!-- Center Panel: Input and Keys -->
      <div class="center-panel">
        <!-- Input Section -->
        <div class="input-section">
          <textarea
            ref="inputRef"
            class="test-input"
            v-model="keyboardStore.interceptedText"
            placeholder="Введите здесь для тестирования захвата клавиш... (Ctrl+Enter для отправки на TTS)"
          ></textarea>
        </div>

        <!-- TTS Settings (full width below input) -->
        <TtsSettings />

        <!-- Plugin Settings -->
        <PluginSettings />
      </div>

      <!-- Left Panel: TTS History -->
      <div class="left-panel">
        <TtsHistory />
      </div>

      <!-- Right Panel: Status, Controls and Playback -->
      <div class="right-panel">
        <TtsPlaybackControls />
        <div class="top-controls-row">
          <StatusIndicator class="status-indicator" />
          <div class="window-controls">
            <div class="hotkey-info">
              <span class="hotkey-label">Горячая клавиша:</span>
              <span class="hotkey-keys">Win+Esc</span>
            </div>
            <div class="mode-selector">
              <button
                class="mode-btn"
                :class="{ active: keyboardStore.hotkeyMode === 'background_blocking' }"
                @click="setHotkeyMode('background_blocking')"
              >
                Перехват в фоне
              </button>
              <button
                class="mode-btn"
                :class="{ active: keyboardStore.hotkeyMode === 'overlay_call' }"
                @click="setHotkeyMode('overlay_call')"
              >
                Вызов
              </button>
            </div>
          </div>
        </div>
        <AudioOutputSettings />
      </div>
    </main>

    <!-- Toast notifications -->
    <Toast
      :show="keyboardStore.toast.show"
      :message="keyboardStore.toast.message"
      :type="keyboardStore.toast.type"
      :duration="5000"
      @hide="keyboardStore.hideToast()"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted, watch } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useKeyboardStore } from './stores/keyboard';
import StatusIndicator from './components/StatusIndicator.vue';
import TtsPlaybackControls from './components/TtsPlaybackControls.vue';
import TtsSettings from './components/TtsSettings.vue';
import TtsHistory from './components/TtsHistory.vue';
import AudioOutputSettings from './components/AudioOutputSettings.vue';
import PluginSettings from './components/PluginSettings.vue';
import Toast from './components/Toast.vue';

const inputRef = ref<HTMLTextAreaElement | null>(null);
const keyboardStore = useKeyboardStore();
let unlistenFocus: (() => void) | null = null;
let lastBlockingState = false;
let focusTimeout: number | null = null;
let metaPressed = false;
let shiftPressed = false;
let altPressed = false;
let ctrlPressed = false;

// Watch interceptedText changes and force DOM update if needed (Vue v-model bug in Tauri)
watch(() => keyboardStore.interceptedText, (newVal) => {
  if (inputRef.value) {
    const domValue = inputRef.value.value;
    // Force DOM update if there's a mismatch
    if (domValue !== newVal) {
      inputRef.value.value = newVal;
    }
  }
}, { immediate: true });

const focusInput = () => {
  if (inputRef.value && document.activeElement !== inputRef.value) {
    inputRef.value.focus();
  }
};

const setHotkeyMode = async (mode: 'background_blocking' | 'overlay_call') => {
  await keyboardStore.setHotkeyMode(mode);
};

// Track window focus state for overlay mode
let isWindowFocused = false;

// Track Win key state for Win+Esc detection in focused window (redundant with metaPressed but kept for clarity)
let winPressed = false;

// Debounce handleWinEscHotkey to prevent double-toggle from both frontend and backend
let lastHotkeyCallTime = 0;
const HOTKEY_DEBOUNCE_MS = 300;

/**
 * Handle Win+Esc hotkey based on actual window state
 * Works the same whether called from hook (window inactive) or frontend (window active)
 */
async function handleWinEscHotkey() {
  const now = Date.now();
  console.log('[handleWinEscHotkey] Starting, hotkeyMode:', keyboardStore.hotkeyMode, 'timeSinceLastCall:', now - lastHotkeyCallTime);

  // Debounce: ignore calls within 300ms of the last call
  if (now - lastHotkeyCallTime < HOTKEY_DEBOUNCE_MS) {
    console.log('[handleWinEscHotkey] Debounced (too soon), returning');
    return;
  }
  lastHotkeyCallTime = now;

  if (keyboardStore.hotkeyMode !== 'overlay_call') {
    console.log('[handleWinEscHotkey] Not in overlay_call mode, returning');
    return;
  }

  const window = getCurrentWindow();
  const isFocused = await window.isFocused();
  const isVisible = await window.isVisible();
  const isMinimized = await window.isMinimized();

  console.log('[handleWinEscHotkey] Window state:', { isFocused, isVisible, isMinimized });

  // Check minimized FIRST - Windows can report minimized windows as "focused"
  // This prevents re-hiding a window that should be shown
  if (isMinimized || !isVisible) {
    // Window is minimized or hidden → show and focus it
    console.log('[handleWinEscHotkey] Window minimized/hidden, showing...');
    await keyboardStore.showWindowOverlay();
  } else if (isFocused) {
    // Window is focused and visible → minimize it
    console.log('[handleWinEscHotkey] Window focused and visible, hiding...');
    await keyboardStore.hideWindowAndRestoreFocus();
  } else {
    // Window is visible but not focused → just focus it
    console.log('[handleWinEscHotkey] Window visible but not focused, setting focus...');
    await window.setFocus();
  }

  console.log('[handleWinEscHotkey] Completed');
}

// Handle keydown events - redirect all input to our text field when window is focused
const handleKeyDown = async (e: KeyboardEvent) => {
  // Track Shift key state
  if (e.key === 'Shift' || e.code === 'ShiftLeft' || e.code === 'ShiftRight') {
    shiftPressed = true;
    // Don't prevent default - allow system hotkeys (including Shift+Alt for language)
    return;
  }

  // Track Alt key state
  if (e.key === 'Alt' || e.code === 'AltLeft' || e.code === 'AltRight') {
    altPressed = true;
    // Don't prevent default - allow system hotkeys (including Shift+Alt for language)
    return;
  }

  // Track Ctrl key state
  if (e.key === 'Control' || e.code === 'ControlLeft' || e.code === 'ControlRight') {
    ctrlPressed = true;
    return;
  }

  // Track Meta (Win) key state
  if (e.key === 'Meta' || e.code === 'MetaLeft' || e.code === 'MetaRight') {
    metaPressed = true;
    winPressed = true;
    e.preventDefault();
    return;
  }

  // Ctrl+Enter for TTS playback
  if (e.key === 'Enter' && ctrlPressed) {
    e.preventDefault();
    const text = keyboardStore.interceptedText;
    if (text) {
      keyboardStore.speakTextWithHistory(text);
      // Clear input after sending
      keyboardStore.interceptedText = '';
    }
    return;
  }

  // Win+Esc in overlay_call mode - handle based on actual window state
  if (e.key === 'Escape' && metaPressed) {
    if (keyboardStore.hotkeyMode === 'overlay_call') {
      await handleWinEscHotkey();
      // Reset Win key tracking
      metaPressed = false;
      winPressed = false;
      e.preventDefault();
      return;
    }
    // Don't return - let the event propagate to the hook
    // Just reset our tracking variables for UI state
    // Note: We don't preventDefault() so the event continues to the hook
  }

  // Allow default behavior for some keys
  if (e.key === 'Tab' || e.key === 'F5' || e.key === 'F12') {
    return;
  }

  // If focus is not on our input, redirect the keystroke
  if (document.activeElement !== inputRef.value && inputRef.value) {
    // For printable characters, type into the input
    if (e.key.length === 1) {
      e.preventDefault();
      const input = inputRef.value;
      const start = input.selectionStart;
      const end = input.selectionEnd;
      const value = input.value;

      input.value = value.slice(0, start) + e.key + value.slice(end);
      input.selectionStart = input.selectionEnd = start + 1;

      // Trigger input event
      input.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (e.key === 'Backspace') {
      e.preventDefault();
      const input = inputRef.value;
      const start = input.selectionStart;
      const end = input.selectionEnd;
      const value = input.value;

      if (start === end && start > 0) {
        input.value = value.slice(0, start - 1) + value.slice(end);
        input.selectionStart = input.selectionEnd = start - 1;
      } else if (start !== end) {
        input.value = value.slice(0, start) + value.slice(end);
        input.selectionStart = input.selectionEnd = start;
      }
      input.dispatchEvent(new Event('input', { bubbles: true }));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      // Allow Enter for form submission or other actions
    }
  }
};

const handleKeyUp = (e: KeyboardEvent) => {
  // Track Shift key release
  if (e.key === 'Shift' || e.code === 'ShiftLeft' || e.code === 'ShiftRight') {
    shiftPressed = false;
  }

  // Track Alt key release
  if (e.key === 'Alt' || e.code === 'AltLeft' || e.code === 'AltRight') {
    altPressed = false;
  }

  // Track Ctrl key release
  if (e.key === 'Control' || e.code === 'ControlLeft' || e.code === 'ControlRight') {
    ctrlPressed = false;
  }

  // Track Meta (Win) key release - reset both variables
  if (e.key === 'Meta' || e.code === 'MetaLeft' || e.code === 'MetaRight') {
    metaPressed = false;
    winPressed = false;
  }
};

onMounted(async () => {
  const appWindow = getCurrentWindow();
  lastBlockingState = keyboardStore.blockingEnabled;

  // Listen for window focus events using the Tauri 2.0 event system
  unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
    isWindowFocused = focused;

    if (focused) {
      // Only focus input if blocking state hasn't just changed
      // This prevents conflict with applyWindowBehavior
      if (keyboardStore.blockingEnabled === lastBlockingState) {
        // Delay slightly to let window behavior settle
        if (focusTimeout) clearTimeout(focusTimeout);
        focusTimeout = window.setTimeout(() => {
          focusInput();
        }, 100);
      }
    }
  });

  // Listen for show window request from hook (for overlay call mode)
  const unlistenShowWindow = await listen('show_window_requested', async () => {
    console.log('[show_window_requested] Event received, hotkeyMode:', keyboardStore.hotkeyMode);
    if (keyboardStore.hotkeyMode === 'overlay_call') {
      console.log('[show_window_requested] Calling handleWinEscHotkey...');
      // Use the same logic as frontend - check actual window state
      await handleWinEscHotkey();
    } else {
      console.log('[show_window_requested] Not in overlay_call mode, ignoring');
    }
  });

  // Store unlisten function for cleanup
  (unlistenFocus as any)._showWindowUnlisten = unlistenShowWindow;

  // Focus input on initial load
  focusInput();

  // Add global keydown listener to capture all keystrokes
  globalThis.window.addEventListener('keydown', handleKeyDown);
  globalThis.window.addEventListener('keyup', handleKeyUp);
});

onUnmounted(() => {
  if (unlistenFocus) {
    unlistenFocus();
    // Clean up show window listener
    const showWindowUnlisten = (unlistenFocus as any)._showWindowUnlisten;
    if (showWindowUnlisten) {
      showWindowUnlisten();
    }
  }
  if (focusTimeout) {
    clearTimeout(focusTimeout);
  }
  globalThis.window.removeEventListener('keydown', handleKeyDown);
  globalThis.window.removeEventListener('keyup', handleKeyUp);
});
</script>

<style scoped>
.app-container {
  min-height: 100vh;
  background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
  padding: 1rem;
  display: flex;
  flex-direction: column;
}

.top-controls-row {
  display: flex;
  gap: 0.75rem;
}

.status-indicator {
  flex: 1;
}

.window-controls {
  flex: 1;
  background: rgba(255, 255, 255, 0.95);
  border-radius: 8px;
  padding: 1rem;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
}

.hotkey-info {
  display: flex;
  justify-content: center;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

.hotkey-label {
  font-size: 0.75rem;
  font-weight: 600;
  color: #6b7280;
}

.hotkey-keys {
  font-size: 0.75rem;
  font-weight: 600;
  color: #374151;
  background: #f3f4f6;
  padding: 0.125rem 0.5rem;
  border-radius: 4px;
  font-family: monospace;
}

.mode-selector {
  display: flex;
  background: #f3f4f6;
  padding: 0.25rem;
  border-radius: 8px;
  gap: 0.25rem;
}

.mode-btn {
  flex: 1;
  padding: 0.5rem 0.75rem;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: #6b7280;
  font-size: 0.75rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s ease;
}

.mode-btn:hover {
  color: #374151;
}

.mode-btn.active {
  background: white;
  color: #3b82f6;
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
}

.app-header {
  text-align: center;
  color: white;
  margin-bottom: 2rem;
}

.app-title {
  font-size: 2.5rem;
  font-weight: 800;
  margin: 0 0 0.5rem 0;
  text-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.app-subtitle {
  font-size: 1.125rem;
  font-weight: 500;
  margin: 0;
  opacity: 0.9;
}

.app-main {
  flex: 1;
  display: flex;
  flex-direction: row;
  gap: 1rem;
  max-width: 1600px;
  width: 100%;
  margin: 0 auto;
  align-items: stretch;
}

.center-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.left-panel {
  flex: 0 0 20%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
  order: -1;
}

.right-panel {
  flex: 0 0 30%;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.input-section {
  display: flex;
  justify-content: center;
}

.test-input {
  width: 100%;
  max-width: 800px;
  min-height: 240px;
  padding: 1rem 1.5rem;
  font-size: 1rem;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.95);
  outline: none;
  resize: vertical;
  font-family: inherit;
}

.test-input:focus {
  border-color: rgba(255, 255, 255, 0.6);
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
}

@media (max-width: 1200px) {
  .app-main {
    flex-direction: column;
  }

  .left-panel,
  .center-panel,
  .right-panel {
    flex: 1;
  }

  .left-panel {
    order: unset;
  }
}

@media (max-width: 768px) {
  .app-container {
    padding-top: 1rem;
    padding-right: 1rem;
    padding-bottom: 1rem;
    padding-left: 1rem;
  }

  .app-title {
    font-size: 2rem;
  }

  .app-subtitle {
    font-size: 1rem;
  }
}
</style>
