<div align="center">

<<<<<<< HEAD
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
=======
<div align="center">
  <video src="https://files.catbox.moe/oxjs9m.webm" width="600" autoplay loop muted playsinline>
    <source src="https://files.catbox.moe/oxjs9m.webm" type="video/webm">
  </video>
</div>

<br /><br />

<img src="https://raw.githubusercontent.com/hcentral07/rewind/main/rewind-gui/icons/128x128.png" width="64" alt="Rewind logo" />

# Rewind

**A time machine for your filesystem.**

Point it at a folder. It snapshots every change. Scrub back through time, diff any version, restore any file.

[Download](#download) | [Features](#features) | [How it works](#how-it-works) | [Build from source](#build-from-source) | [CLI](#cli)
>>>>>>> 0875c6b1c0901cb68339aa06d3c8834d84d0b308

</div>

---

## Why

<<<<<<< HEAD
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

## What's new in 0.4

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
=======
Git only remembers what you commit. Editors only undo what's in the buffer. Rewind watches the folder itself, so the version you overwrote at 2 AM, the file you saved over by accident, and the config you "just tweaked" are all still there.

Everything stays on your machine. No account, no cloud. The only network use is checking GitHub for updates and, only if you opt in, one anonymous ping a day (random ID, version, OS) so the author can count installs.

## Features

* **Automatic snapshots.** Watch any folder; every save becomes a version.

* **Timeline UI.** Pick a file, see every version with timestamp, `+added / -removed` line counts and content hash.

* **Real diffs.** Line-by-line diff of any version against the one before it.

* **Safe restore.** Roll a file back with one click (two clicks to confirm). The current state is snapshotted first, so a restore can always be undone.

* **Deduplicated storage.** Content is stored once by its blake3 hash. Saving the same bytes twice costs nothing.

* **Smart ignoring.** `.git`, `target`, `node_modules`, `__pycache__`, editor temp and swap files are skipped.

* **GUI and CLI.** A Tauri desktop app and a scriptable `rewind` command share the same engine and database.

* **Resumes on launch.** Watched folders are remembered and picked up again when the app starts.

* **Small and fast.** Written in Rust. Native file-system events, not polling.

## Download

### Windows (Tauri GUI + CLI)

Grab the latest installer from the [**Releases**](https://github.com/hcentral07/rewind/releases) page:

| File | What it is | 
| ----- | ----- | 
| `Rewind_x.y.z_x64-setup.exe` | Desktop app installer (per-user, no admin needed) | 
| `rewind-cli-windows-x64.exe` | Standalone command-line tool | 

> The installer is not code-signed, so Windows SmartScreen may show "Unknown publisher". Click **More info > Run anyway**. You can inspect and build everything from source below.

### Linux / Ubuntu (Python CLI)

The Linux version is built in Python and distributed via PyPI as `rwnd` (the CLI command remains `rewind`):

```
pip install rwnd
```

## What's new in 0.2

* **Always on.** Tray icon, optional start with Windows (hidden), and the watcher keeps running when the window is closed. A second launch just wakes the running copy.
* **Delete and rename tracking.** Deleted files land in a recycle-bin-for-everything; renamed files and folders keep their history.
* **Folder rewind.** Put a whole folder back to a time or a named label. A "before" label is saved first, so it can be undone.
* **Pins and labels.** Pinned versions are never pruned; labels name moments like "before mod install".
* **History search**, **compare any two versions**, and a **disk space guard** (warns, or auto-prunes if you turn that on).
* **Plugins.** Each of the features above is a plugin you can switch off. See [docs/PLUGINS.md](docs/PLUGINS.md).
* **Tools button** (bottom-right in the app) opens all of it. CLI: `search`, `deleted`, `pin`, `label`, `rewind-folder`, `compare`.

## Using it

1. Open Rewind.

2. On the welcome screen, paste a folder path or hit **Browse folder**.

3. Edit files in that folder as normal.

4. Open the folder in the sidebar, pick a file, and use the timeline:

   * **View diff** to see what changed in that version

   * **Restore** to bring that version back

   * click a hash to copy it, **Show in Explorer** to jump to the file

## CLI

```
rewind status                     Watched folders and storage stats
rewind log <file>                 Version history of a file
rewind watch <path>                Watch a folder and snapshot changes (Ctrl+C to stop)
rewind restore <file> <version>    Roll a file back to a version number from `rewind log`
rewind diff <file> [version]       What a version changed vs the one before it (default: newest)
rewind prune [--keep N] [--older-than DAYS]
                                   Delete old history. Keeps the newest N versions per file
                                   (default 50); with --older-than, only versions that are
                                   also older than DAYS are removed. Unused blobs are freed.
```

`status` and `log` also take `--json` for scripting.

```
$ rewind watch C:\projects\my-app
watching C:\projects\my-app
press Ctrl+C to stop
[v1] C:\projects\my-app\src\main.rs  +3 -0  Created: 3 lines (33 B)
[v2] C:\projects\my-app\src\main.rs  +2 -1  Modified: +2 -1 lines (52 B)

$ rewind log C:\projects\my-app\src\main.rs
C:\projects\my-app\src\main.rs
- v2    2026-11-09 19:31:28  +2    -1    16cc68e3  Modified: +2 -1 lines (52 B)
  v1    2026-11-09 19:31:26  +3    -0    9db21cba  Created: 3 lines (33 B)
```

## How it works

Rewind has two implementations:

1. **Rust workspace (Windows GUI & CLI)**:

```
rewind/
  Cargo.toml                workspace + shared dependency versions
  rewind-core/              the engine (library)
  rewind-cli/               the `rewind` binary (clap)
  rewind-gui/               desktop app (Tauri v2)
    ui/                     prebuilt, fully offline UI (React + Tailwind, committed)
    ui-src/                 UI source (app.jsx) + build script, only needed to edit the UI
    tauri.conf.json
  rewind-py/                Python port (watchdog + sqlite3 + rich)
  .github/workflows/        ci.yml (fmt, clippy, tests) and release.yml (installer + Release)
  build-installer.ps1       local installer build
  set-logo.ps1              sets the logo and regenerates all icons
```

2. **Python CLI (Linux/PyPI)**:
   Located in `rewind-py/`, published as `rwnd` using `watchdog`, `sqlite3`, and `rich`.

### rewind-core

The `RewindCore` struct is the whole product. Both front ends are thin wrappers over it.

| Concern | Crate | Role | 
| ----- | ----- | ----- | 
| Metadata | `rusqlite` (bundled SQLite, WAL mode) | version rows, watched folders | 
| Hashing | `blake3` | content address of every snapshot | 
| File events | `notify` | native Windows change notifications | 
| Timestamps / IDs | `chrono`, `uuid` | RFC 3339 times, atomic temp-file names | 
| Errors / serde | `anyhow`, `serde` | error plumbing, JSON for the UI and CLI | 

**Storage layout** (`%LOCALAPPDATA%\Rewind`, override with `REWIND_HOME`):

```
Rewind/
  rewind.db        SQLite: versions + watches
  blobs/<hash>     file contents, named by blake3 hash
```

```sql
CREATE TABLE versions (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  path          TEXT    NOT NULL,   -- canonical absolute path
  hash          TEXT    NOT NULL,   -- blake3 of the content
  size          INTEGER NOT NULL,
  created_at    TEXT    NOT NULL,   -- RFC 3339, UTC
  lines_added   INTEGER NOT NULL DEFAULT 0,
  lines_deleted INTEGER NOT NULL DEFAULT 0,
  summary       TEXT    NOT NULL DEFAULT ''
);
CREATE TABLE watches (path TEXT PRIMARY KEY, added_at TEXT NOT NULL);
```

**Public API** (what the CLI and GUI call):

```rust
RewindCore::open_default() / open(dir)
save_version(path)        -> Option<Version>   // None if content unchanged
get_history(path)          -> Vec<Version>      // newest first
get_version(id)            -> Version
diff_version(id)          -> Vec<DiffLine>     // vs the previous version
restore_version(id)       -> Version
list_files(folder)        -> Vec<FileEntry>
get_status()              -> Status
add_watch / remove_watch / list_watches
start_watcher(path)       -> WatcherHandle     // drop the handle to stop
start_watcher_with(path, |version| ...)        // callback per new version
```

**What happens on a save:**

1. `notify` reports a create/modify event.

2. Events are collected for 350 ms (debounce), because editors fire several events per save.

3. For each changed file (ignored paths dropped): read it, hash it with blake3.

4. If the hash equals the latest stored hash, stop. Nothing changed.

5. Otherwise write the content to `blobs/<hash>` (temp file + rename, so it is atomic), compute a line diff against the previous version, insert a row.

6. If the file is locked mid-write, retry a few times with backoff.

When a watch starts, the existing files are snapshotted once so you have a baseline.

**Diffing.** Common prefix and suffix are trimmed, then an LCS table runs on the changed middle. Very large changes fall back to "all old lines removed, all new lines added" to cap memory. The UI view collapses long unchanged runs to `... N unchanged lines ...` with 3 lines of context around each change. Non-UTF-8 files are stored and restored byte for byte but get a size-based summary instead of a text diff.

**Restore.** `restore_version` first snapshots whatever is on disk now, writes the old content back, then records the result as a new version. History is never rewritten, only appended.

### rewind-cli

A [`clap`](https://crates.io/crates/clap) derive app. `watch` runs the watcher and blocks on `tokio::signal::ctrl_c()`.

### rewind-gui

A [Tauri v2](https://tauri.app) app. The UI is plain files in `rewind-gui/ui/` (React 18 and a compiled Tailwind stylesheet, all vendored, no CDN), loaded into the system WebView2 under a strict Content-Security-Policy, so it works fully offline. You never need Node to build or run the app. Only if you edit `ui-src/app.jsx`: `cd rewind-gui/ui-src && npm install && npm run build`, then commit the regenerated files. The Rust side exposes commands:

| Command | Does | 
| ----- | ----- | 
| `watch_folder(path)` | remember + start watching a folder | 
| `unwatch_folder(path)` | stop watching (history is kept) | 
| `get_status()` | totals and per-folder counts | 
| `list_files(folder)` | files with history under a folder | 
| `get_history(path)` | timeline for one file | 
| `get_diff(versionId)` | diff lines for a version | 
| `restore_version(versionId)` | roll back | 
| `pick_folder()` | native folder dialog | 
| `reveal_path(path)` | open Explorer on a file | 

The UI polls every ~2 seconds, which keeps the app free of event-capability plumbing.

## Build from source

### 1. Install the prerequisites (Windows 11)

1. **MSVC build tools.** Install *Build Tools for Visual Studio 2022* and tick **Desktop development with C++** (MSVC toolset and Windows 11 SDK). SQLite and blake3 compile C code.

2. **Rust.** Download `rustup-init.exe` from <https://rustup.rs> and choose the default (`stable-x86_64-pc-windows-msvc`).

3. Open a new terminal and confirm `rustc --version` and `cargo --version` work.

4. WebView2 ships with Windows 11. Nothing else needed.

### 2. Build and run (Rust / Windows)

```powershell
git clone https://github.com/hcentral07/rewind.git
cd rewind

cargo run -p rewind-cli -- status     # CLI
cargo run -p rewind-gui               # desktop app
cargo build --release                 # optimized binaries in target\release
```

### 3. Build and run (Python / Linux)

```bash
cd rewind/rewind-py
pip install -e .
```

### 4. Build the installer

```powershell
.\build-installer.bat
```

This installs `tauri-cli` once, runs `cargo tauri build`, and puts `Rewind_x.y.z_x64-setup.exe` and the CLI in `dist\`.

### 5. Set your own logo

```powershell
.\set-logo.ps1 .\my-logo.png      # or a URL; webp/png/jpg
```

Writes `rewind-gui\ui\logo.png` (sidebar) and regenerates every icon in `rewind-gui\icons`. Commit both.

## Releasing

Pushing a version tag triggers `.github/workflows/release.yml`, which builds on a Windows runner and attaches the installer and CLI to a GitHub Release:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Bump `version` in the workspace `Cargo.toml` and `rewind-gui/tauri.conf.json` before tagging. In the repo's **Settings > Actions > General**, set workflow permissions to **Read and write**.

## Limitations

Being upfront about what this is and isn't:

* **Not a backup.** History lives on the same disk as your files. If the disk dies, so does Rewind. Keep real backups.

* **Only sees changes while it's running.** Edits made while the app or `rewind watch` is closed are picked up as one combined change at the next start, not individually.

* **Files over 25 MB are skipped**, and symlinks are not followed.

* **Storage grows with change volume.** Identical content is deduplicated, but there is no pruning or retention policy yet.

* **Deletions and renames are not tracked** as explicit events.

* **GUI is Windows-only.** The GUI targets Windows 11; Linux users use the Python `rwnd` CLI.

* **Version numbers shift after a prune.** `v6` may become `v2` once older versions are deleted. `rewind log` always shows the current numbers.

## Roadmap

The full plan, including the plugin system design and ideas, is in [ROADMAP.md](ROADMAP.md).

* Automatic retention policy (prune on a schedule, per-folder rules)

* Rename and delete tracking

* Folder-level restore ("rewind this whole folder to 2 hours ago")

* Per-folder ignore rules

* Code signing for the installer

* macOS and Linux GUI builds

## Contributing

Issues and PRs welcome. Before opening a PR:

```powershell
cargo fmt --all
cargo clippy --workspace
cargo test --workspace
```
>>>>>>> 0875c6b1c0901cb68339aa06d3c8834d84d0b308

## License

[MIT](LICENSE).
