#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod updater;

use rewind_core::{
    ApplyReport, DeletedEntry, DiffLine, FileEntry, GuardStatus, Label, PluginInfo, PruneReport,
    RewindCore, RewindPlan, SearchHit, Status, Version, WatcherHandle,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, State};

struct AppState {
    core: RewindCore,
    watchers: Mutex<HashMap<String, WatcherHandle>>,
    /// File handed over by the Explorer right-click entry, waiting for the UI to pick it up.
    launch: Mutex<Option<String>>,
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

#[tauri::command]
fn app_version() -> String {
    updater::CURRENT.to_string()
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> settings::Settings {
    settings::load(state.core.data_dir())
}

#[tauri::command]
fn save_settings(new: settings::Settings, state: State<'_, AppState>) {
    settings::save(state.core.data_dir(), &new);
}

#[tauri::command]
async fn check_update() -> Result<Option<updater::UpdateInfo>, String> {
    tauri::async_runtime::spawn_blocking(updater::check)
        .await
        .map_err(err)?
}

#[tauri::command]
async fn install_update(info: updater::UpdateInfo) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || updater::download_and_run(&info))
        .await
        .map_err(err)??;
    std::process::exit(0);
}

/// File passed by the Explorer right-click entry (`--rewind-file "<path>"`). Returns it once.
#[tauri::command]
fn take_launch_file(state: State<'_, AppState>) -> Option<String> {
    state
        .launch
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .take()
}

// ---- trust and engine features (each one is a plugin in rewind-core) ----------------

