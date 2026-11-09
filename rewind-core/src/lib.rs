//! rewind-core: content-addressed file history.
//!
//! * SQLite (rusqlite) stores version metadata
//! * blake3 hashes file content; blobs are stored by hash (deduplicated)
//! * notify watches folders and snapshots changed files after a short debounce

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;
const DEBOUNCE_MS: u64 = 350;
const MAX_DIFF_CELLS: u64 = 6_000_000;
const IGNORED_DIRS: &[&str] = &[
    ".git", ".hg", ".svn", "target", "node_modules", ".rewind", "__pycache__", ".vs", ".idea",
    ".next",
];

const VCOLS: &str = "id, path, hash, size, created_at, lines_added, lines_deleted, summary";

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: i64,
    /// 1-based version number for this file (v1 = first snapshot).
    pub number: i64,
    pub path: String,
    pub hash: String,
    pub size: i64,
    /// RFC 3339 timestamp (UTC).
    pub created_at: String,
    pub lines_added: i64,
    pub lines_deleted: i64,
    pub summary: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchInfo {
    pub path: String,
    pub added_at: String,
    pub versions: i64,
    pub files: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    pub data_dir: String,
    pub total_versions: i64,
    pub total_files: i64,
    pub watches: Vec<WatchInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    /// Path relative to the watched folder.
    pub name: String,
    pub versions: i64,
    pub last_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    /// "add", "del", "ctx" or "hunk"
    pub kind: String,
    pub text: String,
}

/// Keep this alive to keep watching. Dropping it stops the watcher.
pub struct WatcherHandle {
    root: String,
    _watcher: RecommendedWatcher,
}

impl WatcherHandle {
    pub fn root(&self) -> &str {
        &self.root
    }
}

type Callback = Arc<dyn Fn(&Version) + Send + Sync>;

// ---------------------------------------------------------------------------
// Core
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct RewindCore {
    conn: Arc<Mutex<Connection>>,
    data_dir: Arc<PathBuf>,
    store_dir: Arc<PathBuf>,
}

/// `%LOCALAPPDATA%\Rewind` on Windows, `~/.rewind` elsewhere. Override with REWIND_HOME.
pub fn default_data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("REWIND_HOME") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(p).join("Rewind");
    }
    if let Some(p) = std::env::var_os("HOME") {
        return PathBuf::from(p).join(".rewind");
    }
    PathBuf::from(".rewind")
}

/// Canonical absolute path as a String (Windows `\\?\` prefix stripped).
pub fn normalize_path(p: impl AsRef<Path>) -> Result<String> {
    let p = p.as_ref();
    let c = fs::canonicalize(p).with_context(|| format!("cannot resolve path: {}", p.display()))?;
    let s = c.to_string_lossy().to_string();
    Ok(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string())
}

fn key_for(p: &Path) -> String {
    normalize_path(p).unwrap_or_else(|_| p.to_string_lossy().to_string())
}

impl RewindCore {
    pub fn open_default() -> Result<Self> {
        Self::open(default_data_dir())
    }

    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let store_dir = data_dir.join("blobs");
        fs::create_dir_all(&store_dir)
            .with_context(|| format!("cannot create {}", store_dir.display()))?;
        let conn = Connection::open(data_dir.join("rewind.db"))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS versions (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 path          TEXT    NOT NULL,
                 hash          TEXT    NOT NULL,
                 size          INTEGER NOT NULL,
                 created_at    TEXT    NOT NULL,
                 lines_added   INTEGER NOT NULL DEFAULT 0,
                 lines_deleted INTEGER NOT NULL DEFAULT 0,
                 summary       TEXT    NOT NULL DEFAULT ''
             );
             CREATE INDEX IF NOT EXISTS idx_versions_path ON versions(path, id);
             CREATE TABLE IF NOT EXISTS watches (
                 path     TEXT PRIMARY KEY,
                 added_at TEXT NOT NULL
             );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            data_dir: Arc::new(data_dir),
            store_dir: Arc::new(store_dir),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn blob_path(&self, hash: &str) -> PathBuf {
        self.store_dir.join(hash)
    }

