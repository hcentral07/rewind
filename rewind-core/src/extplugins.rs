//! External plugins, loaded from `<data dir>/plugins/<id>/plugin.toml`.
//!
//! Two kinds, one manifest format (see docs/PLUGINS.md):
//!
//! * **Rule plugins**: no code. `[[rule]]` tables say "when X happens to a file that matches Y,
//!   do Z". Triggers: `file.created`, `file.modified`, `file.deleted`, `file.renamed`,
//!   `version.saved`, `tripwire`, `schedule`. Actions: `move`, `copy`, `delete`, `notify`, `label`, `run`.
//! * **Program plugins**: a `[program]` the engine starts as a child process. It receives events as
//!   JSON lines on stdin and answers with actions as JSON lines on stdout.
//!
//! Safety model: a plugin can only touch files inside the folders it declares, deleting always
//! keeps a copy in Rewind history first, running programs needs an explicit `exec = true`, and a
//! plugin that is merely copied into the folder stays off until the user enables it.
use super::*;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;

const MAX_ACTIONS_PER_MINUTE: u64 = 400;

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Permissions {
    /// Folders the plugin may read and change (`~` is your home folder).
    pub folders: Vec<String>,
    /// Declared for transparency. Rule plugins never get network access; for program plugins this
    /// is shown to the user but cannot be enforced.
    pub network: bool,
    /// Allows `run` actions and program plugins.
    pub exec: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl Default for OneOrMany {
    fn default() -> Self {
        OneOrMany::Many(Vec::new())
    }
}

impl OneOrMany {
    fn list(&self) -> Vec<String> {
        match self {
            OneOrMany::One(s) => vec![s.clone()],
            OneOrMany::Many(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Action {
    #[serde(rename = "move")]
    pub move_to: Option<String>,
    pub copy: Option<String>,
    pub delete: Option<bool>,
    pub notify: Option<String>,
    pub label: Option<String>,
    pub run: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Rule {
    pub when: String,
    #[serde(default, rename = "match")]
    pub matches: OneOrMany,
    pub older_than: Option<String>,
    /// For `when = "schedule"`: how often, e.g. "1h", "1d".
    pub every: Option<String>,
    #[serde(rename = "do")]
    pub action: Action,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Program {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub permissions: Permissions,
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
    pub program: Option<Program>,
}

const WHEN: &[&str] = &[
    "file.created",
    "file.modified",
    "file.deleted",
    "file.renamed",
    "version.saved",
    "tripwire",
    "schedule",
];

/// Parse and validate a manifest. Errors are written for the person installing the plugin.
pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let m: Manifest = toml::from_str(text).map_err(|e| anyhow!("plugin.toml: {e}"))?;
    if m.id.is_empty()
        || m.id.len() > 64
        || !m
            .id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    {
        return Err(anyhow!(
            "plugin id '{}' must be 1-64 characters of a-z, 0-9, - and _",
            m.id
        ));
    }
    if m.name.trim().is_empty() {
        return Err(anyhow!("plugin '{}' needs a name", m.id));
    }
    if m.rules.is_empty() && m.program.is_none() {
        return Err(anyhow!(
            "plugin '{}' has no [[rule]] and no [program]",
            m.id
        ));
    }
    if (m.program.is_some() || m.rules.iter().any(|r| r.action.run.is_some()))
        && !m.permissions.exec
    {
        return Err(anyhow!(
            "plugin '{}' runs programs, so it must say `exec = true` under [permissions]",
            m.id
        ));
    }
    if m.permissions.folders.is_empty() && m.rules.iter().any(|r| r.when != "tripwire") {
        return Err(anyhow!(
            "plugin '{}' must list the folders it works on under [permissions]",
            m.id
        ));
    }
    for (i, r) in m.rules.iter().enumerate() {
        let n = i + 1;
        if !WHEN.contains(&r.when.as_str()) {
            return Err(anyhow!(
                "rule {n}: unknown trigger '{}' (use one of: {})",
                r.when,
                WHEN.join(", ")
            ));
        }
        let a = &r.action;
        let chosen = [
            a.move_to.is_some(),
            a.copy.is_some(),
            a.delete == Some(true),
            a.notify.is_some(),
            a.label.is_some(),
            a.run.is_some(),
        ]
        .iter()
        .filter(|x| **x)
        .count();
        if chosen != 1 {
            return Err(anyhow!(
                "rule {n}: `do` must contain exactly one of move, copy, delete, notify, label, run"
            ));
        }
        let file_action = a.move_to.is_some() || a.copy.is_some() || a.delete == Some(true);
        if file_action && matches!(r.when.as_str(), "file.deleted" | "tripwire") {
            return Err(anyhow!(
                "rule {n}: move, copy and delete need a file that still exists; use them with file.created, file.modified, file.renamed, version.saved or schedule"
            ));
        }
        if r.when == "schedule" {
            match r.every.as_deref().and_then(parse_span) {
                Some(d) if d >= chrono::Duration::minutes(1) => {}
                _ => {
                    return Err(anyhow!(
                        "rule {n}: a schedule rule needs `every`, like \"1h\" (at least 1m)"
                    ))
                }
            }
        }
        if let Some(o) = &r.older_than {
            if parse_span(o).is_none() {
                return Err(anyhow!(
                    "rule {n}: cannot understand older_than = \"{o}\" (try 30d, 12h, 45m)"
                ));
            }
        }
    }
    Ok(m)
}

/// `90s`, `45m`, `12h`, `30d`, `2w`.
pub fn parse_span(s: &str) -> Option<chrono::Duration> {
    let s = s.trim();
    if !s.is_ascii() || s.len() < 2 {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: i64 = num.parse().ok()?;
    if n <= 0 {
        return None;
    }
    match unit {
        "s" => Some(chrono::Duration::seconds(n)),
        "m" => Some(chrono::Duration::minutes(n)),
        "h" => Some(chrono::Duration::hours(n)),
        "d" => Some(chrono::Duration::days(n)),
        "w" => Some(chrono::Duration::weeks(n)),
        _ => None,
    }
}

fn permission_lines(m: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    if !m.permissions.folders.is_empty() {
        let list = m.permissions.folders.join(", ");
        let any = |f: &dyn Fn(&Action) -> bool| m.rules.iter().any(|r| f(&r.action));
        if any(&|a| a.move_to.is_some()) {
            out.push(format!("Move files inside: {list}"));
        }
        if any(&|a| a.copy.is_some()) {
            out.push(format!("Copy files inside: {list}"));
        }
        if any(&|a| a.delete == Some(true)) {
            out.push(format!(
                "Delete files inside: {list} (a copy stays in Rewind history; files over 25 MB go to Rewind's trash for 30 days)"
            ));
        }
        if any(&|a| a.label.is_some()) {
            out.push(format!("Save labelled checkpoints of: {list}"));
        }
        out.push(format!("Read file names and changes inside: {list}"));
    }
    if m.rules.iter().any(|r| r.action.notify.is_some()) {
        out.push("Show notifications".into());
    }
    if m.permissions.exec {
        out.push("Run programs on your computer".into());
    }
    if m.permissions.network {
        out.push("Says it uses the network (Rewind cannot enforce this for programs)".into());
    }
    out
}

// ---------------------------------------------------------------------------
// Paths and patterns
// ---------------------------------------------------------------------------

fn expand_home(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix('~') {
        if rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\') {
            if let Some(h) = home_dir() {
                return h.join(rest.trim_start_matches(['/', '\\']));
            }
        }
    }
    PathBuf::from(p)
}

/// Canonicalize the deepest part of `p` that exists and append the rest, so folders that do
/// not exist yet can still be compared with the permitted folders.
fn resolve_loose(p: &Path) -> String {
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = p.to_path_buf();
    loop {
        if cur.exists() {
            let mut base = PathBuf::from(
                normalize_path(&cur).unwrap_or_else(|_| cur.to_string_lossy().to_string()),
            );
            for part in rest.iter().rev() {
                if part == ".." {
                    base.pop();
                } else if part != "." {
                    base.push(part);
                }
            }
            return base.to_string_lossy().to_string();
        }
        match (cur.file_name().map(|s| s.to_os_string()), cur.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                cur = parent.to_path_buf();
            }
            _ => return p.to_string_lossy().to_string(),
        }
    }
}

/// `*` and `?`, case-insensitive.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

fn fill_placeholders(template: &str, file: &Path) -> String {
    let now = chrono::Local::now();
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let stem = file
        .file_stem()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = file
        .extension()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    template
        .replace("{year}", &now.format("%Y").to_string())
        .replace("{month}", &now.format("%m").to_string())
        .replace("{day}", &now.format("%d").to_string())
        .replace("{ext}", &ext)
        .replace("{name}", &name)
        .replace("{stem}", &stem)
        .replace("{path}", &file.to_string_lossy())
}

fn free_name(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let ext = p
        .extension()
        .map(|s| format!(".{}", s.to_string_lossy()))
        .unwrap_or_default();
    for i in 1..1000 {
        let c = dir.join(format!("{stem} ({i}){ext}"));
        if !c.exists() {
            return c;
        }
    }
    dir.join(format!("{stem}-{}{ext}", Uuid::new_v4()))
}

// ---------------------------------------------------------------------------
// The plugin
// ---------------------------------------------------------------------------

/// What happened, in the shape rules care about.
struct Trigger {
    when: &'static str,
    path: Option<String>,
    folder: Option<String>,
}

struct ProgramHost {
    child: Child,
    stdin: ChildStdin,
}

struct ExternalPlugin {
    manifest: Manifest,
    dir: PathBuf,
    folders: Vec<String>,
    /// Destinations this plugin just wrote, so its own output does not trigger it again.
    own: Mutex<HashMap<String, Instant>>,
    budget: Mutex<(Instant, u64)>,
    host: Mutex<Option<ProgramHost>>,
    last_spawn: Mutex<Option<Instant>>,
    warned_budget: AtomicBool,
    id_s: String,
    name_s: String,
    desc_s: String,
}

impl ExternalPlugin {
    fn new(manifest: Manifest, dir: PathBuf) -> Self {
        let folders = manifest
            .permissions
            .folders
            .iter()
            .map(|f| resolve_loose(&expand_home(f)))
            .collect();
        Self {
            id_s: manifest.id.clone(),
            name_s: manifest.name.clone(),
            desc_s: manifest.description.clone(),
            manifest,
            dir,
            folders,
            own: Mutex::new(HashMap::new()),
            budget: Mutex::new((Instant::now(), 0)),
            host: Mutex::new(None),
            last_spawn: Mutex::new(None),
            warned_budget: AtomicBool::new(false),
        }
    }

    fn allowed(&self, path: &Path) -> bool {
        let p = resolve_loose(path);
        self.folders.iter().any(|f| in_folder(&p, f))
    }

    fn spend(&self, core: &RewindCore) -> bool {
        let mut b = self.budget.lock().unwrap_or_else(|e| e.into_inner());
        if b.0.elapsed() > Duration::from_secs(60) {
            *b = (Instant::now(), 0);
            self.warned_budget.store(false, Ordering::Relaxed);
        }
        b.1 += 1;
        if b.1 > MAX_ACTIONS_PER_MINUTE {
            if !self.warned_budget.swap(true, Ordering::Relaxed) {
                let _ = core.raise_alert(
                    "plugin",
                    &format!("{} paused for a minute", self.name_s),
                    "The plugin tried more than 400 actions in a minute, so Rewind stopped it for now. Check its rules.",
                    "",
                    None,
                    0,
                );
            }
            return false;
        }
        true
    }

    fn is_own(&self, path: &str) -> bool {
        let mut own = self.own.lock().unwrap_or_else(|e| e.into_inner());
        own.retain(|_, t| t.elapsed() < Duration::from_secs(10));
        own.contains_key(path)
    }

    fn mark_own(&self, path: &Path) {
        self.own
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(resolve_loose(path), Instant::now());
    }

    fn file_matches(&self, rule: &Rule, file: &Path) -> bool {
        let pats = rule.matches.list();
        if !pats.is_empty() {
            let name = file
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let full = file.to_string_lossy().replace('\\', "/");
            let ok = pats.iter().any(|p| {
                if p.contains('/') || p.contains('\\') {
                    glob_match(&p.replace('\\', "/"), &full)
                } else {
                    glob_match(p, &name)
                }
            });
            if !ok {
                return false;
            }
        }
        if let Some(span) = rule.older_than.as_deref().and_then(parse_span) {
            let Ok(meta) = fs::metadata(file) else {
                return false;
            };
            let Ok(modified) = meta.modified() else {
                return false;
            };
            let age = std::time::SystemTime::now()
                .duration_since(modified)
                .unwrap_or_default();
            if age < span.to_std().unwrap_or_default() {
                return false;
            }
        }
        true
    }

    /// Run one rule's action. `file` is set for file triggers.
    fn act(&self, core: &RewindCore, rule: &Rule, file: Option<&Path>, folder: Option<&str>) {
        if !self.spend(core) {
            return;
        }
        let a = &rule.action;

        if let Some(text) = &a.notify {
            let msg = match file {
                Some(f) => fill_placeholders(text, f),
                None => text.clone(),
            };
            let _ = core.raise_alert("plugin", &self.name_s, &msg, folder.unwrap_or(""), None, 0);
            return;
        }
        if let Some(label) = &a.label {
            // schedule rules label every permitted folder that exists; event rules label the folder
            let targets: Vec<String> = match folder {
                Some(f) => vec![f.to_string()],
                None => self.folders.clone(),
            };
            for t in targets {
                if Path::new(&t).is_dir() {
                    let _ = core.add_label(&t, label);
                }
            }
            return;
        }
        if let Some(cmd) = &a.run {
            if !self.manifest.permissions.exec || cmd.is_empty() {
                return;
            }
            let path_s = file
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            let args: Vec<String> = cmd[1..]
                .iter()
                .map(|x| x.replace("{path}", &path_s))
                .collect();
            let prog = cmd[0].replace("{path}", &path_s);
            let dir = self.dir.clone();
            thread::spawn(move || {
                let _ = Command::new(prog).args(args).current_dir(dir).status();
            });
            return;
        }

        let Some(file) = file else { return };
        if !file.is_file() || !self.allowed(file) {
            return;
        }
        // whatever happens next, the content is in history first
        let _ = core.save_version(file);

        if a.delete == Some(true) {
            // files too big for the history go to Rewind's own trash instead of vanishing
            let big = fs::metadata(file)
                .map(|m| m.len() > MAX_FILE_BYTES)
                .unwrap_or(false);
            let gone = if big {
                let dir = core
                    .data_dir()
                    .join("trash")
                    .join(&self.id_s)
                    .join(Utc::now().format("%Y%m%d-%H%M%S").to_string());
                let name = file
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                fs::create_dir_all(&dir).is_ok() && {
                    let t = free_name(&dir, &name);
                    fs::rename(file, &t).is_ok()
                        || (fs::copy(file, &t).is_ok() && fs::remove_file(file).is_ok())
                }
            } else {
                fs::remove_file(file).is_ok()
            };
            if gone {
                core.log_deleted(&resolve_loose(file));
            }
            return;
        }
        let (dest_t, is_move) = match (&a.move_to, &a.copy) {
            (Some(d), _) => (d, true),
            (None, Some(d)) => (d, false),
            _ => return,
        };
        let dest_dir = expand_home(&fill_placeholders(dest_t, file));
        if !self.allowed(&dest_dir) {
            let _ = core.raise_alert(
                "plugin",
                &self.name_s,
                &format!(
                    "Blocked: {} is outside the folders this plugin may use.",
                    dest_dir.display()
                ),
                "",
                None,
                0,
            );
            return;
        }
        // already where it belongs
        if resolve_loose(file.parent().unwrap_or(file)) == resolve_loose(&dest_dir) {
            return;
        }
        if fs::create_dir_all(&dest_dir).is_err() {
            return;
        }
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let target = free_name(&dest_dir, &name);
        self.mark_own(&target);
        if is_move {
            let moved = fs::rename(file, &target).is_ok()
                || (fs::copy(file, &target).is_ok() && fs::remove_file(file).is_ok());
            if moved {
                core.log_deleted(&resolve_loose(file));
            }
        } else {
            let _ = fs::copy(file, &target);
        }
    }

    fn run_rules(&self, core: &RewindCore, t: &Trigger) {
        for rule in self.manifest.rules.iter().filter(|r| r.when == t.when) {
            match &t.path {
                Some(p) => {
                    let path = Path::new(p);
                    if !self.allowed(path) || self.is_own(&resolve_loose(path)) {
                        continue;
                    }
                    if rule.when != "file.deleted" && !self.file_matches(rule, path) {
                        continue;
                    }
                    self.act(core, rule, Some(path), t.folder.as_deref());
                }
                None => self.act(core, rule, None, t.folder.as_deref()),
            }
        }
    }

    // -- program plugins --------------------------------------------------------

    fn ensure_host(&self, core: &RewindCore) -> bool {
        let Some(prog) = &self.manifest.program else {
            return false;
        };
        let mut host = self.host.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = host.as_mut() {
            if matches!(h.child.try_wait(), Ok(None)) {
                return true;
            }
            *host = None;
        }
        let mut last = self.last_spawn.lock().unwrap_or_else(|e| e.into_inner());
        if last
            .map(|t| t.elapsed() < Duration::from_secs(30))
            .unwrap_or(false)
        {
            return false;
        }
        *last = Some(Instant::now());
        let mut cmd = Command::new(&prog.command);
        cmd.args(&prog.args)
            .current_dir(&self.dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let Ok(mut child) = cmd.spawn() else {
            return false;
        };
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return false;
        };
        let core2 = core.clone();
        let folders = self.folders.clone();
        let name = self.name_s.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(|l| l.ok()) {
                handle_program_action(&core2, &folders, &name, &line);
            }
        });
        *host = Some(ProgramHost { child, stdin });
        true
    }

    fn send(&self, core: &RewindCore, json: serde_json::Value) {
        if !self.ensure_host(core) {
            return;
        }
        let mut host = self.host.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = host.as_mut() {
            let _ = writeln!(h.stdin, "{json}");
            let _ = h.stdin.flush();
        }
    }
}

fn handle_program_action(core: &RewindCore, folders: &[String], plugin: &str, line: &str) {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };
    let ok_path = |p: &str| {
        let r = resolve_loose(Path::new(p));
        folders.iter().any(|f| in_folder(&r, f))
    };
    match v.get("action").and_then(|a| a.as_str()) {
        Some("notify") => {
            let text = v.get("text").and_then(|t| t.as_str()).unwrap_or("");
            let _ = core.raise_alert("plugin", plugin, text, "", None, 0);
        }
        Some("snapshot") => {
            if let Some(p) = v.get("path").and_then(|p| p.as_str()) {
                if ok_path(p) {
                    let _ = core.save_version(p);
                }
            }
        }
        Some("label") => {
            let (f, n) = (
                v.get("folder").and_then(|x| x.as_str()),
                v.get("name").and_then(|x| x.as_str()),
            );
            if let (Some(f), Some(n)) = (f, n) {
                if ok_path(f) {
                    let _ = core.add_label(f, n);
                }
            }
        }
        _ => {}
    }
}

impl Drop for ExternalPlugin {
    fn drop(&mut self) {
        if let Some(h) = self.host.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            let _ = h.child.kill();
        }
    }
}

impl Plugin for ExternalPlugin {
    fn id(&self) -> &str {
        &self.id_s
    }
    fn name(&self) -> &str {
        &self.name_s
    }
    fn description(&self) -> &str {
        &self.desc_s
    }
    /// A plugin that was only copied into the folder stays off until the user turns it on.
    fn default_enabled(&self) -> bool {
        false
    }
    fn is_builtin(&self) -> bool {
        false
    }
    fn details(&self) -> PluginDetails {
        PluginDetails {
            version: self.manifest.version.clone(),
            kind: if self.manifest.program.is_some() {
                "program"
            } else {
                "rules"
            }
            .into(),
            permissions: permission_lines(&self.manifest),
            rules: self.manifest.rules.len(),
        }
    }

    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {
        let trig = match event {
            RewindEvent::VersionSaved(v) => {
                let when = if v.summary.starts_with("Created") {
                    "file.created"
                } else {
                    "file.modified"
                };
                if self.manifest.program.is_some() {
                    self.send(
                        core,
                        serde_json::json!({"event": "version.saved", "path": v.path, "hash": v.hash,
                            "size": v.size, "created": when == "file.created"}),
                    );
                }
                // `version.saved` rules fire alongside the specific one
                self.run_rules(
                    core,
                    &Trigger {
                        when: "version.saved",
                        path: Some(v.path.clone()),
                        folder: None,
                    },
                );
                Trigger {
                    when,
                    path: Some(v.path.clone()),
                    folder: None,
                }
            }
            RewindEvent::FileDeleted { path } => {
                if self.manifest.program.is_some() {
                    self.send(
                        core,
                        serde_json::json!({"event": "file.deleted", "path": path}),
                    );
                }
                Trigger {
                    when: "file.deleted",
                    path: Some(path.clone()),
                    folder: None,
                }
            }
            RewindEvent::FileRenamed { from, to } => {
                if self.manifest.program.is_some() {
                    self.send(
                        core,
                        serde_json::json!({"event": "file.renamed", "from": from, "to": to}),
                    );
                }
                Trigger {
                    when: "file.renamed",
                    path: Some(to.clone()),
                    folder: None,
                }
            }
            RewindEvent::Alert(a) if a.kind == "tripwire" => {
                if self.manifest.program.is_some() {
                    self.send(
                        core,
                        serde_json::json!({"event": "tripwire", "folder": a.folder, "count": a.count}),
                    );
                }
                Trigger {
                    when: "tripwire",
                    path: None,
                    folder: Some(a.folder.clone()),
                }
            }
            RewindEvent::FolderRewound { folder, restored } => {
                if self.manifest.program.is_some() {
                    self.send(
                        core,
                        serde_json::json!({"event": "folder.rewound", "folder": folder, "restored": restored}),
                    );
                }
                return;
            }
            _ => return,
        };
        self.run_rules(core, &trig);
    }

    fn on_tick(&self, core: &RewindCore, force: bool) {
        for (i, rule) in self.manifest.rules.iter().enumerate() {
            if rule.when != "schedule" {
                continue;
            }
            let key = format!("plugin.{}.last.{}", self.id_s, i);
            let every = rule
                .every
                .as_deref()
                .and_then(parse_span)
                .unwrap_or_else(|| chrono::Duration::days(1));
            let due = force
                || core
                    .kv_get(&key)
                    .and_then(|s| parse_ts(&s))
                    .map(|t| Utc::now() - t >= every)
                    .unwrap_or(true);
            if !due {
                continue;
            }
            let _ = core.kv_set(&key, &Utc::now().to_rfc3339());

            if rule.action.label.is_some()
                || rule.action.notify.is_some()
                || rule.action.run.is_some()
            {
                self.act(core, rule, None, None);
                continue;
            }
            for folder in &self.folders {
                let mut files = Vec::new();
                walk_all(Path::new(folder), &mut files, 0);
                for f in files.into_iter().take(50_000) {
                    if self.file_matches(rule, &f) && !self.is_own(&resolve_loose(&f)) {
                        self.act(core, rule, Some(&f), Some(folder));
                    }
                }
            }
        }
    }
}

/// Like `walk`, but also enters ignored-by-Rewind folders only if they are not VCS or build
/// folders, and stops at a sane depth.
fn walk_all(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 12 {
        return;
    }
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        let p = entry.path();
        if ft.is_dir() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if IGNORED_DIRS.iter().any(|d| name == *d) {
                continue;
            }
            walk_all(&p, out, depth + 1);
        } else if ft.is_file() {
            out.push(p);
        }
    }
}

