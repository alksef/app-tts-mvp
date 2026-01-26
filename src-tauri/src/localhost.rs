// src-tauri/src/localhost.rs
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Структура файла localhost-tts-server.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalhostFile {
    pub config: LocalhostConfig,
    pub voices: Vec<LocalhostVoice>,
    #[serde(rename = "voices_last_updated")]
    pub voices_last_updated: Option<String>,
}

/// Структура для хранения настроек Localhost
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalhostConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(default)]
    pub connected: bool,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
}

fn default_timeout() -> u64 {
    60
}

impl Default for LocalhostConfig {
    fn default() -> Self {
        Self {
            port: None,
            token: None,
            voice: None,
            connected: false,
            timeout: 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalhostVoice {
    pub code: String,
    pub name: String,
}

/// Запрос к /speech
#[derive(Debug, Serialize)]
struct SpeechRequest {
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    voice: Option<String>,
}

/// Ответ ошибки от API
#[derive(Debug, Deserialize)]
struct ApiError {
    error: String,
}

pub struct LocalhostClient {
    data: LocalhostFile,
    file_path: PathBuf,
}

impl LocalhostClient {
    pub fn new(config_dir: PathBuf) -> Result<Self, String> {
        let file_path = config_dir.join("localhost-tts-server.json");

        // Загружаем или создаем файл
        let data = if file_path.exists() {
            Self::load_file(&file_path)?
        } else {
            // Создаем новый файл с настройками по умолчанию
            let new_data = LocalhostFile {
                config: LocalhostConfig::default(),
                voices: Vec::new(),
                voices_last_updated: None,
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
    pub fn new_for_request(config: LocalhostConfig) -> Self {
        Self {
            data: LocalhostFile {
                config,
                voices: Vec::new(),
                voices_last_updated: None,
            },
            file_path: PathBuf::new(), // Dummy path, won't be used
        }
    }

    fn load_file(path: &PathBuf) -> Result<LocalhostFile, String> {
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

    pub fn get_voices(&self) -> Vec<LocalhostVoice> {
        self.data.voices.clone()
    }

    pub fn update_voices(&mut self, voices: Vec<LocalhostVoice>) {
        self.data.voices = voices;
        self.data.voices_last_updated = Some(chrono::Utc::now().to_rfc3339());
        let _ = self.save_file();
    }

    pub fn clear_voices(&mut self) {
        self.data.voices.clear();
        self.data.voices_last_updated = None;
        let _ = self.save_file();
    }

    /// Получить URL сервера
    fn get_server_url(&self) -> Result<String, String> {
        let port = self.data.config.port.as_ref()
            .ok_or_else(|| "Port not set".to_string())?;
        Ok(format!("http://localhost:{}", port))
    }

    /// Проверить соединение с сервером (OPTIONS /speech)
    pub async fn test_connection(&self) -> Result<bool, String> {
        let server_url = self.get_server_url()?;
        let url = format!("{}/speech", server_url);

        eprintln!("[Localhost] Testing connection to {}", url);
        eprintln!("[Localhost] Method: OPTIONS");
        eprintln!("[Localhost] Has token: {}", self.data.config.token.is_some());

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.data.config.timeout))
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;

        let mut request = client.request(reqwest::Method::OPTIONS, &url);

        // Добавляем заголовок Authorization если токен задан
        if let Some(token) = &self.data.config.token {
            if !token.is_empty() {
                eprintln!("[Localhost] Adding Authorization header");
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        match request.send().await {
            Ok(response) => {
                let status = response.status();
                eprintln!("[Localhost] Response status: {}", status);

                // Логируем тело ответа при ошибке
                if !status.is_success() && status.as_u16() != 204 {
                    if let Ok(body) = response.text().await {
                        eprintln!("[Localhost] Error response body: {}", body);
                    }
                }

                if status.is_success() || status.as_u16() == 204 {
                    eprintln!("[Localhost] Connection successful");
                    Ok(true)
                } else if status.as_u16() == 401 {
                    eprintln!("[Localhost] Unauthorized: invalid token");
                    Err("Invalid authorization token".to_string())
                } else if status.as_u16() == 404 {
                    eprintln!("[Localhost] Not found: endpoint does not exist");
                    Err("Endpoint not found. Check if server is running.".to_string())
                } else {
                    eprintln!("[Localhost] Unexpected status: {}", status);
                    Err(format!("Unexpected status code: {}", status))
                }
            }
            Err(e) => {
                if e.is_connect() {
                    eprintln!("[Localhost] Connection failed: {}", e);
                    Err(format!("Failed to connect to server: {}", e))
                } else {
                    eprintln!("[Localhost] Request error: {}", e);
                    Err(format!("Request failed: {}", e))
                }
            }
        }
    }

    /// Загрузить голоса с сервера
    pub async fn fetch_voices(&self) -> Result<Vec<LocalhostVoice>, String> {
        let server_url = self.get_server_url()?;
        let url = format!("{}/voices", server_url);

        eprintln!("[Localhost] Fetching voices from {}", url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.data.config.timeout))
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;

        let mut request = client.get(&url);

        // Добавляем заголовок Authorization если токен задан
        if let Some(token) = &self.data.config.token {
            if !token.is_empty() {
                eprintln!("[Localhost] Adding Authorization header");
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        let response = request.send().await
            .map_err(|e| {
                eprintln!("[Localhost] Failed to send request: {}", e);
                format!("Failed to send request: {}", e)
            })?;

        let status = response.status();
        eprintln!("[Localhost] Response status: {}", status);

        if !response.status().is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            eprintln!("[Localhost] Error response: {}", error_text);
            return Err(format!("Server error ({}): {}", status, error_text));
        }

        let voices = response.json::<Vec<LocalhostVoice>>().await
            .map_err(|e| {
                eprintln!("[Localhost] Failed to parse response: {}", e);
                format!("Failed to parse response: {}", e)
            })?;

        eprintln!("[Localhost] Successfully fetched {} voices", voices.len());

        Ok(voices)
    }

    /// Синтезировать речь с помощью локального сервера
    pub async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        if text.is_empty() {
            return Err("Text cannot be empty".to_string());
        }

        let server_url = self.get_server_url()?;
        let url = format!("{}/speech", server_url);

        eprintln!("[Localhost] Synthesizing speech for text: '{}'", text);
        eprintln!("[Localhost] URL: {}", url);
        eprintln!("[Localhost] Voice: {:?}", self.data.config.voice);
        eprintln!("[Localhost] Has token: {}", self.data.config.token.is_some());
        eprintln!("[Localhost] Timeout: {} sec", self.data.config.timeout);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.data.config.timeout))
            .build()
            .map_err(|e| format!("Failed to build client: {}", e))?;

        // Формируем запрос
        let request_body = SpeechRequest {
            input: text.to_string(),
            voice: self.data.config.voice.clone(),
        };

        eprintln!("[Localhost] Request body: input='{}', voice={:?}", request_body.input, request_body.voice);

        let mut request = client.post(&url).json(&request_body);

        // Добавляем заголовок Authorization если токен задан
        if let Some(token) = &self.data.config.token {
            if !token.is_empty() {
                eprintln!("[Localhost] Adding Authorization header");
                request = request.header("Authorization", format!("Bearer {}", token));
            }
        }

        eprintln!("[Localhost] Sending POST request...");

        // Выполняем запрос
        let response = request.send().await
            .map_err(|e| {
                eprintln!("[Localhost] Request failed: {}", e);
                if e.is_timeout() {
                    eprintln!("[Localhost] Error: Timeout exceeded");
                } else if e.is_connect() {
                    eprintln!("[Localhost] Error: Connection failed");
                }
                if e.is_timeout() {
                    format!("Не удалось выполнить запрос к локальному серверу: превышен таймаут ({} сек).", self.data.config.timeout)
                } else if e.is_connect() {
                    format!("Не удалось подключиться к локальному серверу: {}", e)
                } else {
                    format!("Failed to send request: {}", e)
                }
            })?;

        let status = response.status();
        eprintln!("[Localhost] Response status: {}", status);

        if !response.status().is_success() {
            eprintln!("[Localhost] Response indicates error, reading body...");
            let status = response.status();
            // Пытаемся распарсить ошибку
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());

            eprintln!("[Localhost] Error response body: {}", error_text);

            // Проверяем, это JSON ошибка или простой текст
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                return Err(format!("Server error ({}): {}", status, api_error.error));
            }

            return Err(format!("Server error ({}): {}", status, error_text));
        }

        // Check content type header
        let content_type = response.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        eprintln!("[Localhost] Content-Type: {}", content_type);

        if !content_type.contains("audio") && !content_type.contains("mpeg") {
            eprintln!("[Localhost] Unexpected content type! Reading response body...");
            // We might have gotten a JSON error response with 200 OK status
            let body = response.text().await
                .unwrap_or_else(|_| "Failed to read response body".to_string());
            eprintln!("[Localhost] Response body: {}", body);
            return Err(format!(
                "Unexpected content type '{}'. Response body: {}",
                content_type, body
            ));
        }

        // Получаем аудио данные
        eprintln!("[Localhost] Reading audio data from response...");
        let audio_data = response.bytes().await
            .map_err(|e| {
                eprintln!("[Localhost] Failed to read audio data: {}", e);
                format!("Failed to read response: {}", e)
            })?
            .to_vec();

        // Validate we got some data
        if audio_data.is_empty() {
            eprintln!("[Localhost] Error: Received empty audio data");
            return Err("Received empty audio data from server".to_string());
        }

        eprintln!("[Localhost] Successfully received {} bytes of audio data", audio_data.len());

        Ok(audio_data)
    }

    // Геттеры и сеттеры для настроек
    pub fn set_port(&mut self, port: String) {
        // Проверяем, изменился ли порт
        let old_port = self.data.config.port.clone();
        let new_port = if port.is_empty() { None } else { Some(port) };

        // Если порт изменился, очищаем кеш голосов
        if old_port != new_port {
            self.clear_voices();
            eprintln!("[Localhost] Port changed from {:?} to {:?}, voices cache cleared", old_port, new_port);
        }

        self.data.config.port = new_port;
        let _ = self.save_file();
    }

    pub fn set_token(&mut self, token: String) {
        self.data.config.token = if token.is_empty() { None } else { Some(token) };
        let _ = self.save_file();
    }

    pub fn set_voice(&mut self, voice: Option<String>) {
        self.data.config.voice = voice;
        let _ = self.save_file();
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.data.config.connected = connected;
        let _ = self.save_file();
    }

    pub fn get_config(&self) -> &LocalhostConfig {
        &self.data.config
    }
}
