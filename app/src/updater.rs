//! Update checking and installing.
//!
//! Releases are published on GitHub (https://github.com/jdc20181/OpenWXBroadcasting/releases). The app reads the
//! repository's release list (`settings.update_feed`, default the repository address): the Stable
//! channel takes the newest full release, or the newest prerelease while there is none (alpha);
//! Beta takes the newest release of any kind. Each release carries the installer and a signed
//! `latest.json` written by `openwx-release`. Any other address is read as a `latest.json` feed:
//! ```json
//! {
//!   "version": "0.2.0",
//!   "pub_date": "2026-11-20T12:00:00Z",
//!   "notes": "## 0.2.0\n- What changed…",
//!   "platforms": {
//!     "windows-x86_64": { "url": "https://…/OpenWXBroadcasting_0.2.0_x64-setup.exe",
//!                          "sha256": "…", "signature": "<minisign signature, base64>" }
//!   }
//! }
//! ```
//! `cargo run --bin openwx-release` builds this file. Installers are verified with the bundled
//! minisign public key and SHA-256 before they run.

use std::path::PathBuf;

use anyhow::{anyhow, Context as _, Result};
use base64::Engine as _;
use futures_util::StreamExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    config::{APP_BUILD, APP_VERSION},
    net,
};

/// Public half of the OpenWX release signing key (`openwx-release keygen`; the secret half stays
/// with the maintainer in ~/.tauri/openwx-broadcaster.key).
pub const UPDATE_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IEQzM0MwMDAzNUEzMDVCRjMKUldUeld6QmFBd0E4MHhITXNJQ0JpamFvSjFsNTExcUFOcUt2QXdOUFM4SHlOSk9MNmVzbDlEL0kK";

pub const PLATFORM: &str = "windows-x86_64";

/// Where OpenWX releases are published.
pub const DEFAULT_FEED: &str = "https://github.com/jdc20181/OpenWXBroadcasting";

/// `(owner, repository)` when the feed is a GitHub repository address
/// (`https://github.com/<owner>/<repo>`, with or without anything after it).
pub fn github_repo(feed: &str) -> Option<(String, String)> {
    let rest = feed.trim().strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/').filter(|p| !p.is_empty());
    let owner = parts.next()?;
    let repo = parts.next()?.trim_end_matches(".git");
    Some((owner.to_owned(), repo.to_owned()))
}

#[derive(Clone, Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct GithubRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

/// The release a channel installs from GitHub's list (newest first). Stable: the newest full
/// release, or the newest prerelease when there are only prereleases. Beta: the newest of any kind.
fn pick_release(releases: &[GithubRelease], beta: bool) -> Option<&GithubRelease> {
    let published = || releases.iter().filter(|r| !r.draft);
    if beta {
        published().next()
    } else {
        published().find(|r| !r.prerelease).or_else(|| published().next())
    }
}

/// The update manifest of the release the channel would install, read from its `latest.json`.
async fn github_manifest(owner: &str, repo: &str, beta: bool) -> Result<UpdateManifest> {
    let api = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=30");
    let response = net::http()
        .get(&api)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .with_context(|| format!("could not reach GitHub ({owner}/{repo})"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow!("GitHub has no repository {owner}/{repo}"));
    }
    if response.status() == reqwest::StatusCode::FORBIDDEN || response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(anyhow!("GitHub is limiting update checks from this network; try again in an hour"));
    }
    if !response.status().is_success() {
        return Err(anyhow!("{} from GitHub", response.status()));
    }
    let releases: Vec<GithubRelease> = response.json().await.context("GitHub's release list is not valid JSON")?;
    let Some(release) = pick_release(&releases, beta) else {
        return Err(anyhow!("no releases have been published at github.com/{owner}/{repo} yet"));
    };
    let feed = release.assets.iter().find(|a| a.name == "latest.json").ok_or_else(|| {
        anyhow!("release {} has no latest.json, so it can't be installed from here; download it from {}", release.tag_name, release.html_url)
    })?;
    let mut manifest: UpdateManifest = net::http()
        .get(&feed.browser_download_url)
        .send()
        .await
        .with_context(|| format!("could not download latest.json from release {}", release.tag_name))?
        .error_for_status()?
        .json()
        .await
        .context("latest.json in the release is not valid JSON")?;
    // The installer is downloaded from the same release, whatever address latest.json was made with.
    if let Some(asset) = manifest.platforms.get_mut(PLATFORM) {
        let name = asset.url.rsplit('/').next().unwrap_or_default().to_owned();
        if let Some(found) = release.assets.iter().find(|a| !name.is_empty() && a.name == name) {
            asset.url = found.browser_download_url.clone();
        }
    }
    Ok(manifest)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlatformAsset {
    pub url: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    /// Build number within `version` (0 = not given, older feeds).
    #[serde(default)]
    pub build: u32,
    #[serde(default)]
    pub pub_date: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub platforms: std::collections::HashMap<String, PlatformAsset>,
}

