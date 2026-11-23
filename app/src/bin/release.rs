//! Release helper: signs an installer for the in-app updater and writes `latest.json`.
//!
//! ```text
//! cargo run --bin openwx-release -- bump            # next build number (MMDDYYYY + build of the day)
//! cargo run --release --bin openwx-release -- <installer.exe> <release-tag | download-base-url> [notes.md]
//! ```
//! The feed carries `version` and `build`; the app updates when either is newer.
//! cargo run --bin openwx-release -- keygen          # make the update-signing key pair (once)
//! Signing key: `%USERPROFILE%\.tauri\openwx-broadcaster.key` (override with OPENWX_SIGNING_KEY),
//! password from OPENWX_SIGNING_PASSWORD (empty by default). Upload the installer to
//! `<download-base-url>/<installer name>` and `dist/latest.json` to the update feed URL. With a release
//! tag (e.g. `0.1.2`) the download base is that GitHub release on jdc20181/OpenWXBroadcasting:
//! upload the installer and `dist/latest.json` to it.

use std::{io::Cursor, path::PathBuf};

use base64::Engine as _;
use sha2::{Digest, Sha256};

fn main() {
    if let Err(e) = run() {
        eprintln!("openwx-release: {e}");
        std::process::exit(1);
    }
}

/// The build number compiled into this binary (from BUILD_NUMBER).
fn current_build() -> u32 {
    env!("CWX_BUILD").parse().unwrap_or(1)
}

fn key_path() -> PathBuf {
    std::env::var("OPENWX_SIGNING_KEY").or_else(|_| std::env::var("CWX_SIGNING_KEY")).map(PathBuf::from).unwrap_or_else(|_| {
        dirs::home_dir().unwrap_or_default().join(".tauri").join("openwx-broadcaster.key")
    })
}

fn password() -> String {
    std::env::var("OPENWX_SIGNING_PASSWORD").or_else(|_| std::env::var("CWX_SIGNING_PASSWORD")).unwrap_or_default()
}

