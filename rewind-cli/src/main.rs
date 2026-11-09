use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use clap::{Parser, Subcommand};
use rewind_core::RewindCore;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rewind", version, about = "Rewind - a time machine for your files")]
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
}

fn fmt_time(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&Local).format("%Y-%m-%d %H:%M:%S").to_string())
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
                    println!("    {}  ({} versions, {} files)", w.path, w.versions, w.files);
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
                bail!("no history for {} - is it inside a watched folder?", file.display());
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
