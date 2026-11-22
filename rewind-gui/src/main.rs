#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod settings;
mod updater;

use rewind_core::{
    AgentReport, AgentRun, Alert, ApplyReport, DeletedEntry, DiffLine, FileEntry, GuardStatus,
    Label, PluginInfo, PruneReport, RewindCore, RewindPlan, SchedulerHandle, SearchHit, Status,
    TripwireConfig, Version, WatcherHandle,
};
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, State};

struct AppState {
    core: RewindCore,
    watchers: Mutex<HashMap<String, WatcherHandle>>,
    /// File or folder handed over by the right-click entry, waiting for the UI to pick it up.
    launch: Mutex<Option<String>>,
    /// Keeps the plugin scheduler running while the app is open.
    _scheduler: SchedulerHandle,
}

/// Remember a folder and make sure it is being watched.
fn ensure_watch(state: &AppState, path: &str) -> Result<String, String> {
    let key = state.core.add_watch(path).map_err(err)?;
    let mut watchers = state.watchers.lock().unwrap_or_else(|e| e.into_inner());
    if !watchers.contains_key(&key) {
        let handle = state.core.start_watcher(&key).map_err(err)?;
        watchers.insert(key.clone(), handle);
    }
    Ok(key)
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

/// Show a file in the system file manager.
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
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(err)?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let p = std::path::Path::new(&path);
        let dir = if p.is_dir() {
            p
        } else {
            p.parent().unwrap_or(p)
        };
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(err)?;
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
fn list_labels(folder: Option<String>, state: State<'_, AppState>) -> Result<Vec<Label>, String> {
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

/// Switch "start with the computer" (hidden, in the tray) on or off.
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
            cmd.args([
                "add",
                key,
                "/v",
                "Rewind",
                "/t",
                "REG_SZ",
                "/d",
                value.as_str(),
                "/f",
            ]);
        } else {
            cmd.args(["delete", key, "/v", "Rewind", "/f"]);
        }
        let status = cmd.creation_flags(NO_WINDOW).status().map_err(err)?;
        if enabled && !status.success() {
            return Err("could not write the startup entry".into());
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| rewind_core::home_dir().map(|h| h.join(".config")))
            .ok_or("cannot find your config folder")?
            .join("autostart");
        let file = dir.join("rewind.desktop");
        if enabled {
            let exe = std::env::current_exe().map_err(err)?;
            std::fs::create_dir_all(&dir).map_err(err)?;
            std::fs::write(
                &file,
                format!(
                    "[Desktop Entry]\nType=Application\nName=Rewind\nComment=Time machine for your files\nExec=\"{}\" --background\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
                    exe.display()
                ),
            )
            .map_err(err)?;
        } else if file.exists() {
            std::fs::remove_file(&file).map_err(err)?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = enabled;
    }
    Ok(())
}

// ---- v0.3: protection, agents, plugins, right-click menu ---------------------------

#[tauri::command]
fn path_is_dir(path: String) -> bool {
    std::path::Path::new(&path).is_dir()
}

#[tauri::command]
fn list_alerts(unseen_only: bool, state: State<'_, AppState>) -> Result<Vec<Alert>, String> {
    state.core.list_alerts(unseen_only, 50).map_err(err)
}

#[tauri::command]
fn ack_alerts(id: Option<i64>, state: State<'_, AppState>) -> Result<(), String> {
    state.core.ack_alerts(id).map_err(err)
}

#[tauri::command]
fn plan_alert_rewind(id: i64, state: State<'_, AppState>) -> Result<RewindPlan, String> {
    state.core.plan_alert_rewind(id).map_err(err)
}

#[tauri::command]
fn apply_alert_rewind(
    id: i64,
    remove_new: bool,
    state: State<'_, AppState>,
) -> Result<ApplyReport, String> {
    state.core.apply_alert_rewind(id, remove_new).map_err(err)
}

#[tauri::command]
fn tripwire_get(state: State<'_, AppState>) -> TripwireConfig {
    state.core.tripwire_config()
}

#[tauri::command]
fn tripwire_set(files: usize, seconds: u64, state: State<'_, AppState>) -> Result<(), String> {
    state.core.set_tripwire_config(files, seconds).map_err(err)
}

#[tauri::command]
fn agent_runs(state: State<'_, AppState>) -> Result<Vec<AgentRun>, String> {
    state.core.list_agent_runs(30).map_err(err)
}

#[tauri::command]
fn agent_start(
    folder: String,
    agent: String,
    state: State<'_, AppState>,
) -> Result<AgentRun, String> {
    let run = state
        .core
        .agent_start(&folder, &agent, "app")
        .map_err(err)?;
    ensure_watch(&state, &run.folder)?;
    Ok(run)
}

#[tauri::command]
fn agent_report(id: i64, state: State<'_, AppState>) -> Result<AgentReport, String> {
    state.core.agent_report_live(id).map_err(err)
}

#[tauri::command]
fn agent_end(id: i64, state: State<'_, AppState>) -> Result<AgentReport, String> {
    state.core.agent_end(id).map_err(err)
}

