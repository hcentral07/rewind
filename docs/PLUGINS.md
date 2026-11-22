# Plugins

Everything beyond the bare snapshot engine is a plugin. There are three layers:

1. **Built-in plugins** (Rust, in `rewind-core`): deleted-files, folder-rewind, pins-labels, history-search, compare, agent-guard,
   disk-guard, tripwire. Each can be switched off in **Tools > Plugins & settings**.
2. **Rule plugins** (a `plugin.toml`, no code): "when X happens to a file matching Y, do Z".
3. **Program plugins** (a `plugin.toml` with a `[program]`): your own process receives events and answers with actions.

Plugins live in `<data dir>/plugins/<id>/plugin.toml` (`%LOCALAPPDATA%\Rewind\plugins` on Windows, `~/.local/share/rewind/plugins` on Linux).

## Installing

```bash
rewind plugin list                       # installed + bundled
rewind plugin add downloads-organizer    # bundled: shows permissions, asks, installs and turns it on
rewind plugin add ./my-plugin            # a folder with plugin.toml
rewind plugin inspect ./my-plugin        # check it without installing
rewind plugin disable|enable|remove <id>
rewind plugin run                        # run scheduled rules now
```

A plugin that you merely copy into the plugins folder is loaded but **off** until you enable it. Rewind reads the manifest again on
`plugin add/remove` and on start.

**When do rules run?** Event rules (`file.*`, `version.saved`, `tripwire`) fire for files Rewind watches. `schedule` rules run every
30 seconds when due, but only while Rewind is running: the app (also hidden in the tray), `rewind watch`, `rewind watch-all` or the
service. To make Rewind watch a folder: `rewind watch <folder>` once, or add it in the app.

## plugin.toml

```toml
id          = "downloads-organizer"        # a-z 0-9 - _ , unique
name        = "Downloads Organizer"
description = "Sorts new downloads into folders."
version     = "1.0.0"

[permissions]
folders = ["~/Downloads"]    # REQUIRED for any rule except tripwire. ~ is your home folder.
exec    = false              # true allows `run` actions and [program]
network = false              # informational; see "Safety"

[[rule]]
when       = "file.created"                 # trigger
match      = ["*.pdf", "*.docx"]            # optional: name patterns (string or list)
older_than = "30d"                          # optional: file's modified time at least this old
every      = "1d"                           # schedule rules only
do         = { move = "~/Downloads/Documents" }
```

### Triggers (`when`)

| Trigger | Fires when | Notes |
| --- | --- | --- |
| `file.created` | the first version of a file is saved | |
| `file.modified` | a later version is saved | |
| `version.saved` | any version is saved | alongside the two above |
| `file.renamed` | a file was renamed (matches the new path) | |
| `file.deleted` | a tracked file vanished | only `notify`, `label`, `run` |
| `tripwire` | the tripwire raised an alert | only `notify`, `label`, `run`; no folder needed |
| `schedule` | every `every` (`30s`, `45m`, `12h`, `30d`, `2w`; min 1m) | scans the permitted folders |

### Actions (`do`, exactly one)

| Action | Does |
| --- | --- |
| `move = "<dir>"` | moves the file; creates the folder; renames on collision (`name (1).ext`); skips files already there |
| `copy = "<dir>"` | copies the file |
| `delete = true` | removes the file after saving a copy in Rewind history; files over 25 MB go to Rewind's trash for 30 days |
| `notify = "<text>"` | raises an alert (shown in Tools > Protection and `rewind alerts`) |
| `label = "<name>"` | saves a labelled checkpoint of the event's folder (schedule: every permitted folder) |
| `run = ["prog", "arg", ...]` | starts a program (needs `exec = true`); no shell |

Placeholders in `move`, `copy`, `notify` and `run`: `{year} {month} {day} {ext} {name} {stem} {path}`.

### Matching
Patterns use `*` and `?`, case-insensitive. A pattern without a slash is matched against the file name; with a slash, against the full path
(using `/`). No `match` means every file.

## Program plugins

```toml
id = "my-notifier"
name = "My notifier"
[permissions]
folders = ["~/Projects"]
exec = true
[program]
command = "python3"
args = ["notifier.py"]
```

Rewind starts the process in the plugin folder (when enabled, on the first event) and restarts it at most every 30 seconds if it dies.
It receives one JSON object per line on stdin:

```json
{"event":"version.saved","path":"/home/me/Projects/a.rs","hash":"...","size":123,"created":false}
{"event":"file.deleted","path":"..."}
{"event":"file.renamed","from":"...","to":"..."}
{"event":"tripwire","folder":"...","count":212}
{"event":"folder.rewound","folder":"...","restored":12}
```

and may write actions, one JSON object per line, to stdout:

```json
{"action":"notify","text":"build finished"}
{"action":"snapshot","path":"/home/me/Projects/a.rs"}
{"action":"label","folder":"/home/me/Projects","name":"before deploy"}
```

`snapshot` and `label` are limited to the plugin's permitted folders.

## Safety model

- Plugins only act on files inside `permissions.folders` (symlinks and `..` are resolved before checking); a move or copy to anywhere else is blocked and raises an alert.
- Every `move`, `copy` and `delete` saves the content to Rewind history first, and moved/deleted files show up under **Deleted files**.
- A plugin's own output does not re-trigger it, and a plugin that tries more than 400 actions a minute is paused for that minute with an alert.
- `run` and `[program]` need `exec = true`, and you see "Run programs on your computer" before installing.
- **Limits:** rule plugins are confined by Rewind. A program plugin is a normal process running as you; Rewind cannot sandbox it, and `network = true` is only a declaration shown to you. Install program plugins only from sources you trust.

## Bundled plugins

| id | What it does |
| --- | --- |
| `downloads-organizer` | sorts new downloads into Documents, Pictures, Archives, Audio, Video sub-folders of Downloads |
| `screenshot-sorter` | files screenshots into `Pictures/Screenshots/<year>-<month>` |
| `installer-cleaner` | daily: removes installers older than 30 days from Downloads |
| `game-save-protector` | checkpoints `~/Documents/My Games` and `~/Saved Games` every 6 hours (edit the list for your games) |

## Writing a built-in plugin (Rust)

```rust
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn default_enabled(&self) -> bool { true }
    fn is_builtin(&self) -> bool { true }
    fn details(&self) -> PluginDetails { PluginDetails::default() }
    fn on_event(&self, core: &RewindCore, event: &RewindEvent) {}
    fn on_tick(&self, core: &RewindCore, force: bool) {}
}
```

Events: `VersionSaved`, `FileDeleted`, `FileRenamed`, `FolderRewound`, `Alert`. They are delivered after the database lock is released, so a
plugin may call back into `core`. On/off state is stored in the `kv` table (`plugin.<id>.enabled`); settings use keys starting `cfg.`.
Register with `core.register_plugin(Arc::new(MyPlugin))`; add Tauri commands in `rewind-gui/src/main.rs` and a tab in `ui/panels.js` if it needs UI.
