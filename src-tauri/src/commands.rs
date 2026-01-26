use crate::state::{AppState, HotkeyMode, InputLanguage, KeyEvent, TtsStatus, TtsMessage, TtsMessageStatus, Voice};
use crate::openai::{OpenAIConfig, OpenAIVoice};
use crate::localhost::{LocalhostConfig, LocalhostVoice};
use crate::virtual_mic::{OutputDeviceInfo, VirtualDeviceInfo};
use crate::plugins::{PluginInfo, SerializablePluginStatus};
use serde::{Deserialize, Serialize};
use tauri::Emitter;

/// Response structure for get_status command
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    pub blocking_enabled: bool,
    pub win_pressed: bool,
    pub always_on_top: bool,
    pub auto_show_on_block: bool,
    pub caps_lock: bool,
    pub input_language: String,
    pub hotkey_mode: String,
}

/// Response structure for get_intercepted_keys command
#[derive(Debug, Serialize, Deserialize)]
pub struct KeysResponse {
    pub keys: Vec<KeyEvent>,
    pub latest_key: Option<KeyEvent>,
}

/// Get the current status of the keyboard interceptor
///
/// Returns whether blocking is enabled and if Win is currently pressed
#[tauri::command]
pub fn get_status(state: tauri::State<'_, AppState>) -> StatusResponse {
    // Don't refresh input language on every poll - it's expensive
    // It's updated separately when language actually changes
    // state.refresh_input_language();

    StatusResponse {
        blocking_enabled: state.is_blocking_enabled(),
        win_pressed: state.is_win_pressed(),
        always_on_top: state.is_always_on_top(),
        auto_show_on_block: state.is_auto_show_on_block(),
        caps_lock: state.is_caps_lock(),
        input_language: String::from(state.get_input_language()),
        hotkey_mode: state.get_hotkey_mode().as_str().to_string(),
    }
}

/// Get all intercepted keys from the queue
///
/// Returns the full list of intercepted keys and the most recent one
/// If after_seq_num is provided, only returns keys with seq_num > after_seq_num
#[tauri::command]
pub fn get_intercepted_keys(state: tauri::State<'_, AppState>, after_seq_num: Option<u64>) -> KeysResponse {
    let keys = match after_seq_num {
        Some(seq) => state.get_keys_after(seq),
        None => state.get_keys(),
    };
    let latest_key = state.get_latest_key();

    KeysResponse { keys, latest_key }
}

/// Clear the intercepted keys queue
#[tauri::command]
pub fn clear_keys(state: tauri::State<'_, AppState>) {
    state.clear_keys();
}

/// Toggle blocking mode manually (optional UI control)
///
/// This provides an alternative way to toggle blocking mode besides Win+Esc.
/// Window behavior is handled by the frontend to ensure consistency
/// whether blocking is toggled via UI or Win+Esc hotkey.
#[tauri::command]
pub fn toggle_blocking(state: tauri::State<'_, AppState>) -> bool {
    state.toggle_blocking()
}

// === Active window keys commands ===

/// Get all keys pressed when the app window is active
///
/// Returns the full list of active window keys and the most recent one
#[tauri::command]
pub fn get_active_window_keys(state: tauri::State<'_, AppState>) -> KeysResponse {
    let keys = state.get_active_window_keys();
    let latest_key = state.get_latest_active_window_key();

    KeysResponse { keys, latest_key }
}

/// Clear the active window keys queue
#[tauri::command]
pub fn clear_active_window_keys(state: tauri::State<'_, AppState>) {
    state.clear_active_window_keys();
}

// === Window state commands ===

/// Set always-on-top mode
///
/// When enabled, the window will always stay above all other windows.
/// Window behavior is handled by the frontend.
#[tauri::command]
pub fn set_always_on_top(state: tauri::State<'_, AppState>, enabled: bool) -> bool {
    state.set_always_on_top(enabled);
    enabled
}

/// Set auto-show on block mode
///
/// When enabled, the window will automatically come to the foreground when blocking is enabled
#[tauri::command]
pub fn set_auto_show_on_block(state: tauri::State<'_, AppState>, enabled: bool) -> bool {
    state.set_auto_show_on_block(enabled);
    enabled
}

/// Get the current hotkey mode
#[tauri::command]
pub fn get_hotkey_mode(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.get_hotkey_mode().as_str().to_string())
}

/// Set the hotkey mode
#[tauri::command]
pub fn set_hotkey_mode(state: tauri::State<'_, AppState>, mode: String) -> Result<(), String> {
    let mode_enum = HotkeyMode::from_str(mode.as_str())
        .ok_or_else(|| format!("Invalid hotkey mode: {}", mode))?;
    state.set_hotkey_mode(mode_enum);
    Ok(())
}

