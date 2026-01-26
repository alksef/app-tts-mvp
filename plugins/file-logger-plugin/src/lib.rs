//! File Logger Plugin - логирует полученные тексты в файл
//!
//! Пример плагина для app-tts, который записывает все полученные тексты
//! в указанный файл с временными метками.

use plugins_api::{PluginStatus, PluginVTable};
use std::ffi::{c_char, c_void};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::slice;

/// Состояние плагина
struct FileLoggerState {
    /// Путь к файлу лога
    file_path: PathBuf,
    /// Базовая директория (для относительных путей)
    base_dir: PathBuf,
    /// Последняя ошибка
    last_error: String,
    /// Настроен ли плагин
    configured: bool,
}

/// Глобальное состояние плагина
static mut STATE: Option<FileLoggerState> = None;

/// Разделитель между записями
const SEPARATOR: &str = "\n------\n";

/// JSON схема конфигурации
const CONFIG_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "title": "File Path",
      "description": "Path to log file (relative to exe or absolute)"
    }
  },
  "required": ["file_path"]
}"#;

/// Имя плагина
extern "C" fn plugin_name() -> *const c_char {
    b"File Logger\0".as_ptr() as *const c_char
}

/// Версия плагина
extern "C" fn plugin_version() -> *const c_char {
    b"1.0.0\0".as_ptr() as *const c_char
}

/// Получить схему конфигурации
extern "C" fn plugin_get_config_schema() -> *const c_char {
    CONFIG_SCHEMA.as_ptr() as *const c_char
}

/// Инициализация плагина
extern "C" fn plugin_init() -> *mut c_void {
    unsafe {
        // Получаем директорию exe
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        STATE = Some(FileLoggerState {
            file_path: PathBuf::new(),
            base_dir: exe_dir,
            last_error: String::new(),
            configured: false,
        });

        STATE.as_mut().unwrap() as *mut _ as *mut c_void
    }
}

/// Установить конфигурацию
extern "C" fn plugin_set_config(
    plugin_data: *mut c_void,
    config: *const c_char,
    len: usize,
) -> i32 {
    unsafe {
        let state = &mut *(plugin_data as *mut FileLoggerState);

        // Конвертируем C строку в Rust
        let slice = slice::from_raw_parts(config as *const u8, len);
        let config_str = String::from_utf8_lossy(slice);

        eprintln!("[FileLogger] set_config called: {}", config_str);

        // Парсим JSON
        let config_value: serde_json::Value = match serde_json::from_str(&config_str) {
            Ok(v) => v,
            Err(e) => {
                state.last_error = format!("Invalid JSON: {}", e);
                eprintln!("[FileLogger] JSON parse error: {}", e);
                return -1;
            }
        };

        // Получаем file_path
        let file_path = match config_value.get("file_path") {
            Some(v) if v.is_string() => {
                let s = v.as_str().unwrap();
                eprintln!("[FileLogger] file_path: {}", s);
                s
            },
            other => {
                state.last_error = format!("file_path is required, got: {:?}", other);
                eprintln!("[FileLogger] file_path missing or invalid");
                return -1;
            }
        };

        // Формируем полный путь
        let full_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            state.base_dir.join(file_path)
        };

        eprintln!("[FileLogger] full path: {:?}", full_path);

        // Проверяем что можем создать/открыть файл
        if let Some(parent) = full_path.parent() {
            eprintln!("[FileLogger] creating dir: {:?}", parent);
            if let Err(e) = std::fs::create_dir_all(parent) {
                state.last_error = format!("Failed to create directory: {}", e);
                eprintln!("[FileLogger] dir creation failed: {}", e);
                return -1;
            }
        }

        state.file_path = full_path;
        state.configured = true;
        state.last_error.clear();

        eprintln!("[FileLogger] config OK");
        0 // OK
    }
}

/// Проверить статус плагина
extern "C" fn plugin_check_status(plugin_data: *mut c_void) -> PluginStatus {
    unsafe {
        let state = &*(plugin_data as *mut FileLoggerState);

        if !state.configured {
            return PluginStatus::NotConfigured;
        }

        // Проверяем что можем писать в файл
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&state.file_path)
        {
            Ok(_) => PluginStatus::Ok,
            Err(e) => {
                // Записываем ошибку в состояние (нужен mutable access)
                let state = &mut *(plugin_data as *mut FileLoggerState);
                state.last_error = format!("Cannot open file: {}", e);
                PluginStatus::ConnectionFailed
            }
        }
    }
}

/// Обработать текст
extern "C" fn plugin_on_text(
    plugin_data: *mut c_void,
    text: *const c_char,
    len: usize,
) -> i32 {
    unsafe {
        let state = &mut *(plugin_data as *mut FileLoggerState);

        if !state.configured {
            state.last_error = "Plugin not configured".to_string();
            return -1;
        }

        // Конвертируем C строку в Rust
        let slice = slice::from_raw_parts(text as *const u8, len);
        let text_str = String::from_utf8_lossy(slice);

        // Формируем запись с timestamp
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_entry = format!("[{}]\n{}\n", timestamp, text_str);

        // Открываем файл и дописываем
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&state.file_path)
        {
            Ok(mut file) => {
                // Если файл не пуст, добавляем разделитель
                let metadata = file.metadata();
                let need_separator = metadata
                    .map(|m| m.len() > 0)
                    .unwrap_or(false);

                if let Err(e) = file.write_all(log_entry.as_bytes()) {
                    state.last_error = format!("Failed to write: {}", e);
                    return -1;
                }

                if need_separator {
                    if let Err(e) = file.write_all(SEPARATOR.as_bytes()) {
                        state.last_error = format!("Failed to write separator: {}", e);
                        return -1;
                    }
                }

                // Сбрасываем буфер
                if let Err(e) = file.flush() {
                    state.last_error = format!("Failed to flush: {}", e);
                    return -1;
                }

                state.last_error.clear();
                0 // OK
            }
            Err(e) => {
                state.last_error = format!("Failed to open file: {}", e);
                -1
            }
        }
    }
}

/// Освободить ресурсы
extern "C" fn plugin_destroy(plugin_data: *mut c_void) {
    unsafe {
        let _state = Box::from_raw(plugin_data as *mut FileLoggerState);
    }
}

/// Vtable плагина
static VTABLE: PluginVTable = PluginVTable {
    name: plugin_name,
    version: plugin_version,
    get_config_schema: plugin_get_config_schema,
    set_config: plugin_set_config,
    check_status: plugin_check_status,
    on_text: plugin_on_text,
    init: plugin_init,
    destroy: plugin_destroy,
};

/// Экспортируемая функция для получения vtable
#[no_mangle]
pub extern "C" fn get_plugin_vtable() -> *const PluginVTable {
    &VTABLE
}
