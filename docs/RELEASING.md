# Releasing Rewind

## One-time setup
- Repo **Settings > Actions > General > Workflow permissions: Read and write**.
- Optional later: a code-signing certificate (or SignPath Foundation for open source) to remove the SmartScreen warning.

## Each release

1. Bump the version in **both** `Cargo.toml` (`[workspace.package] version`) and `rewind-gui/tauri.conf.json`.
   (`cargo build` refreshes `Cargo.lock`; commit it.)
2. Add a section to `CHANGELOG.md` and write the release notes to `docs/releases/vX.Y.Z.md` (the workflow uses this file as the release body).
3. Check locally:
   ```powershell
   cargo fmt --all
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
4. Commit and push to `main`, wait for CI to go green.
5. Tag and push:
   ```powershell
   git tag v0.3.0
   git push origin v0.3.0
   ```
6. `.github/workflows/release.yml` builds Windows (installer, CLI, full zip) and Linux (CLI tarball, `.deb`, AppImage when it builds),
   writes a `.sha256` for each file and publishes the release with your notes. Watch the **Actions** tab.
7. Afterwards: `scripts/update-winget.ps1`, then update Scoop/AUR (see PACKAGING.md).

The Windows updater looks for the asset ending in `-setup.exe` and its `.sha256`, so keep that naming.

## Building the Windows installer by hand
```powershell
.\build-installer.bat        # dist\Rewind_x.y.z_x64-setup.exe + the CLI
```
Upload `dist\*` to a draft release on github.com/hcentral07/rewind/releases and paste `docs/releases/vX.Y.Z.md` as the description.
If you do it by hand, also upload the `.sha256` files, or the in-app updater will refuse the update.

## Re-running a failed release
Actions tab > release > **Re-run failed jobs**. If you must change the tag: `git tag -d v0.3.0 && git push origin :refs/tags/v0.3.0`, fix, tag again.
