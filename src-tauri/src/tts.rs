use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::result::Result as StdResult;
use std::fs;

// Re-export OpenAI types
pub use crate::openai::{OpenAIClient, OpenAIConfig, OpenAIVoice};
// Re-export Localhost types
pub use crate::localhost::{LocalhostClient, LocalhostConfig, LocalhostVoice};
// Import audio player for non-blocking Rodio playback
use crate::audio_player::{AudioPlayer, OutputConfig};

/// TTS settings file
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TtsSettingsFile {
    current_provider: TtsProvider,
}

impl Default for TtsSettingsFile {
    fn default() -> Self {
        Self {
            current_provider: TtsProvider::System,
        }
    }
}

/// TTS provider options
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TtsProvider {
    System,
    OpenAI,
    Silero,
    Localhost,
}

impl Default for TtsProvider {
    fn default() -> Self {
        Self::System
    }
}

impl From<String> for TtsProvider {
    fn from(s: String) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => TtsProvider::OpenAI,
            "silero" => TtsProvider::Silero,
            "localhost" => TtsProvider::Localhost,
            _ => TtsProvider::System,
        }
    }
}

impl From<TtsProvider> for String {
    fn from(provider: TtsProvider) -> Self {
        match provider {
            TtsProvider::System => "system".to_string(),
            TtsProvider::OpenAI => "openai".to_string(),
            TtsProvider::Silero => "silero".to_string(),
            TtsProvider::Localhost => "localhost".to_string(),
        }
    }
}

/// TTS status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsStatus {
    pub is_speaking: bool,
    pub provider: String,
    pub continuous_play: bool,
    pub has_openai_key: bool,
    pub sapi_available: bool,
    pub silero_available: bool,
    pub silero_server_url: String,
    pub silero_voice: String,
}

/// Voice information for SAPI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    pub id: String,
    pub name: String,
}

/// TTS Engine abstraction for supporting different providers
pub struct TtsEngine {
    provider: Arc<Mutex<TtsProvider>>,
    config_dir: Arc<Mutex<Option<PathBuf>>>,
    // System TTS - using SyncSynthesizer
    sapi_synthesizer: Arc<Mutex<Option<sapi_lite::tts::SyncSynthesizer>>>,
    // Track if SAPI is available
    sapi_available: Arc<Mutex<bool>>,
    // OpenAI
    api_key: Arc<Mutex<Option<String>>>,
    openai_client: Arc<Mutex<Option<OpenAIClient>>>,
    openai_temp_dir: Arc<Mutex<Option<PathBuf>>>,
    voice: String,
    // Localhost
    localhost_client: Arc<Mutex<Option<LocalhostClient>>>,
    // TTS parameters
    rate: Arc<Mutex<i32>>,
    pitch: Arc<Mutex<i32>>,
    volume: Arc<Mutex<i32>>,
    // Silero
    silero_server_url: Arc<Mutex<String>>,
    silero_voice: Arc<Mutex<String>>,
    silero_available: Arc<Mutex<bool>>,
    is_speaking: Arc<Mutex<bool>>,
    // === Audio output settings ===
    audio_player: Arc<Mutex<Option<AudioPlayer>>>,
    // Speaker settings
    speaker_device_id: Arc<Mutex<Option<String>>>,
    speaker_enabled: Arc<Mutex<bool>>,
    speaker_volume: Arc<Mutex<f32>>,
    // Virtual mic settings
    virtual_mic_device_id: Arc<Mutex<Option<String>>>,
    virtual_mic_volume: Arc<Mutex<f32>>,
}

impl TtsEngine {
    pub fn new() -> Self {
        // Try to initialize SAPI on creation
        let (sapi_synthesizer, sapi_available) = Self::initialize_sapi();

        Self {
            provider: Arc::new(Mutex::new(TtsProvider::System)),
            config_dir: Arc::new(Mutex::new(None)),
            sapi_synthesizer: Arc::new(Mutex::new(sapi_synthesizer)),
            sapi_available: Arc::new(Mutex::new(sapi_available)),
            api_key: Arc::new(Mutex::new(None)),
            openai_client: Arc::new(Mutex::new(None)),
            openai_temp_dir: Arc::new(Mutex::new(None)),
            voice: "alloy".to_string(),
            localhost_client: Arc::new(Mutex::new(None)),
            rate: Arc::new(Mutex::new(0)),
            pitch: Arc::new(Mutex::new(0)),
            volume: Arc::new(Mutex::new(100)),
            silero_server_url: Arc::new(Mutex::new("http://localhost:8002".to_string())),
            silero_voice: Arc::new(Mutex::new("ru_v3".to_string())),
            silero_available: Arc::new(Mutex::new(false)),
            is_speaking: Arc::new(Mutex::new(false)),
            // Audio output settings
            audio_player: Arc::new(Mutex::new(Some(AudioPlayer::new()))),
            speaker_device_id: Arc::new(Mutex::new(None)),
            speaker_enabled: Arc::new(Mutex::new(true)),
            speaker_volume: Arc::new(Mutex::new(1.0)),
            virtual_mic_device_id: Arc::new(Mutex::new(None)),
            virtual_mic_volume: Arc::new(Mutex::new(1.0)),
        }
    }