    // -- versions ----------------------------------------------------------

    /// Snapshot a file. Returns `Ok(None)` when nothing changed since the last
    /// version (same blake3 hash), or when the path is not a regular file / too big.
    pub fn save_version(&self, path: impl AsRef<Path>) -> Result<Option<Version>> {
        let key = normalize_path(path.as_ref())?;
        let meta = fs::metadata(&key)?;
        if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
            return Ok(None);
        }
        let bytes = fs::read(&key)?;
        let hash = blake3::hash(&bytes).to_hex().to_string();

        let conn = self.db();
        let prev: Option<String> = conn
            .query_row(
                "SELECT hash FROM versions WHERE path = ?1 ORDER BY id DESC LIMIT 1",
                params![key],
                |r| r.get(0),
            )
            .optional()?;
        if prev.as_deref() == Some(hash.as_str()) {
            return Ok(None);
        }

        let blob = self.blob_path(&hash);
        if !blob.exists() {
            let tmp = self.store_dir.join(format!("{hash}.{}.tmp", Uuid::new_v4()));
            fs::write(&tmp, &bytes)?;
            fs::rename(&tmp, &blob)?;
        }

        let new_text = std::str::from_utf8(&bytes).ok();
        let old_bytes = prev.as_ref().and_then(|h| fs::read(self.blob_path(h)).ok());
        let old_text = old_bytes.as_deref().and_then(|b| std::str::from_utf8(b).ok());
        let size_h = human_size(bytes.len() as u64);

