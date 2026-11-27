# Packaging: winget, Scoop, AUR, install script

Getting Rewind into package managers is the cheapest way to get discovered: `winget install` and `scoop install` are how many people look for tools.

## winget (Windows)

Manifests live in `packaging/winget/` (id `hcentral07.Rewind`, installer type `nullsoft`, silent switch `/S`).

1. Publish a GitHub release (see RELEASING.md). Check the installer file name, e.g. `Rewind_0.5.1_x64-setup.exe`.
2. Fill in the version and the installer hash, and write the manifests to `manifests/h/hcentral07/Rewind/<version>/`:
   ```powershell
   .\scripts\update-winget.ps1 -Version 0.5.0
   ```
3. Validate and test locally (needs the Windows Package Manager, enable local manifests once with `winget settings --enable LocalManifestFiles`):
   ```powershell
   winget validate --manifest .\winget-out\manifests\h\hcentral07\Rewind\0.5.0
   winget install --manifest .\winget-out\manifests\h\hcentral07\Rewind\0.5.1
   ```
4. Submit: either `wingetcreate submit` (needs a GitHub token) or fork `microsoft/winget-pkgs`, copy the `manifests/` folder in and open a pull request.
   The first submission takes a few days to review (an unsigned installer is allowed, but SmartScreen warnings are expected).
5. After the first one is merged, updates can be automated with a release workflow action such as `vedantmgoyal9/winget-releaser`
   (needs a personal access token with `public_repo` stored as a secret). Check its README for the current inputs.

When it is live, add `winget install hcentral07.Rewind` to the README (there is a commented-out line ready).

## Scoop (Windows, CLI)

`packaging/scoop/rewind.json` installs the standalone CLI. Scoop needs a bucket (a git repo of manifests):

1. Create a repo `hcentral07/scoop-rewind`, put `rewind.json` in `bucket/`.
2. Fill `hash` with the SHA-256 of `rewind-cli-windows-x64.exe` (from the release `.sha256`).
3. Users: `scoop bucket add rewind https://github.com/hcentral07/scoop-rewind` then `scoop install rewind`.
   The `autoupdate` block keeps it current when Scoop's bucket tooling runs.

## Arch (AUR)

`packaging/aur/PKGBUILD` is a `rewind-bin` package that unpacks the release tarball. Follow the AUR submission guide: create an AUR account,
`git clone ssh://aur@aur.archlinux.org/rewind-bin.git`, copy PKGBUILD, run `makepkg --printsrcinfo > .SRCINFO`, commit, push.
Replace `sha256sums=('SKIP')` with the real hash from the release before submitting.

## Linux install script

`scripts/install.sh` downloads the latest release tarball, verifies its checksum and installs `rewind` to `~/.local/bin`
(override with `REWIND_BIN_DIR`, pin with `REWIND_VERSION=v0.5.1`).

## Debian/Ubuntu

The release carries a `.deb` of the desktop app: `sudo apt install ./Rewind_0.5.0_amd64.deb`.

## crates.io (optional)

`cargo install rewind-cli` would need `rewind-core` and `rewind-cli` published (set `publish`, a description and a repository in their Cargo.toml and
give the path dependency a version). Worth doing once the API is stable.
