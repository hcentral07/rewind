//! Trust and engine features: delete/rename tracking, folder rewind, pins, labels,
//! history search, compare any two versions, deleted-files list and the disk guard.
use super::*;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use std::collections::HashMap;

const MAX_SEARCH_BLOB: i64 = 2 * 1024 * 1024;
const MAX_SEARCH_SCANNED: usize = 30_000;
const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub name: String,
    pub folder: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeletedEntry {
    pub path: String,
    pub name: String,
    pub deleted_at: String,
    pub last_version_id: i64,
    pub versions: i64,
    pub size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub version_id: i64,
    pub path: String,
    pub created_at: String,
    pub line_no: i64,
    pub snippet: String,
    /// false means the newest version of the file no longer contains the text.
    pub in_latest: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub path: String,
    /// "restore" (file differs), "recreate" (file is gone) or "remove" (file did not exist yet)
    pub action: String,
    pub version_id: Option<i64>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewindPlan {
    pub folder: String,
    pub to: String,
    pub items: Vec<PlanItem>,
    pub unchanged: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApplyReport {
    pub restored: i64,
    pub recreated: i64,
    pub removed: i64,
    pub failed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardStatus {
    pub used_bytes: i64,
    pub limit_bytes: i64,
    pub over: bool,
    pub auto_prune: bool,
}

// ---------------------------------------------------------------------------
// Time parsing
// ---------------------------------------------------------------------------

/// Accepts `30m`, `2h`, `1d` (ago), an RFC 3339 time, or `YYYY-MM-DD HH:MM[:SS]` in local time.
pub fn parse_when(input: &str) -> Result<DateTime<Utc>> {
    let s = input.trim();
    let s = s.strip_suffix(" ago").unwrap_or(s);
    if s.is_ascii() && s.len() >= 2 {
        let (num, unit) = s.split_at(s.len() - 1);
        if let Ok(n) = num.parse::<i64>() {
            let d = match unit {
                "m" => Some(chrono::Duration::minutes(n)),
                "h" => Some(chrono::Duration::hours(n)),
                "d" => Some(chrono::Duration::days(n)),
                _ => None,
            };
            if let Some(d) = d {
                return Ok(Utc::now() - d);
            }
        }
    }
    if let Ok(t) = DateTime::parse_from_rfc3339(s) {
        return Ok(t.with_timezone(&Utc));
    }
    for fmt in ["%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, fmt) {
            if let Some(l) = Local.from_local_datetime(&n).single() {
                return Ok(l.with_timezone(&Utc));
            }
        }
    }
    Err(anyhow!(
        "cannot understand the time '{input}' (try 2h, 1d, 2026-11-10 15:00 or an RFC 3339 time)"
    ))
}

fn parse_ts(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.with_timezone(&Utc))
}

// ---------------------------------------------------------------------------
// Core extensions
// ---------------------------------------------------------------------------

impl RewindCore {
    // -- small key/value store (plugin settings, flags) --------------------

    pub fn kv_get(&self, key: &str) -> Option<String> {
        self.db()
            .query_row("SELECT value FROM kv WHERE key = ?1", params![key], |r| {
                r.get::<_, String>(0)
            })
            .optional()
            .ok()
            .flatten()
    }

    pub fn kv_set(&self, key: &str, value: &str) -> Result<()> {
        self.db().execute(
            "INSERT INTO kv (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // -- delete / rename tracking ------------------------------------------

    fn log_event(&self, kind: &str, path: &str, new_path: Option<&str>) {
        let _ = self.db().execute(
            "INSERT INTO events (kind, path, new_path, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![kind, path, new_path, Utc::now().to_rfc3339()],
        );
    }

    fn log_deleted(&self, path: &str) {
        let already: bool = self
            .db()
            .query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM events
                     WHERE kind = 'deleted' AND path = ?1
                       AND created_at >= COALESCE(
                           (SELECT MAX(created_at) FROM versions WHERE path = ?1), '')
                 )",
                params![path],
                |r| r.get(0),
            )
            .unwrap_or(false);
        if already {
            return;
        }
        self.log_event("deleted", path, None);
        self.emit(&RewindEvent::FileDeleted {
            path: path.to_string(),
        });
    }

    /// A path (file or folder) disappeared: mark every versioned file under it as deleted.
    pub(crate) fn record_removed(&self, p: &Path) {
        let key = key_for(p);
        let Ok(rows) = self.grouped() else { return };
        for (path, _, _) in rows {
            if in_folder(&path, &key) && !Path::new(&path).exists() {
                self.log_deleted(&path);
            }
        }
    }

    /// Catch deletions that happened while Rewind was not running.
    pub fn reconcile_deleted(&self, root: &str) {
        let Ok(rows) = self.grouped() else { return };
        for (path, _, _) in rows {
            if in_folder(&path, root) && !Path::new(&path).exists() {
                self.log_deleted(&path);
            }
        }
    }

    fn has_history(&self, key: &str) -> bool {
        self.db()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM versions WHERE path = ?1)",
                params![key],
                |r| r.get(0),
            )
            .unwrap_or(false)
    }

    /// (exact stored path, hash of newest version) for a path, ignoring case.
    fn latest_for_path(&self, key: &str) -> Option<(String, String)> {
        self.db()
            .query_row(
                "SELECT path, hash FROM versions WHERE path = ?1 COLLATE NOCASE
                 ORDER BY id DESC LIMIT 1",
                params![key],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()
            .ok()
            .flatten()
    }

    fn rename_path(&self, old: &str, new: &str) {
        let ok = self
            .db()
            .execute(
                "UPDATE versions SET path = ?2 WHERE path = ?1",
                params![old, new],
            )
            .is_ok();
        if ok {
            self.log_event("renamed", old, Some(new));
            self.emit(&RewindEvent::FileRenamed {
                from: old.to_string(),
                to: new.to_string(),
            });
        }
    }

    fn rename_prefix(&self, old_key: &str, new_key: &str) {
        let old_len = old_key.trim_end_matches(is_sep).len();
        let new_base = new_key.trim_end_matches(is_sep);
        let Ok(rows) = self.grouped() else { return };
        for (path, _, _) in rows {
            if !in_folder(&path, old_key) || Path::new(&path).exists() {
                continue;
            }
            let Some(rest) = path.get(old_len..) else {
                continue;
            };
            let target = format!("{new_base}{rest}");
            if !self.has_history(&target) {
                self.rename_path(&path, &target);
            }
        }
    }

    /// Match removed paths against newly appeared ones. A file that reappears with the same
    /// content (or a folder whose files reappear) is a rename: its history moves with it.
    /// Returns the removed paths that were handled as renames.
    pub(crate) fn detect_renames(&self, removed: &[PathBuf], changed: &[PathBuf]) -> Vec<PathBuf> {
        let mut handled: Vec<PathBuf> = Vec::new();
        if removed.is_empty() || changed.is_empty() {
            return handled;
        }
        let Ok(grouped) = self.grouped() else {
            return handled;
        };

        for new in changed {
            if new.is_dir() {
                let Ok(new_key) = normalize_path(new) else {
                    continue;
                };
                let mut files = Vec::new();
                walk(new, &mut files);
                if files.is_empty() {
                    continue;
                }
                let new_rels: Vec<String> = files
                    .iter()
                    .filter_map(|f| f.strip_prefix(new).ok())
                    .map(|r| r.to_string_lossy().to_lowercase())
                    .collect();
                for old in removed {
                    if handled.contains(old) {
                        continue;
                    }
                    let old_key = key_for(old);
                    let old_rels: Vec<String> = grouped
                        .iter()
                        .map(|g| g.0.as_str())
                        .filter(|p| in_folder(p, &old_key) && !Path::new(p).exists())
                        .map(|p| relative_name(p, &old_key).to_lowercase())
                        .collect();
                    if old_rels.is_empty() {
                        continue;
                    }
                    let hits = old_rels.iter().filter(|r| new_rels.contains(r)).count();
                    if hits > 0 && hits * 2 >= old_rels.len().min(new_rels.len()) {
                        self.rename_prefix(&old_key, &new_key);
                        handled.push(old.clone());
                        break;
                    }
                }
            } else if new.is_file() {
                let Ok(new_key) = normalize_path(new) else {
                    continue;
                };
                if self.has_history(&new_key) {
                    continue;
                }
                let small = fs::metadata(new)
                    .map(|m| m.len() <= MAX_FILE_BYTES)
                    .unwrap_or(false);
                if !small {
                    continue;
                }
                let Ok(bytes) = fs::read(new) else { continue };
                let hash = blake3::hash(&bytes).to_hex().to_string();
                for old in removed {
                    if handled.contains(old) {
                        continue;
                    }
                    if let Some((exact, old_hash)) = self.latest_for_path(&key_for(old)) {
                        if old_hash == hash {
                            self.rename_path(&exact, &new_key);
                            handled.push(old.clone());
                            break;
                        }
                    }
                }
            }
        }
        handled
    }

    // -- deleted files -------------------------------------------------------

    /// Files that vanished from watched folders and are still missing, newest first.
    pub fn list_deleted(&self, folder: Option<&str>) -> Result<Vec<DeletedEntry>> {
        let key = folder.map(|f| key_for(Path::new(f)));
        let events: Vec<(String, String)> = {
            let conn = self.db();
            let mut stmt = conn.prepare(
                "SELECT path, MAX(created_at) FROM events WHERE kind = 'deleted'
                 GROUP BY path ORDER BY MAX(created_at) DESC",
            )?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        let mut out = Vec::new();
        for (path, deleted_at) in events {
            if Path::new(&path).exists() {
                continue;
            }
            if let Some(k) = &key {
                if !in_folder(&path, k) {
                    continue;
                }
            }
            let hist = self.get_history(&path)?;
            let Some(last) = hist.first() else { continue };
            let name = Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            out.push(DeletedEntry {
                name,
                deleted_at,
                last_version_id: last.id,
                versions: hist.len() as i64,
                size: last.size,
                path,
            });
        }
        Ok(out)
    }

    /// Bring a deleted file back from its newest stored version.
    pub fn restore_deleted(&self, path: &str) -> Result<Version> {
        let hist = self.get_history(path)?;
        let last = hist
            .first()
            .ok_or_else(|| anyhow!("no history for {path}"))?;
        self.restore_version(last.id)
    }

    // -- pins ------------------------------------------------------------------

    pub fn pin_version(&self, id: i64, pinned: bool) -> Result<()> {
        self.get_version(id)?;
        if pinned {
            self.db().execute(
                "INSERT OR IGNORE INTO pins (version_id, created_at) VALUES (?1, ?2)",
                params![id, Utc::now().to_rfc3339()],
            )?;
        } else {
            self.db()
                .execute("DELETE FROM pins WHERE version_id = ?1", params![id])?;
        }
        Ok(())
    }

    // -- labels ----------------------------------------------------------------

    /// Snapshot every file under `folder` right now, so the moment is captured exactly.
    pub fn snapshot_folder(&self, folder: &str) -> i64 {
        let root = PathBuf::from(folder);
        let mut files = Vec::new();
        if root.is_file() {
            files.push(root.clone());
        } else {
            walk(&root, &mut files);
        }
        let mut n = 0;
        for f in files {
            if is_ignored(&root, &f) {
                continue;
            }
            if let Ok(Some(_)) = self.save_version(&f) {
                n += 1;
            }
        }
        n
    }

    pub fn add_label(&self, folder: &str, name: &str) -> Result<Label> {
        let key = normalize_path(folder)?;
        let name = name.trim();
        if name.is_empty() {
            return Err(anyhow!("a label needs a name"));
        }
        self.snapshot_folder(&key);
        let now = Utc::now().to_rfc3339();
        let conn = self.db();
        conn.execute(
            "INSERT INTO labels (name, folder, created_at) VALUES (?1, ?2, ?3)",
            params![name, key, now],
        )?;
        Ok(Label {
            id: conn.last_insert_rowid(),
            name: name.to_string(),
            folder: key,
            created_at: now,
        })
    }

    pub fn list_labels(&self, folder: Option<&str>) -> Result<Vec<Label>> {
        let key = folder.map(|f| key_for(Path::new(f)));
        let conn = self.db();
        let mut stmt =
            conn.prepare("SELECT id, name, folder, created_at FROM labels ORDER BY id DESC")?;
        let all = stmt
            .query_map([], |r| {
                Ok(Label {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    folder: r.get(2)?,
                    created_at: r.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(all
            .into_iter()
            .filter(|l| key.as_ref().map(|k| in_folder(&l.folder, k)).unwrap_or(true))
            .collect())
    }

    pub fn delete_label(&self, id: i64) -> Result<()> {
        self.db()
            .execute("DELETE FROM labels WHERE id = ?1", params![id])?;
        Ok(())
    }

    // -- folder rewind -----------------------------------------------------------

    /// Work out what "put this folder back to <when>" would change. Nothing is touched.
    pub fn plan_folder_rewind(&self, folder: &str, to: &str) -> Result<RewindPlan> {
        let when = parse_when(to)?;
        let key = normalize_path(folder)?;
        let rows: Vec<(i64, String, String, String)> = {
            let conn = self.db();
            let mut stmt = conn
                .prepare("SELECT id, path, hash, created_at FROM versions ORDER BY path, id")?;
            let rows = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };

        let earliest = rows
            .iter()
            .filter(|r| in_folder(&r.1, &key))
            .filter_map(|r| parse_ts(&r.3))
            .min();
        match earliest {
            None => return Err(anyhow!("Rewind has no history for this folder yet")),
            Some(e) if when < e => {
                return Err(anyhow!(
                    "Rewind only has history for this folder from {} onwards",
                    e.with_timezone(&Local).format("%Y-%m-%d %H:%M")
                ))
            }
            _ => {}
        }

        let mut items = Vec::new();
        let mut unchanged = 0i64;
        let mut i = 0;
        while i < rows.len() {
            let mut j = i;
            while j < rows.len() && rows[j].1 == rows[i].1 {
                j += 1;
            }
            let group = &rows[i..j];
            i = j;
            let path = group[0].1.clone();
            if !in_folder(&path, &key) {
                continue;
            }
            let at = group
                .iter()
                .rev()
                .find(|r| parse_ts(&r.3).map(|t| t <= when).unwrap_or(false));
            let exists = Path::new(&path).is_file();
            match at {
                None => {
                    if exists {
                        items.push(PlanItem {
                            path,
                            action: "remove".into(),
                            version_id: None,
                            summary: "did not exist at that time".into(),
                        });
                    }
                }
                Some(v) => {
                    if exists {
                        let same = fs::read(&path)
                            .map(|b| blake3::hash(&b).to_hex().to_string() == v.2)
                            .unwrap_or(false);
                        if same {
                            unchanged += 1;
                        } else {
                            items.push(PlanItem {
                                path,
                                action: "restore".into(),
                                version_id: Some(v.0),
                                summary: format!("back to the version saved {}", v.3),
                            });
                        }
                    } else {
                        items.push(PlanItem {
                            path,
                            action: "recreate".into(),
                            version_id: Some(v.0),
                            summary: "was deleted since, will be brought back".into(),
                        });
                    }
                }
            }
        }
        items.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(RewindPlan {
            folder: key,
            to: when.to_rfc3339(),
            items,
            unchanged,
        })
    }

    /// Rewind a folder. A label "Before rewinding to ..." is created first, so this can be undone
    /// by rewinding to that label. Files that did not exist yet are only removed when
    /// `remove_new` is true; their content stays in history either way.
    pub fn apply_folder_rewind(
        &self,
        folder: &str,
        to: &str,
        remove_new: bool,
    ) -> Result<ApplyReport> {
        let plan = self.plan_folder_rewind(folder, to)?;
        let mut rep = ApplyReport::default();
        self.add_label(&plan.folder, &format!("Before rewinding to {}", plan.to))?;
        for it in &plan.items {
            match (it.action.as_str(), it.version_id) {
                ("restore", Some(id)) => match self.restore_version(id) {
                    Ok(_) => rep.restored += 1,
                    Err(e) => rep.failed.push(format!("{}: {e}", it.path)),
                },
                ("recreate", Some(id)) => match self.restore_version(id) {
                    Ok(_) => rep.recreated += 1,
                    Err(e) => rep.failed.push(format!("{}: {e}", it.path)),
                },
                ("remove", _) if remove_new => {
                    let _ = self.save_version(&it.path);
                    match fs::remove_file(&it.path) {
                        Ok(()) => {
                            rep.removed += 1;
                            self.log_deleted(&it.path);
                        }
                        Err(e) => rep.failed.push(format!("{}: {e}", it.path)),
                    }
                }
                _ => {}
            }
        }
        self.emit(&RewindEvent::FolderRewound {
            folder: plan.folder.clone(),
            restored: rep.restored + rep.recreated,
        });
        Ok(rep)
    }

    // -- compare -----------------------------------------------------------------

    /// Diff any two stored versions (they may even belong to different files).
    pub fn diff_versions(&self, old_id: i64, new_id: i64) -> Result<Vec<DiffLine>> {
        let old = self.get_version(old_id)?;
        let new = self.get_version(new_id)?;
        let old_b = fs::read(self.blob_path(&old.hash)).context("stored content is missing")?;
        let new_b = fs::read(self.blob_path(&new.hash)).context("stored content is missing")?;
        let (Ok(o), Ok(n)) = (std::str::from_utf8(&old_b), std::str::from_utf8(&new_b)) else {
            return Ok(vec![DiffLine {
                kind: "hunk".into(),
                text: "binary or non-UTF-8 content, no text diff".into(),
            }]);
        };
        Ok(collapse_context(diff_lines(o, n), 3))
    }

    // -- search ------------------------------------------------------------------

    /// Find versions whose content contains `query` (case-insensitive). Each distinct
    /// content is reported once, newest first.
    pub fn search_history(
        &self,
        query: &str,
        folder: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Err(anyhow!("type something to search for"));
        }
        let key = folder.map(|f| key_for(Path::new(f)));
        let (rows, latest): (Vec<(i64, String, String, String, i64)>, HashMap<String, String>) = {
            let conn = self.db();
            let mut stmt = conn.prepare(
                "SELECT id, path, hash, created_at, size FROM versions ORDER BY id DESC",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(stmt);
            let mut stmt = conn.prepare(
                "SELECT path, hash FROM versions
                 WHERE id IN (SELECT MAX(id) FROM versions GROUP BY path)",
            )?;
            let latest = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
                .collect::<rusqlite::Result<HashMap<_, _>>>()?;
            (rows, latest)
        };

        let mut seen: std::collections::HashSet<(String, String)> = Default::default();
        let mut latest_has: HashMap<String, bool> = HashMap::new();
        let mut hits = Vec::new();
        let mut scanned = 0usize;

        for (id, path, hash, created_at, size) in rows {
            if hits.len() >= limit || scanned >= MAX_SEARCH_SCANNED {
                break;
            }
            if let Some(k) = &key {
                if !in_folder(&path, k) {
                    continue;
                }
            }
            if size > MAX_SEARCH_BLOB || !seen.insert((path.clone(), hash.clone())) {
                continue;
            }
            scanned += 1;
            let Ok(bytes) = fs::read(self.blob_path(&hash)) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(&bytes) else {
                continue;
            };
            let Some((n, line)) = text
                .lines()
                .enumerate()
                .find(|(_, l)| l.to_lowercase().contains(&q))
            else {
                continue;
            };
            let in_latest = *latest_has.entry(path.clone()).or_insert_with(|| {
                latest
                    .get(&path)
                    .and_then(|h| fs::read(self.blob_path(h)).ok())
                    .map(|b| String::from_utf8_lossy(&b).to_lowercase().contains(&q))
                    .unwrap_or(false)
            });
            hits.push(SearchHit {
                version_id: id,
                path,
                created_at,
                line_no: n as i64 + 1,
                snippet: line.trim().chars().take(200).collect(),
                in_latest,
            });
        }
        Ok(hits)
    }

    // -- disk guard ----------------------------------------------------------------

    /// Bytes used by stored content (each distinct blob counted once).
    pub fn store_bytes(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT COALESCE(SUM(size), 0) FROM (SELECT DISTINCT hash, size FROM versions)",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }

    pub fn guard_status(&self) -> GuardStatus {
        let gb: f64 = self
            .kv_get("cfg.disk-guard.max_gb")
            .and_then(|v| v.parse().ok())
            .filter(|v: &f64| *v > 0.0)
            .unwrap_or(5.0);
        let limit = (gb * GIB) as i64;
        let used = self.store_bytes();
        GuardStatus {
            used_bytes: used,
            limit_bytes: limit,
            over: used > limit,
            auto_prune: self.kv_get("cfg.disk-guard.auto_prune").as_deref() == Some("1"),
        }
    }

    /// Check the limit; prune automatically only when the user turned auto-prune on.
    pub fn guard_check(&self) -> GuardStatus {
        let s = self.guard_status();
        if s.over && s.auto_prune {
            let _ = self.shrink_store(s.limit_bytes);
        }
        self.guard_status()
    }

    /// Prune progressively harder until the store fits in `max_bytes` (pinned versions stay).
    pub fn shrink_store(&self, max_bytes: i64) -> Result<PruneReport> {
        let mut total = PruneReport::default();
        for keep in [50usize, 20, 10, 5, 3, 1] {
            if self.store_bytes() <= max_bytes {
                break;
            }
            let r = self.prune(keep, None)?;
            total.versions_removed += r.versions_removed;
            total.blobs_removed += r.blobs_removed;
            total.bytes_freed += r.bytes_freed;
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("rewind-test-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn parse_when_understands_relative_and_absolute() {
        assert!(parse_when("2h").unwrap() < Utc::now());
        assert!(parse_when("1d ago").is_ok());
        assert!(parse_when("2026-11-10T15:00:00Z").is_ok());
        assert!(parse_when("2026-11-10 15:00").is_ok());
        assert!(parse_when("soon").is_err());
    }

    #[test]
    fn pinned_versions_survive_prune() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let f = tmp("work").join("a.txt");
        for i in 0..5 {
            fs::write(&f, format!("v{i}\n")).unwrap();
            core.save_version(&f).unwrap();
        }
        let hist = core.get_history(&f).unwrap();
        let oldest = hist.last().unwrap().id;
        core.pin_version(oldest, true).unwrap();
        core.prune(1, None).unwrap();
        let after = core.get_history(&f).unwrap();
        assert_eq!(after.len(), 2);
        assert!(after.iter().any(|v| v.id == oldest && v.pinned));
    }

    #[test]
    fn search_finds_text_that_was_deleted() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let f = tmp("work").join("cfg.txt");
        fs::write(&f, "name=x\nAPI_KEY=abc123\n").unwrap();
        core.save_version(&f).unwrap();
        fs::write(&f, "name=x\n").unwrap();
        core.save_version(&f).unwrap();
        let hits = core.search_history("api_key", None, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(!hits[0].in_latest);
        assert_eq!(hits[0].line_no, 2);
    }

    #[test]
    fn compare_any_two_versions() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let f = tmp("work").join("a.txt");
        fs::write(&f, "one\n").unwrap();
        let v1 = core.save_version(&f).unwrap().unwrap();
        fs::write(&f, "one\ntwo\n").unwrap();
        core.save_version(&f).unwrap();
        fs::write(&f, "one\ntwo\nthree\n").unwrap();
        let v3 = core.save_version(&f).unwrap().unwrap();
        let d = core.diff_versions(v1.id, v3.id).unwrap();
        assert_eq!(d.iter().filter(|l| l.kind == "add").count(), 2);
    }

    #[test]
    fn deleted_files_listed_and_restorable() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let work = tmp("work");
        let f = work.join("gone.txt");
        fs::write(&f, "keep me\n").unwrap();
        core.save_version(&f).unwrap();
        fs::remove_file(&f).unwrap();
        core.reconcile_deleted(&normalize_path(&work).unwrap());
        let list = core.list_deleted(None).unwrap();
        assert_eq!(list.len(), 1);
        core.restore_deleted(&list[0].path).unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "keep me\n");
        assert!(core.list_deleted(None).unwrap().is_empty());
    }

    #[test]
    fn renamed_file_keeps_its_history() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let work = tmp("work");
        let a = work.join("a.txt");
        let b = work.join("b.txt");
        fs::write(&a, "hello\n").unwrap();
        core.save_version(&a).unwrap();
        fs::rename(&a, &b).unwrap();
        let renamed = core.detect_renames(&[a.clone()], &[b.clone()]);
        assert_eq!(renamed.len(), 1);
        assert_eq!(core.get_history(&b).unwrap().len(), 1);
    }

    #[test]
    fn folder_rewind_plan_and_apply() {
        let core = RewindCore::open(tmp("home")).unwrap();
        let work = tmp("work");
        let f = work.join("a.txt");
        fs::write(&f, "first\n").unwrap();
        core.save_version(&f).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let mid = Utc::now().to_rfc3339();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&f, "second\n").unwrap();
        core.save_version(&f).unwrap();
        let plan = core
            .plan_folder_rewind(work.to_str().unwrap(), &mid)
            .unwrap();
        assert_eq!(plan.items.len(), 1);
        core.apply_folder_rewind(work.to_str().unwrap(), &mid, false)
            .unwrap();
        assert_eq!(fs::read_to_string(&f).unwrap(), "first\n");
        assert_eq!(core.list_labels(None).unwrap().len(), 1);
    }
}
