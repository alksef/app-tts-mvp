use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, AtomicU64, Ordering};
use std::sync::mpsc::Sender;

/// Hotkey behavior mode
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyMode {
    /// Background blocking mode - Win+Esc toggles key interception
    BackgroundBlocking,
    /// Overlay call mode - Win+Esc brings window to front without blocking
    OverlayCall,
}

impl Default for HotkeyMode {
    fn default() -> Self {
        Self::OverlayCall
    }
}

impl HotkeyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            HotkeyMode::BackgroundBlocking => "background_blocking",
            HotkeyMode::OverlayCall => "overlay_call",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "background_blocking" => Some(HotkeyMode::BackgroundBlocking),
            "overlay_call" => Some(HotkeyMode::OverlayCall),
            _ => None,
        }
    }
}

/// Application settings file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppSettingsFile {
    hotkey_mode: String,
}

impl Default for AppSettingsFile {
    fn default() -> Self {
        Self {
            hotkey_mode: HotkeyMode::default().as_str().to_string(),
        }
    }
}

/// Input language identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputLanguage {
    Ru,
    En,
}

impl Default for InputLanguage {
    fn default() -> Self {
        Self::En
    }
}

impl From<u32> for InputLanguage {
    fn from(hkl: u32) -> Self {
        // Russian keyboard layout: 0x0419 (LANG_Russian = 0x19)
        // English keyboard layout: 0x0409 (LANG_ENGLISH = 0x09)
        let lang_id = hkl & 0xFFFF;
        match lang_id {
            0x0419 => InputLanguage::Ru,
            _ => InputLanguage::En,
        }
    }
}

impl From<InputLanguage> for String {
    fn from(lang: InputLanguage) -> Self {
        match lang {
            InputLanguage::Ru => "ru".to_string(),
            InputLanguage::En => "en".to_string(),
        }
    }
}

impl InputLanguage {
    pub fn toggle(&self) -> Self {
        match self {
            InputLanguage::Ru => InputLanguage::En,
            InputLanguage::En => InputLanguage::Ru,
        }
    }
}

/// Events that can be sent from the hook thread to the main thread
#[derive(Debug, Clone)]
pub enum AppStateEvent {
    BlockingChanged(bool),
    CapsLockChanged(bool),
    InputLanguageChanged(InputLanguage),
    KeyIntercepted(KeyEvent),
    WinPressedChanged(bool),
    AlwaysOnTopChanged(bool),
    AutoShowOnBlockChanged(bool),
    ContinuousPlayChanged(bool),
    TtsProviderChanged(String),
    TtsConfigChanged,
    PluginsChanged(Vec<crate::plugins::PluginInfo>),
    HotkeyModeChanged(HotkeyMode),
    ShowWindowRequested,
}

// Re-export TTS types for use in other modules
pub use crate::tts::{TtsEngine, TtsProvider, TtsStatus, Voice};

// Re-export audio settings types
pub use crate::virtual_mic::AudioSettingsManager;
// Re-export plugin manager
pub use crate::plugins::PluginManager;

/// Maximum number of intercepted keys to keep in memory
const MAX_KEYS: usize = 100;
/// Maximum number of TTS messages to keep in history
const MAX_TTS_MESSAGES: usize = 100;

/// TTS message status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TtsMessageStatus {
    Queued,
    Playing,
    Completed,
}

impl Default for TtsMessageStatus {
    fn default() -> Self {
        Self::Queued
    }
}

/// TTS message in history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsMessage {
    pub id: String,
    pub text: String,
    pub timestamp: u64,
    pub status: TtsMessageStatus,
    pub locked: bool,
}

impl TtsMessage {
    pub fn new(text: String) -> Self {
        Self {
            id: format!("msg_{}", uuid::Uuid::new_v4().simple()),
            text,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            status: TtsMessageStatus::Queued,
            locked: false,
        }
    }

    #[allow(dead_code)]
    pub fn with_status(mut self, status: TtsMessageStatus) -> Self {
        self.status = status;
        self
    }

    #[allow(dead_code)]
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }
}