#[tauri::command]
fn agent_undo_plan(id: i64, state: State<'_, AppState>) -> Result<RewindPlan, String> {
    state.core.agent_undo_plan(id).map_err(err)
}

#[tauri::command]
fn agent_undo(
    id: i64,
    remove_new: bool,
    state: State<'_, AppState>,
) -> Result<ApplyReport, String> {
    state.core.agent_undo(id, remove_new).map_err(err)
}

#[tauri::command]
fn checkpoint_now(folder: String, state: State<'_, AppState>) -> Result<Option<Label>, String> {
    state.core.checkpoint(&folder, None, 0).map_err(err)
}

#[tauri::command]
fn list_starter_plugins(state: State<'_, AppState>) -> Vec<PluginInfo> {
    state.core.starter_plugin_infos()
}

#[tauri::command]
fn install_starter_plugin(name: String, state: State<'_, AppState>) -> Result<PluginInfo, String> {
    state.core.install_starter_plugin(&name).map_err(err)
}

/// Pick a plugin folder, show what it may do, install it only after a yes.
#[tauri::command]
async fn install_plugin_from_disk(
    state: State<'_, AppState>,
) -> Result<Option<PluginInfo>, String> {
    let core = state.core.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<PluginInfo>, String> {
        use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
        let Some(dir) = rfd::FileDialog::new()
            .set_title("Choose a plugin folder (the one with plugin.toml)")
            .pick_folder()
        else {
            return Ok(None);
        };
        let info = core.inspect_plugin(&dir).map_err(err)?;
        let text = format!(
            "{} {}\n\nThis plugin will be able to:\n - {}\n\nOnly install plugins you trust. Install and turn it on?",
            info.name,
            info.version,
            info.permissions.join("\n - ")
        );
        let yes = MessageDialog::new()
            .set_title("Install plugin")
            .set_description(text)
            .set_level(MessageLevel::Warning)
            .set_buttons(MessageButtons::YesNo)
            .show()
            == MessageDialogResult::Yes;
        if !yes {
            return Ok(None);
        }
        core.install_plugin(&dir).map(Some).map_err(err)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
fn remove_plugin(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.core.remove_plugin(&id).map_err(err)
}

#[tauri::command]
fn plugin_errors(state: State<'_, AppState>) -> Vec<String> {
    state.core.plugin_load_errors()
}

#[tauri::command]
fn run_plugin_schedules_now(state: State<'_, AppState>) {
    state.core.run_plugin_schedules(true);
}

#[tauri::command]
fn get_shell_status() -> rewind_core::ShellStatus {
    rewind_core::shell_status()
}

#[tauri::command]
fn set_shell_integration(enabled: bool) -> Result<Vec<String>, String> {
    if enabled {
        let exe = std::env::current_exe().map_err(err)?;
        rewind_core::shell_install(Some(&exe), None).map_err(err)
    } else {
        rewind_core::shell_uninstall().map_err(err)
    }
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
            "Start Rewind when you log in?\n\nIt runs quietly in the tray so every change is recorded, even when the window is closed. You can change this later.",
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
        yes(
            "Update available",
            &msg,
            MessageLevel::Info,
            MessageButtons::YesNo,
        )
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

    // a tripwire alert gets a native popup, even when the window is hidden in the tray
    core.set_alert_handler(|a: &Alert| {
        if a.kind != "tripwire" {
            return;
        }
        let (title, msg) = (a.title.clone(), a.message.clone());
        std::thread::spawn(move || {
            rfd::MessageDialog::new()
                .set_title(format!("Rewind alert: {title}"))
                .set_description(format!(
                    "{msg}\n\nOpen Rewind > Tools > Protection to review and undo it."
                ))
                .set_level(rfd::MessageLevel::Warning)
                .set_buttons(rfd::MessageButtons::Ok)
                .show();
        });
    });
    let scheduler = core.start_scheduler();

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
            .position(|a| a == "--rewind-file" || a == "--rewind-folder")
            .and_then(|i| args.get(i + 1).cloned())
    };

    tauri::Builder::default()
        // a second launch (double click, right-click entry) wakes the running copy instead
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(i) = args
                .iter()
                .position(|a| a == "--rewind-file" || a == "--rewind-folder")
            {
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
            let quit =
                MenuItem::with_id(app, "quit", "Quit (stops recording)", true, None::<&str>)?;
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
            _scheduler: scheduler,
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
            set_autostart,
            path_is_dir,
            list_alerts,
            ack_alerts,
            plan_alert_rewind,
            apply_alert_rewind,
            tripwire_get,
            tripwire_set,
            agent_runs,
            agent_start,
            agent_report,
            agent_end,
            agent_undo_plan,
            agent_undo,
            checkpoint_now,
            list_starter_plugins,
            install_starter_plugin,
            install_plugin_from_disk,
            remove_plugin,
            plugin_errors,
            run_plugin_schedules_now,
            get_shell_status,
            set_shell_integration
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rewind");
}
