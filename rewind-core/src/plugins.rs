//! Tiny plugin layer. Every feature beyond the bare snapshot engine is a `Plugin`:
//! it has an id, can be switched on/off (stored in the database) and may react to events.
//! The built-in plugins below are the template for future external plugins.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Things the engine announces. Plugins receive them in `on_event`.
#[derive(Debug, Clone)]
pub enum RewindEvent {
    VersionSaved(Version),
    FileDeleted { path: String },
    FileRenamed { from: String, to: String },
    FolderRewound { folder: String, restored: i64 },
}

pub trait Plugin: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn default_enabled(&self) -> bool {
        true
    }
    fn on_event(&self, _core: &RewindCore, _event: &RewindEvent) {}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub builtin: bool,
}

struct FeatureModule {
    id: &'static str,
    name: &'static str,
    description: &'static str,
}

impl Plugin for FeatureModule {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        self.description
    }
}

struct DiskGuard {
    saves: AtomicU64,
}

impl Plugin for DiskGuard {
    fn id(&self) -> &'static str {
        "disk-guard"
    }
    fn name(&self) -> &'static str {
        "Disk space guard"
    }
    fn description(&self) -> &'static str {
        "Warns when the history store passes a size limit, and can prune old versions automatically. Pinned versions are never removed."
    }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        if !matches!(event, RewindEvent::VersionSaved(_)) {
            return;
        }
        if self.saves.fetch_add(1, Ordering::Relaxed) % 25 == 0 {
            core.guard_check();
        }
    }
}

// A1: deleted-notice ring buffer --------------------------------------------------

struct DeletedNotice;

const NOTICE_KEY: &str = "plugin.deleted-notice.ring";
const NOTICE_MAX: usize = 20;

#[derive(Serialize, Deserialize)]
struct Notice {
    kind: String,
    path: String,
    new_path: Option<String>,
    at: String,
}

fn read_notices(core: &RewindCore) -> Vec<Notice> {
    core.kv_get(NOTICE_KEY)
        .and_then(|s| serde_json::from_str::<Vec<Notice>>(&s).ok())
        .unwrap_or_default()
}

fn push_notice(core: &RewindCore, n: Notice) {
    let mut ring = read_notices(core);
    ring.push(n);
    if ring.len() > NOTICE_MAX {
        let drop = ring.len() - NOTICE_MAX;
        ring.drain(0..drop);
    }
    if let Ok(s) = serde_json::to_string(&ring) {
        let _ = core.kv_set(NOTICE_KEY, &s);
    }
}

impl Plugin for DeletedNotice {
    fn id(&self) -> &'static str {
        "deleted-notice"
    }
    fn name(&self) -> &'static str {
        "Deleted / renamed notices"
    }
    fn description(&self) -> &'static str {
        "Queues a small toast-ready notice when a file is deleted or renamed."
    }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        let now = Utc::now().to_rfc3339();
        match event {
            RewindEvent::FileDeleted { path } => push_notice(
                core,
                Notice {
                    kind: "deleted".into(),
                    path: path.clone(),
                    new_path: None,
                    at: now,
                },
            ),
            RewindEvent::FileRenamed { from, to } => push_notice(
                core,
                Notice {
                    kind: "renamed".into(),
                    path: from.clone(),
                    new_path: Some(to.clone()),
                    at: now,
                },
            ),
            _ => {}
        }
    }
}

// A2: large-file-warn -------------------------------------------------------------

struct LargeFileWarn;

const LARGE_FILE_KEY: &str = "plugin.large-file-warn.count";
const LARGE_FILE_THRESHOLD_BYTES: i64 = 10 * 1024 * 1024;

impl Plugin for LargeFileWarn {
    fn id(&self) -> &'static str {
        "large-file-warn"
    }
    fn name(&self) -> &'static str {
        "Large file warnings"
    }
    fn description(&self) -> &'static str {
        "Counts snapshots above 10 MB. The engine hard-skips files over 25 MB."
    }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        if let RewindEvent::VersionSaved(v) = event {
            if v.size >= LARGE_FILE_THRESHOLD_BYTES {
                let n: i64 = core
                    .kv_get(LARGE_FILE_KEY)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                let _ = core.kv_set(LARGE_FILE_KEY, &(n + 1).to_string());
            }
        }
    }
}

// A3: snapshot-on-idle ------------------------------------------------------------