#[derive(Clone, Debug)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate { checked_at: chrono::DateTime<chrono::Local> },
    Available(UpdateManifest),
    Downloading { version: String, downloaded: u64, total: Option<u64> },
    Ready { version: String, installer: PathBuf },
    Failed(String),
}

impl UpdateState {
    pub fn summary(&self) -> String {
        match self {
            UpdateState::Idle => "Not checked yet".into(),
            UpdateState::Checking => "Checking for updates…".into(),
            UpdateState::UpToDate { checked_at } => {
                format!("Up to date (checked {})", checked_at.format("%-I:%M %p"))
            }
            UpdateState::Available(m) => format!("Version {} is available", m.label()),
            UpdateState::Downloading { version, downloaded, total } => match total {
                Some(total) if *total > 0 => format!(
                    "Downloading {version}… {:.0}%",
                    *downloaded as f64 / *total as f64 * 100.0
                ),
                _ => format!("Downloading {version}… {}", crate::library::human_size(*downloaded)),
            },
            UpdateState::Ready { version, .. } => format!("Version {version} is ready to install"),
            UpdateState::Failed(e) => format!("Update check failed: {e}"),
        }
    }
}

/// Newer = higher version, or the same version with a later build.
fn is_newer(candidate: &str, candidate_build: u32, current: &str, current_build: u32) -> bool {
    match (
        semver::Version::parse(candidate.trim_start_matches('v')),
        semver::Version::parse(current.trim_start_matches('v')),
    ) {
        (Ok(c), Ok(cur)) => c > cur || (c == cur && crate::config::build_order(candidate_build) > crate::config::build_order(current_build)),
        _ => false,
    }
}

impl UpdateManifest {
    pub fn label(&self) -> String {
        if self.build > 0 {
            format!("{} (build {})", self.version, self.build)
        } else {
            self.version.clone()
        }
    }
}

/// Fetch the feed. Returns Some(manifest) when a newer version exists for this platform.
pub async fn check(feed: String, beta: bool) -> Result<Option<UpdateManifest>> {
    let manifest = match github_repo(&feed) {
        Some((owner, repo)) => github_manifest(&owner, &repo, beta).await?,
        None => json_feed(&feed).await?,
    };
    if is_newer(&manifest.version, manifest.build, APP_VERSION, APP_BUILD) && manifest.platforms.contains_key(PLATFORM) {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
}

/// A `latest.json` feed at any address.
async fn json_feed(feed: &str) -> Result<UpdateManifest> {
    let response = net::http()
        .get(feed)
        .header("Cache-Control", "no-cache")
        .send()
        .await
        .with_context(|| format!("could not reach {feed}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow!("no update feed has been published at {feed} yet"));
    }
    if !response.status().is_success() {
        return Err(anyhow!("{} from {feed}", response.status()));
    }
    response.json().await.context("update feed is not valid JSON")
}

fn decode_b64_text(value: &str) -> Result<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(value.trim())
        .context("not base64")?;
    String::from_utf8(bytes).context("not UTF-8")
}

/// Verify a downloaded installer against the bundled minisign key.
pub fn verify_signature(data: &[u8], signature_b64: &str) -> Result<()> {
    let public_key = minisign_verify::PublicKey::decode(&decode_b64_text(UPDATE_PUBLIC_KEY)?)
        .map_err(|e| anyhow!("public key: {e}"))?;
    let signature = minisign_verify::Signature::decode(&decode_b64_text(signature_b64)?)
        .map_err(|e| anyhow!("signature: {e}"))?;
    public_key
        .verify(data, &signature, false)
        .map_err(|e| anyhow!("signature does not match: {e}"))
}

/// Download, verify and stage the installer. `progress(downloaded, total)` is called as data arrives.
pub async fn download(
    manifest: UpdateManifest,
    progress: impl Fn(u64, Option<u64>) + Send + 'static,
) -> Result<PathBuf> {
    let asset = manifest
        .platforms
        .get(PLATFORM)
        .ok_or_else(|| anyhow!("no installer for {PLATFORM}"))?
        .clone();
    let response = net::http()
        .get(&asset.url)
        .timeout(std::time::Duration::from_secs(60 * 15))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!("{} downloading the installer", response.status()));
    }
    let total = response.content_length();
    let mut stream = response.bytes_stream();
    let mut data = Vec::with_capacity(total.unwrap_or(0) as usize);
    while let Some(chunk) = stream.next().await {
        data.extend_from_slice(&chunk?);
        progress(data.len() as u64, total);
    }

    if !asset.sha256.is_empty() {
        let digest = Sha256::digest(&data);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        if !hex.eq_ignore_ascii_case(asset.sha256.trim()) {
            return Err(anyhow!("download is corrupt (SHA-256 mismatch)"));
        }
    }
    if asset.signature.trim().is_empty() {
        return Err(anyhow!("the update is not signed; refusing to install"));
    }
    verify_signature(&data, &asset.signature)?;

    let dir = crate::config::update_dir();
    std::fs::create_dir_all(&dir)?;
    let file_name = asset
        .url
        .rsplit('/')
        .next()
        .filter(|n| n.ends_with(".exe") || n.ends_with(".msi"))
        .map(String::from)
        .unwrap_or_else(|| format!("OpenWXBroadcasting_{}_setup.exe", manifest.version));
    let path = dir.join(file_name);
    tokio::fs::write(&path, &data).await?;
    Ok(path)
}