    /// Set config directory and load provider settings
    pub fn set_config_dir(&self, config_dir: PathBuf) {
        if let Ok(mut dir) = self.config_dir.lock() {
            *dir = Some(config_dir);
        }
        // Load saved provider
        self.load_provider_settings();
    }

    /// Load provider settings from file
    fn load_provider_settings(&self) {
        if let Ok(dir_guard) = self.config_dir.lock() {
            if let Some(ref config_dir) = *dir_guard {
                let settings_path = config_dir.join("tts_settings.json");
                if settings_path.exists() {
                    if let Ok(content) = fs::read_to_string(&settings_path) {
                        if let Ok(settings) = serde_json::from_str::<TtsSettingsFile>(&content) {
                            if let Ok(mut provider) = self.provider.lock() {
                                *provider = settings.current_provider;
                                println!("[TTS] Loaded saved provider: {:?}", settings.current_provider);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Save provider settings to file
    fn save_provider_settings(&self) {
        if let Ok(dir_guard) = self.config_dir.lock() {
            if let Some(ref config_dir) = *dir_guard {
                let current_provider = if let Ok(provider) = self.provider.lock() {
                    *provider
                } else {
                    TtsProvider::System
                };
                let settings = TtsSettingsFile {
                    current_provider,
                };
                let settings_path = config_dir.join("tts_settings.json");
                if let Ok(content) = serde_json::to_string_pretty(&settings) {
                    let _ = fs::write(&settings_path, content);
                }
            }
        }
    }

    /// Initialize SAPI synthesizer with COM initialization
    fn initialize_sapi() -> (Option<sapi_lite::tts::SyncSynthesizer>, bool) {
        // Initialize COM for SAPI
        let _ = sapi_lite::initialize();

        let synth = match sapi_lite::tts::SyncSynthesizer::new() {
            Ok(synth) => Some(synth),
            Err(e) => {
                eprintln!("Failed to initialize SAPI TTS synthesizer: {}", e);
                None
            }
        };

        let available = synth.is_some();
        (synth, available)
    }

    /// Ensure SAPI is initialized, lazy initialization if needed
    fn ensure_sapi_initialized(&self) -> std::result::Result<(), String> {
        // Check if already available - handle poisoned mutex
        let is_available = self.sapi_available.lock()
            .map(|available| *available)
            .unwrap_or(false);

        if is_available {
            return Ok(());
        }

        // Try to initialize
        let _ = sapi_lite::initialize();

        // Get synthesizer - handle poisoned mutex
        let mut synth_guard = match self.sapi_synthesizer.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("[TTS] SAPI synthesizer mutex was poisoned during ensure_init, recovering...");
                poisoned.into_inner()
            }
        };

        if synth_guard.is_none() {
            match sapi_lite::tts::SyncSynthesizer::new() {
                Ok(synth) => {
                    *synth_guard = Some(synth);
                    // Update available flag - handle poisoned mutex
                    if let Ok(mut available) = self.sapi_available.lock() {
                        *available = true;
                    } else if let Err(poisoned) = self.sapi_available.lock() {
                        let mut available = poisoned.into_inner();
                        *available = true;
                    }
                    return Ok(());
                }
                Err(e) => {
                    return Err(format!("Failed to initialize SAPI: {}", e));
                }
            }
        }

        Ok(())
    }

    pub fn set_provider(&self, provider: TtsProvider) {
        if let Ok(mut p) = self.provider.lock() {
            *p = provider;
        }
        self.save_provider_settings();
    }

    pub fn set_openai_key(&self, key: String) {
        // Handle poisoned mutex for api_key
        if let Ok(mut api_key) = self.api_key.lock() {
            *api_key = if key.is_empty() { None } else { Some(key.clone()) };
        } else if let Err(poisoned) = self.api_key.lock() {
            let mut api_key = poisoned.into_inner();
            *api_key = if key.is_empty() { None } else { Some(key.clone()) };
        }

        // Также сохраняем в OpenAI клиент (для записи в файл) - handle poisoned mutex
        if let Ok(mut client_guard) = self.openai_client.lock() {
            if let Some(ref mut client) = *client_guard {
                client.set_api_key(key);
            }
        } else if let Err(poisoned) = self.openai_client.lock() {
            let mut client_guard = poisoned.into_inner();
            if let Some(ref mut client) = *client_guard {
                client.set_api_key(key);
            }
        }
    }

    pub fn has_openai_key(&self) -> bool {
        self.api_key.lock()
            .map(|key| key.is_some())
            .unwrap_or(false)
    }

    // === Audio output settings methods ===

    /// Set speaker device (None = default)
    pub fn set_speaker_device(&self, device_id: Option<String>) {
        if let Ok(mut dev) = self.speaker_device_id.lock() {
            *dev = device_id;
        }
    }

    /// Set speaker enabled
    pub fn set_speaker_enabled(&self, enabled: bool) {
        if let Ok(mut e) = self.speaker_enabled.lock() {
            *e = enabled;
        }
    }

    /// Set speaker volume (0.0 - 1.0)
    pub fn set_speaker_volume(&self, volume: f32) {
        if let Ok(mut vol) = self.speaker_volume.lock() {
            *vol = volume.clamp(0.0, 1.0);
        }
    }

    /// Set virtual mic device (None = disabled)
    pub fn set_virtual_mic_device(&self, device_id: Option<String>) {
        if let Ok(mut dev) = self.virtual_mic_device_id.lock() {
            *dev = device_id;
        }
    }

    /// Set virtual mic volume (0.0 - 1.0)
    pub fn set_virtual_mic_volume(&self, volume: f32) {
        if let Ok(mut vol) = self.virtual_mic_volume.lock() {
            *vol = volume.clamp(0.0, 1.0);
        }
    }

    pub fn is_speaking(&self) -> bool {
        self.is_speaking.lock()
            .map(|speaking| *speaking)
            .unwrap_or(false)
    }

    /// Speak text using the current provider
    pub fn speak(&self, text: &str) -> std::result::Result<(), String> {
        if text.is_empty() {
            return Err("Cannot speak empty text".to_string());
        }

        // Set speaking flag - handle poisoned mutex
        if let Ok(mut speaking) = self.is_speaking.lock() {
            *speaking = true;
        } else if let Err(poisoned) = self.is_speaking.lock() {
            let mut speaking = poisoned.into_inner();
            *speaking = true;
        }

        let result = match if let Ok(provider) = self.provider.lock() {
            *provider
        } else {
            TtsProvider::System
        } {
            TtsProvider::System => self.speak_system(text),
            TtsProvider::OpenAI => self.speak_openai(text),
            TtsProvider::Silero => self.speak_silero(text),
            TtsProvider::Localhost => self.speak_localhost(text),
        };

        if result.is_err() {
            // Clear speaking flag on error - handle poisoned mutex
            if let Ok(mut speaking) = self.is_speaking.lock() {
                *speaking = false;
            } else if let Err(poisoned) = self.is_speaking.lock() {
                let mut speaking = poisoned.into_inner();
                *speaking = false;
            }
        }

        result
    }

    /// Stop any current speech
    pub fn stop(&self) -> std::result::Result<(), String> {
        // Clear speaking flag - handle poisoned mutex
        if let Ok(mut speaking) = self.is_speaking.lock() {
            *speaking = false;
        } else if let Err(poisoned) = self.is_speaking.lock() {
            let mut speaking = poisoned.into_inner();
            *speaking = false;
        }

        match if let Ok(provider) = self.provider.lock() {
            *provider
        } else {
            TtsProvider::System
        } {
            TtsProvider::System => self.stop_system(),
            TtsProvider::OpenAI => self.stop_openai(),
            TtsProvider::Silero => self.stop_silero(),
            TtsProvider::Localhost => self.stop_localhost(),
        }
    }

    // System TTS implementation using SAPI
    fn speak_system(&self, text: &str) -> std::result::Result<(), String> {
        println!("[TTS] speak_system: Speaking text: '{}'", text);

        // Ensure SAPI is initialized
        self.ensure_sapi_initialized()?;

        println!("[TTS] speak_system: SAPI initialized, attempting to speak");

        // Helper function to clear speaking flag
        let clear_speaking = || {
            if let Ok(mut speaking) = self.is_speaking.lock() {
                *speaking = false;
            } else if let Err(poisoned) = self.is_speaking.lock() {
                let mut speaking = poisoned.into_inner();
                *speaking = false;
            }
        };

        // Get synthesizer - handle poisoned mutex
        let synth_guard = match self.sapi_synthesizer.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                println!("[TTS] speak_system: SAPI synthesizer mutex was poisoned, recovering...");
                poisoned.into_inner()
            }
        };

        if let Some(ref synth) = *synth_guard {
            println!("[TTS] speak_system: Got synthesizer, calling speak()");

            // Speak the text - use None timeout for synchronous (blocking) speech
            // This ensures the speech completes before returning
            match synth.speak(text, None) {
                Ok(()) => {
                    println!("[TTS] speak_system: Speech completed successfully");
                    // Clear speaking flag after speech completes
                    clear_speaking();
                    Ok(())
                }
                Err(e) => {
                    println!("[TTS] speak_system: Speech failed with error: {}", e);
                    clear_speaking();
                    Err(format!("Failed to speak: {}", e))
                }
            }
        } else {
            println!("[TTS] speak_system: No synthesizer available");
            clear_speaking();
            Err("SAPI synthesizer not initialized. Please ensure Windows Speech API is available.".to_string())
        }
    }

    fn stop_system(&self) -> std::result::Result<(), String> {
        // SAPI doesn't have a direct stop method, but we can speak empty text
        // to interrupt the current speech
        if let Ok(synth_guard) = self.sapi_synthesizer.lock() {
            if let Some(ref synth) = *synth_guard {
                let _ = synth.speak("", None);
            }
        }
        Ok(())
    }

    // OpenAI TTS implementation using Rodio for non-blocking playback
    fn speak_openai(&self, text: &str) -> std::result::Result<(), String> {
        eprintln!("[TTS OpenAI] Starting speech for text: '{}'", text);

        // Get audio output settings
        let speaker_enabled = self.speaker_enabled.lock()
            .map(|e| *e)
            .unwrap_or(true);
        let speaker_device_id = self.speaker_device_id.lock()
            .map(|id| id.clone())
            .unwrap_or(None);
        let speaker_volume = self.speaker_volume.lock()
            .map(|v| *v)
            .unwrap_or(1.0);
        let virtual_mic_device_id = self.virtual_mic_device_id.lock()
            .map(|id| id.clone())
            .unwrap_or(None);
        let virtual_mic_volume = self.virtual_mic_volume.lock()
            .map(|v| *v)
            .unwrap_or(1.0);

        // Check if at least one output is enabled
        if !speaker_enabled && virtual_mic_device_id.is_none() {
            if let Ok(mut speaking) = self.is_speaking.lock() {
                *speaking = false;
            }
            return Err("Both speaker and virtual mic are disabled. Please enable at least one output.".to_string());
        }

        // Check API key - handle poisoned mutex
        let has_key = self.api_key.lock()
            .map(|key| key.is_some())
            .unwrap_or(false);

        if !has_key {
            return Err("OpenAI API key not set".to_string());
        }

        // Clone needed data before releasing mutex
        let (text_clone, client_clone) = {
            let client_guard = match self.openai_client.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    eprintln!("[TTS] OpenAI client mutex was poisoned, recovering...");
                    poisoned.into_inner()
                }
            };

            let client = client_guard.as_ref()
                .ok_or_else(|| "OpenAI client not initialized".to_string())?;

            // Clone the client's config data (not the whole client)
            (text.to_string(), client.get_config().clone())
        };

        eprintln!("[TTS OpenAI] Calling OpenAI API...");
        // Run async HTTP request in a separate thread with its own runtime
        let audio_data = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            rt.block_on(async {
                let temp_client = OpenAIClient::new_for_request(client_clone);
                temp_client.synthesize(&text_clone).await
            })
        })
        .join()
        .map_err(|e| format!("Thread panicked: {:?}", e))??;