/// Represents a single keyboard event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyEvent {
    pub vk_code: u32,
    pub key_name: String,
    pub timestamp: u64,
    pub seq_num: u64,  // Global sequence number for ordering
}

impl KeyEvent {
    pub fn new(vk_code: u32, key_name: String, seq_num: u64) -> Self {
        Self {
            vk_code,
            key_name,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            seq_num,
        }
    }
}

/// Thread-safe application state shared between main thread and hook thread
#[derive(Clone)]
pub struct AppState {
    /// Whether keyboard blocking is currently enabled
    pub blocking_enabled: Arc<AtomicBool>,
    /// Tracks if Win key is currently pressed
    pub win_pressed: Arc<AtomicBool>,
    /// Whether window should always be on top
    pub always_on_top: Arc<AtomicBool>,
    /// Whether to auto-show window when blocking is enabled
    pub auto_show_on_block: Arc<AtomicBool>,
    /// Hotkey mode (BackgroundBlocking = 0, OverlayCall = 1)
    pub hotkey_mode: Arc<AtomicU32>,
    /// Whether Caps Lock is currently enabled
    pub caps_lock: Arc<AtomicBool>,
    /// Global counter for key event sequence numbers
    pub key_seq_counter: Arc<AtomicU64>,
    /// Queue of intercepted keyboard events (when window is NOT active)
    pub intercepted_keys: Arc<Mutex<VecDeque<KeyEvent>>>,
    /// Queue of keyboard events when app window IS active
    pub active_window_keys: Arc<Mutex<VecDeque<KeyEvent>>>,
    // === TTS state ===
    /// TTS engine for text-to-speech functionality
    pub tts_engine: Arc<Mutex<TtsEngine>>,
    /// Whether TTS is currently speaking
    pub tts_is_speaking: Arc<AtomicBool>,
    /// Whether continuous play mode is enabled
    pub continuous_play: Arc<AtomicBool>,
    /// TTS message history (queue + completed)
    pub tts_history: Arc<Mutex<Vec<TtsMessage>>>,
    /// Currently playing TTS message ID
    pub tts_current_message_id: Arc<Mutex<Option<String>>>,
    /// Whether TTS queue is currently being processed
    pub tts_queue_processing: Arc<AtomicBool>,
    /// Flag to cancel current TTS queue processing
    pub tts_queue_cancel: Arc<AtomicBool>,
    // === Input language state ===
    /// Current input language (RU/EN)
    pub input_language: Arc<AtomicU32>,
    // === Audio settings state ===
    /// Audio settings manager for speaker and virtual mic configuration
    /// Will be initialized in main.rs setup() with config_dir
    pub audio_settings_manager: Arc<Mutex<Option<AudioSettingsManager>>>,
    // === Plugin system state ===
    /// Plugin manager for dynamic plugins
    /// Will be initialized in main.rs setup() with exe directory
    pub plugin_manager: Arc<Mutex<Option<PluginManager>>>,
    // === Event channel ===
    /// Sender for events from hook thread to main thread
    /// Will be initialized in main.rs setup()
    pub event_sender: Arc<Mutex<Option<Sender<AppStateEvent>>>>,
    // === Window focus restoration ===
    /// Handle of the previously active window (before showing our app)
    /// Used for restoring focus when hiding the app
    pub previous_window_hwnd: Arc<AtomicIsize>,
    /// Handle of our app's main window
    /// Used for direct Windows API calls
    pub app_window_hwnd: Arc<AtomicIsize>,
    // === Config directory ===
    /// Config directory path for settings persistence
    pub config_dir: Arc<Mutex<Option<PathBuf>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            blocking_enabled: Arc::new(AtomicBool::new(false)),
            win_pressed: Arc::new(AtomicBool::new(false)),
            always_on_top: Arc::new(AtomicBool::new(false)),
            auto_show_on_block: Arc::new(AtomicBool::new(false)),
            hotkey_mode: Arc::new(AtomicU32::new(0)), // 0 = BackgroundBlocking
            caps_lock: Arc::new(AtomicBool::new(false)),
            key_seq_counter: Arc::new(AtomicU64::new(0)),
            intercepted_keys: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_KEYS))),
            active_window_keys: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_KEYS))),
            // TTS state
            tts_engine: Arc::new(Mutex::new(TtsEngine::new())),
            tts_is_speaking: Arc::new(AtomicBool::new(false)),
            continuous_play: Arc::new(AtomicBool::new(false)),
            tts_history: Arc::new(Mutex::new(Vec::with_capacity(MAX_TTS_MESSAGES))),
            tts_current_message_id: Arc::new(Mutex::new(None)),
            tts_queue_processing: Arc::new(AtomicBool::new(false)),
            tts_queue_cancel: Arc::new(AtomicBool::new(false)),
            // Input language state - initialize with current system layout
            input_language: Arc::new(AtomicU32::new(Self::get_system_keyboard_layout())),
            // Audio settings state - initialized later in main.rs setup()
            audio_settings_manager: Arc::new(Mutex::new(None)),
            // Plugin manager - initialized later in main.rs setup()
            plugin_manager: Arc::new(Mutex::new(None)),
            // Event sender - initialized later in main.rs setup()
            event_sender: Arc::new(Mutex::new(None)),
            // Window focus restoration - initialized to 0 (no previous window)
            previous_window_hwnd: Arc::new(AtomicIsize::new(0)),
            // App window handle - initialized later in main.rs setup()
            app_window_hwnd: Arc::new(AtomicIsize::new(0)),
            // Config directory - initialized later in main.rs setup()
            config_dir: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if blocking is enabled
    pub fn is_blocking_enabled(&self) -> bool {
        self.blocking_enabled.load(Ordering::Acquire)
    }

    /// Toggle blocking state
    pub fn toggle_blocking(&self) -> bool {
        let current = self.blocking_enabled.fetch_xor(true, Ordering::AcqRel);
        !current
    }

    /// Check if Win is pressed
    pub fn is_win_pressed(&self) -> bool {
        self.win_pressed.load(Ordering::Acquire)
    }

    /// Check if always on top mode is enabled
    pub fn is_always_on_top(&self) -> bool {
        self.always_on_top.load(Ordering::Acquire)
    }

    /// Set always on top mode
    pub fn set_always_on_top(&self, enabled: bool) {
        self.always_on_top.store(enabled, Ordering::Release);

        // Send event to main thread for UI update
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::AlwaysOnTopChanged(enabled));
            }
        }
    }

    /// Check if auto-show on block is enabled
    pub fn is_auto_show_on_block(&self) -> bool {
        self.auto_show_on_block.load(Ordering::Acquire)
    }

    /// Set auto-show on block mode
    pub fn set_auto_show_on_block(&self, enabled: bool) {
        self.auto_show_on_block.store(enabled, Ordering::Release);

        // Send event to main thread for UI update
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::AutoShowOnBlockChanged(enabled));
            }
        }
    }

    /// Get the current hotkey mode
    pub fn get_hotkey_mode(&self) -> HotkeyMode {
        let mode_value = self.hotkey_mode.load(Ordering::Acquire);
        match mode_value {
            0 => HotkeyMode::BackgroundBlocking,
            1 => HotkeyMode::OverlayCall,
            _ => HotkeyMode::BackgroundBlocking,
        }
    }

    /// Set the hotkey mode
    pub fn set_hotkey_mode(&self, mode: HotkeyMode) {
        let mode_value = match mode {
            HotkeyMode::BackgroundBlocking => 0,
            HotkeyMode::OverlayCall => 1,
        };
        self.hotkey_mode.store(mode_value, Ordering::Release);

        // Save settings to disk
        self.save_settings();

        // Send event to main thread for UI update
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::HotkeyModeChanged(mode));
            }
        }
    }

    /// Check if overlay call mode is enabled
    pub fn is_overlay_call_mode(&self) -> bool {
        self.get_hotkey_mode() == HotkeyMode::OverlayCall
    }

    /// Check if Caps Lock is enabled
    pub fn is_caps_lock(&self) -> bool {
        self.caps_lock.load(Ordering::Acquire)
    }

    /// Toggle Caps Lock state
    pub fn toggle_caps_lock(&self) -> bool {
        let current = self.caps_lock.fetch_xor(true, Ordering::AcqRel);
        !current
    }

    /// Set Caps Lock state directly
    pub fn set_caps_lock(&self, enabled: bool) {
        self.caps_lock.store(enabled, Ordering::Release);
    }

    /// Add an intercepted key to the queue
    pub fn add_key(&self, key: KeyEvent) {
        if let Ok(mut keys) = self.intercepted_keys.lock() {
            if keys.len() >= MAX_KEYS {
                keys.pop_front();
            }
            keys.push_back(key);
        }
    }

    /// Add an intercepted key with auto-incrementing sequence number
    /// Returns the created KeyEvent for event emission
    pub fn add_key_auto(&self, vk_code: u32, key_name: String) -> KeyEvent {
        let seq_num = self.key_seq_counter.fetch_add(1, Ordering::SeqCst);
        let key = KeyEvent::new(vk_code, key_name, seq_num);
        self.add_key(key.clone());
        key
    }

    /// Get all intercepted keys
    pub fn get_keys(&self) -> Vec<KeyEvent> {
        self.intercepted_keys
            .lock()
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get keys with seq_num greater than the given value (for incremental polling)
    pub fn get_keys_after(&self, after_seq_num: u64) -> Vec<KeyEvent> {
        self.intercepted_keys
            .lock()
            .map(|keys| keys.iter().filter(|k| k.seq_num > after_seq_num).cloned().collect())
            .unwrap_or_default()
    }

    /// Get the most recent intercepted key
    pub fn get_latest_key(&self) -> Option<KeyEvent> {
        self.intercepted_keys
            .lock()
            .map(|keys| keys.back().cloned())
            .unwrap_or(None)
    }

    /// Clear all intercepted keys
    pub fn clear_keys(&self) {
        if let Ok(mut keys) = self.intercepted_keys.lock() {
            keys.clear();
        }
        // Reset the sequence counter when clearing keys
        self.key_seq_counter.store(0, Ordering::SeqCst);
    }

    // === Active window keys methods ===

    /// Add a key pressed when app window is active
    pub fn add_active_window_key(&self, key: KeyEvent) {
        if let Ok(mut keys) = self.active_window_keys.lock() {
            if keys.len() >= MAX_KEYS {
                keys.pop_front();
            }
            keys.push_back(key);
        }
    }

    /// Get all active window keys
    pub fn get_active_window_keys(&self) -> Vec<KeyEvent> {
        self.active_window_keys
            .lock()
            .map(|keys| keys.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get the most recent active window key
    pub fn get_latest_active_window_key(&self) -> Option<KeyEvent> {
        self.active_window_keys
            .lock()
            .map(|keys| keys.back().cloned())
            .unwrap_or(None)
    }

    /// Clear all active window keys
    pub fn clear_active_window_keys(&self) {
        if let Ok(mut keys) = self.active_window_keys.lock() {
            keys.clear();
        }
    }

    // === TTS state methods ===

    /// Check if continuous play mode is enabled
    pub fn is_continuous_play(&self) -> bool {
        self.continuous_play.load(Ordering::Acquire)
    }

    /// Set continuous play mode
    pub fn set_continuous_play(&self, enabled: bool) {
        self.continuous_play.store(enabled, Ordering::Release);

        // Send event to main thread for UI update
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::ContinuousPlayChanged(enabled));
            }
        }
    }

    /// Toggle continuous play mode
    #[allow(dead_code)]
    pub fn toggle_continuous_play(&self) -> bool {
        let current = self.continuous_play.fetch_xor(true, Ordering::AcqRel);
        !current
    }

    /// Check if TTS is currently speaking
    #[allow(dead_code)]
    pub fn is_tts_speaking(&self) -> bool {
        self.tts_is_speaking.load(Ordering::Acquire)
    }

    // === Input language methods ===

    /// Get the current system keyboard layout handle for the foreground window
    fn get_system_keyboard_layout() -> u32 {
        use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
        use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};
        unsafe {
            // Get the foreground window and its thread ID
            let hwnd = GetForegroundWindow();
            let thread_id = GetWindowThreadProcessId(hwnd, None);
            // Get the keyboard layout for that thread
            GetKeyboardLayout(thread_id).0 as u32
        }
    }

    /// Refresh the input language from system
    pub fn refresh_input_language(&self) {
        let hkl = Self::get_system_keyboard_layout();
        self.input_language.store(hkl, Ordering::Release);
    }

    /// Get current input language as enum
    pub fn get_input_language(&self) -> InputLanguage {
        let hkl = self.input_language.load(Ordering::Acquire);
        InputLanguage::from(hkl)
    }

    /// Get current input language as raw HKL value
    #[allow(dead_code)]
    pub fn get_input_language_raw(&self) -> u32 {
        self.input_language.load(Ordering::Acquire)
    }

    /// Set input language (for programmatic switching)
    #[allow(dead_code)]
    pub fn set_input_language(&self, hkl: u32) {
        self.input_language.store(hkl, Ordering::Release);
    }

    // === TTS history methods ===

    /// Add a new message to TTS history
    pub fn add_tts_message(&self, text: String) -> String {
        let message = TtsMessage::new(text);
        let id = message.id.clone();

        if let Ok(mut history) = self.tts_history.lock() {
            // Remove oldest non-locked messages if we exceed the limit
            if history.len() >= MAX_TTS_MESSAGES {
                // First, try to remove only completed, non-locked messages from the end
                history.retain(|m| m.status != TtsMessageStatus::Completed || m.locked);

                // If still too many, remove the oldest completed messages (even locked ones at the very bottom)
                while history.len() >= MAX_TTS_MESSAGES {
                    // Find the oldest completed message (from the end of the list)
                    if let Some(pos) = history.iter().rposition(|m| m.status == TtsMessageStatus::Completed) {
                        history.remove(pos);
                    } else {
                        break; // No more completed messages to remove
                    }
                }
            }

            history.push(message);
        }

        id
    }

    /// Get all TTS messages sorted by priority (playing > queued > locked completed > other completed)
    pub fn get_tts_history(&self) -> Vec<TtsMessage> {
        if let Ok(history) = self.tts_history.lock() {
            let mut playing: Vec<TtsMessage> = Vec::new();
            let mut queued: Vec<TtsMessage> = Vec::new();
            let mut locked_completed: Vec<TtsMessage> = Vec::new();
            let mut other_completed: Vec<TtsMessage> = Vec::new();

            for msg in history.iter() {
                match msg.status {
                    TtsMessageStatus::Playing => playing.push(msg.clone()),
                    TtsMessageStatus::Queued => queued.push(msg.clone()),
                    TtsMessageStatus::Completed => {
                        if msg.locked {
                            locked_completed.push(msg.clone());
                        } else {
                            other_completed.push(msg.clone());
                        }
                    }
                }
            }

            // Combine in order: playing, queued, locked completed (newest first), other completed (newest first)
            let mut result = playing;
            result.extend(queued);
            locked_completed.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            result.extend(locked_completed);
            other_completed.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            result.extend(other_completed);

            result
        } else {
            Vec::new()
        }
    }

    /// Update message status
    pub fn update_tts_message_status(&self, id: &str, status: TtsMessageStatus) {
        if let Ok(mut history) = self.tts_history.lock() {
            if let Some(msg) = history.iter_mut().find(|m| m.id == id) {
                msg.status = status;
            }
        }
    }

    /// Toggle message locked state
    pub fn toggle_tts_message_locked(&self, id: &str) -> bool {
        if let Ok(mut history) = self.tts_history.lock() {
            if let Some(msg) = history.iter_mut().find(|m| m.id == id) {
                msg.locked = !msg.locked;
                return msg.locked;
            }
        }
        false
    }

    /// Delete a message from history
    pub fn delete_tts_message(&self, id: &str) -> bool {
        if let Ok(mut history) = self.tts_history.lock() {
            if let Some(pos) = history.iter().position(|m| m.id == id) {
                // Don't delete if currently playing
                if history[pos].status == TtsMessageStatus::Playing {
                    return false;
                }
                history.remove(pos);
                return true;
            }
        }
        false
    }

    /// Clear all non-locked completed messages
    pub fn clear_tts_history(&self) {
        if let Ok(mut history) = self.tts_history.lock() {
            history.retain(|m| m.locked || m.status == TtsMessageStatus::Playing || m.status == TtsMessageStatus::Queued);
        }
    }

    /// Get current playing message ID
    #[allow(dead_code)]
    pub fn get_current_tts_message_id(&self) -> Option<String> {
        if let Ok(current_id) = self.tts_current_message_id.lock() {
            current_id.clone()
        } else {
            None
        }
    }

    /// Set current playing message ID
    pub fn set_current_tts_message_id(&self, id: Option<String>) {
        if let Ok(mut current_id) = self.tts_current_message_id.lock() {
            *current_id = id;
        }
    }

    // === Event emission helpers ===

    /// Emit TTS provider changed event
    pub fn emit_tts_provider_changed(&self, provider: String) {
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::TtsProviderChanged(provider));
            }
        }
    }

    /// Emit TTS config changed event
    pub fn emit_tts_config_changed(&self) {
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::TtsConfigChanged);
            }
        }
    }

    /// Emit plugins changed event
    pub fn emit_plugins_changed(&self, plugins: Vec<crate::plugins::PluginInfo>) {
        if let Ok(sender) = self.event_sender.lock() {
            if let Some(ref tx) = *sender {
                let _ = tx.send(AppStateEvent::PluginsChanged(plugins));
            }
        }
    }

    // === Window focus restoration methods ===

    /// Set the previous window handle (for focus restoration)
    pub fn set_previous_window(&self, hwnd: isize) {
        self.previous_window_hwnd.store(hwnd, Ordering::Release);
    }

    /// Get the previous window handle
    pub fn get_previous_window(&self) -> isize {
        self.previous_window_hwnd.load(Ordering::Acquire)
    }

    /// Set the app window handle
    pub fn set_app_window_hwnd(&self, hwnd: isize) {
        self.app_window_hwnd.store(hwnd, Ordering::Release);
    }

    /// Get the app window handle
    pub fn get_app_window_hwnd(&self) -> isize {
        self.app_window_hwnd.load(Ordering::Acquire)
    }

    // === Settings persistence methods ===

    /// Set config directory for settings persistence
    pub fn set_config_dir(&self, config_dir: PathBuf) {
        if let Ok(mut dir) = self.config_dir.lock() {
            *dir = Some(config_dir);
        }
    }

    /// Load application settings from file
    pub fn load_settings(&self) {
        if let Ok(dir_guard) = self.config_dir.lock() {
            if let Some(ref config_dir) = *dir_guard {
                let settings_path = config_dir.join("app_settings.json");
                if settings_path.exists() {
                    if let Ok(content) = fs::read_to_string(&settings_path) {
                        if let Ok(settings) = serde_json::from_str::<AppSettingsFile>(&content) {
                            // Load hotkey mode
                            if let Some(mode) = HotkeyMode::from_str(&settings.hotkey_mode) {
                                let mode_value = match mode {
                                    HotkeyMode::BackgroundBlocking => 0,
                                    HotkeyMode::OverlayCall => 1,
                                };
                                self.hotkey_mode.store(mode_value, Ordering::Release);
                                eprintln!("[AppState] Loaded hotkey_mode: {:?}", mode);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Save application settings to file
    pub fn save_settings(&self) {
        if let Ok(dir_guard) = self.config_dir.lock() {
            if let Some(ref config_dir) = *dir_guard {
                let current_mode = self.get_hotkey_mode();
                let settings = AppSettingsFile {
                    hotkey_mode: current_mode.as_str().to_string(),
                };
                let settings_path = config_dir.join("app_settings.json");
                if let Ok(content) = serde_json::to_string_pretty(&settings) {
                    let _ = fs::write(&settings_path, content);
                    eprintln!("[AppState] Saved hotkey_mode: {:?}", current_mode);
                }
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
