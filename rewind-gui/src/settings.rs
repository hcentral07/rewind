//! Small JSON settings file stored next to the history database.
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// true = download and install updates silently on start; false = show a popup.
    pub auto_update: bool,
    /// Opt-in anonymous "I'm alive" ping (random id, version, OS). Off until the user says yes.
    pub share_usage: bool,
    /// Whether the first-run usage question has been answered.
    pub asked_usage: bool,
    /// Random id generated locally. Not linked to anything about the person.
    pub install_id: String,
    /// Start with Windows (hidden, in the tray) so history is always being recorded.
    pub autostart: bool,
    pub asked_autostart: bool,
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// Date (YYYY-MM-DD) of the last ping, so we ping at most once a day.
    pub last_ping: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_update: false,
            share_usage: false,
            asked_usage: false,
            autostart: false,
            asked_autostart: false,
            close_to_tray: true,
            install_id: uuid::Uuid::new_v4().to_string(),
            last_ping: String::new(),
        }
    }
}

pub fn load(dir: &Path) -> Settings {
    let p = dir.join("settings.json");
    let s: Settings = fs::read_to_string(&p)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    // make sure a freshly generated id is persisted
    if !p.exists() {
        save(dir, &s);
    }
    s
}

pub fn save(dir: &Path, s: &Settings) {
    let _ = fs::create_dir_all(dir);
    if let Ok(t) = serde_json::to_string_pretty(s) {
        let _ = fs::write(dir.join("settings.json"), t);
    }
}
