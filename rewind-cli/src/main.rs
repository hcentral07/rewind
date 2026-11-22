mod mcp;

use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use clap::{CommandFactory, Parser, Subcommand};
use rewind_core::{AgentReport, Alert, PluginInfo, RewindCore};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "rewind",
    version,
    about = "Rewind - a time machine for your files"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show watched folders and storage stats
    Status {
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Show the version history of a file
    Log {
        file: PathBuf,
        /// Print machine-readable JSON
        #[arg(long)]
        json: bool,
    },
    /// Friendly history of a file or folder (this is what the right-click menu opens)
    History { path: PathBuf },
    /// Watch a folder and snapshot every change (Ctrl+C to stop)
    Watch { path: PathBuf },
    /// Watch every remembered folder in the background-friendly way (for services)
    WatchAll,
    /// Restore a file to a version number shown by `rewind log`
    Restore { file: PathBuf, version: i64 },
    /// Show what a version changed (default: the newest) against the one before it
    Diff { file: PathBuf, version: Option<i64> },
    /// Delete old history to reclaim disk space
    Prune {
        /// Always keep this many newest versions per file
        #[arg(long, default_value_t = 50)]
        keep: usize,
        /// Only delete versions older than this many days
        #[arg(long)]
        older_than: Option<i64>,
    },
    /// Find the version of any file that contained some text
    Search {
        text: String,
        /// Only look inside this folder
        #[arg(long)]
        folder: Option<PathBuf>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    /// List files that vanished from watched folders (restore with --restore <path>)
    Deleted {
        #[arg(long)]
        folder: Option<PathBuf>,
        /// Bring this deleted file back
        #[arg(long)]
        restore: Option<PathBuf>,
    },
    /// Pin a version so prune never deletes it (use --off to unpin)
    Pin {
        file: PathBuf,
        version: i64,
        #[arg(long)]
        off: bool,
    },
    /// Name a moment for a folder, or list labels
    Label {
        folder: PathBuf,
        /// Label name; omit to list the folder's labels
        name: Option<String>,
    },
    /// Put a whole folder back to how it was at a time (2h, 1d, "2026-11-10 15:00")
    RewindFolder {
        folder: PathBuf,
        to: String,
        /// Actually do it; without this only the plan is shown
        #[arg(long)]
        apply: bool,
        /// Also remove files that did not exist yet (their content stays in history)
        #[arg(long)]
        remove_new: bool,
    },
    /// Diff two versions of a file (numbers from `rewind log`)
    Compare { file: PathBuf, from: i64, to: i64 },

    /// Run a command (an AI coding agent, a script, an installer) and report what it changed
    Run {
        /// Folder to protect (default: the current folder)
        #[arg(long)]
        folder: Option<PathBuf>,
        /// Name shown in the run list (default: the command)
        #[arg(long)]
        name: Option<String>,
        #[arg(trailing_var_arg = true, required = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Review and undo agent runs
    Agent {
        #[command(subcommand)]
        action: AgentCmd,
    },
    /// Save a labelled checkpoint (use --every in hooks to avoid saving too often)
    Checkpoint {
        /// Folder to checkpoint (default: the current folder)
        folder: Option<PathBuf>,
        #[arg(long)]
        name: Option<String>,
        /// Skip if a checkpoint was saved in the last N seconds
        #[arg(long, default_value_t = 0)]
        every: u64,
    },
    /// Show the mass-change tripwire settings and recent alerts, or change the settings
    Tripwire {
        #[command(subcommand)]
        action: Option<TripwireCmd>,
    },
    /// List alerts (tripwire, plugins). --ack marks them as seen
    Alerts {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        ack: bool,
    },
    /// Undo a tripwire alert: put its folder back to just before the burst
    UndoAlert {
        id: i64,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        remove_new: bool,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginCmd,
    },
    /// Right-click "Rewind: show history" in your file manager
    Shell {
        #[command(subcommand)]
        action: ShellCmd,
    },
    /// Run Rewind as a background service (Linux: systemd user service)
    Service {
        #[command(subcommand)]
        action: ServiceCmd,
    },
    /// Start the MCP server so AI agents can checkpoint and undo (stdio)
    Mcp,
    /// Print shell completions (bash, zsh, fish, powershell, elvish)
    Completions { shell: clap_complete::Shell },
}

#[derive(Subcommand)]
enum AgentCmd {
    /// List recent runs
    List,
    /// What a run changed (default: the latest)
    Report { id: Option<i64> },
    /// Put the folder back to how it was when the run started
    Undo {
        id: Option<i64>,
        #[arg(long)]
        apply: bool,
        /// Also remove files the agent created (their content stays in history)
        #[arg(long)]
        remove_new: bool,
    },
}

#[derive(Subcommand)]
enum TripwireCmd {
    /// Change the thresholds
    Set {
        /// Files changed within the window that trip the wire
        #[arg(long)]
        files: Option<usize>,
        /// Window length in seconds
        #[arg(long)]
        seconds: Option<u64>,
    },
}

#[derive(Subcommand)]
enum PluginCmd {
    /// List installed plugins and the ones that ship with Rewind
    List,
    /// Install a bundled plugin by name, or one from a folder / plugin.toml
    Add {
        source: String,
        /// Skip the permission question
        #[arg(long, short)]
        yes: bool,
    },
    /// Check a plugin without installing it
    Inspect {
        source: String,
    },
    Remove {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    /// Run scheduled rules now instead of waiting for their interval
    Run,
}

#[derive(Subcommand)]
enum ShellCmd {
    Install {
        /// Path of the Rewind app (default: looked up next to this program)
        #[arg(long)]
        gui: Option<PathBuf>,
    },
    Uninstall,
    Status,
}

#[derive(Subcommand)]
enum ServiceCmd {
    /// Write the service file (add --enable to start it now)
    Install {
        #[arg(long)]
        enable: bool,
    },
    Uninstall,
}

fn fmt_time(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .map(|d| {
            d.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|_| iso.to_string())
}

fn print_alert(a: &Alert) {
    eprintln!("[ALERT] {}: {}", a.title, a.message);
}

pub fn render_report(r: &AgentReport) -> String {
    let mut out = format!(
        "run {} ({}) in {}\n  started {}{}\n  {} created, {} modified, {} deleted  (+{} -{} lines)\n",
        r.run.id,
        r.run.agent,
        r.run.folder,
        fmt_time(&r.run.started_at),
        match &r.run.ended_at {
            Some(e) => format!(", ended {}", fmt_time(e)),
            None => ", still running".to_string(),
        },
        r.created,
        r.modified,
        r.deleted,
        r.lines_added,
        r.lines_deleted
    );
    for f in &r.flags {
        out.push_str(&format!("  ! {f}\n"));
    }
    for c in r.changes.iter().take(60) {
        out.push_str(&format!(
            "  {:<9} {}  (+{} -{})\n",
            c.action, c.path, c.lines_added, c.lines_deleted
        ));
    }
    if r.changes.len() > 60 {
        out.push_str(&format!("  ... and {} more\n", r.changes.len() - 60));
    }
    out
}

fn confirm(question: &str) -> bool {
    if !std::io::stdin().is_terminal() {
        return false;
    }
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
}

fn sibling_exe(name: &str) -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let dir = me.parent()?;
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let p = dir.join(file);
    p.exists().then_some(p)
}

fn print_plugin(p: &PluginInfo) {
    println!(
        "{} {:<22} {}{}",
        if p.enabled { "on " } else { "off" },
        p.id,
        p.name,
        if p.builtin { "  (built in)" } else { "" }
    );
}

fn print_permissions(info: &PluginInfo) {
    println!("{} {} ({})", info.name, info.version, info.kind);
    if !info.description.is_empty() {
        println!("  {}", info.description);
    }
    println!("  This plugin will be able to:");
    for l in &info.permissions {
        println!("    - {l}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // `rewind log | head` should end quietly, not with a panic
    std::panic::set_hook(Box::new(|info| {
        let m = info.to_string();
        if m.contains("Broken pipe") {
            std::process::exit(0);
        }
        eprintln!("{m}");
    }));
    let cli = Cli::parse();
    if let Commands::Completions { shell } = &cli.command {
        clap_complete::generate(
            *shell,
            &mut Cli::command(),
            "rewind",
            &mut std::io::stdout(),
        );
        return Ok(());
    }
    let core = RewindCore::open_default()?;
    core.set_alert_handler(print_alert);

    match cli.command {
        Commands::Completions { .. } => {}

        Commands::Status { json } => {
            let st = core.get_status()?;
            if json {
                println!("{}", serde_json::to_string_pretty(&st)?);
                return Ok(());
            }
            println!("rewind");
            println!("  data dir : {}", st.data_dir);
            println!("  versions : {}", st.total_versions);
            println!("  files    : {}", st.total_files);
            if st.watches.is_empty() {
                println!("  watching : nothing yet - try `rewind watch <path>`");
            } else {
                println!("  watching :");
                for w in &st.watches {
                    println!(
                        "    {}  ({} versions, {} files)",
                        w.path, w.versions, w.files
                    );
                }
            }
            let unseen = core.list_alerts(true, 5)?;
            if !unseen.is_empty() {
                println!("  alerts   : {} unseen - run `rewind alerts`", unseen.len());
            }
        }

        Commands::Log { file, json } => {
            let history = core.get_history(&file)?;
            if json {
                println!("{}", serde_json::to_string_pretty(&history)?);
                return Ok(());
            }
            if history.is_empty() {
                bail!(
                    "no history for {} - is it inside a watched folder?",
                    file.display()
                );
            }
            println!("{}", history[0].path);
            for v in &history {
                println!(
                    "{} v{:<4} {}  +{:<4} -{:<4} {}  {}",
                    if v.is_current { "*" } else { " " },
                    v.number,
                    fmt_time(&v.created_at),
                    v.lines_added,
                    v.lines_deleted,
                    &v.hash[..8],
                    v.summary
                );
            }
        }

        Commands::History { path } => {
            if path.is_dir() {
                let key = rewind_core::normalize_path(&path)?;
                println!("{key}");
                let files = core.list_files(&key)?;
                if files.is_empty() {
                    println!("  no history yet - run `rewind watch \"{key}\"`");
                }
                println!("  most recently changed:");
                for f in files.iter().take(15) {
                    println!(
                        "    {}  {:<40} {} versions",
                        fmt_time(&f.last_change),
                        f.name,
                        f.versions
                    );
                }
                let gone = core.list_deleted(Some(&key))?;
                if !gone.is_empty() {
                    println!("  deleted files you can bring back: {} (rewind deleted --folder \"{key}\")", gone.len());
                }
                for l in core.list_labels(Some(&key))?.iter().take(5) {
                    println!("  moment: {}  {}", fmt_time(&l.created_at), l.name);
                }
                println!("  put the whole folder back: rewind rewind-folder \"{key}\" 1h");
            } else {
                let history = core.get_history(&path)?;
                if history.is_empty() {
                    bail!(
                        "no history for {} - is it inside a watched folder?",
                        path.display()
                    );
                }
                println!("{}", history[0].path);
                for v in history.iter().take(20) {
                    println!(
                        "{} v{:<4} {}  +{:<4} -{:<4} {}",
                        if v.is_current { "*" } else { " " },
                        v.number,
                        fmt_time(&v.created_at),
                        v.lines_added,
                        v.lines_deleted,
                        v.summary
                    );
                }
                println!(
                    "  restore one: rewind restore \"{}\" <version>",
                    history[0].path
                );
            }
        }

        Commands::Watch { path } => {
            let key = core.add_watch(&path)?;
            println!("watching {key}");
            println!("press Ctrl+C to stop");
            let _handle = core.start_watcher_with(&key, |v| {
                println!(
                    "[v{}] {}  +{} -{}  {}",
                    v.number, v.path, v.lines_added, v.lines_deleted, v.summary
                );
            })?;
            let _sched = core.start_scheduler();
            tokio::signal::ctrl_c().await?;
            println!("stopped");
        }

        Commands::WatchAll => {
            let mut handles = Vec::new();
            for w in core.list_watches()? {
                match core.start_watcher(&w) {
                    Ok(h) => {
                        println!("watching {w}");
                        handles.push(h);
                    }
                    Err(e) => eprintln!("skipping {w}: {e}"),
                }
            }
            if handles.is_empty() {
                println!(
                    "nothing to watch yet - use `rewind watch <path>` once to remember a folder"
                );
            }
            let _sched = core.start_scheduler();
            tokio::signal::ctrl_c().await?;
        }

        Commands::Diff { file, version } => {
            let history = core.get_history(&file)?;
            let Some(v) = (match version {
                Some(n) => history.iter().find(|v| v.number == n),
                None => history.first(),
            }) else {
                bail!("no matching version for {}", file.display());
            };
            println!("{}  v{}  {}", v.path, v.number, fmt_time(&v.created_at));
            for l in core.diff_version(v.id)? {
                match l.kind.as_str() {
                    "add" => println!("+ {}", l.text),
                    "del" => println!("- {}", l.text),
                    _ => println!("  {}", l.text),
                }
            }
        }

        Commands::Prune { keep, older_than } => {
            let r = core.prune(keep, older_than)?;
            println!(
                "removed {} versions and {} blobs, freed {:.1} KB",
                r.versions_removed,
                r.blobs_removed,
                r.bytes_freed as f64 / 1024.0
            );
        }

        Commands::Search {
            text,
            folder,
            limit,
        } => {
            let hits =
                core.search_history(&text, folder.as_deref().and_then(|p| p.to_str()), limit)?;
            if hits.is_empty() {
                println!("nothing found");
            }
            for h in hits {
                println!(
                    "{}  {}  line {}{}\n    {}",
                    fmt_time(&h.created_at),
                    h.path,
                    h.line_no,
                    if h.in_latest {
                        ""
                    } else {
                        "  (no longer in the newest version)"
                    },
                    h.snippet
                );
            }
        }

        Commands::Deleted { folder, restore } => {
            if let Some(p) = restore {
                let v = core.restore_deleted(&p.to_string_lossy())?;
                println!("restored {}", v.path);
            } else {
                let list = core.list_deleted(folder.as_deref().and_then(|p| p.to_str()))?;
                if list.is_empty() {
                    println!("no deleted files");
                }
                for d in list {
                    println!(
                        "{}  {}  ({} versions)",
                        fmt_time(&d.deleted_at),
                        d.path,
                        d.versions
                    );
                }
            }
        }

        Commands::Pin { file, version, off } => {
            let history = core.get_history(&file)?;
            let Some(v) = history.iter().find(|v| v.number == version) else {
                bail!("{} has no version v{version}", file.display());
            };
            core.pin_version(v.id, !off)?;
            println!("{} v{version}", if off { "unpinned" } else { "pinned" });
        }

        Commands::Label { folder, name } => match name {
            Some(n) => {
                let l = core.add_label(&folder.to_string_lossy(), &n)?;
                println!("labelled \"{}\" at {}", l.name, fmt_time(&l.created_at));
            }
            None => {
                for l in core.list_labels(folder.to_str())? {
                    println!("{}  {}", fmt_time(&l.created_at), l.name);
                }
            }
        },

        Commands::RewindFolder {
            folder,
            to,
            apply,
            remove_new,
        } => {
            let f = folder.to_string_lossy().to_string();
            let plan = core.plan_folder_rewind(&f, &to)?;
            println!("{} -> {}", plan.folder, fmt_time(&plan.to));
            for it in &plan.items {
                println!("  {:<8} {}  ({})", it.action, it.path, it.summary);
            }
            println!("  {} files already match", plan.unchanged);
            if !apply {
                println!("dry run. add --apply to do it");
            } else {
                let r = core.apply_folder_rewind(&f, &to, remove_new)?;
                println!(
                    "restored {}, recreated {}, removed {}",
                    r.restored, r.recreated, r.removed
                );
                for e in r.failed {
                    println!("  failed: {e}");
                }
            }
        }

        Commands::Compare { file, from, to } => {
            let history = core.get_history(&file)?;
            let find = |n: i64| history.iter().find(|v| v.number == n).map(|v| v.id);
            let (Some(a), Some(b)) = (find(from), find(to)) else {
                bail!("{} needs both v{from} and v{to}", file.display());
            };
            for l in core.diff_versions(a, b)? {
                match l.kind.as_str() {
                    "add" => println!("+ {}", l.text),
                    "del" => println!("- {}", l.text),
                    _ => println!("  {}", l.text),
                }
            }
        }

        Commands::Restore { file, version } => {
            let history = core.get_history(&file)?;
            let Some(v) = history.iter().find(|v| v.number == version) else {
                bail!("{} has no version v{version}", file.display());
            };
            core.restore_version(v.id)?;
            println!("restored {} to v{version}", v.path);
        }

        // ---- agent protection -------------------------------------------------------
        Commands::Run {
            folder,
            name,
            command,
        } => {
            let folder = folder.unwrap_or_else(|| PathBuf::from("."));
            let label = name.unwrap_or_else(|| {
                Path::new(&command[0])
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| command[0].clone())
            });
            let run = core.agent_start(&folder.to_string_lossy(), &label, &command.join(" "))?;
            println!(
                "[rewind] checkpoint saved, run {} started in {}",
                run.id, run.folder
            );
            let watcher = core.start_watcher(&run.folder)?;
            // Ctrl+C belongs to the child; we stay alive to write the report
            tokio::spawn(async {
                loop {
                    let _ = tokio::signal::ctrl_c().await;
                }
            });
            let cmd = command.clone();
            let status = tokio::task::spawn_blocking(move || {
                std::process::Command::new(&cmd[0]).args(&cmd[1..]).status()
            })
            .await?;
            drop(watcher);
            let report = core.agent_end(run.id)?;
            println!("\n[rewind] run finished");
            print!("{}", render_report(&report));
            println!(
                "[rewind] undo all of it: rewind agent undo {} --apply{}",
                run.id,
                if report.created > 0 {
                    " --remove-new"
                } else {
                    ""
                }
            );
            match status {
                Ok(s) => std::process::exit(s.code().unwrap_or(1)),
                Err(e) => bail!("could not start {}: {e}", command[0]),
            }
        }

        Commands::Agent { action } => match action {
            AgentCmd::List => {
                let runs = core.list_agent_runs(30)?;
                if runs.is_empty() {
                    println!("no runs yet - try `rewind run -- <your agent>`");
                }
                for r in runs {
                    println!(
                        "{:<4} {:<8} {}  {}  {}",
                        r.id,
                        r.status,
                        fmt_time(&r.started_at),
                        r.agent,
                        r.folder
                    );
                }
            }
            AgentCmd::Report { id } => {
                let id = match id {
                    Some(i) => i,
                    None => match core.latest_agent_run(None)? {
                        Some(r) => r.id,
                        None => bail!("no runs yet"),
                    },
                };
                print!("{}", render_report(&core.agent_report_live(id)?));
            }
            AgentCmd::Undo {
                id,
                apply,
                remove_new,
            } => {
                let id = match id {
                    Some(i) => i,
                    None => match core.latest_agent_run(None)? {
                        Some(r) => r.id,
                        None => bail!("no runs to undo"),
                    },
                };
                let plan = core.agent_undo_plan(id)?;
                println!("undo run {id}: {}", plan.folder);
                for it in &plan.items {
                    println!("  {:<8} {}  ({})", it.action, it.path, it.summary);
                }
                println!("  {} files already match", plan.unchanged);
                if !apply {
                    println!("dry run. add --apply to do it (and --remove-new to delete files the run created)");
                } else {
                    let r = core.agent_undo(id, remove_new)?;
                    println!(
                        "restored {}, recreated {}, removed {}",
                        r.restored, r.recreated, r.removed
                    );
                    for e in r.failed {
                        println!("  failed: {e}");
                    }
                }
            }
        },

        Commands::Checkpoint {
            folder,
            name,
            every,
        } => {
            let f = folder.unwrap_or_else(|| PathBuf::from("."));
            match core.checkpoint(&f.to_string_lossy(), name.as_deref(), every)? {
                Some(l) => println!("saved \"{}\"", l.name),
                None => println!("a checkpoint is recent enough, skipped"),
            }
        }

        Commands::Tripwire { action } => {
            if let Some(TripwireCmd::Set { files, seconds }) = action {
                let cur = core.tripwire_config();
                core.set_tripwire_config(
                    files.unwrap_or(cur.files),
                    seconds.unwrap_or(cur.seconds),
                )?;
            }
            let c = core.tripwire_config();
            println!(
                "tripwire: {} changed files within {}s, or {} files that turn into random-looking bytes",
                c.files, c.seconds, c.suspicious
            );
            let alerts: Vec<_> = core
                .list_alerts(false, 10)?
                .into_iter()
                .filter(|a| a.kind == "tripwire")
                .collect();
            if alerts.is_empty() {
                println!("no tripwire alerts");
            }
            for a in alerts {
                println!(
                    "#{} {}  {}  {}",
                    a.id,
                    fmt_time(&a.created_at),
                    if a.seen { "seen  " } else { "UNSEEN" },
                    a.message
                );
            }
        }

        Commands::Alerts { all, ack } => {
            let list = core.list_alerts(!all, 50)?;
            if list.is_empty() {
                println!("no alerts");
            }
            for a in &list {
                println!(
                    "#{} {} [{}] {}: {}",
                    a.id,
                    fmt_time(&a.created_at),
                    a.kind,
                    a.title,
                    a.message
                );
            }
            if ack {
                core.ack_alerts(None)?;
            }
        }

        Commands::UndoAlert {
            id,
            apply,
            remove_new,
        } => {
            let plan = core.plan_alert_rewind(id)?;
            println!("{} -> {}", plan.folder, fmt_time(&plan.to));
            for it in &plan.items {
                println!("  {:<8} {}", it.action, it.path);
            }
            if !apply {
                println!("dry run. add --apply to do it");
            } else {
                let r = core.apply_alert_rewind(id, remove_new)?;
                println!(
                    "restored {}, recreated {}, removed {}",
                    r.restored, r.recreated, r.removed
                );
            }
        }

        // ---- plugins ------------------------------------------------------------------
        Commands::Plugin { action } => match action {
            PluginCmd::List => {
                let list = core.list_plugins();
                for p in list.iter().filter(|p| !p.builtin) {
                    print_plugin(p);
                }
                for p in list.iter().filter(|p| p.builtin) {
                    print_plugin(p);
                }
                let bundled = core.starter_plugin_infos();
                let missing: Vec<_> = bundled
                    .iter()
                    .filter(|b| !list.iter().any(|p| p.id == b.id))
                    .collect();
                if !missing.is_empty() {
                    println!("\nbundled, not installed (rewind plugin add <name>):");
                    for b in missing {
                        println!("    {:<22} {}", b.id, b.description);
                    }
                }
                for e in core.plugin_load_errors() {
                    eprintln!("plugin problem: {e}");
                }
            }
            PluginCmd::Add { source, yes } => {
                let is_bundled = core.starter_plugin_infos().iter().any(|b| b.id == source);
                let info = if is_bundled {
                    core.starter_plugin_infos()
                        .into_iter()
                        .find(|b| b.id == source)
                        .unwrap()
                } else {
                    core.inspect_plugin(Path::new(&source))?
                };
                print_permissions(&info);
                if !yes && !confirm("Install and turn it on?") {
                    println!("not installed (pass --yes to skip this question)");
                    return Ok(());
                }
                if is_bundled {
                    core.install_starter_plugin(&source)?;
                } else {
                    core.install_plugin(Path::new(&source))?;
                }
                println!("installed and on. Plugins that react to file events only see folders Rewind watches (rewind watch <folder>); scheduled rules run while `rewind watch`, `rewind watch-all` or the app is running.");
            }
            PluginCmd::Inspect { source } => {
                let info = core.inspect_plugin(Path::new(&source))?;
                print_permissions(&info);
            }
            PluginCmd::Remove { id } => {
                core.remove_plugin(&id)?;
                println!("removed {id}");
            }
            PluginCmd::Enable { id } => {
                core.set_plugin_enabled(&id, true)?;
                println!("{id} is on");
            }
            PluginCmd::Disable { id } => {
                core.set_plugin_enabled(&id, false)?;
                println!("{id} is off");
            }
            PluginCmd::Run => {
                core.run_plugin_schedules(true);
                println!("scheduled rules ran");
            }
        },

        // ---- shell / service / mcp -------------------------------------------------------
        Commands::Shell { action } => match action {
            ShellCmd::Install { gui } => {
                let gui = gui.or_else(|| sibling_exe("rewind-gui"));
                let cli = std::env::current_exe().ok();
                for line in rewind_core::shell_install(gui.as_deref(), cli.as_deref())? {
                    println!("{line}");
                }
            }
            ShellCmd::Uninstall => {
                for line in rewind_core::shell_uninstall()? {
                    println!("{line}");
                }
            }
            ShellCmd::Status => {
                let s = rewind_core::shell_status();
                println!(
                    "{}",
                    if !s.supported {
                        "not supported here"
                    } else if s.installed {
                        "installed"
                    } else {
                        "not installed"
                    }
                );
                for d in s.details {
                    println!("  {d}");
                }
            }
        },

        Commands::Service { action } => service(action)?,

        Commands::Mcp => mcp::serve(core)?,
    }
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn service(action: ServiceCmd) -> Result<()> {
    let dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| rewind_core::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| anyhow::anyhow!("cannot find your config folder"))?
        .join("systemd")
        .join("user");
    let file = dir.join("rewind.service");
    match action {
        ServiceCmd::Install { enable } => {
            let exe = std::env::current_exe()?;
            std::fs::create_dir_all(&dir)?;
            std::fs::write(
                &file,
                format!(
                    "[Unit]\nDescription=Rewind file history\n\n[Service]\nExecStart={} watch-all\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
                    exe.display()
                ),
            )?;
            println!("wrote {}", file.display());
            if enable {
                let ok = std::process::Command::new("systemctl")
                    .args(["--user", "daemon-reload"])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                    && std::process::Command::new("systemctl")
                        .args(["--user", "enable", "--now", "rewind.service"])
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false);
                println!(
                    "{}",
                    if ok {
                        "service enabled and running"
                    } else {
                        "could not start it, run: systemctl --user enable --now rewind.service"
                    }
                );
            } else {
                println!("start it with: systemctl --user daemon-reload && systemctl --user enable --now rewind.service");
            }
        }
        ServiceCmd::Uninstall => {
            let _ = std::process::Command::new("systemctl")
                .args(["--user", "disable", "--now", "rewind.service"])
                .status();
            if file.exists() {
                std::fs::remove_file(&file)?;
            }
            println!("removed the service");
        }
    }
    Ok(())
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
fn service(_action: ServiceCmd) -> Result<()> {
    bail!("on this system, use the Rewind app's \"start with Windows\" setting, or run `rewind watch-all` from a startup task")
}