        eprintln!("[TTS OpenAI] Received {} bytes from API", audio_data.len());

        // Validate we got some data
        if audio_data.is_empty() {
            return Err("Received empty audio data from OpenAI API".to_string());
        }

        eprintln!("[TTS OpenAI] Starting Rodio async playback (speaker={}, virtual_mic={:?})",
            speaker_enabled, virtual_mic_device_id);

        // Clone Arc for the completion callback
        let is_speaking = Arc::clone(&self.is_speaking);

        // Get audio player and start non-blocking dual output playback
        {
            let mut player_guard = self.audio_player.lock()
                .map_err(|e| format!("Failed to lock audio player: {}", e))?;

            if let Some(ref mut player) = *player_guard {
                // Build speaker config
                let speaker_config = if speaker_enabled {
                    Some(OutputConfig {
                        device_id: speaker_device_id,
                        volume: speaker_volume,
                    })
                } else {
                    None
                };

                // Build virtual mic config
                let virtual_mic_config = virtual_mic_device_id.map(|id| OutputConfig {
                    device_id: Some(id),
                    volume: virtual_mic_volume,
                });

                // Set completion callback to clear speaking flag when playback finishes
                player.set_completion_callback(Box::new(move || {
                    eprintln!("[TTS OpenAI] Playback completed, clearing is_speaking flag");
                    if let Ok(mut speaking) = is_speaking.lock() {
                        *speaking = false;
                    }
                }));

                // This is non-blocking - returns immediately
                player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config)
                    .map_err(|e| format!("Failed to start playback: {}", e))?;
            } else {
                return Err("Audio player not initialized".to_string());
            }
        }

        // Return immediately - playback continues in background
        eprintln!("[TTS OpenAI] Returning immediately, playback continues in background");
        Ok(())
    }

    fn stop_openai(&self) -> std::result::Result<(), String> {
        eprintln!("[TTS OpenAI] Stopping playback");
        if let Ok(mut player_guard) = self.audio_player.lock() {
            if let Some(ref mut player) = *player_guard {
                player.stop();
                player.clear_completion_callback();
            }
        }
        Ok(())
    }

    // Silero TTS implementation (placeholder for future)
    fn speak_silero(&self, _text: &str) -> std::result::Result<(), String> {
        // TODO: Implement Silero TTS
        Err("Silero TTS not yet implemented".to_string())
    }

    fn stop_silero(&self) -> std::result::Result<(), String> {
        // TODO: Implement Silero TTS stop
        Ok(())
    }

    /// Get all available SAPI voices
    pub fn get_voices(&self) -> Vec<Voice> {
        let mut voices = Vec::new();

        println!("[TTS] get_voices: Starting voice enumeration");

        // Try multiple registry paths
        let paths_to_try = vec![
            "SOFTWARE\\Microsoft\\Speech\\Voices\\Tokens",
            "SOFTWARE\\Microsoft\\Speech_OneCore\\Voices\\Tokens",
            "SOFTWARE\\Wow6432Node\\Microsoft\\Speech\\Voices\\Tokens",
        ];

        for path in paths_to_try {
            println!("[TTS] get_voices: Trying path: {}", path);
            let result = self.enumerate_voices_from_registry(path);
            println!("[TTS] get_voices: Found {} voices from {}", result.len(), path);
            voices.extend(result);
        }

        // Also try using sapi_lite to get voices via COM
        let sapi_voices = self.get_voices_from_sapi();
        println!("[TTS] get_voices: Found {} voices from SAPI COM", sapi_voices.len());
        voices.extend(sapi_voices);

        println!("[TTS] get_voices: Total voices found: {}", voices.len());

        if voices.is_empty() {
            // Fallback to default if no voices found
            println!("[TTS] get_voices: No voices found, using fallback");
            voices.push(Voice {
                id: "default".to_string(),
                name: "Microsoft David (Desktop)".to_string(),
            });
            voices.push(Voice {
                id: "default2".to_string(),
                name: "Microsoft Zira (Desktop)".to_string(),
            });
        }

        voices
    }

    /// Get voices using SAPI COM interface via sapi_lite
    fn get_voices_from_sapi(&self) -> Vec<Voice> {
        let mut voices = Vec::new();

        // Standard Windows voices that are commonly available
        // English voices
        voices.push(Voice {
            id: "MSSpeech_TTS_en-US_David_11.0".to_string(),
            name: "Microsoft David (English US)".to_string(),
        });
        voices.push(Voice {
            id: "MSSpeech_TTS_en-US_Zira_11.0".to_string(),
            name: "Microsoft Zira (English US)".to_string(),
        });
        voices.push(Voice {
            id: "MSSpeech_TTS_en-GB_George_11.0".to_string(),
            name: "Microsoft George (English UK)".to_string(),
        });
        voices.push(Voice {
            id: "MSSpeech_TTS_en-GB_Hazel_11.0".to_string(),
            name: "Microsoft Hazel (English UK)".to_string(),
        });

        // Russian voices
        voices.push(Voice {
            id: "MSSpeech_TTS_ru-RU_Irina_11.0".to_string(),
            name: "Microsoft Irina (Русский)".to_string(),
        });
        voices.push(Voice {
            id: "MSSpeech_TTS_ru-RU_Pavel_11.0".to_string(),
            name: "Microsoft Pavel (Русский)".to_string(),
        });

        println!("[TTS] get_voices_from_sapi: Added {} standard Windows voices", voices.len());

        voices
    }

    /// Helper function to enumerate voices from registry
    fn enumerate_voices_from_registry(&self, path: &str) -> Vec<Voice> {
        let mut voices = Vec::new();

        println!("[TTS] enumerate_voices_from_registry: Checking path: {}", path);

        use windows::Win32::System::Registry::*;
        use windows::core::{PCSTR, PSTR};

        unsafe {
            let mut hkey = HKEY::default();

            // Convert path to PCSTR
            let path_pcstr = PCSTR::from_raw(path.as_bytes().as_ptr());

            // Open the registry key with KEY_WOW64_64KEY flag to access 64-bit registry
            // This is necessary for 32-bit applications running on 64-bit Windows
            let open_result = RegOpenKeyExA(
                HKEY_LOCAL_MACHINE,
                path_pcstr,
                0,
                KEY_READ | KEY_WOW64_64KEY,
                &mut hkey
            );

            if open_result.is_err() {
                println!("[TTS] enumerate_voices_from_registry: Failed to open registry key");
                return voices;
            }

            println!("[TTS] enumerate_voices_from_registry: Registry key opened successfully");

            // Enumerate all subkeys (voice tokens)
            let mut index = 0;
            let mut name_buf = [0u8; 256];
            loop {
                let mut name_len = name_buf.len() as u32;
                let name_pstr = PSTR::from_raw(name_buf.as_mut_ptr());

                let result = RegEnumKeyExA(
                    hkey,
                    index,
                    name_pstr,
                    &mut name_len,
                    None,
                    PSTR::null(),
                    None,
                    None
                );

                if result.is_err() {
                    break;
                }

                // Convert name to string
                let voice_name = String::from_utf8_lossy(
                    &name_buf[..name_len as usize]
                ).trim_end_matches('\0').to_string();

                println!("[TTS] enumerate_voices_from_registry: Found voice token: {}", voice_name);

                // Get the voice display name from the registry
                if let Some(display_name) = self.get_voice_display_name(hkey, &voice_name) {
                    // Create ID from the token path
                    let id = format!("{}\\{}", path, voice_name);

                    println!("[TTS] enumerate_voices_from_registry: Voice '{}' - '{}'", id, display_name);

                    voices.push(Voice {
                        id,
                        name: display_name,
                    });
                } else {
                    println!("[TTS] enumerate_voices_from_registry: Could not get display name for '{}'", voice_name);
                }

                // Reset buffer for next iteration
                name_buf = [0u8; 256];
                index += 1;
            }

            let _ = RegCloseKey(hkey);
        }

        voices
    }

    /// Get the display name for a voice from the registry
    fn get_voice_display_name(&self, hkey: windows::Win32::System::Registry::HKEY, voice_name: &str) -> Option<String> {
        use windows::Win32::System::Registry::*;
        use windows::core::PCSTR;

        unsafe {
            let mut subkey = HKEY::default();
            let voice_path_cstr = format!("{}\0", voice_name);
            let voice_path_pcstr = PCSTR::from_raw(voice_path_cstr.as_bytes().as_ptr());

            // Open the voice's registry key with KEY_WOW64_64KEY flag
            let open_result = RegOpenKeyExA(
                hkey,
                voice_path_pcstr,
                0,
                KEY_READ | KEY_WOW64_64KEY,
                &mut subkey
            );

            if open_result.is_err() {
                println!("[TTS] get_voice_display_name: Failed to open subkey for '{}'", voice_name);
                return None;
            }

            // Read the default value (display name)
            let mut data_type: REG_VALUE_TYPE = REG_NONE;
            let mut data = [0u16; 256];
            let mut data_size = (data.len() * 2) as u32;

            let result = RegQueryValueExW(
                subkey,
                None,
                None,
                Some(&mut data_type as *mut _),
                Some(data.as_mut_slice() as *mut _ as *mut u8),
                Some(&mut data_size)
            );

            let _ = RegCloseKey(subkey);

            if result.is_ok() && data_type == REG_SZ {
                // Find the null terminator
                let len = data.iter().position(|&c| c == 0).unwrap_or(data.len());
                let name = String::from_utf16_lossy(&data[..len]);
                println!("[TTS] get_voice_display_name: Got display name '{}' for '{}'", name, voice_name);
                if !name.is_empty() {
                    return Some(name);
                }
            }

            println!("[TTS] get_voice_display_name: No display name found for '{}'", voice_name);
            // Fallback: try to get the name from the Attributes value
            None
        }
    }

    /// Set the SAPI voice by ID
    pub fn set_voice(&self, _voice_id: String) -> std::result::Result<(), String> {
        // Ensure SAPI is initialized
        self.ensure_sapi_initialized()?;

        // Note: sapi_lite doesn't expose direct voice changing
        // This would require COM interface calls to ISpVoice::SetVoice
        // For now, just store the voice ID for future use
        // In a full implementation, you would:
        // 1. Get ISpObjectToken for the voice ID
        // 2. Call ISpVoice::SetVoice with the token

        Ok(())
    }

    /// Set TTS rate (speed)
    pub fn set_rate(&self, rate: i32) -> std::result::Result<(), String> {
        if let Ok(mut rate_guard) = self.rate.lock() {
            *rate_guard = rate.clamp(-10, 10);
        }

        // Apply the rate to SAPI synthesizer
        if let Ok(synth_guard) = self.sapi_synthesizer.lock() {
            if let Some(ref _synth) = *synth_guard {
                // sapi_lite doesn't expose rate setting directly
                // This would require COM interface calls to ISpVoice::SetRate
            }
        }

        Ok(())
    }

    /// Set TTS pitch
    pub fn set_pitch(&self, pitch: i32) -> std::result::Result<(), String> {
        if let Ok(mut pitch_guard) = self.pitch.lock() {
            *pitch_guard = pitch.clamp(-10, 10);
        }

        // Apply the pitch to SAPI synthesizer
        if let Ok(synth_guard) = self.sapi_synthesizer.lock() {
            if let Some(ref _synth) = *synth_guard {
                // sapi_lite doesn't expose pitch setting directly
                // This would require COM interface calls to ISpVoice::SetPitch
            }
        }

        Ok(())
    }

    /// Set TTS volume
    pub fn set_volume(&self, volume: i32) -> std::result::Result<(), String> {
        let clamped_volume = volume.clamp(0, 100);

        if let Ok(mut volume_guard) = self.volume.lock() {
            *volume_guard = clamped_volume;
        }

        // Apply the volume to SAPI synthesizer
        if let Ok(synth_guard) = self.sapi_synthesizer.lock() {
            if let Some(ref _synth) = *synth_guard {
                // sapi_lite doesn't expose volume setting directly
                // This would require COM interface calls to ISpVoice::SetVolume
            }
        }

        Ok(())
    }

    pub fn get_status(&self) -> TtsStatus {
        let provider = if let Ok(p) = self.provider.lock() {
            *p
        } else {
            TtsProvider::System
        };

        let sapi_available = if let Ok(available) = self.sapi_available.lock() {
            *available
        } else {
            false
        };

        let silero_available = if let Ok(available) = self.silero_available.lock() {
            *available
        } else {
            false
        };

        let silero_server_url = if let Ok(url) = self.silero_server_url.lock() {
            url.clone()
        } else {
            "http://localhost:8002".to_string()
        };

        let silero_voice = if let Ok(voice) = self.silero_voice.lock() {
            voice.clone()
        } else {
            "ru_v3".to_string()
        };

        TtsStatus {
            is_speaking: self.is_speaking(),
            provider: String::from(provider),
            continuous_play: false, // This is managed by AppState
            has_openai_key: self.has_openai_key(),
            sapi_available,
            silero_available,
            silero_server_url,
            silero_voice,
        }
    }

    // === OpenAI TTS methods ===

    /// Initialize OpenAI client with config directory
    pub fn init_openai_client(&self, config_dir: PathBuf) -> StdResult<(), String> {
        let client = OpenAIClient::new(config_dir)?;

        // Синхронизируем API ключ из загруженного конфига
        let api_key = client.get_config().api_key.clone();
        if let Ok(mut key_guard) = self.api_key.lock() {
            *key_guard = api_key;
        }

        if let Ok(mut client_guard) = self.openai_client.lock() {
            *client_guard = Some(client);
        }
        Ok(())
    }

    /// Set OpenAI temp directory for audio files
    pub fn set_openai_temp_dir(&self, temp_dir: PathBuf) {
        if let Ok(mut dir) = self.openai_temp_dir.lock() {
            *dir = Some(temp_dir);
        }
    }

    /// Get OpenAI voices
    pub fn get_openai_voices(&self) -> Vec<OpenAIVoice> {
        if let Ok(client_guard) = self.openai_client.lock() {
            if let Some(ref client) = *client_guard {
                return client.get_voices();
            }
        }
        OpenAIClient::get_static_voices()
    }

    /// Set OpenAI voice
    pub fn set_openai_voice(&self, voice: String) -> StdResult<(), String> {
        self.openai_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_voice(voice))
            })
    }

    /// Set OpenAI speed
    pub fn set_openai_speed(&self, speed: f32) -> StdResult<(), String> {
        self.openai_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_speed(speed))
            })
    }

    /// Set OpenAI instructions
    pub fn set_openai_instructions(&self, instructions: String) -> StdResult<(), String> {
        self.openai_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_instructions(instructions))
            })
    }

    /// Set OpenAI proxy
    pub fn set_openai_proxy(&self, host: Option<String>, port: Option<u16>) -> StdResult<(), String> {
        self.openai_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_proxy(host, port))
            })
    }

    /// Get OpenAI config
    pub fn get_openai_config(&self) -> OpenAIConfig {
        if let Ok(client_guard) = self.openai_client.lock() {
            if let Some(ref client) = *client_guard {
                return client.get_config().clone();
            }
        }
        OpenAIConfig::default()
    }

    // === Localhost TTS methods ===

    /// Initialize Localhost client with config directory
    pub fn init_localhost_client(&self, config_dir: PathBuf) -> StdResult<(), String> {
        let client = LocalhostClient::new(config_dir)?;

        if let Ok(mut client_guard) = self.localhost_client.lock() {
            *client_guard = Some(client);
        }
        Ok(())
    }

    /// Localhost TTS implementation using Rodio for non-blocking playback
    fn speak_localhost(&self, text: &str) -> std::result::Result<(), String> {
        eprintln!("[TTS Localhost] Starting speech for text: '{}'", text);

        // Get audio output settings
        let speaker_enabled = self.speaker_enabled.lock()
            .map(|e| *e)
            .unwrap_or(true);
        let speaker_device_id = self.speaker_device_id.lock()
            .map(|id| id.clone())
            .unwrap_or(None);
        let speaker_volume = self.speaker_volume.lock()
            .map(|v| *v)
            .unwrap_or(1.0);
        let virtual_mic_device_id = self.virtual_mic_device_id.lock()
            .map(|id| id.clone())
            .unwrap_or(None);
        let virtual_mic_volume = self.virtual_mic_volume.lock()
            .map(|v| *v)
            .unwrap_or(1.0);

        // Check if at least one output is enabled
        if !speaker_enabled && virtual_mic_device_id.is_none() {
            if let Ok(mut speaking) = self.is_speaking.lock() {
                *speaking = false;
            }
            return Err("Both speaker and virtual mic are disabled. Please enable at least one output.".to_string());
        }

        // Clone needed data before releasing mutex
        let (text_clone, client_clone) = {
            let client_guard = match self.localhost_client.lock() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    eprintln!("[TTS] Localhost client mutex was poisoned, recovering...");
                    poisoned.into_inner()
                }
            };

            let client = client_guard.as_ref()
                .ok_or_else(|| "Localhost client not initialized".to_string())?;

            // Clone the client's config data (not the whole client)
            (text.to_string(), client.get_config().clone())
        };

        eprintln!("[TTS Localhost] Calling local server API...");
        // Run async HTTP request in a separate thread with its own runtime
        let audio_data = std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| format!("Failed to create runtime: {}", e))?;

            rt.block_on(async {
                let temp_client = LocalhostClient::new_for_request(client_clone);
                temp_client.synthesize(&text_clone).await
            })
        })
        .join()
        .map_err(|e| format!("Thread panicked: {:?}", e))??;

        eprintln!("[TTS Localhost] Received {} bytes from API", audio_data.len());

        // Validate we got some data
        if audio_data.is_empty() {
            return Err("Received empty audio data from local server".to_string());
        }

        eprintln!("[TTS Localhost] Starting Rodio async playback (speaker={}, virtual_mic={:?})",
            speaker_enabled, virtual_mic_device_id);

        // Clone Arc for the completion callback
        let is_speaking = Arc::clone(&self.is_speaking);

        // Get audio player and start non-blocking dual output playback
        {
            let mut player_guard = self.audio_player.lock()
                .map_err(|e| format!("Failed to lock audio player: {}", e))?;

            if let Some(ref mut player) = *player_guard {
                // Build speaker config
                let speaker_config = if speaker_enabled {
                    Some(OutputConfig {
                        device_id: speaker_device_id,
                        volume: speaker_volume,
                    })
                } else {
                    None
                };

                // Build virtual mic config
                let virtual_mic_config = virtual_mic_device_id.map(|id| OutputConfig {
                    device_id: Some(id),
                    volume: virtual_mic_volume,
                });

                // Set completion callback to clear speaking flag when playback finishes
                player.set_completion_callback(Box::new(move || {
                    eprintln!("[TTS Localhost] Playback completed, clearing is_speaking flag");
                    if let Ok(mut speaking) = is_speaking.lock() {
                        *speaking = false;
                    }
                }));

                // This is non-blocking - returns immediately
                player.play_mp3_async_dual(audio_data, speaker_config, virtual_mic_config)
                    .map_err(|e| format!("Failed to start playback: {}", e))?;
            } else {
                return Err("Audio player not initialized".to_string());
            }
        }

        // Return immediately - playback continues in background
        eprintln!("[TTS Localhost] Returning immediately, playback continues in background");
        Ok(())
    }

    fn stop_localhost(&self) -> std::result::Result<(), String> {
        eprintln!("[TTS Localhost] Stopping playback");
        if let Ok(mut player_guard) = self.audio_player.lock() {
            if let Some(ref mut player) = *player_guard {
                player.stop();
                player.clear_completion_callback();
            }
        }
        Ok(())
    }

    /// Get Localhost voices
    pub fn get_localhost_voices(&self) -> Vec<LocalhostVoice> {
        if let Ok(client_guard) = self.localhost_client.lock() {
            if let Some(ref client) = *client_guard {
                return client.get_voices();
            }
        }
        Vec::new()
    }

    /// Set Localhost port
    pub fn set_localhost_port(&self, port: String) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_port(port))
            })
    }

    /// Set Localhost token
    pub fn set_localhost_token(&self, token: String) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_token(token))
            })
    }

    /// Set Localhost voice
    pub fn set_localhost_voice(&self, voice: Option<String>) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_voice(voice))
            })
    }

    /// Set Localhost connected status
    pub fn set_localhost_connected(&self, connected: bool) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.set_connected(connected))
            })
    }

    /// Get Localhost config
    pub fn get_localhost_config(&self) -> LocalhostConfig {
        if let Ok(client_guard) = self.localhost_client.lock() {
            if let Some(ref client) = *client_guard {
                return client.get_config().clone();
            }
        }
        LocalhostConfig::default()
    }

    /// Update Localhost voices (save to file)
    pub fn update_localhost_voices(&self, voices: Vec<LocalhostVoice>) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.update_voices(voices))
            })
    }

    /// Clear Localhost voices cache
    #[allow(dead_code)]
    pub fn clear_localhost_voices(&self) -> StdResult<(), String> {
        self.localhost_client.lock()
            .map_err(|_| "Failed to lock".to_string())
            .and_then(|mut client| {
                client.as_mut()
                    .ok_or_else(|| "Client not initialized".to_string())
                    .map(|c| c.clear_voices())
            })
    }
}