/// Installers downloaded for earlier updates, newest first (for rolling back).
pub fn previous_installers() -> Vec<PathBuf> {
    let dir = crate::config::update_dir();
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().to_lowercase();
            n.ends_with(".exe") || n.ends_with(".msi")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.into_iter().map(|(_, p)| p).collect()
}

/// Run the staged installer and quit so it can replace the app.
pub fn install_and_exit(installer: &PathBuf) -> Result<()> {
    let is_msi = installer
        .extension()
        .map(|e| e.eq_ignore_ascii_case("msi"))
        .unwrap_or(false);
    if is_msi {
        std::process::Command::new("msiexec")
            .arg("/i")
            .arg(installer)
            .arg("/passive")
            .spawn()?;
    } else {
        // NSIS: /P = passive progress UI, /R = relaunch when done (cargo-packager installers).
        std::process::Command::new(installer).args(["/P", "/R"]).spawn()?;
    }
    std::process::exit(0);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_order_and_rollback_are_consistent() {
        assert!(is_newer("0.1.1", 3, "0.1.0", 99));
        assert!(is_newer("0.1.0", 3, "0.1.0", 2));
        assert!(!is_newer("0.1.0", 99, "0.1.1", 3));
        assert!(!is_newer("0.1.1", 3, "0.1.1", 3));
        assert!(!is_newer("invalid", 100, "0.1.1", 3));
    }
    #[test]
    fn github_feeds_and_channels() {
        assert_eq!(github_repo(DEFAULT_FEED), Some(("jdc20181".into(), "OpenWXBroadcasting".into())));
        assert_eq!(github_repo("https://github.com/a/b.git/"), Some(("a".into(), "b".into())));
        assert_eq!(github_repo("https://example.org/latest.json"), None);
        let rel = |tag: &str, draft: bool, prerelease: bool| GithubRelease {
            tag_name: tag.into(), html_url: String::new(), draft, prerelease, assets: vec![],
        };
        // Alpha: only prereleases, so both channels take the newest one (drafts never count).
        let alpha = [rel("draft", true, false), rel("0.1.2", false, true), rel("0.1.0", false, true)];
        assert_eq!(pick_release(&alpha, false).unwrap().tag_name, "0.1.2");
        assert_eq!(pick_release(&alpha, true).unwrap().tag_name, "0.1.2");
        // Once a full release exists, Stable stays on it and Beta takes newer prereleases.
        let mixed = [rel("0.2.1-beta", false, true), rel("0.2.0", false, false), rel("0.1.2", false, true)];
        assert_eq!(pick_release(&mixed, false).unwrap().tag_name, "0.2.0");
        assert_eq!(pick_release(&mixed, true).unwrap().tag_name, "0.2.1-beta");
        assert!(pick_release(&[], false).is_none());
    }

    #[test]
    fn invalid_installer_signatures_are_rejected() {
        assert!(verify_signature(b"not an installer", "invalid").is_err());
        let unsigned = base64::engine::general_purpose::STANDARD.encode("untrusted content");
        assert!(verify_signature(b"not an installer", &unsigned).is_err());
    }
}