/// Set always-on-top mode for the window
///
/// This sets the window to always stay above all other windows
#[tauri::command]
pub fn set_window_always_on_top(window: tauri::Window, enabled: bool) -> Result<(), String> {
    eprintln!("[set_window_always_on_top] START - enabled: {}", enabled);
    let result = window.set_always_on_top(enabled)
        .map_err(|e| {
            eprintln!("[set_window_always_on_top] FAILED: {:?}", e);
            format!("Failed to set always-on-top: {:?}", e)
        });
    eprintln!("[set_window_always_on_top] SUCCESS - enabled: {}", enabled);
    result
}

/// Show the window (bring to foreground)
#[tauri::command]
pub fn show_window(window: tauri::Window) -> Result<(), String> {
    window.show()
        .map_err(|e| format!("Failed to show window: {:?}", e))?;
    window.set_focus()
        .map_err(|e| format!("Failed to set focus: {:?}", e))
}

/// Show window on top of fullscreen windows (temporary)
/// Uses HWND_TOPMOST to show above fullscreen windows, then removes it after 100ms
/// Uses AttachThreadInput to set focus without showing taskbar
#[tauri::command]
pub fn show_window_on_top(window: tauri::Window) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{SetWindowPos, HWND_TOPMOST, HWND_NOTOPMOST, SWP_NOMOVE, SWP_NOSIZE, GetForegroundWindow, SetForegroundWindow, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use std::thread;
    use std::time::Duration;

    eprintln!("[show_window_on_top] Setting HWND_TOPMOST...");

    let hwnd_raw = window.hwnd()
        .map_err(|e| format!("Failed to get window handle: {:?}", e))?
        .0 as isize;

    let hwnd = HWND(hwnd_raw as *mut _);

    unsafe {
        // Set to topmost to appear above fullscreen windows
        let _ = SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    }

    // Unminimize and show the window
    window.unminimize()
        .map_err(|e| format!("Failed to unminimize window: {:?}", e))?;
    window.show()
        .map_err(|e| format!("Failed to show window: {:?}", e))?;

    // Remove topmost after 100ms and set focus using AttachThreadInput
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        eprintln!("[show_window_on_top] Removing HWND_TOPMOST...");
        unsafe {
            let _ = SetWindowPos(HWND(hwnd_raw as *mut _), HWND_NOTOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }

        // Use AttachThreadInput to set focus without triggering taskbar
        eprintln!("[show_window_on_top] Setting focus using AttachThreadInput...");
        unsafe {
            // Get the foreground window (fullscreen app)
            let foreground_hwnd = GetForegroundWindow();

            // Get thread IDs
            let foreground_thread = GetWindowThreadProcessId(foreground_hwnd, None);
            let current_thread = GetCurrentThreadId();

            eprintln!("[show_window_on_top] foreground_thread: {:?}, current_thread: {:?}", foreground_thread, current_thread);

            // Attach to foreground window's thread to bypass SetForegroundWindow restrictions
            if AttachThreadInput(foreground_thread, current_thread, true).as_bool() {
                eprintln!("[show_window_on_top] AttachThreadInput succeeded");

                // Bring our window to foreground
                let result = SetForegroundWindow(HWND(hwnd_raw as *mut _));
                eprintln!("[show_window_on_top] SetForegroundWindow result: {:?}", result);

                // Detach from the thread
                let _ = AttachThreadInput(foreground_thread, current_thread, false);
                eprintln!("[show_window_on_top] Detached from thread");
            } else {
                eprintln!("[show_window_on_top] AttachThreadInput failed, trying SetForegroundWindow directly");
                let _ = SetForegroundWindow(HWND(hwnd_raw as *mut _));
            }
        }

        eprintln!("[show_window_on_top] Window shown on top with focus");
    });

    eprintln!("[show_window_on_top] Window shown on top");
    Ok(())
}

/// Hide the window (send to background)
#[tauri::command]
pub fn hide_window(window: tauri::Window) -> Result<(), String> {
    window.hide()
        .map_err(|e| format!("Failed to hide window: {:?}", e))
}

/// Set window to ignore cursor events (prevents focus stealing)
#[tauri::command]
pub fn set_ignore_cursor_events(window: tauri::Window, enabled: bool) -> Result<(), String> {
    window.set_ignore_cursor_events(enabled)
        .map_err(|e| format!("Failed to set ignore cursor events: {:?}", e))
}

/// Test command to verify invoke works
#[tauri::command]
pub fn test_invoke() -> String {
    eprintln!("[test_invoke] Called successfully!");
    "Invoke works!".to_string()
}

/// Save the currently active window handle (for later focus restoration)
#[tauri::command]
pub fn save_previous_window(state: tauri::State<'_, AppState>) {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    eprintln!("[save_previous_window] Starting...");
    unsafe {
        let hwnd = GetForegroundWindow();
        eprintln!("[save_previous_window] Saving HWND: {}", hwnd.0 as isize);
        state.set_previous_window(hwnd.0 as isize);
        eprintln!("[save_previous_window] Saved successfully");
    }
}

