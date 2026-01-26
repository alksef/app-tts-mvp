// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod hook;
mod openai;
mod localhost;
mod state;
mod tts;
mod virtual_mic;   // Virtual microphone and dual output
mod plugins;       // Plugin system
mod audio_player;  // Rodio audio player

use commands::{
    clear_active_window_keys, clear_keys, get_active_window_keys, get_input_language,
    get_intercepted_keys, get_status, hide_window, set_always_on_top, set_auto_show_on_block,
    get_hotkey_mode, set_hotkey_mode,
    set_ignore_cursor_events, test_invoke, save_previous_window, send_to_background_and_restore_focus, hide_overlay_and_restore_focus, set_openai_key, set_continuous_play, set_tts_provider,
    set_window_always_on_top, show_window, show_window_on_top, speak_text, stop_speech, toggle_blocking,
    toggle_input_language, get_tts_status,
    // TTS history commands
    get_tts_history, add_tts_message, update_tts_message_status, toggle_tts_message_locked,
    delete_tts_message, clear_tts_history, speak_text_with_history, repeat_tts_message,
    enqueue_tts, cancel_tts_message,
    // System TTS voice and parameters commands
    get_system_voices, set_system_voice, set_tts_rate, set_tts_pitch, set_tts_volume,
    // OpenAI TTS commands
    get_openai_voices, set_openai_voice, set_openai_speed,
    set_openai_instructions, set_openai_proxy, get_openai_config,
    // Localhost TTS commands
    get_localhost_voices, refresh_localhost_voices, test_localhost_connection,
    set_localhost_port, set_localhost_token, set_localhost_voice, get_localhost_config,
    // Audio output and virtual mic commands
    get_output_devices, get_virtual_mic_devices, set_speaker_device, set_speaker_enabled,
    set_speaker_volume, set_virtual_mic_device, enable_virtual_mic, disable_virtual_mic,
    set_virtual_mic_volume, get_audio_settings,
    // Plugin commands
    get_plugins, set_plugin_config, toggle_plugin, check_plugin_status,
};
use state::AppState;
use state::AppStateEvent;
use tauri::{Emitter, Manager};

