//! Update checking and installing.
//!
//! The feed (`settings.update_feed`, default https://dcweather.xyz/broadcaster/latest.json) is:
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

/// Public half of the release signing key (~/.tauri/openwx-broadcaster.key.pub).
pub const UPDATE_PUBLIC_KEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDM5RjgxOEUxQjBGRkZCRDkKUldUWisvK3c0Umo0T1hCQnhJYVhXb3B6MnFXUkRZV2crQ2lRSm1DK0NEV0V2NUJxb0dWai8rR3EK";

pub const PLATFORM: &str = "windows-x86_64";

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
pub async fn check(feed: String) -> Result<Option<UpdateManifest>> {
    let response = net::http()
        .get(&feed)
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
    let manifest: UpdateManifest = response.json().await.context("update feed is not valid JSON")?;
    if is_newer(&manifest.version, manifest.build, APP_VERSION, APP_BUILD) && manifest.platforms.contains_key(PLATFORM) {
        Ok(Some(manifest))
    } else {
        Ok(None)
    }
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
    fn invalid_installer_signatures_are_rejected() {
        assert!(verify_signature(b"not an installer", "invalid").is_err());
        let unsigned = base64::engine::general_purpose::STANDARD.encode("untrusted content");
        assert!(verify_signature(b"not an installer", &unsigned).is_err());
    }
}