/// Send window to background and restore focus to previous window
#[tauri::command]
pub fn send_to_background_and_restore_focus(window: tauri::Window, state: tauri::State<'_, AppState>) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE, GetForegroundWindow};

    eprintln!("[send_to_background] Starting...");

    // Log current foreground window for comparison
    let current_foreground = unsafe { GetForegroundWindow().0 as isize };
    eprintln!("[send_to_background] Current foreground HWND: {}", current_foreground);

    // Get the previous window handle
    let previous_hwnd = state.get_previous_window();
    eprintln!("[send_to_background] Saved previous window HWND: {}", previous_hwnd);

    // Remove always-on-top from our window first - this allows it to be covered by other windows
    eprintln!("[send_to_background] Removing always-on-top...");
    let _ = window.set_always_on_top(false);
    eprintln!("[send_to_background] Always-on-top removed");

    if previous_hwnd == 0 {
        // No previous window saved - just minimize our window to send it to background
        eprintln!("[send_to_background] No previous window, minimizing...");
        let _ = window.minimize();
        eprintln!("[send_to_background] Minimized (no previous window)");
        return Ok(());
    }

    let previous_hwnd = HWND(previous_hwnd as *mut core::ffi::c_void);

    unsafe {
        // Check if the previous window still exists
        if IsWindow(previous_hwnd).as_bool() {
            eprintln!("[send_to_background] Previous window exists, restoring focus...");

            // Restore the previous window if it was minimized
            let _ = ShowWindow(previous_hwnd, SW_RESTORE);

            // Bring it to foreground
            let _ = SetForegroundWindow(previous_hwnd);

            // Minimize our window to send it to background (user can restore it from taskbar)
            eprintln!("[send_to_background] Minimizing our window...");
            let _ = window.minimize();

            eprintln!("[send_to_background] Focus restored and window minimized");
            Ok(())
        } else {
            // Previous window no longer exists - just minimize our window
            eprintln!("[send_to_background] Previous window no longer exists, minimizing...");
            let _ = window.minimize();
            eprintln!("[send_to_background] Minimized (previous window gone)");
            Ok(())
        }
    }
}

/// Hide overlay window and restore focus (for overlay call mode)
/// Unlike send_to_background_and_restore_focus, this does NOT minimize the window
#[tauri::command]
pub fn hide_overlay_and_restore_focus(window: tauri::Window, state: tauri::State<'_, AppState>) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{IsWindow, SetForegroundWindow, ShowWindow, SW_RESTORE, SetWindowPos, HWND_NOTOPMOST, SWP_NOMOVE, SWP_NOSIZE, GetWindowThreadProcessId};
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};

    eprintln!("[hide_overlay] Starting...");

    // Get the previous window handle
    let previous_hwnd = state.get_previous_window();
    eprintln!("[hide_overlay] Previous window HWND: {}", previous_hwnd);

    // Get our app window handle
    let app_hwnd_raw = state.get_app_window_hwnd();
    let app_hwnd = HWND(app_hwnd_raw as *mut core::ffi::c_void);

    if previous_hwnd == 0 {
        // No previous window saved - remove always-on-top and hide
        eprintln!("[hide_overlay] No previous window, removing always-on-top and hiding...");

        // Use SetWindowPos to remove from top
        unsafe {
            let _ = SetWindowPos(app_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE);
        }

        let _ = window.hide();
        eprintln!("[hide_overlay] Hidden (no previous window)");
        return Ok(());
    }

    let previous_hwnd = HWND(previous_hwnd as *mut core::ffi::c_void);

    unsafe {
        // Check if the previous window still exists
        if IsWindow(previous_hwnd).as_bool() {
            eprintln!("[hide_overlay] Previous window exists, restoring focus...");

            // Restore the previous window if it was minimized
            let _ = ShowWindow(previous_hwnd, SW_RESTORE);

            // Get thread IDs for AttachThreadInput technique
            let foreground_thread = GetWindowThreadProcessId(previous_hwnd, None);
            let current_thread = GetCurrentThreadId();

            eprintln!("[hide_overlay] Using AttachThreadInput - foreground_thread: {:?}, current_thread: {:?}", foreground_thread, current_thread);

            // Attach to the foreground window's thread to bypass SetForegroundWindow restrictions
            if AttachThreadInput(foreground_thread, current_thread, true).as_bool() {
                eprintln!("[hide_overlay] AttachThreadInput succeeded");

                // Bring the previous window to foreground
                let result = SetForegroundWindow(previous_hwnd);
                eprintln!("[hide_overlay] SetForegroundWindow result: {:?}", result);

                // Detach from the thread
                let _ = AttachThreadInput(foreground_thread, current_thread, false);
                eprintln!("[hide_overlay] Detached from thread");
            } else {
                eprintln!("[hide_overlay] AttachThreadInput failed, trying SetForegroundWindow directly");
                let _ = SetForegroundWindow(previous_hwnd);
            }

            // Then remove always-on-top using SetWindowPos with HWND_NOTOPMOST
            eprintln!("[hide_overlay] Removing always-on-top with SetWindowPos...");
            let result = SetWindowPos(app_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE);

            if result.is_ok() {
                eprintln!("[hide_overlay] SetWindowPos succeeded - focus restored and always-on-top removed");
            } else {
                eprintln!("[hide_overlay] SetWindowPos failed: {:?}", result);
            }

            Ok(())
        } else {
            // Previous window no longer exists - remove always-on-top and hide
            eprintln!("[hide_overlay] Previous window no longer exists, hiding...");

            let _ = SetWindowPos(app_hwnd, HWND_NOTOPMOST, 0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE);

            let _ = window.hide();
            eprintln!("[hide_overlay] Hidden (previous window gone)");
            Ok(())
        }
    }
}

