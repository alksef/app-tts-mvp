use crate::state::{AppState, AppStateEvent};
use std::mem;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::PCWSTR;

/// Timeout states
const TIMEOUT_ACTIVE: u8 = 0;      // Timer is running
const TIMEOUT_EARLY_RELEASE: u8 = 1; // Win released before timeout - should send
const TIMEOUT_SENT: u8 = 2;         // Already sent via timeout

/// Wrapper for static Arc initialization
struct StaticArc<T> {
    value: Option<Arc<T>>,
}

unsafe impl<T> Sync for StaticArc<T> {}

/// Virtual key codes
const VK_LWIN: u32 = 0x5B;
const VK_ESCAPE: u32 = 0x1B;
const VK_CAPITAL: u32 = 0x14;  // Caps Lock
const VK_SHIFT: u32 = 0x10;    // Shift (any)
const VK_LSHIFT: u32 = 0xA0;   // Left Shift
const VK_RSHIFT: u32 = 0xA1;   // Right Shift
const VK_MENU: u32 = 0x12;     // Alt (any)
const VK_LMENU: u32 = 0xA4;    // Left Alt
const VK_RMENU: u32 = 0xA5;    // Right Alt

/// Window message types for keyboard events
const WM_KEYDOWN: u32 = 0x0100;
const WM_SYSKEYDOWN: u32 = 0x0104;

/// Thread-local storage for the app state and window handle
static mut HOOK_STATE: Option<AppState> = None;
static mut APP_WINDOW_HANDLE: Option<isize> = None;
static mut WIN_PRESSED: bool = false;
static mut WIN_BLOCKED: bool = false;
static mut ESC_PRESSED_WHILE_WIN: bool = false;
// Track Win timeout thread for releasing Win after 200ms
static mut WIN_TIMEOUT_STATE: StaticArc<AtomicU8> = StaticArc { value: None };
// Flag to prevent recursion when we send Win via SendInput
static mut SENDINPUT_IN_PROGRESS: bool = false;
// Track Shift+Alt combination for language switching
static mut SHIFT_PRESSED: bool = false;
static mut ALT_PRESSED: bool = false;
static mut LANGUAGE_SWITCH_DETECTED: bool = false;

/// Timeout in milliseconds to release Win key if Esc is not pressed
const WIN_TIMEOUT_MS: u64 = 200;

/// Magic value to mark our own SendInput events (to avoid re-intercepting them)
const SENDINPUT_MARKER: usize = 0x5A5A5A5A;

/// Track if our app was focused last time we checked (for detecting focus loss)
static mut WAS_APP_FOCUSED: bool = false;

/// Send a Win keydown event to the system using SendInput
unsafe fn send_win_keydown() {
    SENDINPUT_IN_PROGRESS = true;
    println!("[HOOK] SendInput: setting SENDINPUT_IN_PROGRESS = true");

    let inputs = [INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(VK_LWIN as u16),
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: SENDINPUT_MARKER,
            },
        },
    }];

    let result = SendInput(&inputs, mem::size_of::<INPUT>() as i32);
    println!("[HOOK] SendInput Win keydown result: {}", result);
}

/// Convert virtual key code to human-readable name
fn vk_code_to_name(vk_code: u32) -> String {
    match vk_code {
        0x01 => "Left Mouse Button".to_string(),
        0x02 => "Right Mouse Button".to_string(),
        0x04 => "Middle Mouse Button".to_string(),
        0x08 => "Backspace".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x10 => "Shift".to_string(),
        0x11 => "Control".to_string(),
        0x12 => "Alt".to_string(),
        0x13 => "Pause".to_string(),
        0x14 => "Caps Lock".to_string(),
        0x1B => "Escape".to_string(),
        0x20 => "Space".to_string(),
        0x21 => "Page Up".to_string(),
        0x22 => "Page Down".to_string(),
        0x23 => "End".to_string(),
        0x24 => "Home".to_string(),
        0x25 => "Left Arrow".to_string(),
        0x26 => "Up Arrow".to_string(),
        0x27 => "Right Arrow".to_string(),
        0x28 => "Down Arrow".to_string(),
        0x2C => "Print Screen".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x30..=0x39 => {
            let digit = vk_code - 0x30;
            format!("Digit {}", digit)
        }
        0x41..=0x5A => {
            let c = (vk_code as u8) as char;
            c.to_string()
        }
        0x60..=0x69 => {
            let digit = vk_code - 0x60;
            format!("Numpad {}", digit)
        }
        0x6A => "Numpad Multiply".to_string(),
        0x6B => "Numpad Add".to_string(),
        0x6C => "Numpad Separator".to_string(),
        0x6D => "Numpad Subtract".to_string(),
        0x6E => "Numpad Decimal".to_string(),
        0x6F => "Numpad Divide".to_string(),
        0x70..=0x87 => {
            let f_num = vk_code - 0x6F;
            format!("F{}", f_num)
        }
        0x90 => "Num Lock".to_string(),
        0x91 => "Scroll Lock".to_string(),
        0xA0 => "Left Shift".to_string(),
        0xA1 => "Right Shift".to_string(),
        0xA2 => "Left Control".to_string(),
        0xA3 => "Right Control".to_string(),
        0xA4 => "Left Alt".to_string(),
        0xA5 => "Right Alt".to_string(),
        0xBA => "Semicolon".to_string(),
        0xBB => "Equals".to_string(),
        0xBC => "Comma".to_string(),
        0xBD => "Minus".to_string(),
        0xBE => "Period".to_string(),
        0xBF => "Slash".to_string(),
        0xC0 => "Backtick".to_string(),
        0xDB => "Left Bracket".to_string(),
        0xDC => "Backslash".to_string(),
        0xDD => "Right Bracket".to_string(),
        0xDE => "Quote".to_string(),
        _ => format!("VK_{:04X}", vk_code),
    }
}