/// `openwx-release keygen`: a new update-signing key pair. The secret key goes to the key file
/// (never overwritten); the public key is printed for `UPDATE_PUBLIC_KEY` in src/updater.rs.
fn keygen() -> Result<(), Box<dyn std::error::Error>> {
    let path = key_path();
    if path.exists() {
        return Err(format!("{} already exists; move it away first (releases signed with it stop verifying in apps built with a new key)", path.display()).into());
    }
    let pair = minisign::KeyPair::generate_encrypted_keypair(Some(password()))?;
    let secret = pair.sk.to_box(Some("OpenWX Broadcasting update signing key"))?.to_string();
    let public = pair.pk.to_box()?.to_string();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&path, base64::engine::general_purpose::STANDARD.encode(secret))?;
    println!("Secret key written to {} (back it up somewhere safe and private).", path.display());
    println!("UPDATE_PUBLIC_KEY = \"{}\"", base64::engine::general_purpose::STANDARD.encode(public));
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("keygen") {
        return keygen();
    }
    // `openwx-release bump`: the next build number, by date (owner's scheme since 0.1.8):
    // MMDDYYYY + the build of the day, e.g. 111520262 = Nov 15 2026, 2nd build. A second bump
    // the same day counts up; a new day starts at 1. `bump --n 2` sets the day's count by hand.
    // Rebuild + package afterwards.
    if args.first().map(String::as_str) == Some("bump") {
        let current: u32 = std::fs::read_to_string("BUILD_NUMBER")
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or_else(current_build);
        // As a number, January to September lose the month's leading zero (010120271 -> 10120271).
        let today = chrono::Local::now().format("%m%d%Y").to_string().trim_start_matches('0').to_string();
        let cur = current.to_string();
        let forced = args.iter().position(|a| a == "--n").and_then(|i| args.get(i + 1)).and_then(|n| n.parse::<u32>().ok());
        let same_day = cur.len() > today.len() && cur.len() <= today.len() + 2 && cur.starts_with(&today);
        let n = forced.unwrap_or_else(|| if same_day { cur[today.len()..].parse::<u32>().unwrap_or(0) + 1 } else { 1 });
        let next: u32 = format!("{today}{n}").parse()?;
        std::fs::write("BUILD_NUMBER", format!("{next}\n"))?;
        println!("Build number {current} -> {next} (version {} unchanged)", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if args.len() < 2 {
        return Err("usage: openwx-release bump | openwx-release <installer.exe> <release-tag | download-base-url> [notes.md]".into());
    }
    let installer = PathBuf::from(&args[0]);
    // A bare tag means the GitHub release of that name.
    let base_url = if args[1].starts_with("http") {
        args[1].trim_end_matches('/').to_string()
    } else {
        format!("https://github.com/jdc20181/OpenWXBroadcasting/releases/download/{}", args[1])
    };
    let notes = match args.get(2) {
        Some(path) => std::fs::read_to_string(path)?,
        None => latest_changelog_section(),
    };

    let key_path = key_path();
    let password = password();

    // The key file holds the minisign secret key box, base64-encoded.
    let key_b64 = std::fs::read_to_string(&key_path)
        .map_err(|e| format!("cannot read signing key {}: {e}", key_path.display()))?;
    let key_text = String::from_utf8(
        base64::engine::general_purpose::STANDARD.decode(key_b64.trim())?,
    )?;
    let secret = minisign::SecretKeyBox::from_string(&key_text)?.into_secret_key(Some(password))?;

    let data = std::fs::read(&installer)?;
    let name = installer
        .file_name()
        .ok_or("installer has no file name")?
        .to_string_lossy()
        .to_string();
    let signature = minisign::sign(
        None,
        &secret,
        Cursor::new(&data),
        Some(&format!("file:{name}")),
        Some("OpenWXBroadcasting update"),
    )?;
    let signature_b64 = base64::engine::general_purpose::STANDARD.encode(signature.to_string());
    let sha256: String = Sha256::digest(&data).iter().map(|b| format!("{b:02x}")).collect();

    let version = env!("CARGO_PKG_VERSION");
    let build = current_build();
    let manifest = serde_json::json!({
        "version": version,
        "build": build,
        "pub_date": chrono::Utc::now().to_rfc3339(),
        "notes": notes,
        "platforms": {
            "windows-x86_64": {
                "url": format!("{base_url}/{name}"),
                "sha256": sha256,
                "signature": signature_b64,
            }
        }
    });

    std::fs::create_dir_all("dist")?;
    std::fs::write("dist/latest.json", serde_json::to_string_pretty(&manifest)?)?;
    std::fs::write(format!("dist/{name}.sig"), signature.to_string())?;
    println!("Signed {name} ({} bytes), version {version} build {build}", data.len());
    println!("Wrote dist/latest.json - upload it to the update feed, and {name} to {base_url}/");
    Ok(())
}

/// The newest non-empty section of CHANGELOG.md, used as release notes by default. An empty
/// "## Unreleased" heading is skipped. The file is read from disk when run from the repo root,
/// so notes written after this tool was built are still picked up.
fn latest_changelog_section() -> String {
    let text = std::fs::read_to_string("CHANGELOG.md").unwrap_or_else(|_| include_str!("../../../CHANGELOG.md").to_string());
    newest_section(&text)
}

fn newest_section(text: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.starts_with("## ") {
            if out.iter().skip(1).any(|l| !l.trim().is_empty()) {
                break;
            }
            out.clear();
        }
        if !out.is_empty() || line.starts_with("## ") {
            out.push(line);
        }
    }
    out.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_unreleased_is_skipped() {
        let text = "# Changelog\n\n## Unreleased\n\n## v1 build 2\n\n- fixed a thing\n\n## v0\n\n- old\n";
        assert_eq!(super::newest_section(text), "## v1 build 2\n\n- fixed a thing");
        assert_eq!(super::newest_section("## Unreleased\n\n- next\n\n## v1\n- a\n"), "## Unreleased\n\n- next");
    }
}