// === TTS commands ===

/// Speak text using the current TTS provider
#[tauri::command]
pub async fn speak_text(state: tauri::State<'_, AppState>, text: String) -> Result<(), String> {
    if text.is_empty() {
        return Err("Cannot speak empty text".to_string());
    }

    // Set speaking flag
    state.tts_is_speaking.store(true, std::sync::atomic::Ordering::Release);

    // Get the TTS engine and speak - handle poisoned mutex
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    let result = engine.speak(&text);

    // Clear speaking flag on error, but keep it true on success
    // (it will be cleared when speech completes or is stopped)
    if result.is_err() {
        state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);
    }

    result
}

/// Stop any current speech
#[tauri::command]
pub fn stop_speech(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    let result = engine.stop();

    // Always clear speaking flag when stopping
    state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);

    result
}

/// Set continuous play mode
#[tauri::command]
pub fn set_continuous_play(state: tauri::State<'_, AppState>, enabled: bool) -> bool {
    state.set_continuous_play(enabled);
    enabled
}

/// Set the TTS provider
#[tauri::command]
pub fn set_tts_provider(state: tauri::State<'_, AppState>, provider: String) -> Result<(), String> {
    use crate::state::TtsProvider;

    let provider_enum: TtsProvider = provider.clone().into();

    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    engine.set_provider(provider_enum);

    // Emit provider changed event
    state.emit_tts_provider_changed(provider);

    Ok(())
}

/// Set the OpenAI API key
#[tauri::command]
pub fn set_openai_key(state: tauri::State<'_, AppState>, key: String) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    engine.set_openai_key(key.clone());

    // Emit config changed event
    state.emit_tts_config_changed();

    Ok(())
}

/// Get the current TTS status
#[tauri::command]
pub fn get_tts_status(state: tauri::State<'_, AppState>) -> TtsStatus {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    let mut status = engine.get_status();
    status.continuous_play = state.is_continuous_play();
    status
}

// === Input language commands ===

/// Get the current input language
#[tauri::command]
pub fn get_input_language(state: tauri::State<'_, AppState>) -> String {
    state.refresh_input_language();
    String::from(state.get_input_language())
}

/// Toggle input language between RU and EN programmatically
#[tauri::command]
pub fn toggle_input_language(state: tauri::State<'_, AppState>) -> String {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        ActivateKeyboardLayout, HKL, KLF_ACTIVATE, KLF_SETFORPROCESS,
    };

    let current = state.get_input_language();
    let new_lang = current.toggle();

    // HKL values for Russian and English layouts
    let hkl_value = match new_lang {
        InputLanguage::Ru => 0x04190419u32, // Russian
        InputLanguage::En => 0x04090409u32, // English (US)
    };

    unsafe {
        let hkl = HKL(hkl_value as *mut core::ffi::c_void);

        // Activate for current process
        let _ = ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS);

        // Try to activate for the system as well
        let _ = ActivateKeyboardLayout(hkl, KLF_ACTIVATE);
    }

    // Wait a moment and refresh our state
    std::thread::sleep(std::time::Duration::from_millis(50));
    state.refresh_input_language();

    String::from(new_lang)
}

// === TTS history commands ===

/// Get all TTS messages from history
#[tauri::command]
pub fn get_tts_history(state: tauri::State<'_, AppState>) -> Vec<TtsMessage> {
    state.get_tts_history()
}

/// Add a new message to TTS history
#[tauri::command]
pub fn add_tts_message(state: tauri::State<'_, AppState>, text: String) -> String {
    state.add_tts_message(text)
}

/// Update TTS message status
#[tauri::command]
pub fn update_tts_message_status(state: tauri::State<'_, AppState>, id: String, status: String) -> Result<(), String> {
    let status_enum = match status.to_lowercase().as_str() {
        "queued" => TtsMessageStatus::Queued,
        "playing" => TtsMessageStatus::Playing,
        "completed" => TtsMessageStatus::Completed,
        _ => return Err(format!("Invalid status: {}", status)),
    };
    state.update_tts_message_status(&id, status_enum);
    Ok(())
}

/// Toggle TTS message locked state
#[tauri::command]
pub fn toggle_tts_message_locked(state: tauri::State<'_, AppState>, id: String) -> bool {
    state.toggle_tts_message_locked(&id)
}

/// Delete a TTS message from history
#[tauri::command]
pub fn delete_tts_message(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    if state.delete_tts_message(&id) {
        Ok(())
    } else {
        Err("Cannot delete message (it may be currently playing)".to_string())
    }
}

