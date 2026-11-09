<div align="center">

<div align="center">
  <img src="https://files.catbox.moe/zti00f.webp" width="600" alt="Rewind banner" />
</div>

<br /><br />

<img src="https://raw.githubusercontent.com/hcentral07/rewind/main/rewind-gui/icons/128x128.png" width="64" alt="Rewind logo" />

# Rewind

**A time machine for your filesystem.**

Point it at a folder. It snapshots every change. Scrub back through time, diff any version, restore any file.

[Download](#download) | [Features](#features) | [How it works](#how-it-works) | [Build from source](#build-from-source) | [CLI](#cli)

</div>

---

## Why

Git only remembers what you commit. Editors only undo what's in the buffer. Rewind watches the folder itself, so the version you overwrote at 2 AM, the file you saved over by accident, and the config you "just tweaked" are all still there.

Everything stays on your machine. No account, no cloud, no telemetry.

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
    ui/index.html           React + Tailwind via CDN, no build step
    tauri.conf.json
  .github/workflows/        CI: builds the installer and publishes a Release
  build-installer.ps1       local installer build
  set-logo.ps1              sets the logo and regenerates all icons
```

2. **Python CLI (Linux/PyPI)**:
   Located in `python-cli/`, published as `rwnd` using `watchdog`, `sqlite3`, and `rich`.

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

A [Tauri v2](https://tauri.app) app. There is no JS toolchain: `ui/index.html` is a single file using React 18, Babel standalone and Tailwind from CDNs, loaded into the system WebView2. The Rust side exposes commands:

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
cd rewind/python-cli
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

* **The UI needs internet on first paint** because React, Babel and Tailwind load from CDNs.

## Roadmap

* Retention policies and a "prune old versions" command

* Rename and delete tracking

* Folder-level restore ("rewind this whole folder to 2 hours ago")

* Bundle the UI assets locally for fully offline use

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

## License

[MIT](LICENSE).
