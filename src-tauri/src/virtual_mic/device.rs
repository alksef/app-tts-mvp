//! Audio device discovery module
//!
//! Provides functions for discovering audio output devices and virtual microphone devices.

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

/// Information about an audio output device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Information about a virtual audio device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// Find all audio output devices in the system
pub fn find_all_output_devices() -> Vec<OutputDeviceInfo> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    let default_device = host.default_output_device();

    if let Ok(all_devices) = host.devices() {
        for device in all_devices {
            if let Ok(name) = device.name() {
                let is_default = default_device.as_ref()
                    .and_then(|d| d.name().ok())
                    .as_ref()
                    == Some(&name);

                // Use device name as ID since cpal Device doesn't have a stable ID
                devices.push(OutputDeviceInfo {
                    id: name.clone(),
                    name,
                    is_default,
                });
            }
        }
    }

    devices
}

/// Find virtual audio devices (VB-Cable, VoiceMeeter, etc.)
///
/// Discovers devices by keywords in their name: "cable", "virtual",
/// "voicemeeter", "vb-audio", "aux"
pub fn find_virtual_devices() -> Vec<VirtualDeviceInfo> {
    let host = cpal::default_host();
    let mut devices = Vec::new();

    // Keywords for detecting virtual devices
    let keywords = [
        "cable",        // VB-Cable, VoiceMeeter Cable
        "virtual",      // Virtual Speaker, Virtual Audio
        "voicemeeter",  // VoiceMeeter, VAIO
        "vb-audio",     // VB-Audio products
        "aux",          // VoiceMeeter AUX
    ];

    if let Ok(all_devices) = host.devices() {
        for device in all_devices {
            if let Ok(name) = device.name() {
                let name_lower = name.to_lowercase();

                // Check if device name contains any virtual device keyword
                let is_virtual = keywords.iter()
                    .any(|kw| name_lower.contains(kw));

                if is_virtual {
                    devices.push(VirtualDeviceInfo {
                        id: name.clone(),
                        name,
                        is_default: false,
                    });
                }
            }
        }
    }

    devices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_output_devices() {
        let devices = find_all_output_devices();
        println!("Found {} output devices:", devices.len());
        for device in &devices {
            println!("  - {} (default: {})", device.name, device.is_default);
        }
    }

    #[test]
    fn test_find_virtual_devices() {
        let devices = find_virtual_devices();
        println!("Found {} virtual devices:", devices.len());
        for device in &devices {
            println!("  - {}", device.name);
        }
    }
}
