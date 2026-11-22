<div align="center">

<img src="https://raw.githubusercontent.com/hcentral07/rewind/main/rewind-gui/icons/128x128.png" width="72" alt="Rewind logo" />

# Rewind

**Undo for your files. Now with undo for AI agents.**

Point it at a folder. Every change is saved. Scrub back through time, diff any version, restore anything,
undo a whole AI coding session, or roll back a ransomware burst. Everything stays on your machine.

[![Release](https://img.shields.io/github/v/release/hcentral07/rewind?color=8b7cff)](https://github.com/hcentral07/rewind/releases)
[![Downloads](https://img.shields.io/github/downloads/hcentral07/rewind/total?color=8b7cff)](https://github.com/hcentral07/rewind/releases)
[![Stars](https://img.shields.io/github/stars/hcentral07/rewind?color=8b7cff)](https://github.com/hcentral07/rewind/stargazers)
[![License: MIT](https://img.shields.io/badge/license-MIT-8b7cff)](LICENSE)

<img src="docs/demo.gif" width="760" alt="Rewind demo: undo an AI agent run, a ransomware tripwire, right-click history" />

[Download](#download) | [What's new in 0.3](#whats-new-in-03) | [AI agents](#undo-for-ai-coding-agents) | [Tripwire](#ransomware-and-mass-change-tripwire) | [Plugins](#plugins) | [CLI](#cli) | [Build](#build-from-source)

</div>

---

## Why

Git only remembers what you commit. Editors only undo what is in the buffer. Rewind watches the folder itself, so the
version you overwrote at 2 AM, the file an AI agent rewrote, the folder you deleted by accident and the config you
"just tweaked" are all still there.

No account, no cloud. The only network use is checking GitHub for updates (Windows) and, only if you opt in, one anonymous
ping a day (random ID, version, OS) so the author can count installs.

## Download

| | |
| --- | --- |
| **Windows** | Grab `Rewind_x.y.z_x64-setup.exe` from [Releases](https://github.com/hcentral07/rewind/releases). Per-user install, no admin needed. `rewind-cli-windows-x64.exe` is the standalone command-line tool. |
| **Linux (CLI)** | `curl -fsSL https://raw.githubusercontent.com/hcentral07/rewind/main/scripts/install.sh \| sh` installs `rewind` to `~/.local/bin` and verifies its checksum. Or download `rewind-linux-x86_64.tar.gz` from Releases. |
| **Linux (app)** | `.deb` (and an AppImage when the release build produced one) on [Releases](https://github.com/hcentral07/rewind/releases). Needs WebKitGTK 4.1 (Ubuntu 22.04+, Debian 12+, Fedora 38+). |
| **Arch** | `rewind-bin` PKGBUILD in [`packaging/aur`](packaging/aur). |
| **Scoop / winget** | Manifests are in [`packaging/`](packaging) and documented in [docs/PACKAGING.md](docs/PACKAGING.md). |

<!-- Enable this line once the winget package is merged into microsoft/winget-pkgs:
     winget install hcentral07.Rewind -->

> The Windows installer is not code-signed yet, so SmartScreen may show "Unknown publisher". Click **More info > Run anyway**.
> You can read and build everything from source below. Every release ships a `.sha256` next to each file.

The old Python `rwnd` package on PyPI is the first prototype and is **retired**: it only stored hashes, not file contents. The Rust
`rewind` replaces it. See [docs/PYPI-MIGRATION.md](docs/PYPI-MIGRATION.md).

## What's new in 0.3

* **Undo for AI coding agents.** `rewind run -- <agent>` checkpoints your project, runs the agent, then shows exactly what it
  created, changed and deleted (with warnings for mass deletes, touched secrets and gutted files). One command undoes it all.
  There is also an **MCP server** (`rewind mcp`) so agents can checkpoint and undo themselves. See [docs/AGENTS.md](docs/AGENTS.md).
* **Ransomware and mass-change tripwire.** Many files changing in seconds, or files turning into random-looking bytes, raises an
  alert, **pins the last good version of every affected file** so no prune can remove it, and offers a one-click folder undo.
* **Right-click "Rewind: show history"** on files, folders and folder backgrounds. Windows Explorer, plus Nautilus, Dolphin and Nemo on Linux.
* **Plugin engine.** Drop a `plugin.toml` in a folder: sort downloads, clean old installers, checkpoint game saves, no code needed.
  Plugins are sandboxed to the folders they declare and need your OK before they run. Four ship in the box.
* **Linux, in Rust.** The CLI and the desktop app now build and run on Linux from the same engine as Windows, with systemd user
  service, autostart, shell completions and `xdg-open` support.
* **Better everywhere.** `rewind history` for a friendly view of a file or folder, `rewind checkpoint` for hooks, `watch-all`
  for services, alerts list, binary files now count as "created" correctly, Windows installer replaces the old single-file menu entry.

Full details: [CHANGELOG.md](CHANGELOG.md).

## Features

* **Automatic snapshots.** Watch any folder; every save becomes a version. Native file-system events, no polling.
* **Timeline UI and real diffs.** Every version with time, `+added / -removed` lines and content hash. Compare any two versions.
* **Safe restore.** Current state is snapshotted first, so a restore can always be undone.
* **Deleted files and renames.** A recycle bin for everything; renamed files keep their history.
* **Folder rewind.** Put a whole folder back to a time or a named label. A "before" label is saved first.
* **Pins, labels, search.** Pinned versions are never pruned. Label moments. Find the version that contained a word.
* **Deduplicated storage.** Content is stored once by blake3 hash. A disk guard warns or auto-prunes at a size limit.
* **Always on.** Tray icon, optional start at login, keeps recording with the window closed.
* **Protection.** AI agent runs, tripwire, pinning. **Plugins** and a **right-click menu**.
* **GUI and CLI** share one engine and one database.

## Undo for AI coding agents

AI agents are fast and sometimes wrong at scale. Rewind gives you a safety net that does not depend on the agent or on git:

```text
$ rewind run -- claude "refactor the auth module"
[rewind] checkpoint saved, run 1 started in /home/me/code/shop
  ... the agent works ...
[rewind] run finished
run 1 (claude) in /home/me/code/shop
  14 created, 31 modified, 6 deleted  (+812 -1290 lines)
  ! deleted 6 files
  ! touched files that look like secrets: .env
  ! auth/session.rs lost 212 lines and gained 18
  ...
[rewind] undo all of it: rewind agent undo 1 --apply --remove-new
```

* Works with **any command**: Claude Code, Cursor's CLI, Aider, Codex, a script, an installer.
* `rewind agent report` and `rewind agent undo` (dry run by default; `--apply` does it; `--remove-new` also removes files the run created).
* Prefer the app? **Tools > AI agents** has Checkpoint and start, What changed, Undo.
* Agents that speak MCP can protect themselves: `claude mcp add rewind -- rewind mcp`. Tools: begin/report/undo session, checkpoint,
  file history, restore file, deleted files. Undo needs `confirm=true`, otherwise it only shows what would change.
* Hooks: `rewind checkpoint --every 120` saves a checkpoint at most every 2 minutes, built for "before every tool call" hooks.

More in [docs/AGENTS.md](docs/AGENTS.md), including limits (changes outside the folder cannot be undone).

## Ransomware and mass-change tripwire

Rewind counts modified, renamed and deleted files per watched folder. By default **50 files in 10 seconds**, or **5 files that
turn from readable data into random-looking bytes**, trips the wire (higher thresholds while an agent run is active). When it trips:

1. a native popup and an alert in **Tools > Protection** appear,
2. the **last good version of every affected file is pinned**, so prune and the disk guard cannot delete it,
3. **Put the folder back** restores everything to just before the burst (a label is saved first, so even that is undoable).

`rewind tripwire`, `rewind tripwire set --files 80 --seconds 15`, `rewind alerts`, `rewind undo-alert <id> --apply`.

Be realistic: this is a safety net, not antivirus. It cannot stop malware, and malware running as you could delete Rewind's own data
folder. Keep real backups too.

## Right-click: Rewind: show history

* **Windows:** installed by the setup (and toggle in **Tools > Plugins & settings**). Windows 11 shows it under *Show more options*.
* **Linux:** `rewind shell install` adds a Nautilus script, a Dolphin service menu and a Nemo action. Thunar and others: add a custom action
  that runs `~/.local/share/rewind/show-history.sh %f`.

It opens the app on that file's timeline or that folder's history, or `rewind history <path>` in a terminal if only the CLI is installed.

## Plugins

A plugin is a folder with a `plugin.toml`. Most need no code:

```toml
id = "downloads-organizer"
name = "Downloads Organizer"
version = "1.0.0"

[permissions]
folders = ["~/Downloads"]          # the only place this plugin may touch

[[rule]]
when  = "file.created"
match = ["*.pdf", "*.docx", "*.csv"]
do    = { move = "~/Downloads/Documents" }

[[rule]]
when  = "schedule"
every = "1d"
match = ["*.exe", "*.msi"]
older_than = "30d"
do    = { delete = true }          # a copy stays in Rewind history
```

Bundled: **downloads-organizer**, **screenshot-sorter**, **installer-cleaner**, **game-save-protector**.
`rewind plugin list`, `rewind plugin add downloads-organizer`, or **Tools > Plugins & settings > Plugin store**.

Safety: plugins only touch folders they declare, deleting keeps a copy, `run` needs `exec = true`, you see the permissions before
installing, a plugin you merely copy into the folder stays **off** until you enable it, and a runaway plugin is stopped. Program plugins
(JSON lines over stdin/stdout) are supported too. Full reference: [docs/PLUGINS.md](docs/PLUGINS.md).

## Using the app

1. Open Rewind and paste a folder path or press **Browse folder**.
2. Work as normal.
3. Pick a file in the sidebar and use the timeline: **View diff**, **Restore**, copy a hash, **Show in file manager**.
4. **Tools** (bottom right): deleted files, search, versions, folder rewind, labels, AI agents, protection, plugins and settings.

## CLI

```text
rewind status | log <file> | history <path> | watch <path> | watch-all
rewind restore <file> <v> | diff <file> [v] | compare <file> <a> <b>
rewind search <text> | deleted [--restore <path>] | pin <file> <v> | label <folder> [name]
rewind rewind-folder <folder> <time> [--apply] [--remove-new]       (2h, 1d, "2026-11-10 15:00")
rewind prune [--keep N] [--older-than DAYS]

rewind run [--folder DIR] [--name N] -- <command...>               checkpoint, run, report
rewind agent list | report [id] | undo [id] [--apply] [--remove-new]
rewind checkpoint [folder] [--name N] [--every SECS]
rewind tripwire [set --files N --seconds S] | alerts [--all] [--ack] | undo-alert <id> [--apply]
rewind plugin list | add <name|path> | inspect <path> | remove|enable|disable <id> | run
rewind shell install | uninstall | status
rewind service install [--enable] | uninstall                      (Linux systemd user service)
rewind mcp                                                          MCP server for AI agents
rewind completions <bash|zsh|fish|powershell|elvish>
```

`status` and `log` also take `--json`. Destructive commands are a dry run until you pass `--apply`.

## Linux

* Data lives in `$XDG_DATA_HOME/rewind` (default `~/.local/share/rewind`). Override with `REWIND_HOME`.
* Run in the background: `rewind watch <folder>` once to remember it, then `rewind service install --enable`.
* Large trees may hit the inotify limit: `sudo sysctl fs.inotify.max_user_watches=524288`.
* The app needs `libwebkit2gtk-4.1` and, for the tray icon, `libayatana-appindicator3`.

## How it works

```text
rewind/
  rewind-core/     the engine (library): store, watcher, tripwire, agent runs, plugin engine, shell integration
    starters/      bundled plugins (plugin.toml)
  rewind-cli/      the `rewind` binary (clap) + MCP server
  rewind-gui/      desktop app (Tauri v2)
    ui/            prebuilt offline UI (React + Tailwind) and panels.js (Tools panel)
  rewind-py/       the retired Python prototype
  packaging/       winget, scoop, AUR
  scripts/         install.sh, winget updater, demo GIF generator
  docs/            AGENTS, PLUGINS, PACKAGING, RELEASING, PYPI-MIGRATION
```

| Concern | Crate | Role |
| --- | --- | --- |
| Metadata | `rusqlite` (bundled SQLite, WAL) | versions, watches, labels, alerts, agent runs |
| Hashing | `blake3` | content address of every snapshot |
| File events | `notify` | ReadDirectoryChangesW on Windows, inotify on Linux |
| Plugins | `toml`, `serde_json` | `plugin.toml` manifests, program plugin protocol |

**On a save:** events are debounced for 350 ms, each changed file is hashed with blake3, identical content is skipped, otherwise the
blob is written atomically to `blobs/<hash>` and a row with a line diff is inserted. Restores snapshot the current file first, so
history is only ever appended. The tripwire and plugins are `Plugin`s that react to the same event stream.

Storage: `%LOCALAPPDATA%\Rewind` (Windows) or `~/.local/share/rewind` (Linux): `rewind.db`, `blobs/`, `plugins/`, `settings.json`.

## Build from source

**Windows 11:** install *Build Tools for Visual Studio 2022* (Desktop development with C++) and Rust from <https://rustup.rs>.

**Linux:** `sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libgtk-3-dev libssl-dev`
(the GUI needs all of them; the CLI needs only a C compiler).

```bash
git clone https://github.com/hcentral07/rewind.git && cd rewind
cargo run -p rewind-cli -- status      # CLI
cargo run -p rewind-gui                # desktop app
cargo test -p rewind-core -p rewind-cli
cargo build --release                  # optimized binaries in target/release
```

Windows installer: `.\build-installer.bat` (installs `tauri-cli` once, builds `Rewind_x.y.z_x64-setup.exe` into `dist\`).
Your own logo: `.\set-logo.ps1 .\my-logo.png`. Editing the UI source: `cd rewind-gui/ui-src && npm install && npm run build`.

## Releasing

See [docs/RELEASING.md](docs/RELEASING.md). In short: bump the version, write `docs/releases/vX.Y.Z.md`, push a tag. The workflow builds Windows
and Linux, checksums everything and publishes the release with your notes.

## Limitations

Upfront about what this is and is not:

* **Not a backup.** History lives on the same disk as your files, and anything running as you can delete it. Keep real backups.
* **Only records while running** (the app or tray, `rewind watch`, `watch-all` or the service). Offline edits become one combined change at next start.
* **Files over 25 MB are skipped**, symlinks are not followed, and `.git`, `node_modules`, `target` and similar folders are ignored.
* **Agent undo covers files inside the protected folder only.** It cannot undo a database write, a package install, a `git push` or an `rm` outside the folder.
* **The tripwire alerts and preserves, it does not block.** Thresholds are tunable; bulk formatting can trip it once.
* **Plugins:** rule plugins are confined to their folders. Program plugins are separate processes Rewind cannot sandbox; only install ones you trust.
* **Linux GUI and file-manager menus are new.** Nautilus, Dolphin and Nemo are supported; others need a manual action. Windows 11 shows the menu under *Show more options*.
* **Version numbers shift after a prune** (`v6` may become `v2`). `rewind log` always shows the current numbers.

## Roadmap

See [ROADMAP.md](ROADMAP.md): mirror to a second location, per-folder ignore rules and retention, plugin catalog, code signing, macOS.

## Contributing

Issues and PRs welcome. Before a PR: `cargo fmt --all`, `cargo clippy --workspace`, `cargo test --workspace`.

## License

[MIT](LICENSE).