// ---------------------------------------------------------------------------
// Loading, installing, starters
// ---------------------------------------------------------------------------

/// Plugins that ship inside the program: `rewind plugin add <name>`.
pub const STARTER_PLUGINS: &[(&str, &str)] = &[
    (
        "downloads-organizer",
        include_str!("../starters/downloads-organizer.toml"),
    ),
    (
        "screenshot-sorter",
        include_str!("../starters/screenshot-sorter.toml"),
    ),
    (
        "game-save-protector",
        include_str!("../starters/game-save-protector.toml"),
    ),
    (
        "installer-cleaner",
        include_str!("../starters/installer-cleaner.toml"),
    ),
];

impl RewindCore {
    pub fn plugins_dir(&self) -> PathBuf {
        self.data_dir().join("plugins")
    }

    /// Problems found the last time plugins were loaded (bad manifests and so on).
    pub fn plugin_load_errors(&self) -> Vec<String> {
        self.plugin_errors
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// (Re)read every `plugins/<id>/plugin.toml`. Returns the problems found.
    pub fn reload_external_plugins(&self) -> Vec<String> {
        self.drop_external_plugins();
        let mut errors = Vec::new();
        let builtin: Vec<String> = self
            .plugin_list()
            .iter()
            .map(|p| p.id().to_string())
            .collect();
        let mut seen: Vec<String> = Vec::new();
        if let Ok(rd) = fs::read_dir(self.plugins_dir()) {
            let mut dirs: Vec<PathBuf> = rd
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            dirs.sort();
            for dir in dirs {
                let file = dir.join("plugin.toml");
                let dname = dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                let text = match fs::read_to_string(&file) {
                    Ok(t) => t,
                    Err(_) => {
                        errors.push(format!("{dname}: no plugin.toml"));
                        continue;
                    }
                };
                match parse_manifest(&text) {
                    Ok(m) => {
                        if builtin.contains(&m.id) || seen.contains(&m.id) {
                            errors.push(format!("{dname}: the id '{}' is already taken", m.id));
                            continue;
                        }
                        seen.push(m.id.clone());
                        self.register_plugin(Arc::new(ExternalPlugin::new(m, dir.clone())));
                    }
                    Err(e) => errors.push(format!("{dname}: {e}")),
                }
            }
        }
        *self.plugin_errors.lock().unwrap_or_else(|e| e.into_inner()) = errors.clone();
        errors
    }

    /// Check a plugin folder (or a single plugin.toml) without installing it.
    pub fn inspect_plugin(&self, src: &Path) -> Result<PluginInfo> {
        let file = if src.is_dir() {
            src.join("plugin.toml")
        } else {
            src.to_path_buf()
        };
        let text =
            fs::read_to_string(&file).with_context(|| format!("cannot read {}", file.display()))?;
        manifest_info(&parse_manifest(&text)?)
    }

    /// Copy a plugin folder (or a lone plugin.toml) into the plugins folder. The new plugin is
    /// switched on, because installing it is the user saying yes; the caller shows the
    /// permissions from `inspect_plugin` first.
    pub fn install_plugin(&self, src: &Path) -> Result<PluginInfo> {
        let info = self.inspect_plugin(src)?;
        self.install_manifest_files(&info.id, src)?;
        Ok(info)
    }

    fn install_manifest_files(&self, id: &str, src: &Path) -> Result<()> {
        let dest = self.plugins_dir().join(id);
        if dest.exists() {
            fs::remove_dir_all(&dest)?;
        }
        fs::create_dir_all(&dest)?;
        if src.is_dir() {
            copy_dir(src, &dest)?;
        } else {
            fs::copy(src, dest.join("plugin.toml"))?;
        }
        let errors = self.reload_external_plugins();
        if let Some(e) = errors.iter().find(|e| e.starts_with(id)) {
            return Err(anyhow!("{e}"));
        }
        self.set_plugin_enabled(id, true)
    }

    /// Install one of the plugins that ship with Rewind.
    pub fn install_starter_plugin(&self, name: &str) -> Result<PluginInfo> {
        let (_, text) = STARTER_PLUGINS
            .iter()
            .find(|(n, _)| *n == name)
            .ok_or_else(|| anyhow!("no bundled plugin called '{name}'"))?;
        let info = manifest_info(&parse_manifest(text)?)?;
        let tmp = std::env::temp_dir().join(format!("rewind-plugin-{}.toml", Uuid::new_v4()));
        fs::write(&tmp, text)?;
        let r = self.install_manifest_files(&info.id, &tmp);
        let _ = fs::remove_file(&tmp);
        r?;
        Ok(info)
    }

    /// Name, description and permissions of the bundled plugins.
    pub fn starter_plugin_infos(&self) -> Vec<PluginInfo> {
        STARTER_PLUGINS
            .iter()
            .filter_map(|(_, t)| parse_manifest(t).ok())
            .filter_map(|m| manifest_info(&m).ok())
            .collect()
    }

    pub fn remove_plugin(&self, id: &str) -> Result<()> {
        let dir = self.plugins_dir().join(id);
        if !dir.join("plugin.toml").exists() {
            return Err(anyhow!("'{id}' is not an installed external plugin"));
        }
        fs::remove_dir_all(&dir)?;
        self.reload_external_plugins();
        Ok(())
    }

    /// Empty plugin-trash batches older than `days`.
    pub fn purge_plugin_trash(&self, days: u64) {
        let root = self.data_dir().join("trash");
        let limit = Duration::from_secs(days * 86_400);
        let Ok(plugins) = fs::read_dir(&root) else {
            return;
        };
        for plugin in plugins.flatten() {
            let Ok(batches) = fs::read_dir(plugin.path()) else {
                continue;
            };
            for batch in batches.flatten() {
                let old = batch
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|e| e > limit)
                    .unwrap_or(false);
                if old {
                    let _ = fs::remove_dir_all(batch.path());
                }
            }
        }
    }

