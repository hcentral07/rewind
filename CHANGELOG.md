# Changelog

All notable changes. Versions follow [semver](https://semver.org). Newest first.

## 0.5.0

### Added
- **Tray & start-with-Windows.** Tray icon with close-to-tray, single-instance prevention so a second launch just shows the existing window, `tauri-plugin-autostart` (HKCU\Run) with enable/disable toggle in Settings, and `--background/--minimized` args to start straight into the tray.
- **File watcher independent of window.** The Rust backend watcher runs independently of the webview/JS lifetime, continuing to record changes even when the window is closed.
- **External storage (Phase 1).** Specify a second directory where file copies are mirrored, protecting history even if the primary disk fails.
- **UI pass.** Consistent spacing, typography, empty states, loading and error states, dark/light mode, keyboard focus, and a clear "Protected / Paused" status indicator on the home page.
- **Version bump everywhere.** `0.3.0` → `0.5.0` in Cargo.toml, tauri.conf.json, package.json, winget/Scoop/AUR manifests, packaging scripts, and docs.

### Changed
- Plugin trait returns `&str` instead of `&'static str` (needed for loaded plugins). `PluginInfo` gained `version`, `kind`, `permissions`, `rules`.

### Fixed
- `rewind log | head` no longer prints a panic when the pipe closes.

## 0.3.0

### Added
- **AI agent protection.** `rewind run -- <command>` checkpoints a folder, runs the command and reports what changed
  (created / modified / deleted, line counts, warnings for mass deletes, touched secrets, gutted files). `rewind agent
  list|report|undo` reviews and undoes runs. `rewind checkpoint [--every SECS]` is a throttled checkpoint for hooks.
- **MCP server** (`rewind mcp`): begin/report/undo session, checkpoint, file history, restore file, deleted files.
- **Mass-change tripwire** (plugin `tripwire`): alert when many files change, vanish or turn into random-looking bytes within
  seconds; pins the last good version of each affected file; one-click `undo-alert`. Configurable (`rewind tripwire set`).
- **Alerts** (`rewind alerts`, Tools > Protection): tripwire, plugin and system alerts with a native popup for tripwire trips.
- **Right-click "Rewind: show history"** for files, folders and folder backgrounds (Windows registry, Nautilus, Dolphin, Nemo).
  `rewind shell install|uninstall|status`. New `--rewind-folder` launch flag. App has a Folder history tab.
- **Plugin engine.** `plugin.toml` rule plugins (triggers: file.created, file.modified, file.deleted, file.renamed, version.saved,
  tripwire, schedule; actions: move, copy, delete, notify, label, run) and JSON-lines program plugins. Folder-scoped permissions,
  rate limit, loop guard, drop-ins start disabled. Scheduler runs while the app, `watch`, `watch-all` or the service is running.
  Four bundled plugins: downloads-organizer, screenshot-sorter, installer-cleaner, game-save-protector.
  `rewind plugin list|add|inspect|remove|enable|disable|run`.
- **Linux.** Data in `$XDG_DATA_HOME/rewind`, `rewind service` (systemd user unit), `rewind watch-all`, autostart `.desktop`,
  `xdg-open` reveal, shell completions, `.deb` / AppImage builds of the app, CLI tarball and `scripts/install.sh`.
- `rewind history <path>`: friendly file or folder history (what the right-click menu opens in a terminal).
- Packaging: winget and Scoop manifests, AUR PKGBUILD, `scripts/update-winget.ps1`.
- README rewrite, demo GIF (`scripts/make-demo-gif.py`), docs for agents, plugins, packaging and releasing.

### Changed
- Plugin trait returns `&str` instead of `&'static str` (needed for loaded plugins). `PluginInfo` gained `version`, `kind`,
  `permissions`, `rules`.
- The first version of a binary file is now recorded as "Created" (it used to read "Binary or non-UTF-8 change").
- Windows installer replaces the 0.2 "Rewind this file" menu entry with "Rewind: show history" (files, folders, backgrounds).
- Updater only runs on Windows; Linux updates come from the package manager.
- Refreshed `Cargo.lock`.

### Fixed
- `rewind log | head` no longer prints a panic when the pipe closes.
- README no longer claims deletions/renames are untracked or that there is no pruning.

### Removed
- Committed `rewind-py/__pycache__` and a stray `rewind-py/read` file.

## 0.2.0
Always-on tray app, delete and rename tracking, folder rewind, pins and labels, history search, compare any two versions,
disk guard, plugin framework for the built-in features.

## 0.1.0
First release: watcher, content-addressed store, timeline UI, diffs, restore, CLI.