#[tauri::command]
fn list_deleted(
    folder: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<DeletedEntry>, String> {
    state.core.list_deleted(folder.as_deref()).map_err(err)
}

#[tauri::command]
fn restore_deleted(path: String, state: State<'_, AppState>) -> Result<Version, String> {
    state.core.restore_deleted(&path).map_err(err)
}

#[tauri::command]
fn search_history(
    query: String,
    folder: Option<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchHit>, String> {
    state
        .core
        .search_history(&query, folder.as_deref(), limit.unwrap_or(100))
        .map_err(err)
}

#[tauri::command]
fn diff_versions(
    old_id: i64,
    new_id: i64,
    state: State<'_, AppState>,
) -> Result<Vec<DiffLine>, String> {
    state.core.diff_versions(old_id, new_id).map_err(err)
}

#[tauri::command]
fn pin_version(version_id: i64, pinned: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.core.pin_version(version_id, pinned).map_err(err)
}

#[tauri::command]
fn add_label(folder: String, name: String, state: State<'_, AppState>) -> Result<Label, String> {
    state.core.add_label(&folder, &name).map_err(err)
}

#[tauri::command]
fn list_labels(
    folder: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Label>, String> {
    state.core.list_labels(folder.as_deref()).map_err(err)
}

#[tauri::command]
fn delete_label(id: i64, state: State<'_, AppState>) -> Result<(), String> {
    state.core.delete_label(id).map_err(err)
}

#[tauri::command]
fn plan_rewind(
    folder: String,
    to: String,
    state: State<'_, AppState>,
) -> Result<RewindPlan, String> {
    state.core.plan_folder_rewind(&folder, &to).map_err(err)
}

#[tauri::command]
fn apply_rewind(
    folder: String,
    to: String,
    remove_new: bool,
    state: State<'_, AppState>,
) -> Result<ApplyReport, String> {
    state
        .core
        .apply_folder_rewind(&folder, &to, remove_new)
        .map_err(err)
}

#[tauri::command]
fn list_plugins(state: State<'_, AppState>) -> Vec<PluginInfo> {
    state.core.list_plugins()
}

#[tauri::command]
fn set_plugin_enabled(id: String, enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    state.core.set_plugin_enabled(&id, enabled).map_err(err)
}

/// Plugin settings live under keys starting with `cfg.`.
#[tauri::command]
fn cfg_get(key: String, state: State<'_, AppState>) -> Option<String> {
    if key.starts_with("cfg.") {
        state.core.kv_get(&key)
    } else {
        None
    }
}

#[tauri::command]
fn cfg_set(key: String, value: String, state: State<'_, AppState>) -> Result<(), String> {
    if !key.starts_with("cfg.") {
        return Err("not a plugin setting".into());
    }
    state.core.kv_set(&key, &value).map_err(err)
}

#[tauri::command]
fn guard_status(state: State<'_, AppState>) -> GuardStatus {
    state.core.guard_status()
}

#[tauri::command]
fn guard_prune_now(state: State<'_, AppState>) -> Result<PruneReport, String> {
    let limit = state.core.guard_status().limit_bytes;
    state.core.shrink_store(limit).map_err(err)
}

/// Switch "start with Windows" (hidden, in the tray) on or off.
#[tauri::command]
fn set_autostart(enabled: bool, state: State<'_, AppState>) -> Result<(), String> {
    set_autostart_registry(enabled)?;
    let dir = state.core.data_dir();
    let mut s = settings::load(dir);
    s.autostart = enabled;
    s.asked_autostart = true;
    settings::save(dir, &s);
    Ok(())
}

fn set_autostart_registry(enabled: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
        let mut cmd = std::process::Command::new("reg");
        if enabled {
            let exe = std::env::current_exe().map_err(err)?;
            let value = format!("\"{}\" --background", exe.display());
            cmd.args(["add", key, "/v", "Rewind", "/t", "REG_SZ", "/d", value.as_str(), "/f"]);
        } else {
            cmd.args(["delete", key, "/v", "Rewind", "/f"]);
        }
        let status = cmd.creation_flags(NO_WINDOW).status().map_err(err)?;
        if enabled && !status.success() {
            return Err("could not write the startup entry".into());
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
    }
    Ok(())
}

fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Runs in the background at startup: first-run question, usage ping, update check.
fn startup_tasks(dir: std::path::PathBuf) {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    let yes = |title: &str, text: &str, level: MessageLevel, buttons: MessageButtons| {
        MessageDialog::new()
            .set_title(title)
            .set_description(text)
            .set_level(level)
            .set_buttons(buttons)
            .show()
            == MessageDialogResult::Yes
    };

    let mut s = settings::load(&dir);
    if !s.asked_usage {
        s.share_usage = yes(
            "Rewind",
            "Help count how many people use Rewind?\n\nIf you say yes, Rewind sends one anonymous ping a day: a random ID, the app version and your OS. No file names, no paths, nothing about your files. You can change this later.",
            MessageLevel::Info,
            MessageButtons::YesNo,
        );
        s.asked_usage = true;
        settings::save(&dir, &s);
    }
    if !s.asked_autostart {
        s.autostart = yes(
            "Rewind",
            "Start Rewind with Windows?\n\nIt runs quietly in the tray so every change is recorded, even when the window is closed. You can change this later.",
            MessageLevel::Info,
            MessageButtons::YesNo,
        );
        s.asked_autostart = true;
        if s.autostart {
            let _ = set_autostart_registry(true);
        }
        settings::save(&dir, &s);
    }
    if s.share_usage {
        updater::ping(&mut s);
        settings::save(&dir, &s);
    }

    let Ok(Some(info)) = updater::check() else {
        return;
    };

    let go = if s.auto_update {
        true
    } else {
        let msg = format!(
            "Rewind {} is available (you have {}).\n\nPlease update the software to keep using it.\n\nUpdate now?",
            info.version,
            updater::CURRENT
        );
        yes("Update available", &msg, MessageLevel::Info, MessageButtons::YesNo)
    };

    if go {
        match updater::download_and_run(&info) {
            Ok(()) => std::process::exit(0),
            Err(e) => {
                MessageDialog::new()
                    .set_title("Update failed")
                    .set_description(format!("{e}\n\nDownload it manually: {}", info.page))
                    .set_level(MessageLevel::Error)
                    .set_buttons(MessageButtons::Ok)
                    .show();
            }
        }
    }
    if info.required {
        std::process::exit(0);
    }
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

    let data_dir = core.data_dir().to_path_buf();

    let background = std::env::args().any(|a| a == "--background");
    let launch = {
        let args: Vec<String> = std::env::args().collect();
        args.iter()
            .position(|a| a == "--rewind-file")
            .and_then(|i| args.get(i + 1).cloned())
    };

    tauri::Builder::default()
        // a second launch (double click, right-click entry) wakes the running copy instead
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(i) = args.iter().position(|a| a == "--rewind-file") {
                if let Some(p) = args.get(i + 1) {
                    *app.state::<AppState>()
                        .launch
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()) = Some(p.clone());
                }
            }
            show_main(app);
        }))
        .setup(move |app| {
            let show = MenuItem::with_id(app, "show", "Open Rewind", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit (stops recording)", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Rewind is recording changes")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;

            if !background {
                show_main(app.handle());
            }
            std::thread::spawn(move || startup_tasks(data_dir));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let dir = window.state::<AppState>().core.data_dir().to_path_buf();
                if settings::load(&dir).close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .manage(AppState {
            core,
            watchers: Mutex::new(watchers),
            launch: Mutex::new(launch),
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            watch_folder,
            unwatch_folder,
            get_history,
            list_files,
            get_diff,
            restore_version,
            reveal_path,
            pick_folder,
            app_version,
            get_settings,
            save_settings,
            check_update,
            install_update,
            take_launch_file,
            list_deleted,
            restore_deleted,
            search_history,
            diff_versions,
            pin_version,
            add_label,
            list_labels,
            delete_label,
            plan_rewind,
            apply_rewind,
            list_plugins,
            set_plugin_enabled,
            cfg_get,
            cfg_set,
            guard_status,
            guard_prune_now,
            set_autostart
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rewind");
}
