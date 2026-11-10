# Roadmap and ideas

Rewind today is "undo for your files". Underneath, it is three things that are useful far beyond undo:

1. a **watcher** that knows when files change,
2. a **content-addressed store** (blake3 hashes) that knows what every file looked like,
3. an **event stream** (`version saved`, soon `file deleted / renamed`).

The way to grow without turning into a pile of unrelated features is to make those three things a small **platform** and let people add everyday tools on top as plugins. This file is the plan.

---

## 1. Fix the foundations first

These are gaps in the current app. Plugins built on a shaky base will inherit them.

| # | Gap | Why it matters |
| - | --- | --- |
| 1 | **Runs only while the window is open** | A time machine that is off when you need it is useless. Add a tray icon, "start with Windows", and run the watcher in the background. This is the single biggest improvement. |
| 2 | **Deletes and renames are not tracked** | "I deleted the folder" is the most common disaster. Needed for a recycle-bin-for-everything and for most plugins. |
| 3 | **Folder-level restore** | "Put this whole folder back to how it was at 3 PM." One button, huge trust. |
| 4 | **Not a backup** | History sits on the same disk. A built-in "mirror to a second location" (USB, other drive, OneDrive/NAS folder) fixes the biggest honest limitation. |
| 5 | **Per-folder ignore rules and size limits** | Users watch folders with huge junk. Needs a settings screen. |
| 6 | **Scheduled retention** | `rewind prune` exists; run it automatically ("keep 30 days, at least 20 versions"). |
| 7 | **Notifications** | Toast when something notable happens (mass change detected, backup finished). |

## 2. Plugin system (for people who are not developers)

Design goal: a user installs a plugin the way they install a browser extension, by clicking. Most plugins should need **no code at all**.

### Two kinds of plugin

**A. Rule plugins (no code, the default).** A folder with one `plugin.toml`:

```toml
id = "downloads-organizer"
name = "Downloads Organizer"
description = "Sorts new downloads into folders by type and cleans old installers."
version = "1.0.0"
icon = "icon.png"

[permissions]
folders = ["~/Downloads"]          # the plugin can only touch these
network = false

[[rule]]
when   = "file.created"
match  = "*.pdf"
do     = { move = "~/Downloads/Documents" }

[[rule]]
when   = "file.created"
match  = "*.exe"
older_than = "30d"
do     = { delete = true }
```

The engine provides the triggers and actions: `file.created`, `file.modified`, `file.deleted`, `version.saved`, `schedule` (cron-like); actions `move`, `copy`, `rename`, `compress`, `delete`, `notify`. Settings (folders, ages) are declared in the manifest and rendered as a form in the UI, so the user never edits a file.

**B. Program plugins (for developers).** Any executable the engine launches as a child process, talking JSON lines over stdin/stdout:

```json
{"event":"version.saved","path":"C:\\docs\\report.docx","hash":"7f9a...","size":48211}
{"action":"notify","text":"Report saved (v12)"}
```

Any language works (Python, Go, a shell script). Same manifest and permission model as rule plugins.

### Safety rules (decide these before the first public plugin)

- A plugin sees only the folders in its `permissions.folders`, and the install dialog shows them in plain words ("This plugin can read and move files in Downloads").
- No network unless declared, and then shown to the user.
- Deleting is always routed through Rewind's own history, so a bad plugin can be undone.
- The plugin catalog is a GitHub repo with an `index.json` (name, version, download URL, **sha256**). The app checks the hash before installing.
- Later: signed plugins, and a "verified" badge.

### Why not WASM / Lua / a JS runtime?

Not yet. Declarative rules cover most real "daily things", and they are the only format a non-developer can read and trust. Add a scripting runtime later if rules prove too limiting.

## 3. Plugin ideas, ranked by everyday value

**Ship first (most people, immediate payoff)**

- **Second-copy backup.** Mirrors watched folders to a USB drive, other disk or synced folder. Also solves foundation gap 4.
- **Downloads Organizer.** Sort by type/date, delete installers and old files after N days.
- **Duplicate Finder.** Rewind already hashes every file with blake3, so finding duplicates is nearly free. Show them, let the user pick what to delete (via history, so it is undoable).
- **Game Save Protector.** Knows common save locations, snapshots before and after a play session, one-click restore. Also fits modders ("I broke my modlist").
- **Office Guardian.** Versions Word, Excel, PowerPoint, Photoshop files. Targets "I saved over my report".
- **Panic button.** "Restore this folder to 2 hours ago." (Foundation gap 3 as a plugin.)

**Next**

- **Ransomware tripwire.** Many files change very fast, so pause, alert, offer to roll back.
- **Screenshot sorter.** Rename by date/app, file into folders.
- **Photo importer.** Sort by EXIF date into Year/Month.
- **Receipts and PDFs.** Auto-name and file by date and vendor.
- **Notes vault history** (Obsidian, plain-text notes).
- **Desktop and temp cleaner**, folder-size alerts.
- **Share a version.** Export any version, or a folder snapshot, as a zip.

**Later / maybe**

- Encrypted cloud copy, phone companion, VirusTotal hash lookup for new downloads (privacy trade-off: only the hash leaves the machine, still needs opt-in).

## 4. Suggested order of work

1. **Sprint 1: be always on.** Tray + autostart + background watcher + settings screen (ignore rules, size limit) + notifications.
2. **Sprint 2: trust.** Delete/rename tracking, folder-level restore, scheduled retention, second-copy backup.
3. **Sprint 3: plugin engine.** Event bus, `plugin.toml` loader, rule runner, permissions, "Plugins" tab with install/enable/disable. Ship Downloads Organizer, Duplicate Finder and Game Save Protector as the first three.
4. **Sprint 4: catalog.** `rewind-plugins` repo + `index.json`, install from the app, docs on writing a plugin.

## 5. Distribution

- Publish on **winget** and **Scoop** (`winget install rewind`).
- Offer a **portable zip** next to the installer.
- Code signing removes the SmartScreen warning. Options include the SignPath Foundation (free for open source) or Azure Trusted Signing. Check current eligibility and pricing.
- A short GIF at the top of the README (mascot following the mouse, timeline scrubbing) and a one-line pitch ("undo for your files") does most of the marketing.

## 6. About the name

**"NectarLink" is a risky choice, for three reasons:**

- There is already a Rust project named `nectarlink` on GitHub ("Your Android phone and your Windows PC, working as one", local-first, no account). A web search shows it listed on Trendshift with roughly 1k stars. It is the same ecosystem (Rust, Windows desktop), so people will confuse the two and search results will bury yours.
- "Nectar" is a heavily used brand name: the UK Nectar loyalty scheme, a Nectar HR/recognition software company, several apps. That makes trademark and search visibility hard.
- "Link" suggests sync, URLs or connecting devices. It says nothing about history or undo, which is your actual product.

**"Rewind" has its own problem:** it is a very common word, and to my knowledge an AI app by that name already exists, so searching for it is noisy. Check before you invest in the brand.

**How to pick:**

- One or two syllables, easy to type, says "history / undo / safety".
- Search the name on GitHub, crates.io, winget, a domain registrar and the trademark databases (USPTO, EUIPO) **before** you commit.
- If you build the plugin platform, you can name the platform and keep "Rewind" (or whatever you pick) as the name of the time-machine module.

Candidates to check (none verified for availability): Hindsight, Backtrack, Tidemark, Echoes, Undone, Keepsake, Lookback.
