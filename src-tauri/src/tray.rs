use crate::uploader::{UploadManager, UploadStatus};
use std::sync::Arc;
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle,
};

#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum TrayState {
    Connected,
    Syncing(usize),
    Offline,
    Pending(usize),
    NotAuthenticated,
}

/// Create the initial tray icon with menu
pub fn create_tray(app: &AppHandle) -> Result<TrayIcon, String> {
    let tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("Inmobiliaria Inbox")
        .icon(load_tray_icon(app, "tray-default"))
        .menu(&build_menu(app, &TrayState::Connected, &[])?)
        .show_menu_on_left_click(true)
        .build(app)
        .map_err(|e| format!("Failed to create tray: {}", e))?;

    Ok(tray)
}

/// Update the tray icon and menu based on current state
pub fn update_tray(
    app: &AppHandle,
    tray: &TrayIcon,
    upload_manager: &Arc<UploadManager>,
) -> Result<(), String> {
    let state = determine_state(upload_manager);
    let recent = upload_manager.get_recent();

    // Update icon based on state
    let icon_name = match &state {
        TrayState::Connected => "tray-default",
        TrayState::Syncing(_) => "tray-syncing",
        TrayState::Offline => "tray-offline",
        TrayState::Pending(_) => "tray-default",
        TrayState::NotAuthenticated => "tray-offline",
    };

    let _ = tray.set_icon(Some(load_tray_icon(app, icon_name)));

    // Update tooltip
    let tooltip = match &state {
        TrayState::Connected => "Inmobiliaria Inbox — Conectado".to_string(),
        TrayState::Syncing(n) => format!("Inmobiliaria Inbox — Subiendo {} archivo(s)...", n),
        TrayState::Offline => "Inmobiliaria Inbox — Sin conexión".to_string(),
        TrayState::Pending(n) => format!("Inmobiliaria Inbox — {} pendiente(s)", n),
        TrayState::NotAuthenticated => "Inmobiliaria Inbox — No autenticado".to_string(),
    };
    let _ = tray.set_tooltip(Some(&tooltip));

    // Update menu
    if let Ok(menu) = build_menu(app, &state, &recent) {
        let _ = tray.set_menu(Some(menu));
    }

    Ok(())
}

fn determine_state(upload_manager: &Arc<UploadManager>) -> TrayState {
    if !upload_manager.is_online() {
        return TrayState::Offline;
    }
    if upload_manager.is_uploading() {
        return TrayState::Syncing(upload_manager.queue_size() + 1);
    }
    let queue_size = upload_manager.queue_size();
    if queue_size > 0 {
        return TrayState::Pending(queue_size);
    }
    TrayState::Connected
}

fn build_menu(
    app: &AppHandle,
    state: &TrayState,
    recent: &[crate::uploader::RecentUpload],
) -> Result<tauri::menu::Menu<tauri::Wry>, String> {
    let open_folder = MenuItemBuilder::with_id("open_folder", "Abrir carpeta Inbox")
        .build(app)
        .map_err(|e| e.to_string())?;

    let open_web = MenuItemBuilder::with_id("open_web", "Abrir Inmobiliaria Web")
        .build(app)
        .map_err(|e| e.to_string())?;

    let status_text = match state {
        TrayState::Connected => "✓ Conectado",
        TrayState::Syncing(n) => &format!("↑ Subiendo {} archivo(s)...", n),
        TrayState::Offline => "✕ Sin conexión",
        TrayState::Pending(n) => &format!("● {} pendiente(s) de subida", n),
        TrayState::NotAuthenticated => "⚠ No autenticado",
    };

    // Status needs to be owned for lifetimes
    let status_label = status_text.to_string();
    let status_item = MenuItemBuilder::with_id("status", &status_label)
        .enabled(false)
        .build(app)
        .map_err(|e| e.to_string())?;

    let settings = MenuItemBuilder::with_id("settings", "Configuración...")
        .build(app)
        .map_err(|e| e.to_string())?;

    let quit = MenuItemBuilder::with_id("quit", "Salir")
        .build(app)
        .map_err(|e| e.to_string())?;

    // Build recent files submenu
    let mut recent_sub = SubmenuBuilder::with_id(app, "recent", "Archivos recientes");

    if recent.is_empty() {
        let no_files = MenuItemBuilder::with_id("no_recent", "Sin archivos recientes")
            .enabled(false)
            .build(app)
            .map_err(|e| e.to_string())?;
        recent_sub = recent_sub.item(&no_files);
    } else {
        for (i, upload) in recent.iter().take(10).enumerate() {
            let icon = match upload.status {
                UploadStatus::Success => "✓",
                UploadStatus::Failed => "✕",
                UploadStatus::Pending => "●",
                UploadStatus::Uploading => "↑",
            };
            let label = format!("{} {} ({})", icon, truncate_name(&upload.name, 30), upload.timestamp);
            let item = MenuItemBuilder::with_id(format!("recent_{}", i), &label)
                .enabled(false)
                .build(app)
                .map_err(|e| e.to_string())?;
            recent_sub = recent_sub.item(&item);
        }
    }

    let recent_submenu = recent_sub.build().map_err(|e| e.to_string())?;

    let menu = MenuBuilder::new(app)
        .item(&open_folder)
        .item(&open_web)
        .separator()
        .item(&status_item)
        .separator()
        .item(&recent_submenu)
        .separator()
        .item(&settings)
        .item(&quit)
        .build()
        .map_err(|e| e.to_string())?;

    Ok(menu)
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        let ext = std::path::Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let stem_max = max_len.saturating_sub(ext.len() + 4); // "..." + "."
        let stem = &name[..stem_max.min(name.len())];
        if ext.is_empty() {
            format!("{}...", stem)
        } else {
            format!("{}...{}", stem, ext)
        }
    }
}

fn load_tray_icon(_app: &AppHandle, name: &str) -> Image<'static> {
    let bytes: &[u8] = match name {
        "tray-syncing" => include_bytes!("../icons/tray-syncing.png"),
        "tray-offline" => include_bytes!("../icons/tray-offline.png"),
        "tray-error" => include_bytes!("../icons/tray-error.png"),
        _ => include_bytes!("../icons/tray-default.png"),
    };
    // Decode PNG to raw RGBA
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().expect("Failed to read PNG info");
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).expect("Failed to decode PNG");
    buf.truncate(info.buffer_size());
    Image::new_owned(buf, info.width, info.height)
}
