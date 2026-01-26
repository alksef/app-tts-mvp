// src-tauri/src/openai.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Структура файла openai.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFile {
    pub config: OpenAIConfig,
    pub voices: Vec<OpenAIVoice>,
    #[serde(rename = "voices_last_updated")]
    pub voices_last_updated: Option<String>,
}

/// Структура для хранения настроек OpenAI
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OpenAIConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy_port: Option<u16>,
    pub model: String,
    pub voice: String,
    pub speed: f32,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub instructions: String,
}

fn default_timeout() -> u64 {
    20
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            proxy_host: Some("localhost".to_string()),
            proxy_port: None,
            model: "gpt-4o-mini-tts".to_string(),
            voice: "alloy".to_string(),
            speed: 1.0,
            timeout: 20,
            instructions: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIVoice {
    pub id: String,
    pub name: String,
}

/// Запрос к OpenAI TTS API
#[derive(Debug, Serialize)]
struct OpenAITtsRequest {
    model: String,
    voice: String,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    speed: Option<f32>,
}

pub struct OpenAIClient {
    data: OpenAIFile,
    file_path: PathBuf,
}

impl OpenAIClient {
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let file_path = config_dir.join("openai.json");

        // Загружаем или создаем файл
        let data = if file_path.exists() {
            Self::load_file(&file_path)?
        } else {
            // Создаем новый файл с настройками по умолчанию
            let new_data = OpenAIFile {
                config: OpenAIConfig::default(),
                voices: Self::get_static_voices(),
                voices_last_updated: Some(chrono::Utc::now().to_rfc3339()),
            };
            // Сохраняем
            let content = serde_json::to_string_pretty(&new_data)
                .map_err(|e| format!("Failed to serialize: {}", e))?;
            fs::write(&file_path, content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
            new_data
        };

        Ok(Self { data, file_path })
    }

    /// Create a temporary client for a single request (doesn't save to file)
    pub fn new_for_request(config: OpenAIConfig) -> Self {
        Self {
            data: OpenAIFile {
                config,
                voices: Self::get_static_voices(),
                voices_last_updated: None,
            },
            file_path: PathBuf::new(), // Dummy path, won't be used
        }
    }

    fn load_file(path: &PathBuf) -> Result<OpenAIFile, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse file: {}", e))
    }

    fn save_file(&self) -> Result<(), String> {
        // Skip saving if this is a temporary client (no file path)
        if self.file_path.as_os_str().is_empty() {
            return Ok(());
        }

        let content = serde_json::to_string_pretty(&self.data)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        fs::write(&self.file_path, content)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(())
    }

    pub fn get_voices(&self) -> Vec<OpenAIVoice> {
        self.data.voices.clone()
    }

    #[allow(dead_code)]
    pub fn update_voices(&mut self, voices: Vec<OpenAIVoice>) {
        self.data.voices = voices;
        self.data.voices_last_updated = Some(chrono::Utc::now().to_rfc3339());
        let _ = self.save_file();
    }

    pub fn get_static_voices() -> Vec<OpenAIVoice> {
        vec![
            OpenAIVoice { id: "alloy".to_string(), name: "Alloy".to_string() },
            OpenAIVoice { id: "ash".to_string(), name: "Ash".to_string() },
            OpenAIVoice { id: "ballad".to_string(), name: "Ballad".to_string() },
            OpenAIVoice { id: "coral".to_string(), name: "Coral".to_string() },
            OpenAIVoice { id: "echo".to_string(), name: "Echo".to_string() },
            OpenAIVoice { id: "fable".to_string(), name: "Fable".to_string() },
            OpenAIVoice { id: "nova".to_string(), name: "Nova".to_string() },
            OpenAIVoice { id: "onyx".to_string(), name: "Onyx".to_string() },
            OpenAIVoice { id: "sage".to_string(), name: "Sage".to_string() },
            OpenAIVoice { id: "shimmer".to_string(), name: "Shimmer".to_string() },
            OpenAIVoice { id: "verse".to_string(), name: "Verse".to_string() },
            OpenAIVoice { id: "marin".to_string(), name: "Marin".to_string() },
            OpenAIVoice { id: "cedar".to_string(), name: "Cedar".to_string() },
        ]
    }

    /// Синтезировать речь с помощью OpenAI API
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        if text.is_empty() {
            return Err("Text cannot be empty".to_string());
        }

        let api_key = self.data.config.api_key.as_ref()
            .ok_or_else(|| "OpenAI API key not set".to_string())?;

        // Создаем HTTP клиент с прокси если настроен
        let timeout_duration = std::time::Duration::from_secs(self.data.config.timeout);
        let client = if let (Some(host), Some(port)) = (&self.data.config.proxy_host, self.data.config.proxy_port) {
            let proxy_url = format!("http://{}:{}", host, port);
            let proxy = reqwest::Proxy::all(&proxy_url)
                .map_err(|e| format!("Failed to create proxy: {}", e))?;
            reqwest::Client::builder()
                .proxy(proxy)
                .timeout(timeout_duration)
                .build()
                .map_err(|e| format!("Failed to build client with proxy: {}", e))?
        } else {
            reqwest::Client::builder()
                .timeout(timeout_duration)
                .build()
                .map_err(|e| format!("Failed to build client: {}", e))?
        };

        // Формируем запрос
        let request = OpenAITtsRequest {
            model: self.data.config.model.clone(),
            voice: self.data.config.voice.clone(),
            input: text.to_string(),
            instructions: if self.data.config.instructions.is_empty() {
                None
            } else {
                Some(self.data.config.instructions.clone())
            },
            response_format: Some("mp3".to_string()),
            speed: if (self.data.config.speed - 1.0).abs() < 0.001 {
                None
            } else {
                Some(self.data.config.speed)
            },
        };

        // Выполняем запрос
        let response = client
            .post("https://api.openai.com/v1/audio/speech")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    format!("Не удалось выполнить запрос к OpenAI: превышен таймаут ({} сек). Проверьте подключение к интернету или настройки прокси.", self.data.config.timeout)
                } else if e.is_connect() {
                    format!("Не удалось подключиться к OpenAI: {}", e)
                } else {
                    format!("Failed to send request: {}", e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("OpenAI API error ({}): {}", status, error_text));
        }

        // Check content type header to ensure we got audio
        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        if !content_type.contains("audio") && !content_type.contains("octet-stream") {
            // We might have gotten a JSON error response with 200 OK status
            let body = response.text().await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            return Err(format!(
                "Unexpected content type '{}'. Response body: {}",
                content_type, body
            ));
        }

        // Получаем аудио данные
        let audio_data = response.bytes().await
            .map_err(|e| format!("Failed to read response: {}", e))?
            .to_vec();

        // Validate we got some data
        if audio_data.is_empty() {
            return Err("Received empty audio data from OpenAI API".to_string());
        }

        eprintln!("[OpenAI] Received {} bytes of audio data, content-type: {}", audio_data.len(), content_type);

        Ok(audio_data)
    }

    // Геттеры и сеттеры для настроек
    pub fn set_api_key(&mut self, key: String) {
        self.data.config.api_key = if key.is_empty() { None } else { Some(key) };
        let _ = self.save_file();
    }

    pub fn set_proxy(&mut self, host: Option<String>, port: Option<u16>) {
        self.data.config.proxy_host = host;
        self.data.config.proxy_port = port;
        let _ = self.save_file();
    }

    pub fn set_voice(&mut self, voice: String) {
        self.data.config.voice = voice;
        let _ = self.save_file();
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.data.config.speed = speed.clamp(0.25, 4.0);
        let _ = self.save_file();
    }

    #[allow(dead_code)]
    pub fn set_timeout(&mut self, timeout: u64) {
        self.data.config.timeout = timeout.max(1);
        let _ = self.save_file();
    }

    pub fn set_instructions(&mut self, instructions: String) {
        self.data.config.instructions = instructions;
        let _ = self.save_file();
    }

    pub fn get_config(&self) -> &OpenAIConfig {
        &self.data.config
    }
}