/// Low-level keyboard hook procedure
///
/// This callback is invoked by Windows for every keyboard event.
/// It implements the Win+Esc detection logic and optional key blocking.
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code as u32 == HC_ACTION {
        let kb_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;
        let message = w_param.0 as u32;
        let extra_info = kb_struct.dwExtraInfo;

        // Skip events generated by our own SendInput (to avoid infinite loop)
        if extra_info == SENDINPUT_MARKER {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        // Determine if this is a key down event
        let is_keydown = message == WM_KEYDOWN || message == WM_SYSKEYDOWN;

        // Check if our app's window is the foreground window
        let foreground_window = GetForegroundWindow();
        let app_hwnd = HWND(APP_WINDOW_HANDLE.unwrap_or(0) as *mut _);
        // In Tauri 2.0, the webview is a child window, so we need to check if
        // the foreground window is either our main window OR a child of it
        let is_app_window = !app_hwnd.is_invalid()
            && (foreground_window == app_hwnd || IsChild(app_hwnd, foreground_window).as_bool());

        // Get reference to the app state
        if let Some(ref state) = HOOK_STATE {
            // Track focus changes to save previous window for restoration
            let current_previous = state.get_previous_window();

            if !WAS_APP_FOCUSED && is_app_window {
                // App gained focus - current foreground is the previous window
                let new_foreground = foreground_window.0 as isize;
                if new_foreground != 0 && new_foreground != app_hwnd.0 as isize {
                    state.set_previous_window(new_foreground);
                    println!("[HOOK] App GAINED focus, saved previous window: {} (was: {})", new_foreground, current_previous);
                }
            } else if WAS_APP_FOCUSED && !is_app_window {
                // App lost focus - save where focus went
                let new_foreground = foreground_window.0 as isize;
                if new_foreground != 0 && new_foreground != app_hwnd.0 as isize {
                    state.set_previous_window(new_foreground);
                    println!("[HOOK] App LOST focus, saved previous window: {} (was: {})", new_foreground, current_previous);
                }
            }
            WAS_APP_FOCUSED = is_app_window;

            // Debug log for Win and Esc keys to see if they reach the hook
            if vk_code == VK_LWIN || vk_code == VK_ESCAPE {
                let win_pressed = WIN_PRESSED;
                println!("[HOOK] DEBUG: {} VK_{:04X} is_app_window:{} blocking:{} WIN_PRESSED:{}",
                    if is_keydown { "DOWN" } else { "UP " },
                    vk_code,
                    is_app_window,
                    state.is_blocking_enabled(),
                    win_pressed
                );
            }

            // Handle Win key for Win+Esc detection
            if vk_code == VK_LWIN {
                if is_keydown {
                    // Skip Win keydown if we're currently sending it via SendInput
                    if SENDINPUT_IN_PROGRESS {
                        println!("[HOOK] Win keydown - skipping (SendInput in progress)");
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    // If our app window is focused, let Win key through for normal use
                    // Only block Win when other apps are focused (for Win+Esc detection)
                    if is_app_window {
                        println!("[HOOK] Win keydown - app window focused, passing through");
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    // Win key pressed - block it to prevent start menu
                    WIN_PRESSED = true;
                    WIN_BLOCKED = true;
                    ESC_PRESSED_WHILE_WIN = false;
                    println!("[HOOK] Win keydown - is_app_window: {}, blocking to detect Win+Esc", is_app_window);

                    // Start timeout thread to release Win after 200ms if Esc not pressed
                    let timeout_state = Arc::new(AtomicU8::new(TIMEOUT_ACTIVE));
                    WIN_TIMEOUT_STATE.value = Some(timeout_state.clone());

                    println!("[HOOK] Starting Win timeout thread ({}ms)", WIN_TIMEOUT_MS);
                    thread::spawn(move || {
                        println!("[HOOK] Timeout thread: sleeping for {}ms", WIN_TIMEOUT_MS);
                        thread::sleep(std::time::Duration::from_millis(WIN_TIMEOUT_MS));
                        let state = timeout_state.load(Ordering::SeqCst);
                        println!("[HOOK] Timeout thread: woke up, state={}", state);

                        if state == TIMEOUT_ACTIVE {
                            // Timeout elapsed with Esc not pressed - send Win keydown
                            unsafe {
                                println!("[HOOK] Win timeout elapsed - sending Win keydown to system");
                                WIN_BLOCKED = false;
                                // send_win_keydown();  // DISABLED: only send on early release
                            }
                            timeout_state.store(TIMEOUT_SENT, Ordering::SeqCst);
                        } else if state == TIMEOUT_EARLY_RELEASE {
                            // Early release already handled in keyup
                            println!("[HOOK] Timeout thread: early release was handled");
                        } else {
                            println!("[HOOK] Timeout thread: already sent or Esc pressed");
                        }
                    });

                    // Send event to main thread for UI update
                    if let Ok(sender) = state.event_sender.lock() {
                        if let Some(ref tx) = *sender {
                            let _ = tx.send(AppStateEvent::WinPressedChanged(true));
                        }
                    }

                    return LRESULT(1);
                } else {
                    // Win key released - if our app is focused, pass through
                    if is_app_window {
                        println!("[HOOK] Win keyup - app window focused, passing through");
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }

                    // Win key released - mark as early release if still active
                    let was_early_release = if let Some(ref state) = WIN_TIMEOUT_STATE.value {
                        let current = state.load(Ordering::SeqCst);
                        println!("[HOOK] Win keyup - timeout_state={}", current);
                        if current == TIMEOUT_ACTIVE {
                            state.store(TIMEOUT_EARLY_RELEASE, Ordering::SeqCst);
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    let win_blocked = WIN_BLOCKED;
                    let esc_pressed = ESC_PRESSED_WHILE_WIN;
                    let blocking_enabled = state.is_blocking_enabled();
                    println!("[HOOK] Win keyup - is_app_window: {}, blocked: {}, esc_pressed: {}, early_release={}, blocking_enabled={}",
                        is_app_window, win_blocked, esc_pressed, was_early_release, blocking_enabled);
                    WIN_PRESSED = false;
                    let was_blocked = WIN_BLOCKED;
                    WIN_BLOCKED = false;
                    let esc_was_pressed = ESC_PRESSED_WHILE_WIN;
                    ESC_PRESSED_WHILE_WIN = false;

                    // Reset SendInput flag when Win is released
                    if SENDINPUT_IN_PROGRESS {
                        SENDINPUT_IN_PROGRESS = false;
                        println!("[HOOK] Win keyup: resetting SENDINPUT_IN_PROGRESS = false");
                    }

                    // Send event to main thread for UI update
                    if let Ok(sender) = state.event_sender.lock() {
                        if let Some(ref tx) = *sender {
                            let _ = tx.send(AppStateEvent::WinPressedChanged(false));
                        }
                    }

                    // If we blocked the Win keydown and Esc wasn't pressed
                    // Only send Win keydown if blocking is STILL enabled (not toggled off by Esc)
                    if was_blocked && !esc_was_pressed && blocking_enabled {
                        if was_early_release {
                            // Win was released before timeout - send Win keydown now
                            println!("[HOOK] Win released before timeout - sending Win keydown to system");
                            send_win_keydown();
                        }
                        // Let the keyup through to end the "blocked" Win state
                        return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                    }
                }
            }

            // Check for Win+Esc combination (works from any window)
            if vk_code == VK_ESCAPE {
                if is_keydown && WIN_PRESSED {
                    let win_pressed = WIN_PRESSED;
                    let win_blocked = WIN_BLOCKED;
                    println!("[HOOK] ESC keydown while Win pressed - checking for Win+Esc");
                    println!("[HOOK] WIN_PRESSED={}, WIN_BLOCKED={}", win_pressed, win_blocked);

                    // Cancel Win timeout since Esc was pressed (mark as sent to skip)
                    if let Some(ref state) = WIN_TIMEOUT_STATE.value {
                        let prev_state = state.load(Ordering::SeqCst);
                        state.store(TIMEOUT_SENT, Ordering::SeqCst);
                        println!("[HOOK] Cancelled Win timeout (was: {})", prev_state);
                    }
                    ESC_PRESSED_WHILE_WIN = true;

                    // Check hotkey mode
                    let is_overlay_call = state.is_overlay_call_mode();
                    println!("[HOOK] Hotkey mode check - is_overlay_call: {}", is_overlay_call);

                    if is_overlay_call {
                        // Overlay call mode - show window without blocking
                        println!("[HOOK] Win+Esc in OverlayCall mode - requesting window show");
                        println!("[HOOK] is_app_window: {}", is_app_window);

                        // Send event to main thread to show window
                        if let Ok(sender) = state.event_sender.lock() {
                            if let Some(ref tx) = *sender {
                                let result = tx.send(AppStateEvent::ShowWindowRequested);
                                println!("[HOOK] ShowWindowRequested send result: {:?}", result);
                            }
                        } else {
                            println!("[HOOK] ERROR: Failed to lock event_sender");
                        }

                        // Don't block - let the Escape through to system (or block it? we'll see)
                        // For now, block it to prevent the escape from reaching other apps
                        return LRESULT(1);
                    } else {
                        // Background blocking mode - toggle blocking (current behavior)
                        println!("[HOOK] Win+Esc in BackgroundBlocking mode - toggling blocking. is_app_window: {}", is_app_window);
                        let new_blocking_state = state.toggle_blocking();
                        println!("[HOOK] Blocking is now: {}", new_blocking_state);

                        // Send event to main thread for UI update
                        if let Ok(sender) = state.event_sender.lock() {
                            if let Some(ref tx) = *sender {
                                let _ = tx.send(AppStateEvent::BlockingChanged(new_blocking_state));
                            }
                        }

                        // Add the toggle event to intercepted keys
                        state.add_key_auto(VK_ESCAPE, format!("Win+Esc (Toggle -> {})", if new_blocking_state { "ON" } else { "OFF" }));

                        // Block this Esc as part of Win+Esc combination
                        return LRESULT(1);
                    }
                }
            }

            // Handle Caps Lock - toggle state and track it
            if vk_code == VK_CAPITAL && is_keydown {
                let new_caps_state = state.toggle_caps_lock();
                println!("[HOOK] Caps Lock toggled: {}", new_caps_state);

                // Send event to main thread for UI update
                if let Ok(sender) = state.event_sender.lock() {
                    if let Some(ref tx) = *sender {
                        let _ = tx.send(AppStateEvent::CapsLockChanged(new_caps_state));
                    }
                }

                // Don't block Caps Lock - let it through to the system
                return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
            }

            // Track Shift key for language switching
            if vk_code == VK_SHIFT || vk_code == VK_LSHIFT || vk_code == VK_RSHIFT {
                if is_keydown {
                    SHIFT_PRESSED = true;
                    // Check if Alt is already pressed - this is Shift+Alt for language switch
                    if ALT_PRESSED {
                        LANGUAGE_SWITCH_DETECTED = true;
                        println!("[HOOK] Shift+Alt detected - language switch combination");
                    }
                } else {
                    let was_lang_switch = LANGUAGE_SWITCH_DETECTED && ALT_PRESSED;
                    SHIFT_PRESSED = false;
                    if was_lang_switch {
                        LANGUAGE_SWITCH_DETECTED = false;
                        // Refresh input language from system after language switch
                        state.refresh_input_language();
                        let new_lang = state.get_input_language();
                        println!("[HOOK] Language switch completed, new language: {:?}", new_lang);

                        // Send event to main thread for UI update
                        if let Ok(sender) = state.event_sender.lock() {
                            if let Some(ref tx) = *sender {
                                let _ = tx.send(AppStateEvent::InputLanguageChanged(new_lang));
                            }
                        }
                    }
                }
            }

            // Track Alt key for language switching
            if vk_code == VK_MENU || vk_code == VK_LMENU || vk_code == VK_RMENU {
                if is_keydown {
                    ALT_PRESSED = true;
                    // Check if Shift is already pressed - this is Alt+Shift for language switch
                    if SHIFT_PRESSED {
                        LANGUAGE_SWITCH_DETECTED = true;
                        println!("[HOOK] Alt+Shift detected - language switch combination");
                    }
                } else {
                    let was_lang_switch = LANGUAGE_SWITCH_DETECTED && SHIFT_PRESSED;
                    ALT_PRESSED = false;
                    if was_lang_switch {
                        LANGUAGE_SWITCH_DETECTED = false;
                        // Refresh input language from system after language switch
                        state.refresh_input_language();
                        let new_lang = state.get_input_language();
                        println!("[HOOK] Language switch completed, new language: {:?}", new_lang);

                        // Send event to main thread for UI update
                        if let Ok(sender) = state.event_sender.lock() {
                            if let Some(ref tx) = *sender {
                                let _ = tx.send(AppStateEvent::InputLanguageChanged(new_lang));
                            }
                        }
                    }
                }
            }

            // Track all keydown events when app window is active
            if is_keydown && is_app_window {
                let key_name = vk_code_to_name(vk_code);
                println!("[HOOK] Active window key: {} (VK_{:04X})", key_name, vk_code);
                // Use add_active_window_key with seq_num
                let seq_num = state.key_seq_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                state.add_active_window_key(crate::state::KeyEvent::new(vk_code, key_name, seq_num));
            }

            // If blocking is enabled and NOT our app window, block all keydown events
            // When our app window is focused, allow keys through for UI interaction
            if state.is_blocking_enabled() && !is_app_window {
                // Check if this is Shift or Alt - ALWAYS allow through for language switching
                // We must pass both keydown and keyup events for language switch to work
                let is_shift = vk_code == VK_SHIFT || vk_code == VK_LSHIFT || vk_code == VK_RSHIFT;
                let is_alt = vk_code == VK_MENU || vk_code == VK_LMENU || vk_code == VK_RMENU;

                if is_shift || is_alt {
                    // Always pass Shift and Alt through to the system for language switching
                    println!("[HOOK] Passing modifier through: {} (VK_{:04X})", vk_code_to_name(vk_code), vk_code);
                    return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                }

                // For other keys, only block on keydown events
                if !is_keydown {
                    // Pass through all keyup events for non-modifier keys
                    return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
                }

                // Normal blocking mode - block all keys
                let key_name = vk_code_to_name(vk_code);
                println!("[HOOK] Blocking key: {} (VK_{:04X})", key_name, vk_code);
                let key_event = state.add_key_auto(vk_code, key_name);

                // Send event to main thread for instant UI update
                if let Ok(sender) = state.event_sender.lock() {
                    if let Some(ref tx) = *sender {
                        let _ = tx.send(AppStateEvent::KeyIntercepted(key_event));
                    }
                }

                // Block the key press
                return LRESULT(1);
            }
        }
    }

    // Pass the event to the next hook
    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

/// Spawns the keyboard hook thread with a Windows message pump
///
/// This function creates a dedicated thread that:
/// 1. Stores the app state in static storage
/// 2. Stores the app window handle for foreground detection
/// 3. Installs the low-level keyboard hook
/// 4. Runs a message pump to process Windows messages
/// 5. Keeps the hook active for the application lifetime
pub fn initialize_hotkey_system(state: AppState, window_handle: HWND) -> JoinHandle<()> {
    // Convert HWND to isize for thread-safe storage
    let hwnd_raw = window_handle .0 as isize;

    thread::spawn(move || {
        unsafe {
            // Store the app state in static storage
            HOOK_STATE = Some(state.clone());

            // Initialize Caps Lock state from system
            let caps_lock_state = GetKeyState(VK_CAPITAL as i32) != 0;
            state.set_caps_lock(caps_lock_state);
            println!("[HOOK] Initial Caps Lock state: {}", caps_lock_state);

            // Store the app window handle
            APP_WINDOW_HANDLE = Some(hwnd_raw);

            // Get module handle for the current process
            let module_handle = GetModuleHandleW(PCWSTR::null()).unwrap_or(HMODULE::default());

            // Install the low-level keyboard hook
            let hook_result = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(low_level_keyboard_proc),
                module_handle,
                0,
            );

            let hook = match hook_result {
                Ok(h) => h,
                Err(_) => {
                    eprintln!("Failed to set keyboard hook");
                    return;
                }
            };

            if hook.is_invalid() {
                eprintln!("Failed to set keyboard hook");
                return;
            }

            println!("Keyboard hook installed successfully");

            // Run message pump - this blocks until WM_QUIT is received
            let mut msg: MSG = mem::zeroed();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).into() {
                DispatchMessageW(&msg);
            }

            // Clean up hook when thread exits
            let _ = UnhookWindowsHookEx(hook);

            // Clean up state
            HOOK_STATE = None;
            APP_WINDOW_HANDLE = None;

            println!("Keyboard hook uninstalled");
        }
    })
}
