//! In-app updates from GitHub releases: check the latest release, download
//! Rustype.exe, verify its SHA-256 against the digest GitHub publishes, swap
//! it in place of the running exe and restart.

use eframe::egui;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const REPO: &str = "MusAhmedd/rustype";
const ASSET: &str = "Rustype.exe";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    url: String,
    /// Lowercase hex SHA-256 from GitHub's asset digest.
    sha256: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(Release),
    /// Bytes received / total (0 when unknown).
    Downloading(Release, u64, u64),
    /// New exe is in place; the app should restart.
    Installed,
    Failed(String),
}

impl Status {
    pub fn busy(&self) -> bool {
        matches!(self, Status::Checking | Status::Downloading(..) | Status::Installed)
    }
}

#[derive(Clone, Default)]
pub struct Updater {
    status: Arc<Mutex<Status>>,
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(concat!("Rustype/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// "v1.2.3" -> (1, 2, 3); missing parts count as 0.
fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).map(|p| p.parse::<u64>());
    let a = it.next()?.ok()?;
    let b = it.next().and_then(Result::ok).unwrap_or(0);
    let c = it.next().and_then(Result::ok).unwrap_or(0);
    Some((a, b, c))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

fn fetch_latest() -> Result<Option<Release>, String> {
    let url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let body = agent()
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "No releases published yet.".to_string(),
            ureq::Error::Status(code, _) => format!("GitHub answered {code}."),
            ureq::Error::Transport(_) => "Could not reach GitHub. Check your connection.".to_string(),
        })?
        .into_string()
        .map_err(|e| e.to_string())?;
    let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("Release has no version tag.")?;
    if !is_newer(tag, CURRENT) {
        return Ok(None);
    }
    let asset = v["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(ASSET)))
        .ok_or_else(|| format!("Release {tag} has no {ASSET} to download."))?;
    let sha256 = asset["digest"].as_str().and_then(|d| d.strip_prefix("sha256:")).map(str::to_lowercase);
    Ok(Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: v["body"].as_str().unwrap_or_default().replace("\r\n", "\n"),
        page: v["html_url"].as_str().unwrap_or_default().to_string(),
        url: asset["browser_download_url"].as_str().ok_or("Download link missing.")?.to_string(),
        sha256,
    }))
}

fn exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("Cannot find Rustype.exe: {e}"))
}

/// Removes the previous exe left behind by an update.
pub fn cleanup_old() {
    if let Ok(exe) = exe_path() {
        let _ = std::fs::remove_file(exe.with_extension("exe.old"));
        let _ = std::fs::remove_file(exe.with_extension("exe.new"));
    }
}

impl Updater {
    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn set(&self, s: Status, ctx: &egui::Context) {
        if let Ok(mut g) = self.status.lock() {
            *g = s;
        }
        ctx.request_repaint();
    }

    /// Asks GitHub for the latest release in the background.
    pub fn check(&self, ctx: &egui::Context) {
        if self.status().busy() {
            return;
        }
        self.set(Status::Checking, ctx);
        let (me, ctx) = (self.clone(), ctx.clone());
        std::thread::spawn(move || {
            let s = match fetch_latest() {
                Ok(Some(r)) => Status::Available(r),
                Ok(None) => Status::UpToDate,
                Err(e) => Status::Failed(e),
            };
            me.set(s, &ctx);
        });
    }

    /// Downloads, verifies and swaps in the new exe in the background.
    pub fn install(&self, ctx: &egui::Context) {
        let Status::Available(rel) = self.status() else { return };
        let (me, ctx) = (self.clone(), ctx.clone());
        self.set(Status::Downloading(rel.clone(), 0, 0), &ctx);
        std::thread::spawn(move || {
            let s = match me.download_and_swap(&rel, &ctx) {
                Ok(()) => Status::Installed,
                Err(e) => Status::Failed(e),
            };
            me.set(s, &ctx);
        });
    }

    fn download_and_swap(&self, rel: &Release, ctx: &egui::Context) -> Result<(), String> {
        let expected = rel.sha256.clone().ok_or("This release has no checksum, so it can't be installed safely. Download it from the release page.")?;
        let resp = agent().get(&rel.url).call().map_err(|e| format!("Download failed: {e}"))?;
        let total: u64 = resp.header("Content-Length").and_then(|v| v.parse().ok()).unwrap_or(0);
        let mut reader = resp.into_reader().take(MAX_DOWNLOAD);
        let mut data = Vec::with_capacity(total as usize);
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("Download failed: {e}"))?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&buf[..n]);
            self.set(Status::Downloading(rel.clone(), data.len() as u64, total), ctx);
        }
        let got: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();
        if got != expected {
            return Err("The download didn't match its checksum, so it was discarded.".into());
        }

        let exe = exe_path()?;
        let new = exe.with_extension("exe.new");
        let old = exe.with_extension("exe.old");
        std::fs::write(&new, &data).map_err(|e| format!("Can't write next to Rustype.exe ({e}). Download it from the release page instead."))?;
        let _ = std::fs::remove_file(&old);
        // Windows allows renaming a running exe, just not overwriting it.
        std::fs::rename(&exe, &old).map_err(|e| format!("Can't replace Rustype.exe: {e}"))?;
        if let Err(e) = std::fs::rename(&new, &exe) {
            let _ = std::fs::rename(&old, &exe); // put the old one back
            return Err(format!("Can't replace Rustype.exe: {e}"));
        }
        Ok(())
    }
}

/// Starts the (new) exe. Called after the app has saved everything.
pub fn relaunch() {
    if let Ok(exe) = exe_path() {
        let _ = std::process::Command::new(exe).spawn();
    }
}

/// Opens a web page in the default browser.
pub fn open_url(url: &str) {
    if url.starts_with("https://") {
        let _ = std::process::Command::new("explorer").arg(url).spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert!(is_newer("v0.2.0", "0.1.0"));
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("v0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
    }
}
