//! Audio settings persistence module
//!
//! Manages saving and loading audio output settings to/from disk.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Audio output settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSettings {
    /// Speaker device ID (None = default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_device: Option<String>,
    /// Speaker enabled
    pub speaker_enabled: bool,
    /// Speaker volume (0-100)
    pub speaker_volume: u8,
    /// Last speaker device (for fallback)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_speaker_device: Option<String>,
    /// Virtual mic device ID (None = not selected = disabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_mic_device: Option<String>,
    /// Virtual mic volume (0-100)
    pub virtual_mic_volume: u8,
    /// Last virtual mic device (for quick enable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_virtual_mic_device: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            speaker_device: None,
            speaker_enabled: true,
            speaker_volume: 100,
            last_speaker_device: None,
            virtual_mic_device: None,
            virtual_mic_volume: 100,
            last_virtual_mic_device: None,
        }
    }
}

/// Manager for audio settings persistence
pub struct AudioSettingsManager {
    file_path: PathBuf,
    settings: AudioSettings,
}

impl AudioSettingsManager {
    /// Create a new AudioSettingsManager
    ///
    /// # Arguments
    /// * `config_dir` - Configuration directory path
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let file_path = config_dir.join("audio_settings.json");

        let settings = if file_path.exists() {
            let content = fs::read_to_string(&file_path)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse JSON: {}", e))?
        } else {
            AudioSettings::default()
        };

        Ok(Self { file_path, settings })
    }

    /// Save current settings to disk
    pub fn save(&self) -> Result<(), String> {
        let content = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| format!("Failed to serialize: {}", e))?;
        fs::write(&self.file_path, content)
            .map_err(|e| format!("Failed to write file: {}", e))?;
        Ok(())
    }

    /// Get current settings
    pub fn get(&self) -> &AudioSettings {
        &self.settings
    }

    /// Update settings with a closure and save
    pub fn update<F>(&mut self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut AudioSettings),
    {
        f(&mut self.settings);
        self.save()
    }

    /// Set speaker device and save
    pub fn set_speaker_device(&mut self, device: Option<String>) -> Result<(), String> {
        self.update(|s| {
            if device.is_some() {
                s.last_speaker_device = device.clone();
            }
            s.speaker_device = device;
        })
    }

    /// Set speaker enabled and save
    pub fn set_speaker_enabled(&mut self, enabled: bool) -> Result<(), String> {
        self.update(|s| s.speaker_enabled = enabled)
    }

    /// Set speaker volume and save
    pub fn set_speaker_volume(&mut self, volume: u8) -> Result<(), String> {
        self.update(|s| s.speaker_volume = volume.min(100))
    }

    /// Set virtual mic device and save
    pub fn set_virtual_mic_device(&mut self, device: Option<String>) -> Result<(), String> {
        self.update(|s| {
            if device.is_some() {
                s.last_virtual_mic_device = device.clone();
            }
            s.virtual_mic_device = device;
        })
    }

    /// Enable virtual mic (use last device)
    pub fn enable_virtual_mic(&mut self) -> Result<(), String> {
        let last_device = self.settings.last_virtual_mic_device.clone();
        if last_device.is_none() {
            return Err("No previous virtual mic device".to_string());
        }
        self.set_virtual_mic_device(last_device)
    }

    /// Disable virtual mic
    pub fn disable_virtual_mic(&mut self) -> Result<(), String> {
        self.set_virtual_mic_device(None)
    }

    /// Set virtual mic volume and save
    pub fn set_virtual_mic_volume(&mut self, volume: u8) -> Result<(), String> {
        self.update(|s| s.virtual_mic_volume = volume.min(100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_audio_settings_default() {
        let settings = AudioSettings::default();
        assert_eq!(settings.speaker_device, None);
        assert_eq!(settings.speaker_enabled, true);
        assert_eq!(settings.speaker_volume, 100);
        assert_eq!(settings.virtual_mic_device, None);
        assert_eq!(settings.virtual_mic_volume, 100);
    }

    #[test]
    fn test_audio_settings_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = AudioSettingsManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Test setting values
        manager.set_speaker_volume(75).unwrap();
        assert_eq!(manager.get().speaker_volume, 75);

        // Test save/load
        let manager2 = AudioSettingsManager::new(temp_dir.path().to_path_buf()).unwrap();
        assert_eq!(manager2.get().speaker_volume, 75);
    }

    #[test]
    fn test_volume_f32() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = AudioSettingsManager::new(temp_dir.path().to_path_buf()).unwrap();

        manager.set_speaker_volume(50).unwrap();
        assert_eq!(manager.speaker_volume_f32(), 0.5);

        manager.set_virtual_mic_volume(25).unwrap();
        assert_eq!(manager.virtual_mic_volume_f32(), 0.25);
    }
}
