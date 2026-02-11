use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;

const DEFAULT_INBOX_FOLDER_NAME: &str = "Inmobiliaria Inbox";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server_url: String,
    pub inbox_path: String,
    pub delete_after_upload: bool,
    pub auto_start: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_path = dirs_default_inbox();
        Self {
            server_url: String::new(),
            inbox_path: default_path,
            delete_after_upload: true,
            auto_start: true,
        }
    }
}

fn dirs_default_inbox() -> String {
    let home = dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(DEFAULT_INBOX_FOLDER_NAME)
        .to_string_lossy()
        .to_string()
}

/// Persistent configuration manager using a JSON file in the app data directory
pub struct ConfigManager {
    config: Mutex<AppConfig>,
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new(app_data_dir: PathBuf) -> Self {
        let config_path = app_data_dir.join("config.json");
        let config = if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
                Err(_) => AppConfig::default(),
            }
        } else {
            AppConfig::default()
        };

        Self {
            config: Mutex::new(config),
            config_path,
        }
    }

    pub fn get(&self) -> AppConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn save(&self, new_config: AppConfig) -> Result<(), String> {
        // Ensure parent directory exists
        if let Some(parent) = self.config_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(&new_config).map_err(|e| e.to_string())?;
        std::fs::write(&self.config_path, json).map_err(|e| e.to_string())?;
        *self.config.lock().unwrap() = new_config;
        Ok(())
    }

    pub fn update_server_url(&self, url: &str) -> Result<(), String> {
        let mut config = self.get();
        config.server_url = url.to_string();
        self.save(config)
    }

    pub fn ensure_inbox_folder(&self) -> Result<PathBuf, String> {
        let config = self.get();
        let path = PathBuf::from(&config.inbox_path);
        if !path.exists() {
            std::fs::create_dir_all(&path).map_err(|e| {
                format!("Failed to create inbox folder '{}': {}", config.inbox_path, e)
            })?;
        }
        Ok(path)
    }
}

/// Helper to get the "Subidos" subfolder path
pub fn uploaded_subfolder(inbox_path: &str) -> PathBuf {
    PathBuf::from(inbox_path).join("Subidos")
}

/// Directories helper — uses the `dirs` crate functionality via std
mod dirs {
    use std::path::PathBuf;

    pub fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }

    pub fn document_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            home_dir().map(|h| h.join("Documents"))
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Documents"))
        }
        #[cfg(target_os = "linux")]
        {
            home_dir().map(|h| h.join("Documents"))
        }
    }
}
