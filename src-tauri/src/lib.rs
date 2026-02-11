mod auth;
mod commands;
mod config;
mod tray;
mod uploader;
mod watcher;

use commands::AppState;
use config::ConfigManager;
use std::sync::Arc;
use tauri::Manager;
use uploader::UploadManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // If user tries to open another instance, show settings window
            if let Some(window) = app.get_webview_window("settings") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostarted"]),
        ))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            commands::login,
            commands::logout,
            commands::check_auth,
            commands::get_config,
            commands::save_config,
            commands::get_status,
            commands::open_inbox_folder,
        ])
        .setup(|app| {
            let app_handle = app.handle().clone();

            // Initialize config manager
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data dir");
            let config_manager = ConfigManager::new(app_data_dir);

            // Initialize upload manager
            let upload_manager = Arc::new(UploadManager::new());

            // Store app state
            app.manage(AppState {
                config_manager,
                upload_manager: upload_manager.clone(),
            });

            // Create the tray icon
            let tray_icon =
                tray::create_tray(&app_handle).expect("Failed to create tray icon");

            // Handle tray menu events
            let app_handle_menu = app_handle.clone();
            tray_icon.on_menu_event(move |app, event| {
                match event.id().as_ref() {
                    "open_folder" => {
                        let state = app.state::<AppState>();
                        let config = state.config_manager.get();
                        let _ = open::that(&config.inbox_path);
                    }
                    "open_web" => {
                        let state = app.state::<AppState>();
                        let config = state.config_manager.get();
                        if !config.server_url.is_empty() {
                            let _ = open::that(&config.server_url);
                        }
                    }
                    "settings" => {
                        show_settings_window(&app_handle_menu);
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                }
            });

            // Check if authenticated and start services
            let app_handle_setup = app_handle.clone();
            let upload_manager_setup = upload_manager.clone();
            tauri::async_runtime::spawn(async move {
                let state = app_handle_setup.state::<AppState>();
                let config = state.config_manager.get();

                // Check if we have stored credentials
                let has_auth = if !config.server_url.is_empty() {
                    auth::check_auth(&config.server_url).await.is_ok()
                } else {
                    false
                };

                if has_auth {
                    log::info!("Authenticated, starting services...");
                    start_services(&app_handle_setup, upload_manager_setup).await;
                } else {
                    log::info!("Not authenticated, showing settings window...");
                    show_settings_window(&app_handle_setup);
                }
            });

            // Periodic tray update
            let app_handle_tray = app_handle.clone();
            let upload_manager_tray = upload_manager.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if let Some(tray) = app_handle_tray.tray_by_id("main-tray") {
                        let _ = tray::update_tray(
                            &app_handle_tray,
                            &tray,
                            &upload_manager_tray,
                        );
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn show_settings_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.set_focus();
    } else {
        let _ = tauri::WebviewWindowBuilder::new(
            app,
            "settings",
            tauri::WebviewUrl::App("index.html".into()),
        )
        .title("Inmobiliaria Inbox — Configuración")
        .inner_size(440.0, 560.0)
        .resizable(false)
        .center()
        .build();
    }
}

async fn start_services(app: &tauri::AppHandle, upload_manager: Arc<UploadManager>) {
    let state = app.state::<AppState>();
    let config = state.config_manager.get();

    // Ensure inbox folder exists
    match state.config_manager.ensure_inbox_folder() {
        Ok(inbox_path) => {
            log::info!("Inbox folder ready: {:?}", inbox_path);

            // Scan existing files first
            let existing = watcher::scan_existing_files(&inbox_path);
            for file in existing {
                upload_manager.enqueue(file);
            }

            // Start file watcher
            let upload_manager_watcher = upload_manager.clone();
            let inbox_path_watcher = inbox_path.clone();
            std::thread::spawn(move || {
                match watcher::start_watching(&inbox_path_watcher) {
                    Ok((rx, _debouncer)) => {
                        log::info!("File watcher started successfully");
                        // Keep receiving file events
                        while let Ok(path) = rx.recv() {
                            upload_manager_watcher.enqueue(path);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to start file watcher: {}", e);
                    }
                }
            });

            // Start upload worker
            let server_url = config.server_url.clone();
            let delete_after = config.delete_after_upload;
            let inbox_str = config.inbox_path.clone();
            let upload_manager_worker = upload_manager.clone();

            // Send notification for successful uploads
            let app_handle = app.clone();
            let upload_manager_notif = upload_manager.clone();
            tauri::async_runtime::spawn(async move {
                upload_manager_worker
                    .start_worker(server_url, delete_after, inbox_str)
                    .await;
            });

            // Notification watcher: check for new successful uploads periodically
            tauri::async_runtime::spawn(async move {
                let mut last_success_count = 0usize;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    let recent = upload_manager_notif.get_recent();
                    let current_success = recent
                        .iter()
                        .filter(|r| r.status == uploader::UploadStatus::Success)
                        .count();

                    if current_success > last_success_count {
                        let new_count = current_success - last_success_count;
                        let body = if new_count == 1 {
                            let name = recent
                                .iter()
                                .find(|r| r.status == uploader::UploadStatus::Success)
                                .map(|r| r.name.clone())
                                .unwrap_or_default();
                            format!("{} subido exitosamente", name)
                        } else {
                            format!("{} archivos subidos exitosamente", new_count)
                        };

                        if let Ok(true) =
                            tauri_plugin_notification::NotificationExt::notification(
                                &app_handle,
                            )
                            .permission_state()
                            .map(|s| s == tauri_plugin_notification::PermissionState::Granted)
                        {
                            let _ = tauri_plugin_notification::NotificationExt::notification(
                                &app_handle,
                            )
                            .builder()
                            .title("Inmobiliaria Inbox")
                            .body(&body)
                            .show();
                        }
                    }
                    last_success_count = current_success;
                }
            });
        }
        Err(e) => {
            log::error!("Failed to create inbox folder: {}", e);
        }
    }
}
