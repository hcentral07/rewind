//! "Rewind: show history" in the right-click menu of files and folders.
//!
//! * **Windows**: per-user registry entries (no admin needed) for files, folders and the empty
//!   space inside a folder. On Windows 11 they appear under "Show more options".
//! * **Linux**: a Nautilus script, a Dolphin service menu and a Nemo action, all calling one small
//!   launcher script that opens the Rewind app, or `rewind history` in a terminal when only the
//!   command line tool is installed.
//! * **macOS**: not supported yet.
use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellStatus {
    pub supported: bool,
    pub installed: bool,
    pub details: Vec<String>,
}

pub const MENU_TEXT: &str = "Rewind: show history";

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::process::CommandExt;
    use std::process::Command;

    const NO_WINDOW: u32 = 0x0800_0000;
    const KEYS: [(&str, &str); 3] = [
        (
            r"HKCU\Software\Classes\*\shell\RewindHistory",
            "--rewind-file \"%1\"",
        ),
        (
            r"HKCU\Software\Classes\Directory\shell\RewindHistory",
            "--rewind-folder \"%1\"",
        ),
        (
            r"HKCU\Software\Classes\Directory\Background\shell\RewindHistory",
            "--rewind-folder \"%V\"",
        ),
    ];

    fn reg(args: &[&str]) -> bool {
        Command::new("reg")
            .args(args)
            .creation_flags(NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    pub fn status() -> ShellStatus {
        let installed = reg(&["query", KEYS[1].0]);
        ShellStatus {
            supported: true,
            installed,
            details: vec!["Windows 11: right-click, then \"Show more options\"".into()],
        }
    }

    pub fn install(gui: Option<&Path>, _cli: Option<&Path>) -> Result<Vec<String>> {
        let gui = gui.ok_or_else(|| {
            anyhow!("the right-click menu opens the Rewind app, which was not found")
        })?;
        let exe = gui.to_string_lossy().to_string();
        let _ = reg(&[
            "delete",
            r"HKCU\Software\Classes\*\shell\RewindThisFile",
            "/f",
        ]);
        for (key, args) in KEYS {
            let cmd = format!("\"{exe}\" {args}");
            let ok = reg(&["add", key, "/ve", "/d", MENU_TEXT, "/f"])
                && reg(&["add", key, "/v", "Icon", "/d", &exe, "/f"])
                && reg(&["add", &format!("{key}\\command"), "/ve", "/d", &cmd, "/f"]);
            if !ok {
                return Err(anyhow!("could not write {key}"));
            }
        }
        Ok(vec![
            "files, folders and folder background (Windows 11: under \"Show more options\")".into(),
        ])
    }

    pub fn uninstall() -> Result<Vec<String>> {
        for (key, _) in KEYS {
            let _ = reg(&["delete", key, "/f"]);
        }
        let _ = reg(&[
            "delete",
            r"HKCU\Software\Classes\*\shell\RewindThisFile",
            "/f",
        ]);
        Ok(vec!["removed the Explorer menu entries".into()])
    }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn data_home() -> PathBuf {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .or_else(|| home_dir().map(|h| h.join(".local").join("share")))
            .unwrap_or_else(|| PathBuf::from(".local/share"))
    }

    fn launcher_path() -> PathBuf {
        default_data_dir().join("show-history.sh")
    }

    fn files() -> Vec<PathBuf> {
        let d = data_home();
        vec![
            d.join("nautilus")
                .join("scripts")
                .join("Rewind - show history"),
            d.join("kio")
                .join("servicemenus")
                .join("rewind-history.desktop"),
            d.join("nemo")
                .join("actions")
                .join("rewind-history.nemo_action"),
        ]
    }

    fn write_exec(path: &Path, text: &str) -> Result<()> {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
        }
        fs::write(path, text)?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }

    pub fn status() -> ShellStatus {
        let found: Vec<String> = files()
            .iter()
            .filter(|f| f.exists())
            .map(|f| f.to_string_lossy().to_string())
            .collect();
        ShellStatus {
            supported: true,
            installed: !found.is_empty(),
            details: found,
        }
    }

    pub fn install(gui: Option<&Path>, cli: Option<&Path>) -> Result<Vec<String>> {
        let launcher = launcher_path();
        let script = r#"#!/bin/sh
# Rewind: show history for a file or folder. Written by `rewind shell install`.
p="${1:-$PWD}"
GUI='@GUI@'
CLI='@CLI@'
[ -z "$GUI" ] && GUI="$(command -v rewind-gui 2>/dev/null)"
[ -z "$CLI" ] && CLI="$(command -v rewind 2>/dev/null)"
if [ -n "$GUI" ] && [ -x "$GUI" ]; then
  if [ -d "$p" ]; then exec "$GUI" --rewind-folder "$p"; else exec "$GUI" --rewind-file "$p"; fi
fi
[ -z "$CLI" ] && exit 1
for t in x-terminal-emulator gnome-terminal konsole xfce4-terminal kitty alacritty xterm; do
  if command -v "$t" >/dev/null 2>&1; then
    if [ "$t" = gnome-terminal ]; then sep="--"; else sep="-e"; fi
    exec "$t" $sep sh -c '"$0" history "$1"; printf "\nPress Enter to close..."; read _' "$CLI" "$p"
  fi
done
exit 1
"#
        .replace(
            "@GUI@",
            &gui.map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        )
        .replace(
            "@CLI@",
            &cli.map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
        );
        write_exec(&launcher, &script)?;
        let l = launcher.to_string_lossy().to_string();

        let f = files();
        write_exec(&f[0], &format!("#!/bin/sh\nexec \"{l}\" \"$@\"\n"))?;
        write_exec(
            &f[1],
            &format!(
                "[Desktop Entry]\nType=Service\nServiceTypes=KonqPopupMenu/Plugin\nMimeType=all/all;\nActions=rewindHistory;\nX-KDE-Priority=TopLevel\n\n[Desktop Action rewindHistory]\nName={MENU_TEXT}\nIcon=document-open-recent\nExec=\"{l}\" %f\n"
            ),
        )?;
        write_exec(
            &f[2],
            &format!(
                "[Nemo Action]\nName={MENU_TEXT}\nComment=Open the saved versions of this file or folder\nExec=\"{l}\" %F\nIcon-Name=document-open-recent\nSelection=s\nExtensions=any;dir;\n"
            ),
        )?;
        Ok(vec![
            "Nautilus (Files): right-click > Scripts > Rewind - show history".into(),
            "Dolphin: right-click > Rewind: show history (restart Dolphin if it is missing)".into(),
            "Nemo: right-click > Rewind: show history".into(),
            "Thunar and others: add a custom action that runs the launcher script (see docs)"
                .into(),
        ])
    }

    pub fn uninstall() -> Result<Vec<String>> {
        let mut removed = Vec::new();
        for f in files().into_iter().chain([launcher_path()]) {
            if f.exists() && fs::remove_file(&f).is_ok() {
                removed.push(format!("removed {}", f.display()));
            }
        }
        Ok(removed)
    }
}

// ---------------------------------------------------------------------------
// macOS and anything else
// ---------------------------------------------------------------------------

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
mod imp {
    use super::*;

    pub fn status() -> ShellStatus {
        ShellStatus {
            supported: false,
            installed: false,
            details: vec!["file-manager integration is not available on this system yet".into()],
        }
    }
    pub fn install(_gui: Option<&Path>, _cli: Option<&Path>) -> Result<Vec<String>> {
        Err(anyhow!(
            "file-manager integration is not available on this system yet"
        ))
    }
    pub fn uninstall() -> Result<Vec<String>> {
        Ok(Vec::new())
    }
}

pub fn shell_status() -> ShellStatus {
    imp::status()
}

/// Add the right-click entries. `gui` is the Rewind app, `cli` the `rewind` command; either may be
/// missing (Linux falls back to whatever is on the PATH).
pub fn shell_install(gui: Option<&Path>, cli: Option<&Path>) -> Result<Vec<String>> {
    imp::install(gui, cli)
}

pub fn shell_uninstall() -> Result<Vec<String>> {
    imp::uninstall()
}