/// Clear all non-locked completed TTS messages
#[tauri::command]
pub fn clear_tts_history(state: tauri::State<'_, AppState>) {
    state.clear_tts_history()
}

/// Speak text with TTS and add to history (non-blocking - adds to queue)
#[tauri::command]
pub async fn enqueue_tts(state: tauri::State<'_, AppState>, app: tauri::AppHandle, text: String) -> Result<String, String> {
    let start = std::time::Instant::now();
    eprintln!("[enqueue_tts] START");

    if text.is_empty() {
        return Err("Cannot speak empty text".to_string());
    }

    // Clone app early before any checks
    let app_clone = app.clone();
    eprintln!("[enqueue_tts] After app.clone: {:?}", start.elapsed());

    // Broadcast to plugins first
    if let Ok(mut plugin_manager) = state.plugin_manager.lock() {
        if let Some(manager) = plugin_manager.as_mut() {
            let changed = manager.broadcast_text(&text);
            if changed {
                // Emit plugins changed event if any plugin was disabled due to error
                let plugins = manager.get_plugins();
                state.emit_plugins_changed(plugins);
            }
        }
    }
    eprintln!("[enqueue_tts] After plugin broadcast: {:?}", start.elapsed());

    // Add to history with Queued status
    let message_id = state.add_tts_message(text.clone());
    eprintln!("[enqueue_tts] After add_tts_message: {:?}", start.elapsed());

    // Emit enqueued event
    let _ = app.emit("tts:enqueued", serde_json::json!({
        "id": message_id,
        "text": text
    }));
    eprintln!("[enqueue_tts] After emit: {:?}", start.elapsed());

    // Clone state for background task
    let state_clone: AppState = (*state).clone();
    eprintln!("[enqueue_tts] After state.clone(): {:?}", start.elapsed());

    // Start queue processing in background if not already running
    if !state_clone.tts_queue_processing.load(std::sync::atomic::Ordering::Acquire) {
        state_clone.tts_queue_processing.store(true, std::sync::atomic::Ordering::Release);

        // Use std::thread instead of tokio::spawn to avoid blocking async runtime
        std::thread::spawn(move || {
            process_tts_queue_sync(state_clone, app_clone);
        });
    }
    eprintln!("[enqueue_tts] After thread spawn: {:?}", start.elapsed());

    // Return message ID immediately (non-blocking)
    eprintln!("[enqueue_tts] END: {:?}", start.elapsed());
    Ok(message_id)
}

/// Process TTS queue - plays messages sequentially (synchronous, runs in dedicated thread)
fn process_tts_queue_sync(state: AppState, app: tauri::AppHandle) {
    loop {
        // Check if we should stop processing
        if state.tts_queue_cancel.load(std::sync::atomic::Ordering::Acquire) {
            state.tts_queue_cancel.store(false, std::sync::atomic::Ordering::Release);
            state.tts_queue_processing.store(false, std::sync::atomic::Ordering::Release);
            break;
        }

        // Find next queued message
        let next_message = {
            let history = state.tts_history.lock().unwrap();
            history.iter()
                .filter(|m| m.status == TtsMessageStatus::Queued)
                .min_by_key(|m| m.timestamp)
                .map(|m| (m.id.clone(), m.text.clone()))
        };

        match next_message {
            Some((msg_id, msg_text)) => {
                // Set as playing
                state.update_tts_message_status(&msg_id, TtsMessageStatus::Playing);
                state.set_current_tts_message_id(Some(msg_id.clone()));
                state.tts_is_speaking.store(true, std::sync::atomic::Ordering::Release);

                // Emit started event
                let _ = app.emit("tts:started", serde_json::json!({
                    "id": msg_id,
                    "text": msg_text
                }));

                // Get TTS engine and speak
                let result = {
                    let lock_result = state.tts_engine.lock();
                    let engine = match lock_result {
                        Ok(guard) => guard,
                        Err(poisoned) => {
                            eprintln!("TTS engine mutex was poisoned, recovering...");
                            poisoned.into_inner()
                        }
                    };
                    engine.speak(&msg_text)
                };

                // For OpenAI TTS, playback happens in background thread
                // Wait for playback to complete before processing next message
                if result.is_ok() {
                    // Poll the is_speaking flag until playback completes
                    let mut sleep_count = 0;
                    loop {
                        // Check is_speaking flag
                        let still_speaking = {
                            let lock_result = state.tts_engine.lock();
                            match lock_result {
                                Ok(engine) => engine.is_speaking(),
                                Err(poisoned) => {
                                    let engine = poisoned.into_inner();
                                    engine.is_speaking()
                                }
                            }
                        };

                        if !still_speaking {
                            break;
                        }

                        if state.tts_queue_cancel.load(std::sync::atomic::Ordering::Acquire) {
                            state.update_tts_message_status(&msg_id, TtsMessageStatus::Completed);
                            state.tts_queue_cancel.store(false, std::sync::atomic::Ordering::Release);
                            state.tts_queue_processing.store(false, std::sync::atomic::Ordering::Release);
                            state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);
                            let _ = app.emit("tts:cancelled", serde_json::json!({ "id": msg_id }));
                            return;
                        }

                        // Sleep for 50ms (blocking sleep is OK in dedicated thread)
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        sleep_count += 1;

                        // Timeout after 5 minutes (safety check)
                        if sleep_count > 6000 {
                            eprintln!("TTS playback timeout for message {}", msg_id);
                            break;
                        }
                    }
                }

                // Check if cancelled during playback
                if state.tts_queue_cancel.load(std::sync::atomic::Ordering::Acquire) {
                    state.update_tts_message_status(&msg_id, TtsMessageStatus::Completed);
                    state.tts_queue_cancel.store(false, std::sync::atomic::Ordering::Release);
                    state.tts_queue_processing.store(false, std::sync::atomic::Ordering::Release);
                    state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);
                    let _ = app.emit("tts:cancelled", serde_json::json!({ "id": msg_id }));
                    break;
                }

                match result {
                    Ok(_) => {
                        state.update_tts_message_status(&msg_id, TtsMessageStatus::Completed);
                        state.set_current_tts_message_id(None);
                        let _ = app.emit("tts:completed", serde_json::json!({ "id": msg_id }));
                    }
                    Err(e) => {
                        eprintln!("TTS error: {}", e);
                        state.update_tts_message_status(&msg_id, TtsMessageStatus::Completed);
                        state.set_current_tts_message_id(None);
                        state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);
                        let _ = app.emit("tts:failed", serde_json::json!({
                            "id": msg_id,
                            "error": e
                        }));
                    }
                }
            }
            None => {
                // No more queued messages
                state.tts_queue_processing.store(false, std::sync::atomic::Ordering::Release);
                break;
            }
        }
    }
}

