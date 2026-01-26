//! Dynamic library loader for plugins

use super::types::LoadedPlugin;
use libloading::{Library, Symbol};
use plugins_api::{c_str_to_string, PluginVTable};
use std::path::{Path, PathBuf};

/// Function pointer type for getting plugin vtable
type GetVTable = extern "C" fn() -> *const PluginVTable;

/// Load a single plugin from a DLL file
pub fn load_plugin(path: &Path) -> Result<LoadedPlugin, String> {
    unsafe {
        // Load the DLL
        let library = Library::new(path)
            .map_err(|e| format!("Failed to load DLL: {}", e))?;

        // Get the get_plugin_vtable function
        let get_vtable: Symbol<GetVTable> = library.get(b"get_plugin_vtable")
            .map_err(|e| format!("Missing get_plugin_vtable export: {}", e))?;

        // Get the vtable pointer
        let vtable_ptr = get_vtable();
        if vtable_ptr.is_null() {
            return Err("get_plugin_vtable returned null".to_string());
        }

        // Copy the vtable
        let vtable = *vtable_ptr;

        // Get plugin info (name, version, schema)
        let name = c_str_to_string((vtable.name)());
        let version = c_str_to_string((vtable.version)());

        let schema_ptr = (vtable.get_config_schema)();
        let schema_json = c_str_to_string(schema_ptr);
        let config_schema: serde_json::Value = serde_json::from_str(&schema_json)
            .unwrap_or_else(|_| serde_json::json!({}));

        // Initialize the plugin
        let data = (vtable.init)();
        if data.is_null() {
            return Err("Plugin init returned null".to_string());
        }

        Ok(LoadedPlugin {
            library,
            vtable,
            data,
            name,
            version,
            config_schema,
            config: serde_json::json!({}),
            enabled: false,
            last_error: None,
        })
    }
}

/// Scan directory for plugin DLLs
pub fn scan_plugins_dir(dir: &Path) -> Vec<PathBuf> {
    let mut plugins = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("dll") {
                plugins.push(path);
            }
        }
    }

    plugins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_empty_dir() {
        let temp = tempfile::tempdir().unwrap();
        let plugins = scan_plugins_dir(temp.path());
        assert_eq!(plugins.len(), 0);
    }

    #[test]
    fn test_scan_with_dll() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("test.dll"), b"fake dll").unwrap();
        std::fs::write(temp.path().join("test.txt"), b"not a dll").unwrap();

        let plugins = scan_plugins_dir(temp.path());
        assert_eq!(plugins.len(), 1);
        assert!(plugins[0].ends_with("test.dll"));
    }
}
