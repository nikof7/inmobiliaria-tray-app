use crate::auth;
use crate::config::uploaded_subfolder;
use reqwest::multipart;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};

/// Maximum number of recent uploads to track
const MAX_RECENT: usize = 15;

/// Retry delay base (seconds) — uses exponential backoff
const RETRY_DELAY_BASE_SECS: u64 = 5;

/// Max retries before giving up on a single file
const MAX_RETRIES: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentUpload {
    pub name: String,
    pub status: UploadStatus,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UploadStatus {
    Success,
    Failed,
    Pending,
    Uploading,
}

#[derive(Debug, Clone)]
struct QueueItem {
    path: PathBuf,
    retries: u32,
}

/// Shared upload state
pub struct UploadManager {
    queue: Arc<Mutex<VecDeque<QueueItem>>>,
    recent: Arc<Mutex<VecDeque<RecentUpload>>>,
    is_uploading: Arc<Mutex<bool>>,
    is_online: Arc<Mutex<bool>>,
}

impl UploadManager {
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            recent: Arc::new(Mutex::new(VecDeque::new())),
            is_uploading: Arc::new(Mutex::new(false)),
            is_online: Arc::new(Mutex::new(true)),
        }
    }

    /// Add a file to the upload queue
    pub fn enqueue(&self, path: PathBuf) {
        let mut queue = self.queue.lock().unwrap();

        // Avoid duplicates
        if queue.iter().any(|item| item.path == path) {
            return;
        }

        log::info!("Enqueuing file: {:?}", path);

        // Add to recent as pending
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        self.add_recent(RecentUpload {
            name: file_name,
            status: UploadStatus::Pending,
            timestamp: chrono::Local::now().format("%H:%M:%S").to_string(),
        });

        queue.push_back(QueueItem { path, retries: 0 });
    }

    /// Get the current queue size
    pub fn queue_size(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Check if currently uploading
    pub fn is_uploading(&self) -> bool {
        *self.is_uploading.lock().unwrap()
    }

    /// Get recent uploads
    pub fn get_recent(&self) -> Vec<RecentUpload> {
        self.recent.lock().unwrap().iter().cloned().collect()
    }

    /// Check online status
    pub fn is_online(&self) -> bool {
        *self.is_online.lock().unwrap()
    }

    /// Set online status
    pub fn set_online(&self, online: bool) {
        *self.is_online.lock().unwrap() = online;
    }

    fn add_recent(&self, entry: RecentUpload) {
        let mut recent = self.recent.lock().unwrap();
        recent.push_front(entry);
        while recent.len() > MAX_RECENT {
            recent.pop_back();
        }
    }

    fn update_recent_status(&self, name: &str, status: UploadStatus) {
        let mut recent = self.recent.lock().unwrap();
        if let Some(entry) = recent.iter_mut().find(|r| r.name == name) {
            entry.status = status;
        }
    }

    /// Start the upload worker loop — runs indefinitely
    pub async fn start_worker(
        self: Arc<Self>,
        server_url: String,
        delete_after_upload: bool,
        inbox_path: String,
    ) {
        log::info!("Upload worker started");
        loop {
            // Check connectivity
            let online = check_server(&server_url).await;
            self.set_online(online);

            if !online {
                log::debug!("Server offline, waiting...");
                sleep(Duration::from_secs(15)).await;
                continue;
            }

            // Try to get next item from queue
            let item = {
                let mut queue = self.queue.lock().unwrap();
                queue.pop_front()
            };

            match item {
                Some(mut item) => {
                    let file_name = item
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();

                    *self.is_uploading.lock().unwrap() = true;
                    self.update_recent_status(&file_name, UploadStatus::Uploading);

                    match upload_file(&item.path, &server_url).await {
                        Ok(_) => {
                            log::info!("Successfully uploaded: {}", file_name);
                            self.update_recent_status(&file_name, UploadStatus::Success);

                            // Handle post-upload file cleanup
                            if delete_after_upload {
                                if let Err(e) = std::fs::remove_file(&item.path) {
                                    log::error!("Failed to delete file after upload: {}", e);
                                }
                            } else {
                                // Move to "Subidos" subfolder
                                let dest_dir = uploaded_subfolder(&inbox_path);
                                if let Err(e) = std::fs::create_dir_all(&dest_dir) {
                                    log::error!("Failed to create Subidos folder: {}", e);
                                } else {
                                    let dest = dest_dir.join(&file_name);
                                    if let Err(e) = std::fs::rename(&item.path, &dest) {
                                        log::error!("Failed to move file to Subidos: {}", e);
                                    }
                                }
                            }

                            *self.is_uploading.lock().unwrap() = false;
                        }
                        Err(e) => {
                            log::error!("Upload failed for {}: {}", file_name, e);

                            item.retries += 1;
                            *self.is_uploading.lock().unwrap() = false;

                            if item.retries < MAX_RETRIES {
                                // Re-enqueue with exponential backoff
                                self.update_recent_status(&file_name, UploadStatus::Pending);
                                self.queue.lock().unwrap().push_back(item.clone());
                                let delay =
                                    RETRY_DELAY_BASE_SECS * 2u64.pow(item.retries.min(6));
                                log::info!(
                                    "Retrying {} in {}s (attempt {}/{})",
                                    file_name,
                                    delay,
                                    item.retries,
                                    MAX_RETRIES
                                );
                                sleep(Duration::from_secs(delay)).await;
                            } else {
                                log::error!(
                                    "Giving up on {} after {} retries",
                                    file_name,
                                    MAX_RETRIES
                                );
                                self.update_recent_status(&file_name, UploadStatus::Failed);
                            }
                        }
                    }
                }
                None => {
                    // Queue is empty, wait before checking again
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
}

/// Upload a single file to PocketBase
async fn upload_file(path: &PathBuf, server_url: &str) -> Result<(), String> {
    let token = auth::get_token().ok_or("Not authenticated")?;
    let user_id = auth::get_user_id().ok_or("No user ID found")?;

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Read file bytes
    let file_bytes = tokio::fs::read(path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    // Determine MIME type
    let mime_type = mime_guess::from_path(path)
        .first_or_octet_stream()
        .to_string();

    // Build multipart form
    let file_part = multipart::Part::bytes(file_bytes)
        .file_name(file_name.clone())
        .mime_str(&mime_type)
        .map_err(|e| format!("Invalid MIME type: {}", e))?;

    let form = multipart::Form::new()
        .part("file", file_part)
        .text("name", file_name)
        .text("user", user_id)
        .text("status", "pending".to_string());

    let url = format!(
        "{}/api/collections/files_inbox/records",
        server_url.trim_end_matches('/')
    );

    let client = reqwest::Client::new();
    let response = client
        .post(&url)
        .header("Authorization", token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("Upload request failed: {}", e))?;

    if response.status().is_success() {
        Ok(())
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        Err(format!("Upload failed ({}): {}", status, body))
    }
}

/// Check if the PocketBase server is reachable
async fn check_server(server_url: &str) -> bool {
    let url = format!(
        "{}/api/health",
        server_url.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    client.get(&url).send().await.is_ok()
}
