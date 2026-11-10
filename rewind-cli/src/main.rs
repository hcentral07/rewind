use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use rewind_core::RewindCore;
use std::path::PathBuf;

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
    /// Watch a folder and snapshot every change (Ctrl+C to stop)
    Watch { path: PathBuf },
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let core = RewindCore::open_default()?;

    match cli.command {
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
            tokio::signal::ctrl_c().await?;
            println!("stopped");
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
                    "hunk" => println!("  {}", l.text),
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
            let hits = core.search_history(&text, folder.as_deref().and_then(|p| p.to_str()), limit)?;
            if hits.is_empty() {
                println!("nothing found");
            }
            for h in hits {
                println!(
                    "{}  {}  line {}{}\n    {}",
                    fmt_time(&h.created_at),
                    h.path,
                    h.line_no,
                    if h.in_latest { "" } else { "  (no longer in the newest version)" },
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
                    println!("{}  {}  ({} versions)", fmt_time(&d.deleted_at), d.path, d.versions);
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
    }
    Ok(())
}
