# Rewind

A time machine for your filesystem. Rust workspace:

- rewind-core : SQLite (rusqlite) + blake3 + notify, the RewindCore engine
- rewind-cli  : the `rewind` binary (status, log, watch, restore)
- rewind-gui  : Tauri desktop app (ui/index.html, React + Tailwind via CDN)

Data lives in %LOCALAPPDATA%\Rewind (override with REWIND_HOME).

Build:   cargo build --release
CLI:     cargo run -p rewind-cli -- status
GUI:     cargo run -p rewind-gui

Installer:
  build-installer.bat          builds dist\Rewind_0.1.0_x64-setup.exe locally
  git tag v0.1.0 && git push --tags     GitHub Actions builds it and publishes a Release

Logo:
  set-logo.bat                 downloads the logo, sets the sidebar logo + all app icons