/// Cancel a TTS message (if queued) or stop current playback
#[tauri::command]
pub fn cancel_tts_message(state: tauri::State<'_, AppState>, app: tauri::AppHandle, id: String) -> Result<(), String> {
    let history = state.tts_history.lock().unwrap();
    let message = history.iter().find(|m| m.id == id)
        .ok_or_else(|| "Message not found".to_string())?;

    match message.status {
        TtsMessageStatus::Queued => {
            drop(history); // Release lock before updating
            state.update_tts_message_status(&id, TtsMessageStatus::Completed);
            let _ = app.emit("tts:cancelled", serde_json::json!({ "id": id }));
            Ok(())
        }
        TtsMessageStatus::Playing => {
            drop(history); // Release lock
            // Stop current playback
            let lock_result = state.tts_engine.lock();
            let engine = match lock_result {
                Ok(guard) => guard,
                Err(poisoned) => {
                    eprintln!("TTS engine mutex was poisoned, recovering...");
                    poisoned.into_inner()
                }
            };
            engine.stop()?;
            state.tts_queue_cancel.store(true, std::sync::atomic::Ordering::Release);
            state.update_tts_message_status(&id, TtsMessageStatus::Completed);
            let _ = app.emit("tts:cancelled", serde_json::json!({ "id": id }));
            Ok(())
        }
        TtsMessageStatus::Completed => {
            Err("Message already completed".to_string())
        }
    }
}

/// Speak text with TTS and add to history (legacy - for compatibility)
#[tauri::command]
pub async fn speak_text_with_history(state: tauri::State<'_, AppState>, app: tauri::AppHandle, text: String) -> Result<String, String> {
    enqueue_tts(state, app, text).await
}

/// Repeat a TTS message from history
#[tauri::command]
pub async fn repeat_tts_message(state: tauri::State<'_, AppState>, app: tauri::AppHandle, id: String) -> Result<(), String> {
    // Find the message
    let history = state.get_tts_history();
    let message = history.iter().find(|m| m.id == id)
        .ok_or_else(|| "Message not found".to_string())?;

    let text = message.text.clone();

    // Update status to playing
    state.update_tts_message_status(&id, TtsMessageStatus::Playing);
    state.set_current_tts_message_id(Some(id.clone()));

    // Set speaking flag
    state.tts_is_speaking.store(true, std::sync::atomic::Ordering::Release);

    // Emit started event
    let _ = app.emit("tts:started", serde_json::json!({
        "id": id,
        "text": text.clone()
    }));

    // Speak - handle poisoned mutex
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };

    let result = engine.speak(&text);

    // Only mark as completed if there was an error
    // For successful playback (OpenAI), completion is handled by callback
    if result.is_err() {
        state.update_tts_message_status(&id, TtsMessageStatus::Completed);
        state.set_current_tts_message_id(None);
        state.tts_is_speaking.store(false, std::sync::atomic::Ordering::Release);
        let _ = app.emit("tts:completed", serde_json::json!({ "id": id }));
    }

    result
}

// === System TTS voice and parameters commands ===

/// Get all available system voices
#[tauri::command]
pub fn get_system_voices(state: tauri::State<'_, AppState>) -> Vec<Voice> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.get_voices()
}

/// Set system TTS voice
#[tauri::command]
pub fn set_system_voice(state: tauri::State<'_, AppState>, voice_id: String) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.set_voice(voice_id)
}

