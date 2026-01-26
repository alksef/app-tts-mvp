//! Plugin API for app-tts dynamic plugins
//!
//! This crate defines the C ABI interface for loading plugins as dynamic libraries (.dll)

use std::ffi::{c_char, c_void};

/// Status of plugin connection/operation
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStatus {
    /// Plugin is working correctly
    Ok = 0,
    /// Plugin is not configured (missing required config)
    NotConfigured = 1,
    /// Connection to service failed
    ConnectionFailed = 2,
    /// Authentication failed (invalid credentials)
    AuthFailed = 3,
    /// Unknown error occurred
    UnknownError = 4,
}

impl PluginStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Ok,
            1 => Self::NotConfigured,
            2 => Self::ConnectionFailed,
            3 => Self::AuthFailed,
            _ => Self::UnknownError,
        }
    }
}

/// Function table exported by plugin DLL
///
/// All functions use C ABI (extern "C") and C-compatible types
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PluginVTable {
    /// Plugin display name
    /// Returns null-terminated C string
    pub name: extern "C" fn() -> *const c_char,

    /// Plugin version string
    /// Returns null-terminated C string (e.g. "1.0.0")
    pub version: extern "C" fn() -> *const c_char,

    /// Get JSON schema of configuration
    /// Returns null-terminated C string with JSON Schema
    pub get_config_schema: extern "C" fn() -> *const c_char,

    /// Set plugin configuration
    /// Returns 0 on success, non-zero on error
    pub set_config: extern "C" fn(
        plugin_data: *mut c_void,
        config: *const c_char,
        len: usize,
    ) -> i32,

    /// Check current plugin status
    pub check_status: extern "C" fn(plugin_data: *mut c_void) -> PluginStatus,

    /// Handle text (e.g., send to chat)
    /// Returns 0 on success, non-zero on error
    pub on_text: extern "C" fn(
        plugin_data: *mut c_void,
        text: *const c_char,
        len: usize,
    ) -> i32,

    /// Initialize plugin
    /// Returns opaque pointer to plugin data
    pub init: extern "C" fn() -> *mut c_void,

    /// Cleanup and free plugin data
    pub destroy: extern "C" fn(*mut c_void),
}

/// Helper to convert C string to Rust String
pub unsafe fn c_str_to_string(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let len = std::ffi::CStr::from_ptr(ptr)
        .to_bytes()
        .len();
    let slice = std::slice::from_raw_parts(ptr as *const u8, len);
    String::from_utf8_lossy(slice).to_string()
}

/// Helper to convert Rust String to C string (leaked)
pub fn string_to_c_str(s: &str) -> *const c_char {
    std::ffi::CString::new(s)
        .expect("Invalid string (contains null byte)")
        .into_raw()
}

/// Macro to create null-terminated static string for C
#[macro_export]
macro_rules! c_str {
    ($s:expr) => {
        concat!($s, "\0") as *const str as *const ::std::ffi::c_char
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_conversion() {
        assert_eq!(PluginStatus::from_i32(0), PluginStatus::Ok);
        assert_eq!(PluginStatus::from_i32(1), PluginStatus::NotConfigured);
        assert_eq!(PluginStatus::from_i32(2), PluginStatus::ConnectionFailed);
        assert_eq!(PluginStatus::from_i32(3), PluginStatus::AuthFailed);
        assert_eq!(PluginStatus::from_i32(99), PluginStatus::UnknownError);
    }

    #[test]
    fn test_vtable_size() {
        // Ensure VTable has expected size for C compatibility
        // Changed from u32 to *const c_char, so size increased by 4 bytes (64 -> 68 on 64-bit)
        assert_eq!(std::mem::size_of::<PluginVTable>(), 68);
    }
}