fn main() {
    // Create the shared application state
    let app_state = AppState::new();

    tauri::Builder::default()
        // Manage the app state so it's accessible in commands
        .manage(app_state.clone())
        // Register Tauri commands
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_intercepted_keys,
            clear_keys,
            toggle_blocking,
            get_active_window_keys,
            clear_active_window_keys,
            set_always_on_top,
            set_auto_show_on_block,
            get_hotkey_mode,
            set_hotkey_mode,
            set_window_always_on_top,
            show_window,
            show_window_on_top,
            hide_window,
            set_ignore_cursor_events,
            test_invoke,
            save_previous_window,
            send_to_background_and_restore_focus,
            hide_overlay_and_restore_focus,
            // TTS commands
            speak_text,
            stop_speech,
            set_continuous_play,
            set_tts_provider,
            set_openai_key,
            get_tts_status,
            // Input language commands
            get_input_language,
            toggle_input_language,
            // TTS history commands
            get_tts_history,
            add_tts_message,
            update_tts_message_status,
            toggle_tts_message_locked,
            delete_tts_message,
            clear_tts_history,
            speak_text_with_history,
            repeat_tts_message,
            enqueue_tts,
            cancel_tts_message,
            // System TTS voice and parameters commands
            get_system_voices,
            set_system_voice,
            set_tts_rate,
            set_tts_pitch,
            set_tts_volume,
            // OpenAI TTS commands
            get_openai_voices,
            set_openai_voice,
            set_openai_speed,
            set_openai_instructions,
            set_openai_proxy,
            get_openai_config,
            // Localhost TTS commands
            get_localhost_voices,
            refresh_localhost_voices,
            test_localhost_connection,
            set_localhost_port,
            set_localhost_token,
            set_localhost_voice,
            get_localhost_config,
            // Audio output and virtual mic commands
            get_output_devices,
            get_virtual_mic_devices,
            set_speaker_device,
            set_speaker_enabled,
            set_speaker_volume,
            set_virtual_mic_device,
            enable_virtual_mic,
            disable_virtual_mic,
            set_virtual_mic_volume,
            get_audio_settings,
            // Plugin commands
            get_plugins,
            set_plugin_config,
            toggle_plugin,
            check_plugin_status,
        ])
        // Setup on window initialization
        .setup(move |app| {
            #[cfg(debug_assertions)]
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                // Don't close devtools so we can see console logs
            }

            // Initialize OpenAI client with config directory
            let config_dir = match app.path().app_config_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("Failed to get config dir: {:?}", e);
                    std::path::PathBuf::from(".")
                }
            };
            let _ = std::fs::create_dir_all(&config_dir);

            // Set config dir and load app settings (including hotkey mode)
            app_state.set_config_dir(config_dir.clone());
            app_state.load_settings();

            // Initialize OpenAI client - handle poisoned mutex
            let engine = match app_state.tts_engine.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    eprintln!("Mutex was poisoned, recovering...");
                    poisoned.into_inner()
                }
            };

            // Initialize OpenAI client safely
            if let Err(e) = engine.init_openai_client(config_dir.clone()) {
                eprintln!("Failed to initialize OpenAI client: {}", e);
            }

            // Initialize Localhost client safely
            if let Err(e) = engine.init_localhost_client(config_dir.clone()) {
                eprintln!("Failed to initialize Localhost client: {}", e);
            }

            // Set config dir and load TTS provider settings
            engine.set_config_dir(config_dir.clone());

            // Create temp directory for OpenAI audio files
            let temp_dir = std::env::temp_dir().join("app-tts");
            let _ = std::fs::create_dir_all(&temp_dir);
            engine.set_openai_temp_dir(temp_dir);

            drop(engine);

            // Initialize audio settings manager
            use virtual_mic::{AudioSettingsManager, find_all_output_devices, find_virtual_devices};

            // Enumerate all output devices (silently)
            let _output_devices = find_all_output_devices();

            // Enumerate virtual mic devices (silently)
            let _virtual_mics = find_virtual_devices();

            match AudioSettingsManager::new(config_dir.clone()) {
                Ok(manager) => {
                    // Load settings into TtsEngine
                    let settings = manager.get();
                    if let Ok(engine) = app_state.tts_engine.lock() {
                        engine.set_speaker_device(settings.speaker_device.clone());
                        engine.set_speaker_enabled(settings.speaker_enabled);
                        engine.set_speaker_volume(settings.speaker_volume as f32 / 100.0);
                        engine.set_virtual_mic_device(settings.virtual_mic_device.clone());
                        engine.set_virtual_mic_volume(settings.virtual_mic_volume as f32 / 100.0);
                    }

                    if let Ok(mut audio_manager) = app_state.audio_settings_manager.lock() {
                        *audio_manager = Some(manager);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize audio settings manager: {}", e);
                }
            }

            // Initialize plugin manager
            let exe_dir = std::env::current_exe()
                .map(|p| p.parent().map(|p| p.to_path_buf()).unwrap_or_default())
                .unwrap_or_default();
            let plugins_dir = exe_dir.join("plugins");

            match plugins::PluginManager::new(plugins_dir) {
                Ok(mut manager) => {
                    match manager.load_all() {
                        Ok(count) => {
                            eprintln!("Loaded {} plugin(s)", count);
                        }
                        Err(e) => {
                            eprintln!("Failed to load plugins: {}", e);
                        }
                    }
                    if let Ok(mut plugin_manager) = app_state.plugin_manager.lock() {
                        *plugin_manager = Some(manager);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to initialize plugin manager: {}", e);
                }
            }

            // Initialize event channel for hook thread -> main thread communication
            use std::sync::mpsc;
            let (event_tx, event_rx) = mpsc::channel::<AppStateEvent>();

            // Store sender in AppState for hook thread to use
            {
                let mut sender = app_state.event_sender.lock().unwrap();
                *sender = Some(event_tx);
            }

            // Spawn event handler thread
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                for event in event_rx {
                    match event {
                        AppStateEvent::BlockingChanged(enabled) => {
                            eprintln!("[Event] BlockingChanged: {}", enabled);
                            let _ = app_handle.emit("blocking_changed", enabled);
                        }
                        AppStateEvent::CapsLockChanged(enabled) => {
                            eprintln!("[Event] CapsLockChanged: {}", enabled);
                            let _ = app_handle.emit("caps_lock_changed", enabled);
                        }
                        AppStateEvent::InputLanguageChanged(lang) => {
                            let lang_str: String = lang.into();
                            eprintln!("[Event] InputLanguageChanged: {}", lang_str);
                            let _ = app_handle.emit("input_language_changed", lang_str);
                        }
                        AppStateEvent::KeyIntercepted(key) => {
                            // No debug log for every key to avoid spam
                            let _ = app_handle.emit("key_intercepted", key);
                        }
                        AppStateEvent::WinPressedChanged(enabled) => {
                            eprintln!("[Event] WinPressedChanged: {}", enabled);
                            let _ = app_handle.emit("win_pressed_changed", enabled);
                        }
                        AppStateEvent::AlwaysOnTopChanged(enabled) => {
                            eprintln!("[Event] AlwaysOnTopChanged: {}", enabled);
                            let _ = app_handle.emit("always_on_top_changed", enabled);
                        }
                        AppStateEvent::AutoShowOnBlockChanged(enabled) => {
                            eprintln!("[Event] AutoShowOnBlockChanged: {}", enabled);
                            let _ = app_handle.emit("auto_show_on_block_changed", enabled);
                        }
                        AppStateEvent::ContinuousPlayChanged(enabled) => {
                            eprintln!("[Event] ContinuousPlayChanged: {}", enabled);
                            let _ = app_handle.emit("continuous_play_changed", enabled);
                        }
                        AppStateEvent::TtsProviderChanged(provider) => {
                            eprintln!("[Event] TtsProviderChanged: {}", provider);
                            let _ = app_handle.emit("tts_provider_changed", provider);
                        }
                        AppStateEvent::TtsConfigChanged => {
                            eprintln!("[Event] TtsConfigChanged");
                            let _ = app_handle.emit("tts_config_changed", ());
                        }
                        AppStateEvent::PluginsChanged(plugins) => {
                            eprintln!("[Event] PluginsChanged: {} plugins", plugins.len());
                            let _ = app_handle.emit("plugins_changed", plugins);
                        }
                        AppStateEvent::HotkeyModeChanged(mode) => {
                            eprintln!("[Event] HotkeyModeChanged: {:?}", mode);
                            let _ = app_handle.emit("hotkey_mode_changed", mode.as_str());
                        }
                        AppStateEvent::ShowWindowRequested => {
                            eprintln!("[Event] ShowWindowRequested");
                            let _ = app_handle.emit("show_window_requested", ());
                        }
                    }
                }
                eprintln!("[Event] Event handler thread exiting");
            });

            // Get the main window handle and initialize the hotkey system
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                {
                    use windows::Win32::Foundation::HWND;

                    let hwnd = window.hwnd().expect("Failed to get window handle");

                    // Convert HWND to isize for thread-safe storage
                    let hwnd_raw = hwnd.0 as isize;

                    // Store app window handle in AppState for direct Windows API calls
                    app_state.set_app_window_hwnd(hwnd_raw);
                    eprintln!("[main] App window HWND stored: {}", hwnd_raw);

                    // Initialize the hotkey system with the main window handle
                    let app_state_for_thread = app_state.clone();
                    std::thread::spawn(move || {
                        // Small delay to let the window fully initialize
                        std::thread::sleep(std::time::Duration::from_millis(100));

                        let _ = hook::initialize_hotkey_system(app_state_for_thread, HWND(hwnd_raw as *mut _));
                    });
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
