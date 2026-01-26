//! Plugin configuration management

use super::types::{LoadedPlugin, PluginsConfigFile};
use std::fs;
use std::path::PathBuf;

/// Manages plugin configuration file
pub struct PluginConfigManager {
    plugins_dir: PathBuf,
    config_path: PathBuf,
}

impl PluginConfigManager {
    /// Create new config manager
    pub fn new(plugins_dir: PathBuf) -> Result<Self, String> {
        let config_path = plugins_dir.join("plugins-config.json");

        // Ensure plugins directory exists
        if !plugins_dir.exists() {
            fs::create_dir_all(&plugins_dir)
                .map_err(|e| format!("Failed to create plugins dir: {}", e))?;
        }

        Ok(Self {
            plugins_dir,
            config_path,
        })
    }

    /// Get plugins directory path
    pub fn plugins_dir(&self) -> PathBuf {
        self.plugins_dir.clone()
    }

    /// Load config file
    fn load_config(&self) -> Result<PluginsConfigFile, String> {
        if !self.config_path.exists() {
            return Ok(PluginsConfigFile::default());
        }

        let content = fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))
    }

    /// Save config file
    fn save_config(&self, config: &PluginsConfigFile) -> Result<(), String> {
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Get saved config for a plugin
    pub fn get_plugin_config(&self, name: &str) -> Option<super::types::PluginConfig> {
        let config = self.load_config().ok()?;
        config.plugins.get(name).cloned()
    }

    /// Save config for a single plugin
    pub fn save_plugin_config(&self, plugin: &LoadedPlugin) -> Result<(), String> {
        let mut config = self.load_config()?;
        config.plugins.insert(
            plugin.name.clone(),
            super::types::PluginConfig {
                enabled: plugin.enabled,
                config: plugin.config.clone(),
                last_error: plugin.last_error.clone(),
            },
        );
        self.save_config(&config)
    }

    /// Save all plugin states
    pub fn save_all_from_manager(&self, plugins: &[LoadedPlugin]) -> Result<(), String> {
        let mut config = PluginsConfigFile::default();

        for plugin in plugins {
            config.plugins.insert(
                plugin.name.clone(),
                super::types::PluginConfig {
                    enabled: plugin.enabled,
                    config: plugin.config.clone(),
                    last_error: plugin.last_error.clone(),
                },
            );
        }

        self.save_config(&config)
    }
}
