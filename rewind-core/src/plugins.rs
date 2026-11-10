//! Tiny plugin layer. Every feature beyond the bare snapshot engine is a `Plugin`:
//! it has an id, can be switched on/off (stored in the database) and may react to events.
//! The built-in plugins below are the template for future external plugins.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Called after the engine has released its database lock, so it is safe to call
    /// back into `core` from here.
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

/// A feature that lives in the UI / core API and has no event hook of its own.
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

/// Warns (and optionally auto-prunes) when the history store grows past a limit.
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
        // checking the store size walks the database, so only do it every 25 saves
        if self.saves.fetch_add(1, Ordering::Relaxed) % 25 == 0 {
            core.guard_check();
        }
    }
}

impl RewindCore {
    pub(crate) fn register_builtin_plugins(&self) {
        let mut list = self.plugins.lock().unwrap_or_else(|e| e.into_inner());
        let modules: [(&'static str, &'static str, &'static str); 5] = [
            (
                "deleted-files",
                "Deleted files",
                "A recycle bin for everything: every file that vanished from a watched folder, with one-click bring-back.",
            ),
            (
                "folder-rewind",
                "Folder rewind",
                "Put a whole folder back to how it was at a chosen time, or to a named label.",
            ),
            (
                "pins-labels",
                "Pins and labels",
                "Pin a version so prune never deletes it, and name moments like \"before mod install\".",
            ),
            (
                "history-search",
                "History search",
                "Find the version of any file that contained a word.",
            ),
            (
                "compare",
                "Compare versions",
                "Diff any two versions of a file, not just a version against the one before it.",
            ),
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
    }

    /// Register an additional plugin (used by future plugin loaders).
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

    /// Deliver an event to every enabled plugin.
    pub fn emit(&self, event: &RewindEvent) {
        for p in self.plugin_list() {
            if self.plugin_is_enabled(p.as_ref()) {
                p.on_event(self, event);
            }
        }
    }
}
