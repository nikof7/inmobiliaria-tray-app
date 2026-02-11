use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Files to ignore — system files, temp files, hidden files
const IGNORED_EXACT: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".localized",
    "Icon\r",
];

const IGNORED_PREFIXES: &[&str] = &["~$", "._"];

const IGNORED_SUFFIXES: &[&str] = &[".tmp", ".swp", ".crdownload", ".part", ".partial"];

/// Name of the "uploaded" subfolder (to ignore)
const UPLOADED_FOLDER: &str = "Subidos";

/// Check if a file should be ignored
fn should_ignore(path: &Path) -> bool {
    let file_name = match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return true,
    };

    // Hidden files (start with .)
    if file_name.starts_with('.') {
        return true;
    }

    // Exact matches
    if IGNORED_EXACT.contains(&file_name) {
        return true;
    }

    // Prefix matches
    if IGNORED_PREFIXES.iter().any(|p| file_name.starts_with(p)) {
        return true;
    }

    // Suffix matches
    if IGNORED_SUFFIXES.iter().any(|s| file_name.ends_with(s)) {
        return true;
    }

    // Ignore files inside the "Subidos" subfolder
    if path
        .components()
        .any(|c| c.as_os_str() == UPLOADED_FOLDER)
    {
        return true;
    }

    // Ignore directories
    if path.is_dir() {
        return true;
    }

    false
}

/// Check if a file is ready (not being written to) by checking size stability
fn is_file_ready(path: &Path) -> bool {
    let first_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };

    std::thread::sleep(Duration::from_millis(500));

    let second_size = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(_) => return false,
    };

    first_size == second_size && first_size > 0
}

/// Scan existing files in the inbox folder (for files that arrived while offline)
pub fn scan_existing_files(inbox_path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(inbox_path) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !should_ignore(&path) && path.is_file() {
                files.push(path);
            }
        }
    }
    files
}

/// Start watching the inbox folder for new/changed files.
/// Returns a channel receiver that emits file paths when new files are detected.
/// Also returns the watcher handle (must be kept alive).
pub fn start_watching(
    inbox_path: &Path,
) -> Result<
    (
        mpsc::Receiver<PathBuf>,
        notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    ),
    String,
> {
    let (tx, rx) = mpsc::channel::<PathBuf>();

    let tx_clone = tx.clone();
    let inbox_path_owned = inbox_path.to_path_buf();

    let mut debouncer = new_debouncer(Duration::from_secs(2), move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
        match result {
            Ok(events) => {
                for event in events {
                    if event.kind == DebouncedEventKind::Any {
                        let path = event.path;
                        // Only process files directly in the inbox (not subdirectories' contents will be filtered by should_ignore)
                        if !should_ignore(&path) && path.is_file() {
                            // Check if file is inside the watched inbox directory (not a subdirectory situation)
                            if let Some(parent) = path.parent() {
                                if parent == inbox_path_owned {
                                    if is_file_ready(&path) {
                                        log::info!("New file detected: {:?}", path);
                                        let _ = tx_clone.send(path);
                                    } else {
                                        log::debug!("File not ready yet: {:?}", path);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Watcher error: {:?}", e);
            }
        }
    })
    .map_err(|e| format!("Failed to create file watcher: {}", e))?;

    debouncer
        .watcher()
        .watch(inbox_path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch folder: {}", e))?;

    log::info!("Watching folder: {:?}", inbox_path);

    Ok((rx, debouncer))
}
