use keyring::Entry;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "inmobiliaria-inbox";
const KEYRING_TOKEN_KEY: &str = "auth-token";
const KEYRING_USER_ID_KEY: &str = "user-id";
const KEYRING_USER_EMAIL_KEY: &str = "user-email";

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

/// Store credentials securely in the OS keychain
fn store_credentials(auth_data: &AuthData) -> Result<(), String> {
    let token_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY).map_err(|e| e.to_string())?;
    token_entry
        .set_password(&auth_data.token)
        .map_err(|e| format!("Failed to store token: {}", e))?;

    let user_id_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_USER_ID_KEY).map_err(|e| e.to_string())?;
    user_id_entry
        .set_password(&auth_data.user_id)
        .map_err(|e| format!("Failed to store user ID: {}", e))?;

    let email_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_USER_EMAIL_KEY).map_err(|e| e.to_string())?;
    email_entry
        .set_password(&auth_data.email)
        .map_err(|e| format!("Failed to store email: {}", e))?;

    Ok(())
}

/// Retrieve stored credentials from the OS keychain
pub fn get_stored_credentials() -> Result<AuthData, String> {
    let token_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY).map_err(|e| e.to_string())?;
    let token = token_entry
        .get_password()
        .map_err(|_| "No stored token found".to_string())?;

    let user_id_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_USER_ID_KEY).map_err(|e| e.to_string())?;
    let user_id = user_id_entry
        .get_password()
        .map_err(|_| "No stored user ID found".to_string())?;

    let email_entry =
        Entry::new(KEYRING_SERVICE, KEYRING_USER_EMAIL_KEY).map_err(|e| e.to_string())?;
    let email = email_entry
        .get_password()
        .map_err(|_| "No stored email found".to_string())?;

    Ok(AuthData {
        token,
        user_id,
        email,
    })
}

/// Remove stored credentials (logout)
pub fn logout() -> Result<(), String> {
    let entries = [KEYRING_TOKEN_KEY, KEYRING_USER_ID_KEY, KEYRING_USER_EMAIL_KEY];
    for key in entries {
        if let Ok(entry) = Entry::new(KEYRING_SERVICE, key) {
            let _ = entry.delete_credential(); // Ignore errors if not found
        }
    }
    Ok(())
}

/// Get the stored token (if any) without validation
pub fn get_token() -> Option<String> {
    Entry::new(KEYRING_SERVICE, KEYRING_TOKEN_KEY)
        .ok()
        .and_then(|e| e.get_password().ok())
}

/// Get stored user ID
pub fn get_user_id() -> Option<String> {
    Entry::new(KEYRING_SERVICE, KEYRING_USER_ID_KEY)
        .ok()
        .and_then(|e| e.get_password().ok())
}