    /// Run the scheduled rules every 30 seconds until the handle is dropped. Rules keep their
    /// own "every" interval, so this just checks which ones are due.
    pub fn start_scheduler(&self) -> SchedulerHandle {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let core = self.clone();
        let last_run = Arc::new(AtomicU64::new(0));
        thread::spawn(move || {
            let _ = &last_run;
            let mut waited = 0u64;
            while !flag.load(Ordering::Relaxed) {
                if waited.is_multiple_of(30) {
                    core.run_plugin_schedules(false);
                }
                if waited.is_multiple_of(3600) {
                    core.purge_plugin_trash(30);
                }
                thread::sleep(Duration::from_secs(1));
                waited += 1;
            }
        });
        SchedulerHandle { stop }
    }
}

/// Keep alive to keep the scheduler running.
pub struct SchedulerHandle {
    stop: Arc<AtomicBool>,
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn manifest_info(m: &Manifest) -> Result<PluginInfo> {
    Ok(PluginInfo {
        id: m.id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        enabled: false,
        builtin: false,
        version: m.version.clone(),
        kind: if m.program.is_some() {
            "program"
        } else {
            "rules"
        }
        .into(),
        permissions: permission_lines(m),
        rules: m.rules.len(),
    })
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    for entry in fs::read_dir(from)?.flatten() {
        let p = entry.path();
        let dest = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir_all(&dest)?;
            copy_dir(&p, &dest)?;
        } else {
            fs::copy(&p, &dest)?;
        }
    }
    Ok(())
}
