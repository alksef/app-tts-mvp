//! Plugin system for dynamic loading of extensions

pub mod config;
pub mod dynamic;
pub mod manager;
pub mod types;

pub use manager::PluginManager;
pub use types::{PluginInfo, SerializablePluginStatus};
