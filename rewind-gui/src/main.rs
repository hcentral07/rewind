#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rewind_core::{DiffLine, FileEntry, RewindCore, Status, Version, WatcherHandle};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::State;

struct AppState {
    core: RewindCore,
    watchers: Mutex<HashMap<String, WatcherHandle>>,
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
fn get_status(state: State<'_, AppState>) -> Result<Status, String> {
    state.core.get_status().map_err(err)
}

/// Remember a folder and start watching it (initial snapshot runs in the background).
#[tauri::command]
fn watch_folder(path: String, state: State<'_, AppState>) -> Result<String, String> {
    let key = state.core.add_watch(&path).map_err(err)?;
    let mut watchers = state.watchers.lock().unwrap_or_else(|e| e.into_inner());
    if !watchers.contains_key(&key) {
        let handle = state.core.start_watcher(&key).map_err(err)?;
        watchers.insert(key.clone(), handle);
    }
    Ok(key)
}

#[tauri::command]
fn unwatch_folder(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut watchers = state.watchers.lock().unwrap_or_else(|e| e.into_inner());
    watchers.remove(&path);
    state.core.remove_watch(&path).map_err(err)
}

#[tauri::command]
fn get_history(path: String, state: State<'_, AppState>) -> Result<Vec<Version>, String> {
    state.core.get_history(&path).map_err(err)
}

#[tauri::command]
fn list_files(folder: String, state: State<'_, AppState>) -> Result<Vec<FileEntry>, String> {
    state.core.list_files(&folder).map_err(err)
}

#[tauri::command]
fn get_diff(version_id: i64, state: State<'_, AppState>) -> Result<Vec<DiffLine>, String> {
    state.core.diff_version(version_id).map_err(err)
}

#[tauri::command]
fn restore_version(version_id: i64, state: State<'_, AppState>) -> Result<Version, String> {
    state.core.restore_version(version_id).map_err(err)
}

/// Show a file in Windows Explorer.
#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path))
            .spawn()
            .map_err(err)?;
    }
    #[cfg(not(windows))]
    {
        let _ = &path;
    }
    Ok(())
}

/// Native folder picker (runs off the UI thread).
#[tauri::command]
async fn pick_folder() -> Option<String> {
    tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .set_title("Choose a folder for Rewind to watch")
            .pick_folder()
            .map(|p| p.to_string_lossy().to_string())
    })
    .await
    .ok()
    .flatten()
}

fn main() {
    let core = RewindCore::open_default().expect("failed to open the Rewind database");

    // resume every folder that was being watched last time
    let mut watchers = HashMap::new();
    for path in core.list_watches().unwrap_or_default() {
        if let Ok(handle) = core.start_watcher(&path) {
            watchers.insert(path, handle);
        }
    }

    tauri::Builder::default()
        .manage(AppState { core, watchers: Mutex::new(watchers) })
        .invoke_handler(tauri::generate_handler![
            get_status,
            watch_folder,
            unwatch_folder,
            get_history,
            list_files,
            get_diff,
            restore_version,
            reveal_path,
            pick_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rewind");
}
