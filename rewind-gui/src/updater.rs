//! GitHub-release updater + optional anonymous usage ping.
//!
//! Flow: ask the GitHub API for the latest release, compare it with this build's
//! version, download the `*-setup.exe` asset, verify its SHA-256 against the
//! `<name>.sha256` asset published by the release workflow, then run it silently.
use crate::settings::Settings;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

pub const REPO: &str = "hcentral07/rewind";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// URL of your stats worker (see docs/stats-worker.js). Leave empty to disable pings entirely.
pub const STATS_URL: &str = "";

const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    /// true when the release notes contain the marker `[required]`.
    pub required: bool,
    pub url: String,
    pub name: String,
    pub sha256_url: Option<String>,
    pub page: String,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    body: Option<String>,
    html_url: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("rewind/{CURRENT}"))
        .build()
}

/// Ok(None) = up to date. Network problems come back as Err and should be ignored silently.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let rel: Release = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    if rel.draft || rel.prerelease {
        return Ok(None);
    }
    let latest =
        semver::Version::parse(rel.tag_name.trim_start_matches('v')).map_err(|e| e.to_string())?;
    let current = semver::Version::parse(CURRENT).map_err(|e| e.to_string())?;
    if latest <= current {
        return Ok(None);
    }
    let setup = rel
        .assets
        .iter()
        .find(|a| a.name.ends_with("-setup.exe"))
        .ok_or("the release has no setup.exe asset")?;
    let sha_name = format!("{}.sha256", setup.name);
    let notes = rel.body.unwrap_or_default();
    Ok(Some(UpdateInfo {
        version: latest.to_string(),
        required: notes.contains("[required]"),
        notes,
        url: setup.browser_download_url.clone(),
        name: setup.name.clone(),
        sha256_url: rel
            .assets
            .iter()
            .find(|a| a.name == sha_name)
            .map(|a| a.browser_download_url.clone()),
        page: rel.html_url,
    }))
}

/// Download, verify and launch the installer. The caller should exit the app right after Ok.
pub fn download_and_run(info: &UpdateInfo) -> Result<(), String> {
    let sha_url = info
        .sha256_url
        .as_ref()
        .ok_or("this release has no .sha256 file, refusing to install it")?;
    let want = agent()
        .get(sha_url)
        .call()
        .map_err(|e| e.to_string())?
        .into_string()
        .map_err(|e| e.to_string())?
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    let mut buf = Vec::new();
    agent()
        .get(&info.url)
        .call()
        .map_err(|e| e.to_string())?
        .into_reader()
        .take(MAX_DOWNLOAD)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;

    let got = format!("{:x}", Sha256::digest(&buf));
    if got != want {
        return Err("downloaded file failed its checksum, update cancelled".into());
    }

    let dir: PathBuf = std::env::temp_dir().join("Rewind-update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let file = dir.join(&info.name);
    std::fs::write(&file, &buf).map_err(|e| e.to_string())?;
    std::process::Command::new(&file)
        .arg("/S") // NSIS silent install
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Anonymous, opt-in, at most once a day. Sends: random id, app version, OS. Nothing else.
pub fn ping(s: &mut Settings) {
    if STATS_URL.is_empty() || !s.share_usage {
        return;
    }
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    if s.last_ping == today {
        return;
    }
    let body = serde_json::json!({ "id": s.install_id, "v": CURRENT, "os": std::env::consts::OS });
    if agent().post(STATS_URL).send_json(body).is_ok() {
        s.last_ping = today;
    }
}