impl Default for TtsEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Clone implementation for sharing between threads
impl Clone for TtsEngine {
    fn clone(&self) -> Self {
        Self {
            provider: Arc::clone(&self.provider),
            config_dir: Arc::clone(&self.config_dir),
            sapi_synthesizer: Arc::clone(&self.sapi_synthesizer),
            sapi_available: Arc::clone(&self.sapi_available),
            api_key: Arc::clone(&self.api_key),
            openai_client: Arc::clone(&self.openai_client),
            openai_temp_dir: Arc::clone(&self.openai_temp_dir),
            voice: self.voice.clone(),
            localhost_client: Arc::clone(&self.localhost_client),
            rate: Arc::clone(&self.rate),
            pitch: Arc::clone(&self.pitch),
            volume: Arc::clone(&self.volume),
            silero_server_url: Arc::clone(&self.silero_server_url),
            silero_voice: Arc::clone(&self.silero_voice),
            silero_available: Arc::clone(&self.silero_available),
            is_speaking: Arc::clone(&self.is_speaking),
            // Audio output settings
            audio_player: Arc::clone(&self.audio_player),
            speaker_device_id: Arc::clone(&self.speaker_device_id),
            speaker_enabled: Arc::clone(&self.speaker_enabled),
            speaker_volume: Arc::clone(&self.speaker_volume),
            virtual_mic_device_id: Arc::clone(&self.virtual_mic_device_id),
            virtual_mic_volume: Arc::clone(&self.virtual_mic_volume),
        }
    }
}
