use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

/// Global path to the credentials file, set once at app startup
static AUTH_FILE_PATH: OnceLock<PathBuf> = OnceLock::new();

const AUTH_FILE_NAME: &str = "credentials.json";

/// Initialize the auth module with the app data directory.
/// Must be called once at startup before any other auth function.
pub fn init(app_data_dir: &PathBuf) {
    let path = app_data_dir.join(AUTH_FILE_NAME);
    AUTH_FILE_PATH.set(path).ok();
}

fn auth_file() -> &'static PathBuf {
    AUTH_FILE_PATH
        .get()
        .expect("auth::init() must be called before using auth functions")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthData {
    pub token: String,
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Deserialize)]
struct PocketBaseAuthResponse {
    token: String,
    record: PocketBaseUser,
}

#[derive(Debug, Deserialize)]
struct PocketBaseUser {
    id: String,
    email: String,
}

/// Authenticate with PocketBase using email/password
pub async fn login(server_url: &str, email: &str, password: &str) -> Result<AuthData, String> {
    let client = Client::new();
    let url = format!(
        "{}/api/collections/users/auth-with-password",
        server_url.trim_end_matches('/')
    );

    let response = client
        .post(&url)
        .json(&serde_json::json!({
            "identity": email,
            "password": password
        }))
        .send()
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("Authentication failed ({}): {}", status, body));
    }

    let auth_response: PocketBaseAuthResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let auth_data = AuthData {
        token: auth_response.token,
        user_id: auth_response.record.id,
        email: auth_response.record.email,
    };

    store_credentials(&auth_data)?;

    Ok(auth_data)
}

/// Refresh the auth token
pub async fn refresh_token(server_url: &str) -> Result<AuthData, String> {
    let current = get_stored_credentials()?;
    let client = Client::new();
    let url = format!(
        "{}/api/collections/users/auth-refresh",
        server_url.trim_end_matches('/')
    );

    let response = client
        .post(&url)
        .header("Authorization", &current.token)
        .send()
        .await
        .map_err(|e| format!("Connection error: {}", e))?;

    if !response.status().is_success() {
        return Err("Token refresh failed — please log in again".to_string());
    }

    let auth_response: PocketBaseAuthResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let auth_data = AuthData {
        token: auth_response.token,
        user_id: auth_response.record.id,
        email: auth_response.record.email,
    };

    store_credentials(&auth_data)?;

    Ok(auth_data)
}

/// Check if valid credentials are stored and token is still valid
pub async fn check_auth(server_url: &str) -> Result<AuthData, String> {
    let current = get_stored_credentials()?;
    // Try to refresh to verify the token is still valid
    match refresh_token(server_url).await {
        Ok(data) => Ok(data),
        Err(_) => {
            // Token might be expired but credentials exist
            Ok(current)
        }
    }
}

/// Store credentials in a local JSON file
fn store_credentials(auth_data: &AuthData) -> Result<(), String> {
    let path = auth_file();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(auth_data).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| format!("Failed to store credentials: {}", e))?;
    Ok(())
}

/// Retrieve stored credentials from the local JSON file
pub fn get_stored_credentials() -> Result<AuthData, String> {
    let path = auth_file();
    if !path.exists() {
        return Err("No stored credentials found".to_string());
    }
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read credentials: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse credentials: {}", e))
}

/// Remove stored credentials (logout)
pub fn logout() -> Result<(), String> {
    let path = auth_file();
    if path.exists() {
        std::fs::remove_file(path).map_err(|e| format!("Failed to remove credentials: {}", e))?;
    }
    Ok(())
}

/// Get the stored token (if any) without validation
pub fn get_token() -> Option<String> {
    get_stored_credentials().ok().map(|c| c.token)
}

/// Get stored user ID
pub fn get_user_id() -> Option<String> {
    get_stored_credentials().ok().map(|c| c.user_id)
}