/// Set TTS rate (speed)
#[tauri::command]
pub fn set_tts_rate(state: tauri::State<'_, AppState>, rate: i32) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.set_rate(rate)
}

/// Set TTS pitch
#[tauri::command]
pub fn set_tts_pitch(state: tauri::State<'_, AppState>, pitch: i32) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.set_pitch(pitch)
}

/// Set TTS volume
#[tauri::command]
pub fn set_tts_volume(state: tauri::State<'_, AppState>, volume: i32) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.set_volume(volume)
}

// === OpenAI TTS commands ===

/// Получить список голосов OpenAI
#[tauri::command]
pub fn get_openai_voices(state: tauri::State<'_, AppState>) -> Vec<OpenAIVoice> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.get_openai_voices()
}

/// Установить голос OpenAI
#[tauri::command]
pub fn set_openai_voice(state: tauri::State<'_, AppState>, voice: String) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_openai_voice(voice);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Установить скорость OpenAI
#[tauri::command]
pub fn set_openai_speed(state: tauri::State<'_, AppState>, speed: f64) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_openai_speed(speed as f32);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Установить инструкции OpenAI
#[tauri::command]
pub fn set_openai_instructions(state: tauri::State<'_, AppState>, instructions: String) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_openai_instructions(instructions);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Установить прокси OpenAI
#[tauri::command]
pub fn set_openai_proxy(state: tauri::State<'_, AppState>, host: Option<String>, port: Option<u16>) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_openai_proxy(host, port);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Получить конфигурацию OpenAI
#[tauri::command]
pub fn get_openai_config(state: tauri::State<'_, AppState>) -> OpenAIConfig {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.get_openai_config()
}

// === Localhost TTS commands ===

/// Получить список голосов Localhost
#[tauri::command]
pub fn get_localhost_voices(state: tauri::State<'_, AppState>) -> Vec<LocalhostVoice> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.get_localhost_voices()
}

/// Обновить список голосов с сервера
#[tauri::command]
pub async fn refresh_localhost_voices(state: tauri::State<'_, AppState>) -> Result<Vec<LocalhostVoice>, String> {
    let (config, ) = {
        let lock_result = state.tts_engine.lock();
        let engine = match lock_result {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("TTS engine mutex was poisoned, recovering...");
                poisoned.into_inner()
            }
        };
        (engine.get_localhost_config().clone(), )
    };

    // Direct async call - we're already in tokio runtime
    let temp_client = crate::localhost::LocalhostClient::new_for_request(config);
    let voices = temp_client.fetch_voices().await?;

    // Save voices to file
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.update_localhost_voices(voices.clone())?;

    Ok(voices)
}

/// Проверить соединение с сервером
#[tauri::command]
pub async fn test_localhost_connection(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    let (config, ) = {
        let lock_result = state.tts_engine.lock();
        let engine = match lock_result {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("TTS engine mutex was poisoned, recovering...");
                poisoned.into_inner()
            }
        };
        (engine.get_localhost_config().clone(), )
    };

    // Direct async call - we're already in tokio runtime
    let temp_client = crate::localhost::LocalhostClient::new_for_request(config);
    let connected = temp_client.test_connection().await?;

    // Update connected status
    if let Ok(engine_guard) = state.tts_engine.lock() {
        let _ = engine_guard.set_localhost_connected(connected);
    }

    Ok(connected)
}

/// Установить порт Localhost
#[tauri::command]
pub fn set_localhost_port(state: tauri::State<'_, AppState>, port: i64) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_localhost_port(port.to_string());

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Установить токен Localhost
#[tauri::command]
pub fn set_localhost_token(state: tauri::State<'_, AppState>, token: String) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_localhost_token(token);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Установить голос Localhost
#[tauri::command]
pub fn set_localhost_voice(state: tauri::State<'_, AppState>, voice: Option<String>) -> Result<(), String> {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    let result = engine.set_localhost_voice(voice);

    // Emit config changed event
    state.emit_tts_config_changed();

    result
}

/// Получить конфигурацию Localhost
#[tauri::command]
pub fn get_localhost_config(state: tauri::State<'_, AppState>) -> LocalhostConfig {
    let lock_result = state.tts_engine.lock();
    let engine = match lock_result {
        Ok(guard) => guard,
        Err(poisoned) => {
            eprintln!("TTS engine mutex was poisoned, recovering...");
            poisoned.into_inner()
        }
    };
    engine.get_localhost_config()
}

// === Audio output and virtual mic commands ===

/// Get all audio output devices (for speakers)
#[tauri::command]
pub fn get_output_devices() -> Vec<OutputDeviceInfo> {
    crate::virtual_mic::find_all_output_devices()
}

/// Get virtual microphone devices
#[tauri::command]
pub fn get_virtual_mic_devices() -> Vec<VirtualDeviceInfo> {
    crate::virtual_mic::find_virtual_devices()
}

/// Set speaker device (None = default)
#[tauri::command]
pub async fn set_speaker_device(state: tauri::State<'_, AppState>, device_id: Option<String>) -> Result<(), String> {
    // Save to audio settings manager
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.set_speaker_device(device_id.clone())?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_speaker_device(device_id);
    }
    Ok(())
}

/// Set speaker enabled
#[tauri::command]
pub async fn set_speaker_enabled(state: tauri::State<'_, AppState>, enabled: bool) -> Result<(), String> {
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.set_speaker_enabled(enabled)?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_speaker_enabled(enabled);
    }
    Ok(())
}

/// Set speaker volume (0-100)
#[tauri::command]
pub async fn set_speaker_volume(state: tauri::State<'_, AppState>, volume: f32) -> Result<(), String> {
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.set_speaker_volume(volume as u8)?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_speaker_volume(volume / 100.0);
    }
    Ok(())
}

/// Set virtual mic device (None = disabled)
#[tauri::command]
pub async fn set_virtual_mic_device(state: tauri::State<'_, AppState>, device_id: Option<String>) -> Result<(), String> {
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.set_virtual_mic_device(device_id.clone())?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_virtual_mic_device(device_id);
    }
    Ok(())
}

