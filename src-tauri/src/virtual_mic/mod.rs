//! Virtual microphone and dual audio output module
//!
//! This module provides functionality for simultaneous audio output to speakers
//! and virtual microphone devices (e.g., VB-Cable, VoiceMeeter).

pub mod audio_settings;
pub mod device;

pub use audio_settings::{AudioSettings, AudioSettingsManager};
pub use device::{find_all_output_devices, find_virtual_devices, OutputDeviceInfo, VirtualDeviceInfo};
