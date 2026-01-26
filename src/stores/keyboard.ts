import { defineStore } from 'pinia';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';

export interface KeyEvent {
  vk_code: number;
  key_name: string;
  timestamp: number;
  seq_num: number;
}

export interface StatusResponse {
  blocking_enabled: boolean;
  win_pressed: boolean;
  always_on_top: boolean;
  auto_show_on_block: boolean;
  caps_lock: boolean;
  input_language: string;
  hotkey_mode: 'background_blocking' | 'overlay_call';
}

export interface KeysResponse {
  keys: KeyEvent[];
  latest_key: KeyEvent | null;
}

export interface TtsStatus {
  is_speaking: boolean;
  provider: string;
  continuous_play: boolean;
  has_openai_key: boolean;
  sapi_available: boolean;
  silero_available: boolean;
  silero_server_url: string;
  silero_voice: string;
}

export type TtsMessageStatus = 'queued' | 'playing' | 'completed';

export interface TtsMessage {
  id: string;
  text: string;
  timestamp: number;
  status: TtsMessageStatus;
  locked: boolean;
}

export const useKeyboardStore = defineStore('keyboard', {
  state: () => ({
    blockingEnabled: false as boolean,
    winPressed: false as boolean,
    alwaysOnTop: false as boolean,
    autoShowOnBlock: false as boolean,
    hotkeyMode: 'overlay_call' as 'background_blocking' | 'overlay_call',
    overlayPreviousWindowSaved: false as boolean,  // Track if previous window was saved for overlay mode
    capsLock: false as boolean,
    interceptedKeys: [] as KeyEvent[],
    latestKey: null as KeyEvent | null,
    pollInterval: null as number | null,
    interceptedText: '' as string,  // Captured text from intercepted keys
    lastProcessedSeqNum: -1 as number,  // Track last processed key sequence number
    fetchingKeys: false as boolean,  // Flag to prevent parallel fetchKeys calls
    pendingTextUpdate: '' as string,  // Batched text update
    textUpdateScheduled: false as boolean,  // Flag to prevent duplicate updates
    lastStatusFetch: null as number | null,  // Track last status fetch to reduce polling
    // TTS state
    isPlaying: false as boolean,
    continuousPlay: false as boolean,
    ttsProvider: 'system' as 'system' | 'openai',
    // Input language state
    inputLanguage: 'en' as 'ru' | 'en',
    // TTS history state
    ttsHistory: [] as TtsMessage[],
    ttsCurrentMessageId: null as string | null,
    ttsHistoryInitialized: false as boolean,
    stateEventListenersInitialized: false as boolean,
    // Toast notification state
    toast: {
      show: false as boolean,
      message: '' as string,
      type: 'error' as 'error' | 'warning' | 'info',
    },
  }),

  getters: {
    statusText: (state) => {
      return state.blockingEnabled ? 'Blocking Enabled' : 'Blocking Disabled';
    },

    statusColor: (state) => {
      return state.blockingEnabled ? '#ef4444' : '#22c55e'; // Red for enabled, green for disabled
    },

    hasLatestKey: (state) => {
      return state.latestKey !== null;
    },
  },

  actions: {
    /**
     * Fetch the current status from the backend
     */
    async fetchStatus() {
      try {
        const response = await invoke<StatusResponse>('get_status');
        const wasBlockingEnabled = this.blockingEnabled;
        this.blockingEnabled = response.blocking_enabled;
        this.winPressed = response.win_pressed;
        this.alwaysOnTop = response.always_on_top;
        this.autoShowOnBlock = response.auto_show_on_block;
        this.capsLock = response.caps_lock;
        this.inputLanguage = response.input_language as 'ru' | 'en';
        this.hotkeyMode = response.hotkey_mode;

      } catch (error) {
        console.error('Failed to fetch status:', error);
      }
    },

    /**
     * Fetch intercepted keys from the backend
     * Uses seq_num for incremental polling to avoid duplicates
     * Protected against parallel calls to prevent race conditions
     */
    async fetchKeys() {
      // Prevent parallel calls
      if (this.fetchingKeys) {
        return;
      }

      this.fetchingKeys = true;

      try {
        const afterSeq = this.lastProcessedSeqNum >= 0 ? this.lastProcessedSeqNum : null;

        // Only fetch keys after the last processed seq_num
        const response = await invoke<KeysResponse>('get_intercepted_keys', {
          afterSeqNum: afterSeq,
        });

        // Note: interceptedKeys and latestKey are updated by events, not here
        // This polling is only a fallback mechanism to ensure no keys are missed

        // Process new keys if blocking is enabled
        if (this.blockingEnabled && response.keys.length > 0) {
          // Sort by seq_num to ensure correct order
          const sortedKeys = [...response.keys].sort((a, b) => a.seq_num - b.seq_num);

          for (const key of sortedKeys) {
            this.processInterceptedKey(key);
          }

          // Update last processed seq_num
          const newLastSeq = Math.max(...sortedKeys.map(k => k.seq_num));
          this.lastProcessedSeqNum = newLastSeq;

          // Schedule a batched update after processing all keys
          this.scheduleTextUpdate();
        }
      } catch (error) {
        console.error('Failed to fetch keys:', error);
      } finally {
        this.fetchingKeys = false;
      }
    },

    /**
     * Schedule a batched text update to reduce re-renders
     */
    scheduleTextUpdate() {
      if (this.textUpdateScheduled) {
        return;
      }

      this.textUpdateScheduled = true;
      requestAnimationFrame(() => {
        this.interceptedText += this.pendingTextUpdate;
        this.pendingTextUpdate = '';
        this.textUpdateScheduled = false;
      });
    },

    /**
     * Process an intercepted key and convert it to text (batched)
     */
    processInterceptedKey(key: KeyEvent) {
      // Skip Win+Esc toggle events
      if (key.key_name.includes('Win+Esc')) {
        return;
      }

      // Handle special keys
      if (key.key_name === 'Backspace') {
        // For backspace, we need to update immediately
        this.interceptedText = this.interceptedText.slice(0, -1);
        // Also clear any pending updates
        this.pendingTextUpdate = '';
      } else if (key.key_name === 'Enter') {
        this.pendingTextUpdate += '\n';
      } else if (key.key_name === 'Space') {
        this.pendingTextUpdate += ' ';
      } else if (key.key_name === 'Tab') {
        this.pendingTextUpdate += '\t';
      } else if (key.key_name === 'Comma') {
        // Comma key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'б' : ',';
      } else if (key.key_name === 'Period') {
        // Period key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'ю' : '.';
      } else if (key.key_name === 'Slash') {
        // Slash key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? '.' : '/';
      } else if (key.key_name === 'Semicolon') {
        // Semicolon key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'ж' : ';';
      } else if (key.key_name === 'Quote') {
        // Quote key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'э' : '\'';
      } else if (key.key_name === 'Backtick') {
        // Backtick key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'ё' : '`';
      } else if (key.key_name === 'Left Bracket') {
        // Left bracket key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'х' : '[';
      } else if (key.key_name === 'Right Bracket') {
        // Right bracket key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'ъ' : ']';
      } else if (key.key_name === 'Backslash') {
        // Backslash key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? 'ё' : '\\';
      } else if (key.key_name === 'Minus') {
        // Minus key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? '-' : '-';
      } else if (key.key_name === 'Equals') {
        // Equals key
        this.pendingTextUpdate += (this.inputLanguage === 'ru') ? '=' : '=';
      } else if (key.key_name.startsWith('Digit ')) {
        // Digit 0-9
        const digit = key.key_name.slice(6);
        this.pendingTextUpdate += digit;
      } else if (key.key_name.length === 1) {
        // Single character (letter A-Z)
        // Convert based on current input language
        const char = key.key_name;
        let result: string;

        if (this.inputLanguage === 'ru') {
          // Russian layout mapping (QWERTY -> ЙЦУКЕН)
          const ruLayout: Record<string, string> = {
            'Q': 'Й', 'W': 'Ц', 'E': 'У', 'R': 'К', 'T': 'Е', 'Y': 'Н', 'U': 'Г', 'I': 'Ш', 'O': 'Щ', 'P': 'З',
            'A': 'Ф', 'S': 'Ы', 'D': 'В', 'F': 'А', 'G': 'П', 'H': 'Р', 'J': 'О', 'K': 'Л', 'L': 'Д',
            'Z': 'Я', 'X': 'Ч', 'C': 'С', 'V': 'М', 'B': 'И', 'N': 'Т', 'M': 'Ь',
            'q': 'й', 'w': 'ц', 'e': 'у', 'r': 'к', 't': 'е', 'y': 'н', 'u': 'г', 'i': 'ш', 'o': 'щ', 'p': 'з',
            'a': 'ф', 's': 'ы', 'd': 'в', 'f': 'а', 'g': 'п', 'h': 'р', 'j': 'о', 'k': 'л', 'l': 'д',
            'z': 'я', 'x': 'ч', 'c': 'с', 'v': 'м', 'b': 'и', 'n': 'т', 'm': 'ь',
          };
          result = ruLayout[char] || char;
        } else {
          // English layout - use the character as-is
          result = char;
        }

        // Apply Caps Lock: if Caps Lock is ON, uppercase; if OFF, lowercase
        // Note: Caps Lock should apply AFTER layout conversion
        if (this.capsLock) {
          this.pendingTextUpdate += result.toUpperCase();
        } else {
          this.pendingTextUpdate += result.toLowerCase();
        }
      }
      // Ignore other special keys (Shift, Control, Alt, F-keys, etc.)
    },

    /**
     * Clear all intercepted keys and text
     */
    async clearKeys() {
      try {
        await invoke('clear_keys');
        this.interceptedKeys = [];
        this.latestKey = null;
        this.interceptedText = '';
        this.lastProcessedSeqNum = -1;
        this.fetchingKeys = false;  // Reset fetching flag
      } catch (error) {
        console.error('Failed to clear keys:', error);
      }
    },

    /**
     * Toggle blocking mode manually
     */
    async toggleBlocking() {
      try {
        const newState = await invoke<boolean>('toggle_blocking');
        const wasBlockingEnabled = this.blockingEnabled;
        this.blockingEnabled = newState;

        // Fetch status after toggle to get fresh values
        await this.fetchStatus();

        // Start or stop polling based on blocking state
        if (newState && !wasBlockingEnabled) {
          // Blocking enabled - polling is already running
        } else if (!newState && wasBlockingEnabled) {
          // Blocking disabled - polling will be skipped
        }
      } catch (error) {
        console.error('Failed to toggle blocking:', error);
      }
    },

    /**
     * Set always-on-top mode
     */
    async setAlwaysOnTop(enabled: boolean) {
      try {
        const newState = await invoke<boolean>('set_always_on_top', { enabled });
        this.alwaysOnTop = newState;
        // Apply window behavior after setting always-on-top
        await this.applyWindowBehavior();
      } catch (error) {
        console.error('Failed to set always-on-top:', error);
      }
    },

    /**
     * Set auto-show on block mode
     */
    async setAutoShowOnBlock(enabled: boolean) {
      try {
        const newState = await invoke<boolean>('set_auto_show_on_block', { enabled });
        this.autoShowOnBlock = newState;
      } catch (error) {
        console.error('Failed to set auto-show on block:', error);
      }
    },

    /**
     * Set hotkey mode
     */
    async setHotkeyMode(mode: 'background_blocking' | 'overlay_call') {
      try {
        await invoke('set_hotkey_mode', { mode });
        this.hotkeyMode = mode;
        // Reset overlay previous window saved flag when switching modes
        if (mode === 'background_blocking') {
          this.overlayPreviousWindowSaved = false;
        }
      } catch (error) {
        console.error('Failed to set hotkey mode:', error);
      }
    },

    /**
     * Show window overlay (for overlay call mode)
     */
    async showWindowOverlay() {
      try {
        console.log('[showWindowOverlay] Starting...');
        const window = getCurrentWindow();
        const isFocused = await window.isFocused();
        const isMinimized = await window.isMinimized();

        console.log('[showWindowOverlay] isFocused:', isFocused, 'isMinimized:', isMinimized, 'overlayPreviousWindowSaved:', this.overlayPreviousWindowSaved);

        // Check if window needs to be shown (minimized or hidden)
        // Windows can report minimized windows as "focused", so check minimized state first
        const needsShow = isMinimized || !(await window.isVisible());

        if (!needsShow && isFocused) {
          console.log('[showWindowOverlay] Window already visible and focused, returning early');
          return;
        }

        // Save previous window only on first call
        if (!this.overlayPreviousWindowSaved) {
          console.log('[showWindowOverlay] Saving previous window...');
          await invoke('save_previous_window');
          this.overlayPreviousWindowSaved = true;
        }

        // Show window on top (including fullscreen windows)
        // Focus is set by Rust using AttachThreadInput to avoid taskbar
        console.log('[showWindowOverlay] Calling show_window_on_top...');
        await invoke('show_window_on_top');
        console.log('[showWindowOverlay] Completed');
      } catch (error) {
        console.error('[Keyboard] Failed to show window overlay:', error);
      }
    },

    /**
     * Hide window and restore focus to previous window
     * Minimizes the window instead of just hiding it
     */
    async hideWindowAndRestoreFocus() {
      try {
        console.log('[hideWindowAndRestoreFocus] Starting...');
        await invoke('send_to_background_and_restore_focus');
        console.log('[hideWindowAndRestoreFocus] Completed');
        // Don't reset overlayPreviousWindowSaved - keep it for the session
      } catch (error) {
        console.error('[Keyboard] Failed to hide window and restore focus:', error);
      }
    },

    /**
     * Apply window behavior based on current settings and blocking state
     */
    async applyWindowBehavior() {
      try {
        const window = getCurrentWindow();

        // In overlay_call mode, window behavior is manual (Win+Esc)
        // Skip automatic behavior to prevent conflicts
        if (this.hotkeyMode === 'overlay_call') {
          console.log('[Keyboard] Skipping applyWindowBehavior in overlay_call mode');
          return;
        }

        if (this.alwaysOnTop) {
          // Always on top is enabled - window should always be on top
          await invoke('set_window_always_on_top', { enabled: true });
          await window.show();
          await window.setFocus();
        } else if (this.autoShowOnBlock) {
          // Auto-show is enabled - window position depends on blocking state
          if (this.blockingEnabled) {
            // Blocking enabled - bring to front
            try {
              // Save the current foreground window before showing our window
              await invoke('save_previous_window');
              await invoke('set_window_always_on_top', { enabled: true });
              await window.unminimize();
              await window.show();
              await window.setFocus();
            } catch (err) {
              console.error('[Keyboard] Error showing window:', err);
            }
          } else {
            // Blocking disabled - send to background and restore focus
            try {
              await invoke('send_to_background_and_restore_focus');
            } catch (err) {
              console.error('[Keyboard] Error sending to background:', err);
            }
          }
        } else {
          // Both disabled - remove always-on-top
          await invoke('set_window_always_on_top', { enabled: false });
        }
      } catch (error) {
        console.error('[Keyboard] Failed to apply window behavior:', error);
      }
    },

    /**
     * Start polling for status updates
     *
     * Note: Most state is now updated via events for instant response.
     * Polling is disabled - we rely on events only.
     */
    startPolling(interval: number = 500) {
      // Polling disabled - all state updates come via events
      // Keep method for compatibility but do nothing
    },

    /**
     * Stop polling for updates
     */
    stopPolling() {
      if (this.pollInterval !== null) {
        window.clearInterval(this.pollInterval);
        this.pollInterval = null;
      }
    },

    /**
     * Initialize the store with backend data
     */
    async initialize() {
      // Initial state fetch
      await this.fetchStatus();
      await this.fetchKeys();
      await this.fetchTtsStatus();

      // Setup event listeners for real-time updates
      this.setupTtsEventListeners();
      this.setupStateEventListeners();

      // Start polling (disabled - events only)
      this.startPolling();
    },

    /**
     * Fetch TTS status from backend
     */
    async fetchTtsStatus() {
      try {
        const status = await invoke<TtsStatus>('get_tts_status');
        this.isPlaying = status.is_speaking;
        this.continuousPlay = status.continuous_play;
        this.ttsProvider = status.provider as 'system' | 'openai';
      } catch (error) {
        console.error('Failed to fetch TTS status:', error);
      }
    },

    /**
     * Setup TTS event listeners for real-time updates
     */
    setupTtsEventListeners() {
      if (this.ttsHistoryInitialized) {
        return;
      }
      this.ttsHistoryInitialized = true;

      // Helper to find message by ID or fall back to newest queued/playing temp message
      const findMessage = (id: string) => {
        // First try to find by exact ID
        let msg = this.ttsHistory.find(m => m.id === id);
        if (msg) {
          return { msg, needsIdUpdate: false };
        }

        // If not found, look for newest temp message with queued status
        const tempMsg = this.ttsHistory.find(m =>
          m.id.startsWith('temp-') && (m.status === 'queued' || m.status === 'playing')
        );
        if (tempMsg) {
          tempMsg.id = id;
          return { msg: tempMsg, needsIdUpdate: true };
        }

        return { msg: null, needsIdUpdate: false };
      };

      // Listen for TTS started event
      listen('tts:started', (event: any) => {
        const { id } = event.payload;
        const { msg } = findMessage(id);
        if (msg) {
          msg.status = 'playing';
        }
      });

      // Listen for TTS completed event
      listen('tts:completed', (event: any) => {
        const { id } = event.payload;
        const { msg } = findMessage(id);
        if (msg) {
          msg.status = 'completed';
        }
      });

      // Listen for TTS failed event
      listen('tts:failed', (event: any) => {
        const { id, error } = event.payload;
        const { msg } = findMessage(id);
        if (msg) {
          msg.status = 'completed';
        }
        // Show toast for TTS errors
        this.showToast(
          typeof error === 'string' ? error : String(error),
          'error'
        );
      });

      // Listen for TTS cancelled event
      listen('tts:cancelled', (event: any) => {
        const { id } = event.payload;
        const { msg } = findMessage(id);
        if (msg) {
          msg.status = 'completed';
        }
      });
    },

    /**
     * Setup state event listeners for real-time updates from backend events
     * These events are sent via MPSC channel from the hook thread
     */
    setupStateEventListeners() {
      if (this.stateEventListenersInitialized) {
        return;
      }
      this.stateEventListenersInitialized = true;

      // Listen for blocking state changes
      listen<boolean>('blocking_changed', (event) => {
        const enabled = event.payload;
        const wasBlockingEnabled = this.blockingEnabled;
        this.blockingEnabled = enabled;

        // Apply window behavior when blocking state changes
        if (wasBlockingEnabled !== enabled) {
          this.applyWindowBehavior();
        }
      });

      // Listen for Caps Lock state changes
      listen<boolean>('caps_lock_changed', (event) => {
        const enabled = event.payload;
        this.capsLock = enabled;
      });

      // Listen for input language changes
      listen<string>('input_language_changed', (event) => {
        const lang = event.payload;
        this.inputLanguage = lang as 'ru' | 'en';
      });

      // Listen for Win key pressed state changes
      listen<boolean>('win_pressed_changed', (event) => {
        const enabled = event.payload;
        this.winPressed = enabled;
      });

      // Listen for always on top changes
      listen<boolean>('always_on_top_changed', (event) => {
        const enabled = event.payload;
        this.alwaysOnTop = enabled;
      });

      // Listen for auto show on block changes
      listen<boolean>('auto_show_on_block_changed', (event) => {
        const enabled = event.payload;
        this.autoShowOnBlock = enabled;
      });

      // Listen for hotkey mode changes
      listen<string>('hotkey_mode_changed', (event) => {
        this.hotkeyMode = event.payload as 'background_blocking' | 'overlay_call';
      });

      // Listen for continuous play changes
      listen<boolean>('continuous_play_changed', (event) => {
        const enabled = event.payload;
        this.continuousPlay = enabled;
      });

      // Listen for TTS provider changes
      listen<string>('tts_provider_changed', (event) => {
        const provider = event.payload;
        this.ttsProvider = provider as 'system' | 'openai';
      });

      // Listen for TTS config changes
      listen<{}>('tts_config_changed', () => {
        // Fetch updated TTS status when config changes
        this.fetchTtsStatus();
      });

      // Listen for intercepted keys - instant update via event
      listen<KeyEvent>('key_intercepted', (event) => {
        const key = event.payload;

        // Update lastProcessedSeqNum to prevent polling from fetching this key again
        this.lastProcessedSeqNum = Math.max(this.lastProcessedSeqNum, key.seq_num);

        // Update local state
        this.interceptedKeys.push(key);
        this.latestKey = key;

        // Process immediately if blocking is enabled
        if (this.blockingEnabled) {
          this.processInterceptedKey(key);
          this.scheduleTextUpdate();
        }
      });
    },

    /**
     * Refresh input language from system
     */
    async refreshInputLanguage() {
      try {
        const lang = await invoke<string>('get_input_language');
        this.inputLanguage = lang as 'ru' | 'en';
      } catch (error) {
        console.error('Failed to refresh input language:', error);
      }
    },

    /**
     * Toggle input language between RU and EN
     */
    async toggleInputLanguage() {
      try {
        const lang = await invoke<string>('toggle_input_language');
        this.inputLanguage = lang as 'ru' | 'en';
      } catch (error) {
        console.error('Failed to toggle input language:', error);
      }
    },

    /**
     * Cleanup when the store is no longer needed
     */
    destroy() {
      this.stopPolling();
    },

    // === TTS history actions ===

    /**
     * Fetch TTS history from backend
     * Smart merge: preserves optimistic updates and local status changes
     */
    async fetchTtsHistory() {
      try {
        const serverHistory = await invoke<TtsMessage[]>('get_tts_history');

        // Create a map of server messages by ID for quick lookup
        const serverMap = new Map<string, TtsMessage>();
        for (const msg of serverHistory) {
          serverMap.set(msg.id, msg);
        }

        // Merge with existing history, preserving local status for optimistic updates
        const mergedHistory: TtsMessage[] = [];

        // First, add server messages, updating status from local if exists
        for (const serverMsg of serverHistory) {
          const localMsg = this.ttsHistory.find(m => m.id === serverMsg.id);
          if (localMsg) {
            // Preserve local status (which might be more recent from events)
            mergedHistory.push({
              ...serverMsg,
              status: localMsg.status,
            });
          } else {
            mergedHistory.push(serverMsg);
          }
        }

        // Then, add optimistic updates (temp IDs) that aren't on server yet
        for (const localMsg of this.ttsHistory) {
          if (!serverMap.has(localMsg.id) && localMsg.id.startsWith('temp-')) {
            mergedHistory.push(localMsg);
          }
        }

        this.ttsHistory = mergedHistory;
      } catch (error) {
        console.error('Failed to fetch TTS history:', error);
      }
    },

    /**
     * Add a message to TTS history
     */
    async addTtsMessage(text: string): Promise<string> {
      try {
        const messageId = await invoke<string>('add_tts_message', { text });
        await this.fetchTtsHistory();
        return messageId;
      } catch (error) {
        console.error('Failed to add TTS message:', error);
        throw error;
      }
    },

    /**
     * Update TTS message status
     */
    async updateTtsMessageStatus(id: string, status: TtsMessageStatus) {
      try {
        await invoke('update_tts_message_status', { id, status });
        await this.fetchTtsHistory();
      } catch (error) {
        console.error('Failed to update TTS message status:', error);
      }
    },

    /**
     * Toggle TTS message locked state
     */
    async toggleTtsMessageLocked(id: string): Promise<boolean> {
      try {
        const locked = await invoke<boolean>('toggle_tts_message_locked', { id });
        await this.fetchTtsHistory();
        return locked;
      } catch (error) {
        console.error('Failed to toggle TTS message locked:', error);
        return false;
      }
    },

    /**
     * Delete a TTS message from history
     */
    async deleteTtsMessage(id: string) {
      try {
        await invoke('delete_tts_message', { id });
        await this.fetchTtsHistory();
      } catch (error) {
        console.error('Failed to delete TTS message:', error);
        throw error;
      }
    },

    /**
     * Clear all non-locked completed TTS messages
     */
    async clearTtsHistory() {
      try {
        await invoke('clear_tts_history');
        await this.fetchTtsHistory();
      } catch (error) {
        console.error('Failed to clear TTS history:', error);
      }
    },

    /**
     * Speak text with TTS and add to history (non-blocking - adds to queue)
     */
    speakTextWithHistory(text: string): void {
      // Generate temporary ID for optimistic update
      const tempId = `temp-${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;
      const timestamp = Math.floor(Date.now() / 1000);

      // Optimistic update: add message immediately with 'queued' status
      const tempMessage: TtsMessage = {
        id: tempId,
        text,
        timestamp,
        status: 'queued',
        locked: false,
      };
      this.ttsHistory.unshift(tempMessage);

      // Invoke without await - non-blocking
      invoke<string>('enqueue_tts', { text })
        .then((realId) => {
          // Update temp message with real ID
          const msgIndex = this.ttsHistory.findIndex(m => m.id === tempId);
          if (msgIndex !== -1) {
            this.ttsHistory[msgIndex].id = realId;
          }
        })
        .catch((error) => {
          // Remove optimistic update on error
          this.ttsHistory = this.ttsHistory.filter(m => m.id !== tempId);
          // Show toast for all TTS errors
          this.showToast(
            error instanceof Error ? error.message : String(error),
            'error'
          );
        });
    },

    /**
     * Cancel a TTS message by ID
     */
    async cancelTtsMessage(id: string): Promise<void> {
      try {
        await invoke('cancel_tts_message', { id });
        await this.fetchTtsHistory();
      } catch (error) {
        console.error('Failed to cancel TTS message:', error);
        throw error;
      }
    },

    /**
     * Repeat a TTS message from history
     */
    async repeatTtsMessage(id: string) {
      try {
        await invoke('repeat_tts_message', { id });
        await this.fetchTtsHistory();
      } catch (error) {
        console.error('Failed to repeat TTS message:', error);
        // Show toast for all TTS errors
        this.showToast(
          error instanceof Error ? error.message : String(error),
          'error'
        );
        throw error;
      }
    },

    /**
     * Show toast notification
     */
    showToast(message: string, type: 'error' | 'warning' | 'info' = 'error', duration: number = 5000) {
      this.toast = { show: true, message, type };
      if (duration > 0) {
        setTimeout(() => {
          this.toast.show = false;
        }, duration);
      }
    },

    /**
     * Hide toast notification
     */
    hideToast() {
      this.toast.show = false;
    },

    /**
     * Format timestamp to readable time
     */
    formatTimestamp(timestamp: number): string {
      const date = new Date(timestamp * 1000);
      const now = new Date();
      const diffMs = now.getTime() - date.getTime();
      const diffMins = Math.floor(diffMs / 60000);

      if (diffMins < 1) {
        return 'только что';
      } else if (diffMins < 60) {
        return `${diffMins} мин назад`;
      } else if (diffMins < 1440) {
        const hours = Math.floor(diffMins / 60);
        return `${hours} ч назад`;
      } else {
        return date.toLocaleDateString('ru-RU') + ' ' + date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
      }
    },
  },
});