struct SnapshotOnIdle {
    saves: AtomicU64,
    last_fire: Mutex<Option<std::time::Instant>>,
}

const IDLE_SAVES: u64 = 25;
const IDLE_WINDOW: std::time::Duration = std::time::Duration::from_secs(300);

impl Plugin for SnapshotOnIdle {
    fn id(&self) -> &'static str {
        "snapshot-on-idle"
    }
    fn name(&self) -> &'static str {
        "Idle snapshots"
    }
    fn description(&self) -> &'static str {
        "After 25 saves within 5 minutes, snapshots the newest watched folder and saves an auto-label."
    }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        if !matches!(event, RewindEvent::VersionSaved(_)) {
            return;
        }
        let n = self.saves.fetch_add(1, Ordering::Relaxed) + 1;
        if n % IDLE_SAVES != 0 {
            return;
        }
        {
            let mut last = self.last_fire.lock().unwrap_or_else(|e| e.into_inner());
            let now = std::time::Instant::now();
            if let Some(prev) = *last {
                if now.duration_since(prev) < IDLE_WINDOW {
                    return;
                }
            }
            *last = Some(now);
        }
        let Ok(folders) = core.list_watches() else {
            return;
        };
        let Some(folder) = folders.first() else {
            return;
        };
        let _ = core.snapshot_folder(folder);
        let _ = core.add_label(folder, &format!("auto {}", Utc::now().to_rfc3339()));
    }
}

impl RewindCore {
    pub(crate) fn register_builtin_plugins(&self) {
        let mut list = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        let modules: [(&'static str, &'static str, &'static str); 5] = [
            ("deleted-files", "Deleted files",
             "A recycle bin for everything: every file that vanished from a watched folder, with one-click bring-back."),
            ("folder-rewind", "Folder rewind",
             "Put a whole folder back to how it was at a chosen time, or to a named label."),
            ("pins-labels", "Pins and labels",
             "Pin a version so prune never deletes it, and name moments like \"before mod install\"."),
            ("history-search", "History search",
             "Find the version of any file that contained a word."),
            ("compare", "Compare versions",
             "Diff any two versions of a file, not just a version against the one before it."),
        ];
        for (id, name, description) in modules {
            list.push(Arc::new(FeatureModule {
                id,
                name,
                description,
            }));
        }
        list.push(Arc::new(DiskGuard {
            saves: AtomicU64::new(0),
        }));
        list.push(Arc::new(DeletedNotice));
        list.push(Arc::new(LargeFileWarn));
        list.push(Arc::new(SnapshotOnIdle {
            saves: AtomicU64::new(0),
            last_fire: Mutex::new(None),
        }));
    }

    pub fn register_plugin(&self, plugin: Arc<dyn Plugin>) {
        self.plugins
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(plugin);
    }

    fn plugin_list(&self) -> Vec<Arc<dyn Plugin>> {
        self.plugins
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn plugin_is_enabled(&self, p: &dyn Plugin) -> bool {
        match self.kv_get(&format!("plugin.{}.enabled", p.id())) {
            Some(v) => v == "1",
            None => p.default_enabled(),
        }
    }

    pub fn list_plugins(&self) -> Vec<PluginInfo> {
        self.plugin_list()
            .iter()
            .map(|p| PluginInfo {
                id: p.id().to_string(),
                name: p.name().to_string(),
                description: p.description().to_string(),
                enabled: self.plugin_is_enabled(p.as_ref()),
                builtin: true,
            })
            .collect()
    }

    pub fn set_plugin_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        if !self.plugin_list().iter().any(|p| p.id() == id) {
            return Err(anyhow!("no plugin called {id}"));
        }
        self.kv_set(
            &format!("plugin.{id}.enabled"),
            if enabled { "1" } else { "0" },
        )
    }

    pub fn emit(&self, event: &RewindEvent) {
        for p in self.plugin_list() {
            if self.plugin_is_enabled(p.as_ref()) {
                p.on_event(self, event);
            }
        }
    }

    pub fn list_notices(&self) -> serde_json::Value {
        serde_json::to_value(read_notices(self)).unwrap_or(serde_json::Value::Array(vec![]))
    }

    pub fn clear_notices(&self) -> Result<()> {
        self.kv_set(NOTICE_KEY, "[]")
    }

    pub fn large_file_warnings(&self) -> i64 {
        self.kv_get(LARGE_FILE_KEY)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }
}
