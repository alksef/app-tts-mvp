// Audio player using Rodio for non-blocking playback
// The stream and sink live in the playback thread only - they're not Send/Sync

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::Device;
use rodio::{Decoder, OutputStream, Sink, Source};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::sync::Mutex as StdMutex;

/// Handle to control background playback
#[derive(Clone)]
pub struct PlaybackHandle {
    stop_flag: Arc<AtomicBool>,
}

impl PlaybackHandle {
    pub fn new() -> Self {
        Self {
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
    }

    fn should_stop(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }
}

/// Configuration for audio output to a specific device
#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub device_id: Option<String>,
    pub volume: f32,  // 0.0 - 1.0
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            device_id: None,
            volume: 1.0,
        }
    }
}

/// Callback type for playback completion notification
pub type PlaybackCompleteCallback = Arc<StdMutex<Box<dyn FnOnce() + Send>>>;

/// Simple audio player for MP3 playback with dual output support
pub struct AudioPlayer {
    current_handle: Option<PlaybackHandle>,
    completion_callback: Option<PlaybackCompleteCallback>,
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            current_handle: None,
            completion_callback: None,
        }
    }

    /// Set a callback to be invoked when playback completes
    pub fn set_completion_callback(&mut self, callback: Box<dyn FnOnce() + Send>) {
        self.completion_callback = Some(Arc::new(StdMutex::new(callback)));
    }

    /// Clear the completion callback
    pub fn clear_completion_callback(&mut self) {
        self.completion_callback = None;
    }

    /// Find a device by its name (id)
    fn find_device_by_name(device_id: &str) -> Option<Device> {
        let host = cpal::default_host();
        if let Ok(all_devices) = host.devices() {
            for device in all_devices {
                if let Ok(name) = device.name() {
                    if name == device_id {
                        return Some(device);
                    }
                }
            }
        }
        None
    }

    /// Get device for playback, falling back to default if needed
    fn get_device(device_id: &Option<String>) -> Result<Device, String> {
        match device_id {
            Some(id) => {
                if let Some(device) = Self::find_device_by_name(id) {
                    Ok(device)
                } else {
                    eprintln!("[AudioPlayer] Device '{}' not found, using default", id);
                    let host = cpal::default_host();
                    host.default_output_device()
                        .ok_or_else(|| "No default output device".to_string())
                }
            }
            None => {
                let host = cpal::default_host();
                host.default_output_device()
                    .ok_or_else(|| "No default output device".to_string())
            }
        }
    }

    /// Play MP3 audio data to a single device asynchronously
    fn play_to_device(
        device: Device,
        audio_data: Vec<u8>,
        volume: f32,
        handle: PlaybackHandle,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let device_name = device.name().unwrap_or_default();
            eprintln!("[AudioPlayer] Playback thread starting for device: {}", device_name);

            // Create stream and sink in this thread (they're not Send)
            let (stream, stream_handle) = match OutputStream::try_from_device(&device) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[AudioPlayer] Failed to create output stream for '{}': {}", device_name, e);
                    return;
                }
            };

            let sink = match Sink::try_new(&stream_handle) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[AudioPlayer] Failed to create sink: {}", e);
                    return;
                }
            };

            // Decode MP3 from memory
            let cursor = Cursor::new(audio_data);

            // Rodio's Decoder auto-detects format, works with MP3
            let source = match Decoder::new(cursor) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[AudioPlayer] Failed to decode audio: {}", e);
                    return;
                }
            };

            // Apply volume
            let source = source.amplify(volume);

            // Append to sink and play
            sink.append(source);

            // Keep stream alive until playback finishes or stop is requested
            while !sink.empty() && !handle.should_stop() {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            if handle.should_stop() {
                eprintln!("[AudioPlayer] Playback stopped by request for device: {}", device_name);
            } else {
                eprintln!("[AudioPlayer] Playback completed for device: {}", device_name);
            }

            // Drop sink and stream here
            drop(sink);
            drop(stream);
        })
    }

    /// Play MP3 audio data asynchronously to multiple outputs (speaker + virtual mic)
    ///
    /// # Arguments
    /// * `audio_data` - MP3 audio data bytes
    /// * `speaker_config` - Speaker output configuration (None = disabled)
    /// * `virtual_mic_config` - Virtual mic output configuration (None = disabled)
    pub fn play_mp3_async_dual(
        &mut self,
        audio_data: Vec<u8>,
        speaker_config: Option<OutputConfig>,
        virtual_mic_config: Option<OutputConfig>,
    ) -> Result<(), String> {
        eprintln!("[AudioPlayer] play_mp3_async_dual START, {} bytes, speaker={:?}, virtual_mic={:?}",
            audio_data.len(),
            speaker_config.as_ref().map(|c| &c.device_id),
            virtual_mic_config.as_ref().map(|c| &c.device_id)
        );

        // Stop any existing playback
        self.stop();

        // Check at least one output is enabled
        if speaker_config.is_none() && virtual_mic_config.is_none() {
            return Err("No output enabled".to_string());
        }

        // Create a new playback handle
        let handle = PlaybackHandle::new();
        self.current_handle = Some(handle.clone());

        // Take the completion callback (if set)
        let completion_callback = self.completion_callback.take();

        let mut handles = vec![];

        // Play to speaker if enabled
        if let Some(config) = speaker_config {
            let device = Self::get_device(&config.device_id)?;
            eprintln!("[AudioPlayer] Starting speaker playback: '{}'", device.name().unwrap_or_default());
            let audio_data_clone = audio_data.clone();
            handles.push(Self::play_to_device(device, audio_data_clone, config.volume, handle.clone()));
        }

        // Play to virtual mic if enabled
        if let Some(config) = virtual_mic_config {
            let device = Self::get_device(&config.device_id)?;
            eprintln!("[AudioPlayer] Starting virtual mic playback: '{}'", device.name().unwrap_or_default());
            handles.push(Self::play_to_device(device, audio_data, config.volume, handle.clone()));
        }

        // Spawn a thread to wait for all playback threads and call completion callback when done
        thread::spawn(move || {
            for h in handles {
                let _ = h.join();
            }
            eprintln!("[AudioPlayer] All playback threads finished");

            // Call completion callback if set
            if let Some(callback) = completion_callback {
                let callback = Arc::try_unwrap(callback).ok();
                if let Some(mutex) = callback {
                    if let Ok(cb) = mutex.into_inner() {
                        cb();
                    }
                }
            }
        });

        eprintln!("[AudioPlayer] play_mp3_async_dual END (background playback started)");
        Ok(())
    }

    /// Stop playback
    pub fn stop(&mut self) {
        eprintln!("[AudioPlayer] Stopping playback");

        if let Some(ref handle) = self.current_handle {
            handle.stop();
        }

        self.current_handle = None;
        eprintln!("[AudioPlayer] Stop signal sent");
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}