/// Enable virtual mic (use last device)
#[tauri::command]
pub async fn enable_virtual_mic(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let device_id = if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.enable_virtual_mic()?;
            manager.get().last_virtual_mic_device.clone()
        } else {
            return Err("Audio settings manager not initialized".to_string());
        }
    } else {
        return Err("Failed to lock audio settings manager".to_string());
    };

    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_virtual_mic_device(device_id);
    }
    Ok(())
}

/// Disable virtual mic
#[tauri::command]
pub async fn disable_virtual_mic(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.disable_virtual_mic()?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_virtual_mic_device(None);
    }
    Ok(())
}

/// Set virtual mic volume (0-100)
#[tauri::command]
pub async fn set_virtual_mic_volume(state: tauri::State<'_, AppState>, volume: f32) -> Result<(), String> {
    if let Ok(mut manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref mut manager) = *manager_guard {
            manager.set_virtual_mic_volume(volume as u8)?;
        }
    }
    // Also update TtsEngine
    if let Ok(engine) = state.tts_engine.lock() {
        engine.set_virtual_mic_volume(volume / 100.0);
    }
    Ok(())
}

/// Get audio settings
#[tauri::command]
pub fn get_audio_settings(state: tauri::State<'_, AppState>) -> Result<crate::virtual_mic::AudioSettings, String> {
    if let Ok(manager_guard) = state.audio_settings_manager.lock() {
        if let Some(ref manager) = *manager_guard {
            return Ok(manager.get().clone());
        }
    }
    Err("Failed to get audio settings".to_string())
}

// === Plugin commands ===

/// Get all loaded plugins
#[tauri::command]
pub fn get_plugins(state: tauri::State<'_, AppState>) -> Result<Vec<PluginInfo>, String> {
    if let Ok(plugin_manager) = state.plugin_manager.lock() {
        if let Some(ref manager) = *plugin_manager {
            return Ok(manager.get_plugins());
        }
    }
    Ok(Vec::new())
}

/// Set plugin configuration
#[tauri::command]
pub fn set_plugin_config(
    state: tauri::State<'_, AppState>,
    name: String,
    config: serde_json::Value,
) -> Result<(), String> {
    if let Ok(mut plugin_manager) = state.plugin_manager.lock() {
        if let Some(ref mut manager) = *plugin_manager {
            return match manager.set_plugin_config(&name, &config) {
                Ok(()) => {
                    // Emit plugins changed event
                    let plugins = manager.get_plugins();
                    state.emit_plugins_changed(plugins);
                    Ok(())
                }
                Err(e) => {
                    // Disable plugin on config error
                    let _ = manager.toggle_plugin(&name, false);
                    // Emit plugins changed event after disabling
                    let plugins = manager.get_plugins();
                    state.emit_plugins_changed(plugins);
                    Err(e)
                }
            };
        }
    }
    Err("Plugin manager not initialized".to_string())
}

/// Toggle plugin enabled state
#[tauri::command]
pub fn toggle_plugin(
    state: tauri::State<'_, AppState>,
    name: String,
    enabled: bool,
) -> Result<(), String> {
    if let Ok(mut plugin_manager) = state.plugin_manager.lock() {
        if let Some(ref mut manager) = *plugin_manager {
            let result = manager.toggle_plugin(&name, enabled);
            if result.is_ok() {
                // Emit plugins changed event
                let plugins = manager.get_plugins();
                state.emit_plugins_changed(plugins);
            }
            return result;
        }
    }
    Err("Plugin manager not initialized".to_string())
}

/// Check plugin status
#[tauri::command]
pub fn check_plugin_status(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<SerializablePluginStatus, String> {
    if let Ok(plugin_manager) = state.plugin_manager.lock() {
        if let Some(ref manager) = *plugin_manager {
            let status = manager.check_plugin_status(&name)?;
            return Ok(status.into());
        }
    }
    Err("Plugin manager not initialized".to_string())
}