        let (added, deleted, summary) = match (new_text, old_text, prev.is_some()) {
            (Some(n), Some(o), true) => {
                let d = diff_lines(o, n);
                let a = d.iter().filter(|l| l.kind == "add").count() as i64;
                let r = d.iter().filter(|l| l.kind == "del").count() as i64;
                (a, r, format!("Modified: +{a} -{r} lines ({size_h})"))
            }
            (Some(n), _, false) => {
                let l = n.lines().count() as i64;
                (l, 0, format!("Created: {l} lines ({size_h})"))
            }
            _ => (0, 0, format!("Binary or non-UTF-8 change ({size_h})")),
        };

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO versions (path, hash, size, created_at, lines_added, lines_deleted, summary)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![key, hash, bytes.len() as i64, now, added, deleted, summary],
        )?;
        let id = conn.last_insert_rowid();
        let number: i64 = conn.query_row(
            "SELECT COUNT(*) FROM versions WHERE path = ?1",
            params![key],
            |r| r.get(0),
        )?;
        Ok(Some(Version {
            id,
            number,
            path: key,
            hash,
            size: bytes.len() as i64,
            created_at: now,
            lines_added: added,
            lines_deleted: deleted,
            summary,
            is_current: true,
        }))
    }

    /// Full history of one file, newest first.
    pub fn get_history(&self, path: impl AsRef<Path>) -> Result<Vec<Version>> {
        let key = key_for(path.as_ref());
        let conn = self.db();
        let mut stmt = conn.prepare(&format!(
            "SELECT {VCOLS} FROM versions WHERE path = ?1 ORDER BY id DESC"
        ))?;
        let mut list: Vec<Version> = stmt
            .query_map(params![key], row_to_version)?
            .collect::<rusqlite::Result<_>>()?;
        let total = list.len() as i64;
        for (i, v) in list.iter_mut().enumerate() {
            v.number = total - i as i64;
            v.is_current = i == 0;
        }
        Ok(list)
    }

    pub fn get_version(&self, id: i64) -> Result<Version> {
        let conn = self.db();
        let mut v = conn
            .query_row(
                &format!("SELECT {VCOLS} FROM versions WHERE id = ?1"),
                params![id],
                row_to_version,
            )
            .optional()?
            .ok_or_else(|| anyhow!("version {id} not found"))?;
        v.number = conn.query_row(
            "SELECT COUNT(*) FROM versions WHERE path = ?1 AND id <= ?2",
            params![v.path, id],
            |r| r.get(0),
        )?;
        let later: i64 = conn.query_row(
            "SELECT COUNT(*) FROM versions WHERE path = ?1 AND id > ?2",
            params![v.path, id],
            |r| r.get(0),
        )?;
        v.is_current = later == 0;
        Ok(v)
    }

    /// Roll a file back to a stored version. The current on-disk state is
    /// snapshotted first, so a restore can always be undone.
    pub fn restore_version(&self, id: i64) -> Result<Version> {
        let v = self.get_version(id)?;
        let bytes = fs::read(self.blob_path(&v.hash))
            .context("stored content for this version is missing")?;
        let _ = self.save_version(&v.path);
        fs::write(&v.path, &bytes).with_context(|| format!("cannot write {}", v.path))?;
        Ok(self.save_version(&v.path)?.unwrap_or(v))
    }

    /// Line diff of a version against the version before it.
    pub fn diff_version(&self, id: i64) -> Result<Vec<DiffLine>> {
        let v = self.get_version(id)?;
        let prev_hash: Option<String> = self
            .db()
            .query_row(
                "SELECT hash FROM versions WHERE path = ?1 AND id < ?2 ORDER BY id DESC LIMIT 1",
                params![v.path, id],
                |r| r.get(0),
            )
            .optional()?;
        let new_b = fs::read(self.blob_path(&v.hash)).context("stored content is missing")?;
        let old_b = match prev_hash {
            Some(h) => fs::read(self.blob_path(&h)).unwrap_or_default(),
            None => Vec::new(),
        };
        let (Ok(n), Ok(o)) = (std::str::from_utf8(&new_b), std::str::from_utf8(&old_b)) else {
            return Ok(vec![DiffLine {
                kind: "hunk".into(),
                text: "binary or non-UTF-8 content, no text diff".into(),
            }]);
        };
        Ok(collapse_context(diff_lines(o, n), 3))
    }

    // -- folders / status --------------------------------------------------

    fn grouped(&self) -> Result<Vec<(String, i64, String)>> {
        let conn = self.db();
        let mut stmt = conn
            .prepare("SELECT path, COUNT(*), MAX(created_at) FROM versions GROUP BY path")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Files with history under a folder, most recently changed first.
    pub fn list_files(&self, folder: impl AsRef<Path>) -> Result<Vec<FileEntry>> {
        let key = key_for(folder.as_ref());
        let mut out: Vec<FileEntry> = self
            .grouped()?
            .into_iter()
            .filter(|(p, _, _)| in_folder(p, &key))
            .map(|(p, c, last)| FileEntry {
                name: relative_name(&p, &key),
                path: p,
                versions: c,
                last_change: last,
            })
            .collect();
        out.sort_by(|a, b| b.last_change.cmp(&a.last_change));
        Ok(out)
    }

    pub fn get_status(&self) -> Result<Status> {
        let grouped = self.grouped()?;
        let watches = self
            .watch_rows()?
            .into_iter()
            .map(|(path, added_at)| {
                let (mut versions, mut files) = (0, 0);
                for (p, c, _) in &grouped {
                    if in_folder(p, &path) {
                        versions += c;
                        files += 1;
                    }
                }
                WatchInfo { path, added_at, versions, files }
            })
            .collect();
        Ok(Status {
            data_dir: self.data_dir.to_string_lossy().to_string(),
            total_versions: grouped.iter().map(|g| g.1).sum(),
            total_files: grouped.len() as i64,
            watches,
        })
    }

    fn watch_rows(&self) -> Result<Vec<(String, String)>> {
        let conn = self.db();
        let mut stmt = conn.prepare("SELECT path, added_at FROM watches ORDER BY added_at DESC")?;
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn list_watches(&self) -> Result<Vec<String>> {
        Ok(self.watch_rows()?.into_iter().map(|r| r.0).collect())
    }

    /// Remember a folder (or file) as watched. Returns its canonical path.
    pub fn add_watch(&self, path: impl AsRef<Path>) -> Result<String> {
        let key = normalize_path(path.as_ref())?;
        self.db().execute(
            "INSERT OR IGNORE INTO watches (path, added_at) VALUES (?1, ?2)",
            params![key, Utc::now().to_rfc3339()],
        )?;
        Ok(key)
    }

    /// Stop remembering a watch. History is kept.
    pub fn remove_watch(&self, path: impl AsRef<Path>) -> Result<()> {
        let key = key_for(path.as_ref());
        self.db()
            .execute("DELETE FROM watches WHERE path = ?1", params![key])?;
        Ok(())
    }

    // -- watcher -----------------------------------------------------------

    pub fn start_watcher(&self, path: impl AsRef<Path>) -> Result<WatcherHandle> {
        self.start_watcher_with(path, |_| {})
    }

    /// Snapshot everything under `path` once, then snapshot every change
    /// (debounced). `on_version` fires for each new version recorded.
    pub fn start_watcher_with<F>(&self, path: impl AsRef<Path>, on_version: F) -> Result<WatcherHandle>
    where
        F: Fn(&Version) + Send + Sync + 'static,
    {
        let root = normalize_path(path.as_ref())?;
        let (tx, rx) = mpsc::channel::<notify::Result<notify::Event>>();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let _ = tx.send(res);
        })?;
        watcher.watch(Path::new(&root), RecursiveMode::Recursive)?;

        let core = self.clone();
        let cb: Callback = Arc::new(on_version);
        let root_thread = root.clone();
        thread::spawn(move || {
            let root_path = PathBuf::from(&root_thread);

            // initial snapshot of whatever is already there
            let mut files = Vec::new();
            if root_path.is_file() {
                files.push(root_path.clone());
            } else {
                walk(&root_path, &mut files);
            }
            for f in files {
                snapshot_file(&core, &root_path, &f, &cb);
            }

            // live changes; ends when the WatcherHandle (and its sender) is dropped
            while let Ok(first) = rx.recv() {
                let mut pending: HashSet<PathBuf> = HashSet::new();
                collect(first, &mut pending);
                loop {
                    match rx.recv_timeout(Duration::from_millis(DEBOUNCE_MS)) {
                        Ok(ev) => collect(ev, &mut pending),
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                for p in pending {
                    snapshot_file(&core, &root_path, &p, &cb);
                }
            }
        });

        Ok(WatcherHandle { root, _watcher: watcher })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn row_to_version(r: &rusqlite::Row) -> rusqlite::Result<Version> {
    Ok(Version {
        id: r.get(0)?,
        number: 0,
        path: r.get(1)?,
        hash: r.get(2)?,
        size: r.get(3)?,
        created_at: r.get(4)?,
        lines_added: r.get(5)?,
        lines_deleted: r.get(6)?,
        summary: r.get(7)?,
        is_current: false,
    })
}

fn collect(ev: notify::Result<notify::Event>, set: &mut HashSet<PathBuf>) {
    if let Ok(ev) = ev {
        if matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            for p in ev.paths {
                set.insert(p);
            }
        }
    }
}

fn snapshot_file(core: &RewindCore, root: &Path, p: &Path, cb: &Callback) {
    if !p.is_file() || is_ignored(root, p) {
        return;
    }
    for attempt in 0..3u64 {
        match core.save_version(p) {
            Ok(Some(v)) => {
                cb(&v);
                return;
            }
            Ok(None) => return,
            // file may be mid-write or locked by the editor: retry shortly
            Err(_) => thread::sleep(Duration::from_millis(150 * (attempt + 1))),
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if ft.is_dir() {
            if IGNORED_DIRS.iter().any(|d| name == *d) {
                continue;
            }
            walk(&p, out);
        } else if ft.is_file() {
            out.push(p);
        }
    }
}

fn is_ignored(root: &Path, p: &Path) -> bool {
    let rel = p.strip_prefix(root).unwrap_or(p);
    for c in rel.components() {
        if let std::path::Component::Normal(os) = c {
            let s = os.to_string_lossy().to_lowercase();
            if IGNORED_DIRS.iter().any(|d| s == *d) {
                return true;
            }
        }
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.ends_with(".tmp")
        || name.ends_with('~')
        || name.ends_with(".swp")
        || name.starts_with("~$")
        || name.starts_with(".#")
}

fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}

fn in_folder(path: &str, folder: &str) -> bool {
    let p = path.to_lowercase();
    let f = folder.to_lowercase();
    let f = f.trim_end_matches(is_sep);
    p == f || p.starts_with(&format!("{f}\\")) || p.starts_with(&format!("{f}/"))
}

fn relative_name(path: &str, folder: &str) -> String {
    let f = folder.trim_end_matches(is_sep);
    path.get(f.len()..)
        .map(|s| s.trim_start_matches(is_sep))
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| {
            Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string())
        })
}

fn human_size(b: u64) -> String {
    if b < 1024 {
        format!("{b} B")
    } else if b < 1024 * 1024 {
        format!("{:.1} KB", b as f64 / 1024.0)
    } else {
        format!("{:.1} MB", b as f64 / (1024.0 * 1024.0))
    }
}

fn dl(kind: &str, text: &str) -> DiffLine {
    DiffLine { kind: kind.to_string(), text: text.to_string() }
}

/// Line-based diff (common prefix/suffix trimmed, LCS on the middle).
pub fn diff_lines(old: &str, new: &str) -> Vec<DiffLine> {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();

    let mut start = 0;
    while start < a.len() && start < b.len() && a[start] == b[start] {
        start += 1;
    }
    let (mut ea, mut eb) = (a.len(), b.len());
    while ea > start && eb > start && a[ea - 1] == b[eb - 1] {
        ea -= 1;
        eb -= 1;
    }
    let ma = &a[start..ea];
    let mb = &b[start..eb];

    let mut out = Vec::new();
    for l in &a[..start] {
        out.push(dl("ctx", l));
    }

    let (n, m) = (ma.len(), mb.len());
    if (n as u64) * (m as u64) > MAX_DIFF_CELLS {
        for l in ma {
            out.push(dl("del", l));
        }
        for l in mb {
            out.push(dl("add", l));
        }
    } else {
        let w = m + 1;
        let mut t = vec![0u32; (n + 1) * w];
        for i in (0..n).rev() {
            for j in (0..m).rev() {
                t[i * w + j] = if ma[i] == mb[j] {
                    t[(i + 1) * w + j + 1] + 1
                } else {
                    t[(i + 1) * w + j].max(t[i * w + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < n && j < m {
            if ma[i] == mb[j] {
                out.push(dl("ctx", ma[i]));
                i += 1;
                j += 1;
            } else if t[(i + 1) * w + j] >= t[i * w + j + 1] {
                out.push(dl("del", ma[i]));
                i += 1;
            } else {
                out.push(dl("add", mb[j]));
                j += 1;
            }
        }
        while i < n {
            out.push(dl("del", ma[i]));
            i += 1;
        }
        while j < m {
            out.push(dl("add", mb[j]));
            j += 1;
        }
    }

    for l in &a[ea..] {
        out.push(dl("ctx", l));
    }
    out
}

fn collapse_context(lines: Vec<DiffLine>, ctx: usize) -> Vec<DiffLine> {
    let n = lines.len();
    let mut keep = vec![false; n];
    for (i, l) in lines.iter().enumerate() {
        if l.kind != "ctx" {
            let s = i.saturating_sub(ctx);
            let e = (i + ctx + 1).min(n);
            for k in keep.iter_mut().take(e).skip(s) {
                *k = true;
            }
        }
    }
    if !keep.iter().any(|k| *k) {
        return vec![dl("hunk", "no textual changes")];
    }
    let mut out = Vec::new();
    let mut skipped = 0usize;
    for (i, l) in lines.into_iter().enumerate() {
        if keep[i] {
            if skipped > 0 {
                out.push(dl("hunk", &format!("... {skipped} unchanged lines ...")));
                skipped = 0;
            }
            out.push(l);
        } else {
            skipped += 1;
        }
    }
    if skipped > 0 {
        out.push(dl("hunk", &format!("... {skipped} unchanged lines ...")));
    }
    out
}
