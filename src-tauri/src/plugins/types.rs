//! Plugin types and structures

use plugins_api::{PluginStatus, PluginVTable};
use serde::{Deserialize, Serialize};

/// Serializable wrapper for PluginStatus
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(i32)]
pub enum SerializablePluginStatus {
    Ok = 0,
    NotConfigured = 1,
    ConnectionFailed = 2,
    AuthFailed = 3,
    UnknownError = 4,
}

impl From<PluginStatus> for SerializablePluginStatus {
    fn from(status: PluginStatus) -> Self {
        match status {
            PluginStatus::Ok => Self::Ok,
            PluginStatus::NotConfigured => Self::NotConfigured,
            PluginStatus::ConnectionFailed => Self::ConnectionFailed,
            PluginStatus::AuthFailed => Self::AuthFailed,
            PluginStatus::UnknownError => Self::UnknownError,
        }
    }
}

impl From<SerializablePluginStatus> for PluginStatus {
    fn from(status: SerializablePluginStatus) -> Self {
        match status {
            SerializablePluginStatus::Ok => Self::Ok,
            SerializablePluginStatus::NotConfigured => Self::NotConfigured,
            SerializablePluginStatus::ConnectionFailed => Self::ConnectionFailed,
            SerializablePluginStatus::AuthFailed => Self::AuthFailed,
            SerializablePluginStatus::UnknownError => Self::UnknownError,
        }
    }
}

/// Information about a loaded plugin for UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub status: SerializablePluginStatus,
    pub config_schema: serde_json::Value,
    pub config: serde_json::Value,
    pub last_error: Option<String>,
}

/// Configuration stored for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub enabled: bool,
    pub config: serde_json::Value,
    pub last_error: Option<String>,
}

/// Internal loaded plugin state
pub struct LoadedPlugin {
    /// Library handle (kept to prevent unloading)
    #[allow(dead_code)]
    pub library: libloading::Library,
    /// Plugin vtable
    pub vtable: PluginVTable,
    /// Opaque plugin data pointer
    pub data: *mut std::ffi::c_void,
    /// Plugin name (cached)
    pub name: String,
    /// Plugin version string (cached)
    pub version: String,
    /// Config schema (cached)
    pub config_schema: serde_json::Value,
    /// Current configuration values
    pub config: serde_json::Value,
    /// Is plugin enabled
    pub enabled: bool,
    /// Last error message
    pub last_error: Option<String>,
}

// SAFETY: LoadedPlugin is Send because all access is synchronized through Mutex
// and the plugin vtable functions are called with proper synchronization
unsafe impl Send for LoadedPlugin {}

impl LoadedPlugin {
    /// Get plugin info for UI
    pub fn info(&self) -> PluginInfo {
        PluginInfo {
            name: self.name.clone(),
            version: self.version.clone(),
            enabled: self.enabled,
            status: self.check_status().into(),
            config_schema: self.config_schema.clone(),
            config: self.config.clone(),
            last_error: self.last_error.clone(),
        }
    }

    /// Check current plugin status
    pub fn check_status(&self) -> PluginStatus {
        (self.vtable.check_status)(self.data)
    }

    /// Set configuration for plugin (always saves config locally)
    pub fn set_config(&mut self, config: &serde_json::Value) -> Result<(), String> {
        // Always save config locally first
        self.config = config.clone();

        let json = serde_json::to_string(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        let result = (self.vtable.set_config)(
            self.data,
            json.as_ptr() as *const ::std::ffi::c_char,
            json.len(),
        );

        if result == 0 {
            self.last_error = None;
            Ok(())
        } else {
            let err = format!("Plugin set_config returned error code: {}", result);
            self.last_error = Some(err.clone());
            Err(err)
        }
    }

    /// Broadcast text to this plugin (if enabled)
    pub fn on_text(&mut self, text: &str) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        // Use catch_unwind to prevent plugin panics from crashing the app
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = (self.vtable.on_text)(
                self.data,
                text.as_ptr() as *const ::std::ffi::c_char,
                text.len(),
            );

            if result == 0 {
                Ok(())
            } else {
                Err(format!("Plugin on_text returned error code: {}", result))
            }
        }))
        .unwrap_or_else(|_| {
            Err("Plugin panicked during on_text".to_string())
        })
    }

    /// Toggle enabled state
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.last_error = None;
        }
    }

    /// Set error message
    pub fn set_error(&mut self, error: String) {
        self.last_error = Some(error);
    }
}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        (self.vtable.destroy)(self.data);
    }
}

/// Config file structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginsConfigFile {
    pub plugins: std::collections::HashMap<String, PluginConfig>,
}
