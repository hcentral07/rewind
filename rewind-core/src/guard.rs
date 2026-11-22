//! Protection features: alerts, the mass-change tripwire and AI-agent runs.
//!
//! * **Alerts** are small records the UI, CLI and plugins can show or react to.
//! * **Tripwire** watches the stream of changes. Many files changing in a few seconds, or files
//!   whose content suddenly turns into random-looking bytes, raises an alert and pins the last
//!   good version of every affected file so that no prune can remove it.
//! * **Agent runs** wrap an AI coding agent: checkpoint first, report what it touched, undo it all.
use super::*;
use chrono::{DateTime, Local};
use std::collections::{HashMap, VecDeque};
use std::time::Instant;

pub type AlertHandler = Arc<dyn Fn(&Alert) + Send + Sync>;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub id: i64,
    /// "tripwire", "agent", "plugin" or "disk"
    pub kind: String,
    pub title: String,
    pub message: String,
    pub folder: String,
    pub created_at: String,
    /// Tripwire alerts: a moment just before the burst began. Rewind the folder to it to undo.
    pub window_start: Option<String>,
    pub count: i64,
    pub seen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TripwireConfig {
    /// This many modified / renamed / deleted files inside the window trips the wire.
    pub files: usize,
    pub seconds: u64,
    /// This many files that turned into random-looking bytes trips it much sooner.
    pub suspicious: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRun {
    pub id: i64,
    pub folder: String,
    pub agent: String,
    pub command: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    /// "running", "ended" or "undone"
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChange {
    pub path: String,
    /// "created", "modified" or "deleted"
    pub action: String,
    pub versions: i64,
    pub lines_added: i64,
    pub lines_deleted: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentReport {
    pub run: AgentRun,
    pub changes: Vec<AgentChange>,
    pub created: i64,
    pub modified: i64,
    pub deleted: i64,
    pub lines_added: i64,
    pub lines_deleted: i64,
    /// Things worth a second look: mass deletes, touched secrets, files gutted.
    pub flags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn entropy(bytes: &[u8]) -> f64 {
    if bytes.is_empty() {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for &b in bytes {
        counts[b as usize] += 1;
    }
    let n = bytes.len() as f64;
    -counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / n;
            p * p.log2()
        })
        .sum::<f64>()
}

fn ext_of(p: &str) -> String {
    Path::new(p)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn looks_secret(path: &str) -> bool {
    let name = Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name == ".env"
        || name.starts_with(".env.")
        || name.ends_with(".pem")
        || name.ends_with(".key")
        || name.ends_with(".kdbx")
        || name.starts_with("id_rsa")
        || name.starts_with("id_ed25519")
        || name.starts_with("credentials")
        || name.starts_with("secrets")
        || name == ".npmrc"
        || name == ".pypirc"
        || name == ".netrc"
}

// ---------------------------------------------------------------------------
// Alerts
// ---------------------------------------------------------------------------

const ALERT_COLS: &str = "id, kind, title, message, folder, created_at, window_start, count, seen";

fn row_to_alert(r: &rusqlite::Row) -> rusqlite::Result<Alert> {
    Ok(Alert {
        id: r.get(0)?,
        kind: r.get(1)?,
        title: r.get(2)?,
        message: r.get(3)?,
        folder: r.get(4)?,
        created_at: r.get(5)?,
        window_start: r.get(6)?,
        count: r.get(7)?,
        seen: r.get::<_, i64>(8)? != 0,
    })
}

impl RewindCore {
    /// Register a function that is called for every new alert (the GUI shows a dialog,
    /// the CLI prints a line).
    pub fn set_alert_handler(&self, f: impl Fn(&Alert) + Send + Sync + 'static) {
        *self.alert_cb.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
    }

    pub fn raise_alert(
        &self,
        kind: &str,
        title: &str,
        message: &str,
        folder: &str,
        window_start: Option<String>,
        count: i64,
    ) -> Result<Alert> {
        let now = Utc::now().to_rfc3339();
        let id = {
            let conn = self.db();
            conn.execute(
                "INSERT INTO alerts (kind, title, message, folder, created_at, window_start, count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![kind, title, message, folder, now, window_start, count],
            )?;
            conn.last_insert_rowid()
        };
        let alert = self.get_alert(id)?;
        let cb = self
            .alert_cb
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(cb) = cb {
            cb(&alert);
        }
        self.emit(&RewindEvent::Alert(alert.clone()));
        Ok(alert)
    }

    pub fn get_alert(&self, id: i64) -> Result<Alert> {
        self.db()
            .query_row(
                &format!("SELECT {ALERT_COLS} FROM alerts WHERE id = ?1"),
                params![id],
                row_to_alert,
            )
            .optional()?
            .ok_or_else(|| anyhow!("alert {id} not found"))
    }

    /// Newest first.
    pub fn list_alerts(&self, unseen_only: bool, limit: usize) -> Result<Vec<Alert>> {
        let conn = self.db();
        let sql = format!(
            "SELECT {ALERT_COLS} FROM alerts {} ORDER BY id DESC LIMIT ?1",
            if unseen_only { "WHERE seen = 0" } else { "" }
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![limit as i64], row_to_alert)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Mark one alert (or all of them when `id` is `None`) as seen.
    pub fn ack_alerts(&self, id: Option<i64>) -> Result<()> {
        match id {
            Some(id) => self
                .db()
                .execute("UPDATE alerts SET seen = 1 WHERE id = ?1", params![id])?,
            None => self.db().execute("UPDATE alerts SET seen = 1", [])?,
        };
        Ok(())
    }

    /// What rewinding the folder of a tripwire alert to just before the burst would change.
    pub fn plan_alert_rewind(&self, id: i64) -> Result<RewindPlan> {
        let a = self.get_alert(id)?;
        let when = a
            .window_start
            .ok_or_else(|| anyhow!("this alert has no moment to rewind to"))?;
        self.plan_folder_rewind(&a.folder, &when)
    }

    /// Undo a tripwire burst: put the folder back to just before it began.
    pub fn apply_alert_rewind(&self, id: i64, remove_new: bool) -> Result<ApplyReport> {
        let a = self.get_alert(id)?;
        let when = a
            .window_start
            .ok_or_else(|| anyhow!("this alert has no moment to rewind to"))?;
        let rep = self.apply_folder_rewind(&a.folder, &when, remove_new)?;
        self.ack_alerts(Some(id))?;
        Ok(rep)
    }

    fn update_alert_count(&self, id: i64, count: i64) {
        let _ = self.db().execute(
            "UPDATE alerts SET count = ?2 WHERE id = ?1",
            params![id, count],
        );
    }

    /// The watched folder that contains `path` (the deepest one), or its parent folder.
    pub(crate) fn watch_root_for(&self, path: &str) -> String {
        let mut best: Option<String> = None;
        if let Ok(rows) = self.watch_rows() {
            for (w, _) in rows {
                if in_folder(path, &w) && best.as_ref().map(|b| w.len() > b.len()).unwrap_or(true) {
                    best = Some(w);
                }
            }
        }
        best.unwrap_or_else(|| {
            Path::new(path)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        })
    }

    // -- tripwire settings ---------------------------------------------------

    pub fn tripwire_config(&self) -> TripwireConfig {
        let num = |k: &str, d: u64| -> u64 {
            self.kv_get(k)
                .and_then(|v| v.trim().parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(d)
        };
        let files = num("cfg.tripwire.files", 50) as usize;
        let seconds = num("cfg.tripwire.seconds", 10);
        TripwireConfig {
            files,
            seconds,
            suspicious: (files / 10).max(5),
        }
    }

    pub fn set_tripwire_config(&self, files: usize, seconds: u64) -> Result<()> {
        if files < 2 || seconds == 0 {
            return Err(anyhow!("the tripwire needs at least 2 files and 1 second"));
        }
        self.kv_set("cfg.tripwire.files", &files.to_string())?;
        self.kv_set("cfg.tripwire.seconds", &seconds.to_string())
    }

    fn pin_before(&self, path: &str, version_id: i64) {
        let conn = self.db();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO pins (version_id, created_at)
             SELECT id, ?3 FROM versions WHERE path = ?1 AND id < ?2 ORDER BY id DESC LIMIT 1",
            params![path, version_id, Utc::now().to_rfc3339()],
        );
    }

    fn pin_latest(&self, path: &str) {
        let conn = self.db();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO pins (version_id, created_at)
             SELECT id, ?2 FROM versions WHERE path = ?1 ORDER BY id DESC LIMIT 1",
            params![path, Utc::now().to_rfc3339()],
        );
    }

    /// Did this new version turn readable content into random-looking bytes?
    fn looks_encrypted(&self, v: &Version) -> bool {
        if v.size < 1024 {
            return false;
        }
        let prev: Option<String> = self
            .db()
            .query_row(
                "SELECT hash FROM versions WHERE path = ?1 AND id < ?2 ORDER BY id DESC LIMIT 1",
                params![v.path, v.id],
                |r| r.get(0),
            )
            .optional()
            .ok()
            .flatten();
        let Some(prev) = prev else { return false };
        let head = |hash: &str| -> Option<Vec<u8>> {
            use std::io::Read;
            let mut buf = vec![0u8; 64 * 1024];
            let n = fs::File::open(self.blob_path(hash))
                .ok()?
                .read(&mut buf)
                .ok()?;
            buf.truncate(n);
            Some(buf)
        };
        let (Some(new_b), Some(old_b)) = (head(&v.hash), head(&prev)) else {
            return false;
        };
        let (en, eo) = (entropy(&new_b), entropy(&old_b));
        en >= 7.2 && eo <= en - 1.5
    }

    fn has_running_agent(&self, root: &str) -> bool {
        self.db()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM agent_runs WHERE status = 'running')",
                [],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
            && self
                .list_agent_runs(20)
                .map(|runs| {
                    runs.iter().any(|r| {
                        r.status == "running"
                            && (in_folder(root, &r.folder) || in_folder(&r.folder, root))
                    })
                })
                .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tripwire plugin
// ---------------------------------------------------------------------------

struct Hit {
    at: Instant,
    ts: DateTime<Utc>,
    path: String,
    version_id: Option<i64>,
    suspicious: bool,
}

#[derive(Default)]
struct RootState {
    hits: VecDeque<Hit>,
    tripped_until: Option<Instant>,
    alert_id: Option<i64>,
    alert_count: i64,
}

pub(crate) struct Tripwire {
    state: Mutex<HashMap<String, RootState>>,
}

impl Tripwire {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(HashMap::new()),
        }
    }
}

const COOLDOWN: Duration = Duration::from_secs(90);

impl Plugin for Tripwire {
    fn id(&self) -> &str {
        "tripwire"
    }
    fn name(&self) -> &str {
        "Ransomware / mass-change tripwire"
    }
    fn description(&self) -> &str {
        "Raises an alert when many files change, vanish or turn into random-looking bytes within seconds, and pins the last good version of each so no prune can remove it."
    }

    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        let (path, version_id, suspicious, ts) = match event {
            RewindEvent::VersionSaved(v) => {
                if v.summary.starts_with("Created") {
                    return;
                }
                let ts = parse_ts(&v.created_at).unwrap_or_else(Utc::now);
                (v.path.clone(), Some(v.id), core.looks_encrypted(v), ts)
            }
            RewindEvent::FileDeleted { path } => (path.clone(), None, false, Utc::now()),
            RewindEvent::FileRenamed { from, to } => {
                (to.clone(), None, ext_of(from) != ext_of(to), Utc::now())
            }
            _ => return,
        };

        let cfg = core.tripwire_config();
        let root = core.watch_root_for(&path);
        let agent_busy = core.has_running_agent(&root);
        let files_needed = if agent_busy { cfg.files * 4 } else { cfg.files };
        let window = Duration::from_secs(cfg.seconds);
        let now = Instant::now();

        enum Next {
            Nothing,
            Pin { alert_id: Option<i64>, count: i64 },
            Trip(Vec<(String, Option<i64>)>, DateTime<Utc>, usize, usize),
        }

        let next = {
            let mut all = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let rs = all.entry(root.clone()).or_default();
            rs.hits.push_back(Hit {
                at: now,
                ts,
                path: path.clone(),
                version_id,
                suspicious,
            });
            while rs
                .hits
                .front()
                .map(|h| now.duration_since(h.at) > window)
                .unwrap_or(false)
            {
                rs.hits.pop_front();
            }
            if rs.tripped_until.map(|t| now < t).unwrap_or(false) {
                rs.tripped_until = Some(now + COOLDOWN);
                rs.alert_count += 1;
                Next::Pin {
                    alert_id: rs.alert_id,
                    count: rs.alert_count,
                }
            } else {
                let distinct: HashMap<&str, ()> =
                    rs.hits.iter().map(|h| (h.path.as_str(), ())).collect();
                let susp = rs.hits.iter().filter(|h| h.suspicious).count();
                if distinct.len() >= files_needed || susp >= cfg.suspicious {
                    let first = rs.hits.iter().map(|h| h.ts).min().unwrap_or_else(Utc::now);
                    let list: Vec<(String, Option<i64>)> = rs
                        .hits
                        .iter()
                        .map(|h| (h.path.clone(), h.version_id))
                        .collect();
                    let n = distinct.len();
                    rs.tripped_until = Some(now + COOLDOWN);
                    rs.alert_count = n as i64;
                    rs.hits.clear();
                    Next::Trip(list, first, n, susp)
                } else {
                    Next::Nothing
                }
            }
        };

        match next {
            Next::Nothing => {}
            Next::Pin { alert_id, count } => {
                match version_id {
                    Some(id) => core.pin_before(&path, id),
                    None => core.pin_latest(&path),
                }
                if let Some(a) = alert_id {
                    core.update_alert_count(a, count);
                }
            }
            Next::Trip(list, first, n, susp) => {
                for (p, vid) in &list {
                    match vid {
                        Some(id) => core.pin_before(p, *id),
                        None => core.pin_latest(p),
                    }
                }
                let before = (first - chrono::Duration::microseconds(1)).to_rfc3339();
                let msg = format!(
                    "{n} files changed within {} seconds in {root}{}. The last good version of each is pinned. Rewind the folder to before {} to undo it.",
                    cfg.seconds,
                    if susp > 0 {
                        format!(" ({susp} of them now look encrypted)")
                    } else {
                        String::new()
                    },
                    first.with_timezone(&Local).format("%H:%M:%S"),
                );
                if let Ok(a) = core.raise_alert(
                    "tripwire",
                    "Many files changed at once",
                    &msg,
                    &root,
                    Some(before),
                    n as i64,
                ) {
                    let mut all = self.state.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(rs) = all.get_mut(&root) {
                        rs.alert_id = Some(a.id);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Agent runs
// ---------------------------------------------------------------------------

fn row_to_run(r: &rusqlite::Row) -> rusqlite::Result<AgentRun> {
    Ok(AgentRun {
        id: r.get(0)?,
        folder: r.get(1)?,
        agent: r.get(2)?,
        command: r.get(3)?,
        started_at: r.get(4)?,
        ended_at: r.get(5)?,
        status: r.get(6)?,
    })
}

const RUN_COLS: &str = "id, folder, agent, command, started_at, ended_at, status";

impl RewindCore {
    /// Checkpoint `folder` and open a run. Everything the agent changes after this can be listed
    /// with `agent_report` and put back with `agent_undo`.
    pub fn agent_start(&self, folder: &str, agent: &str, command: &str) -> Result<AgentRun> {
        let key = normalize_path(folder)?;
        if !Path::new(&key).is_dir() {
            return Err(anyhow!("{key} is not a folder"));
        }
        let name = if agent.trim().is_empty() {
            "agent"
        } else {
            agent.trim()
        };
        self.add_watch(&key)?;
        self.add_label(&key, &format!("Before agent: {name}"))?;
        let now = Utc::now().to_rfc3339();
        let id = {
            let conn = self.db();
            conn.execute(
                "INSERT INTO agent_runs (folder, agent, command, started_at, status)
                 VALUES (?1, ?2, ?3, ?4, 'running')",
                params![key, name, command, now],
            )?;
            conn.last_insert_rowid()
        };
        self.get_agent_run(id)
    }

    pub fn get_agent_run(&self, id: i64) -> Result<AgentRun> {
        self.db()
            .query_row(
                &format!("SELECT {RUN_COLS} FROM agent_runs WHERE id = ?1"),
                params![id],
                row_to_run,
            )
            .optional()?
            .ok_or_else(|| anyhow!("agent run {id} not found"))
    }

    /// Newest first.
    pub fn list_agent_runs(&self, limit: usize) -> Result<Vec<AgentRun>> {
        let conn = self.db();
        let mut stmt = conn.prepare(&format!(
            "SELECT {RUN_COLS} FROM agent_runs ORDER BY id DESC LIMIT ?1"
        ))?;
        let rows = stmt
            .query_map(params![limit as i64], row_to_run)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// The newest run that has not been undone, optionally for one folder.
    pub fn latest_agent_run(&self, folder: Option<&str>) -> Result<Option<AgentRun>> {
        let key = folder.map(key_for_str);
        Ok(self
            .list_agent_runs(200)?
            .into_iter()
            .find(|r| r.status != "undone" && key.as_ref().map(|k| &r.folder == k).unwrap_or(true)))
    }

    /// Close a run: capture the final state of the folder and mark it ended.
    pub fn agent_end(&self, id: i64) -> Result<AgentReport> {
        let run = self.get_agent_run(id)?;
        if run.ended_at.is_none() {
            self.snapshot_folder(&run.folder);
            self.reconcile_deleted(&run.folder);
            self.db().execute(
                "UPDATE agent_runs SET ended_at = ?2, status = 'ended' WHERE id = ?1 AND status = 'running'",
                params![id, Utc::now().to_rfc3339()],
            )?;
        }
        self.agent_report(id)
    }

    /// Like `agent_report`, but first captures the folder's current state, so it is right even
    /// when no watcher is running. Does not end the run.
    pub fn agent_report_live(&self, id: i64) -> Result<AgentReport> {
        let run = self.get_agent_run(id)?;
        if run.ended_at.is_none() {
            self.snapshot_folder(&run.folder);
            self.reconcile_deleted(&run.folder);
        }
        self.agent_report(id)
    }

    /// What the agent did between the checkpoint and now (or the end of the run).
    pub fn agent_report(&self, id: i64) -> Result<AgentReport> {
        let run = self.get_agent_run(id)?;
        let end = run
            .ended_at
            .clone()
            .unwrap_or_else(|| Utc::now().to_rfc3339());

        let rows: Vec<(String, i64, i64)> = {
            let conn = self.db();
            let mut stmt = conn.prepare(
                "SELECT path, lines_added, lines_deleted FROM versions
                 WHERE created_at > ?1 AND created_at <= ?2 ORDER BY id",
            )?;
            let rows = stmt
                .query_map(params![run.started_at, end], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        let mut order: Vec<String> = Vec::new();
        let mut agg: HashMap<String, (i64, i64, i64)> = HashMap::new();
        for (p, a, d) in rows {
            if !in_folder(&p, &run.folder) {
                continue;
            }
            let e = agg.entry(p.clone()).or_insert_with(|| {
                order.push(p.clone());
                (0, 0, 0)
            });
            e.0 += 1;
            e.1 += a;
            e.2 += d;
        }

        let existed_before = |path: &str| -> bool {
            self.db()
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM versions WHERE path = ?1 AND created_at <= ?2)",
                    params![path, run.started_at],
                    |r| r.get(0),
                )
                .unwrap_or(false)
        };

        let mut changes = Vec::new();
        for p in &order {
            let (n, a, d) = agg[p];
            let exists = Path::new(p).exists();
            let before = existed_before(p);
            let action = match (before, exists) {
                (false, true) => "created",
                (false, false) => continue, // made and removed again: net nothing
                (true, true) => "modified",
                (true, false) => "deleted",
            };
            changes.push(AgentChange {
                path: p.clone(),
                action: action.into(),
                versions: n,
                lines_added: a,
                lines_deleted: d,
            });
        }

        // files that vanished without leaving a newer version
        let gone: Vec<String> = {
            let conn = self.db();
            let mut stmt = conn.prepare(
                "SELECT DISTINCT path FROM events
                 WHERE kind = 'deleted' AND created_at > ?1 AND created_at <= ?2",
            )?;
            let rows = stmt
                .query_map(params![run.started_at, end], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for p in gone {
            if in_folder(&p, &run.folder)
                && !Path::new(&p).exists()
                && existed_before(&p)
                && !changes.iter().any(|c| c.path == p)
            {
                changes.push(AgentChange {
                    path: p,
                    action: "deleted".into(),
                    versions: 0,
                    lines_added: 0,
                    lines_deleted: 0,
                });
            }
        }
        changes.sort_by(|a, b| a.path.cmp(&b.path));

        let count = |a: &str| changes.iter().filter(|c| c.action == a).count() as i64;
        let (created, modified, deleted) = (count("created"), count("modified"), count("deleted"));
        let lines_added = changes.iter().map(|c| c.lines_added).sum();
        let lines_deleted = changes.iter().map(|c| c.lines_deleted).sum();

        let mut flags = Vec::new();
        if deleted >= 5 {
            flags.push(format!("deleted {deleted} files"));
        }
        let secrets: Vec<&AgentChange> = changes.iter().filter(|c| looks_secret(&c.path)).collect();
        if !secrets.is_empty() {
            flags.push(format!(
                "touched files that look like secrets: {}",
                secrets
                    .iter()
                    .take(3)
                    .map(|c| relative_name(&c.path, &run.folder))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        for c in changes
            .iter()
            .filter(|c| {
                c.action == "modified"
                    && c.lines_deleted >= 50
                    && c.lines_deleted > c.lines_added * 3
            })
            .take(3)
        {
            flags.push(format!(
                "{} lost {} lines and gained {}",
                relative_name(&c.path, &run.folder),
                c.lines_deleted,
                c.lines_added
            ));
        }
        if modified + created >= 100 {
            flags.push(format!("touched {} files", modified + created));
        }

        Ok(AgentReport {
            run,
            changes,
            created,
            modified,
            deleted,
            lines_added,
            lines_deleted,
            flags,
        })
    }

    /// What undoing the run would change. Nothing is touched.
    pub fn agent_undo_plan(&self, id: i64) -> Result<RewindPlan> {
        let run = self.get_agent_run(id)?;
        self.plan_folder_rewind(&run.folder, &run.started_at)
    }

    /// Put the folder back to how it was when the run started. A label is saved first, so even
    /// the undo can be undone. Files the agent created are only removed with `remove_new`.
    pub fn agent_undo(&self, id: i64, remove_new: bool) -> Result<ApplyReport> {
        let run = self.get_agent_run(id)?;
        if run.status == "running" {
            self.agent_end(id)?;
        }
        let rep = self.apply_folder_rewind(&run.folder, &run.started_at, remove_new)?;
        self.db().execute(
            "UPDATE agent_runs SET status = 'undone' WHERE id = ?1",
            params![id],
        )?;
        Ok(rep)
    }

    /// Save a labelled checkpoint unless one was saved less than `min_interval_secs` ago.
    /// Meant for hooks that fire before every agent tool call.
    pub fn checkpoint(
        &self,
        folder: &str,
        name: Option<&str>,
        min_interval_secs: u64,
    ) -> Result<Option<Label>> {
        let key = normalize_path(folder)?;
        if min_interval_secs > 0 {
            let cutoff = Utc::now() - chrono::Duration::seconds(min_interval_secs as i64);
            let recent = self
                .list_labels(Some(&key))?
                .into_iter()
                .filter(|l| l.folder == key && l.name.starts_with("Checkpoint"))
                .filter_map(|l| parse_ts(&l.created_at))
                .any(|t| t > cutoff);
            if recent {
                return Ok(None);
            }
        }
        let label = match name {
            Some(n) if !n.trim().is_empty() => format!("Checkpoint: {}", n.trim()),
            _ => format!("Checkpoint {}", Local::now().format("%H:%M:%S")),
        };
        self.add_watch(&key)?;
        Ok(Some(self.add_label(&key, &label)?))
    }
}

fn key_for_str(s: &str) -> String {
    key_for(Path::new(s))
}
