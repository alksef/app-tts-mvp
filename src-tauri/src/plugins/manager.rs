//! Plugin manager - handles loading, config, and broadcasting

use super::config::PluginConfigManager;
use super::dynamic::{load_plugin, scan_plugins_dir};
use super::types::{LoadedPlugin, PluginInfo};

/// Manages all loaded plugins
pub struct PluginManager {
    plugins: Vec<LoadedPlugin>,
    config_manager: PluginConfigManager,
}

impl PluginManager {
    /// Create new plugin manager
    pub fn new(plugins_dir: std::path::PathBuf) -> Result<Self, String> {
        let config_manager = PluginConfigManager::new(plugins_dir.clone())?;
        Ok(Self {
            plugins: Vec::new(),
            config_manager,
        })
    }

    /// Load all plugins from directory
    pub fn load_all(&mut self) -> Result<usize, String> {
        let plugin_paths = scan_plugins_dir(&self.config_manager.plugins_dir());
        let mut loaded = 0;

        for path in plugin_paths {
            match load_plugin(&path) {
                Ok(mut plugin) => {
                    // Load saved config
                    if let Some(saved_config) = self.config_manager.get_plugin_config(&plugin.name) {
                        plugin.enabled = saved_config.enabled;
                        plugin.last_error = saved_config.last_error;

                        // Set config if available
                        if !saved_config.config.is_null() {
                            let _ = plugin.set_config(&saved_config.config);
                        }
                    }

                    self.plugins.push(plugin);
                    loaded += 1;
                }
                Err(e) => {
                    eprintln!("Failed to load plugin {:?}: {}", path.display(), e);
                }
            }
        }

        // Save updated config
        self.config_manager.save_all_from_manager(&self.plugins)?;

        Ok(loaded)
    }

    /// Get all plugins info
    pub fn get_plugins(&self) -> Vec<PluginInfo> {
        self.plugins.iter().map(|p| p.info()).collect()
    }

    /// Find plugin index by name
    fn find_plugin_index(&self, name: &str) -> Option<usize> {
        self.plugins.iter().position(|p| p.name == name)
    }

    /// Set plugin config
    pub fn set_plugin_config(&mut self, name: &str, config: &serde_json::Value) -> Result<(), String> {
        if let Some(idx) = self.find_plugin_index(name) {
            self.plugins[idx].set_config(config)?;
            self.config_manager.save_plugin_config(&self.plugins[idx])?;
            Ok(())
        } else {
            Err(format!("Plugin '{}' not found", name))
        }
    }

    /// Toggle plugin enabled state
    pub fn toggle_plugin(&mut self, name: &str, enabled: bool) -> Result<(), String> {
        if let Some(idx) = self.find_plugin_index(name) {
            self.plugins[idx].set_enabled(enabled);
            self.config_manager.save_plugin_config(&self.plugins[idx])?;
            Ok(())
        } else {
            Err(format!("Plugin '{}' not found", name))
        }
    }

    /// Check plugin status
    pub fn check_plugin_status(&self, name: &str) -> Result<plugins_api::PluginStatus, String> {
        self.plugins
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.check_status())
            .ok_or_else(|| format!("Plugin '{}' not found", name))
    }

    /// Broadcast text to all enabled plugins
    /// Returns true if any plugin state changed (was disabled due to error)
    pub fn broadcast_text(&mut self, text: &str) -> bool {
        let mut has_changes = false;
        for plugin in &mut self.plugins {
            if !plugin.enabled {
                continue;
            }

            match plugin.on_text(text) {
                Ok(_) => {
                    plugin.last_error = None;
                }
                Err(e) => {
                    // Disable plugin on error and save error
                    plugin.set_enabled(false);
                    plugin.set_error(e.clone());
                    let _ = self.config_manager.save_plugin_config(plugin);
                    eprintln!("Plugin '{}' failed: {}, disabling", plugin.name, e);
                    has_changes = true;
                }
            }
        }

        // Save updated errors
        let _ = self.config_manager.save_all_from_manager(&self.plugins);
        has_changes
    }

    #[allow(dead_code)]
    /// Get plugins slice for config manager
    pub fn get_plugins_slice(&self) -> &[LoadedPlugin] {
        &self.plugins
    }
}

// SAFETY: PluginManager is Send because all mutable access is through Mutex
unsafe impl Send for PluginManager {}
