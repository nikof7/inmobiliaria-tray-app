use crate::auth::{self, AuthData};
use crate::config::{AppConfig, ConfigManager};
use crate::uploader::{RecentUpload, UploadManager};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// Application state accessible from commands
pub struct AppState {
    pub config_manager: ConfigManager,
    pub upload_manager: Arc<UploadManager>,
}

#[derive(Debug, Serialize)]
pub struct StatusInfo {
    pub authenticated: bool,
    pub email: Option<String>,
    pub online: bool,
    pub uploading: bool,
    pub queue_size: usize,
    pub recent: Vec<RecentUpload>,
    pub config: AppConfig,
}

#[tauri::command]
pub async fn login(
    email: String,
    password: String,
    server_url: String,
    state: State<'_, AppState>,
) -> Result<AuthData, String> {
    // Save the server URL to config first
    state.config_manager.update_server_url(&server_url)?;

    // Authenticate
    let auth_data = auth::login(&server_url, &email, &password).await?;

    // Ensure inbox folder exists
    state.config_manager.ensure_inbox_folder()?;

    Ok(auth_data)
}

#[tauri::command]
pub async fn logout() -> Result<(), String> {
    auth::logout()
}

#[tauri::command]
pub async fn check_auth(state: State<'_, AppState>) -> Result<AuthData, String> {
    let config = state.config_manager.get();
    if config.server_url.is_empty() {
        return Err("No server configured".to_string());
    }
    auth::check_auth(&config.server_url).await
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config_manager.get())
}

#[tauri::command]
pub async fn save_config(config: AppConfig, state: State<'_, AppState>) -> Result<(), String> {
    state.config_manager.save(config)?;
    Ok(())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<StatusInfo, String> {
    let config = state.config_manager.get();
    let credentials = auth::get_stored_credentials();

    Ok(StatusInfo {
        authenticated: credentials.is_ok(),
        email: credentials.ok().map(|c| c.email),
        online: state.upload_manager.is_online(),
        uploading: state.upload_manager.is_uploading(),
        queue_size: state.upload_manager.queue_size(),
        recent: state.upload_manager.get_recent(),
        config,
    })
}

#[tauri::command]
pub async fn open_inbox_folder(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config_manager.get();
    open::that(&config.inbox_path).map_err(|e| format!("Failed to open folder: {}", e))
}
