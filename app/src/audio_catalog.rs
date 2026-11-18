//! Bundled CC0 music. The catalog is available without scanning the install directory.

use std::{path::Path, sync::OnceLock};

use anyhow::{anyhow, Context as _, Result};
use serde::Deserialize;

const RESOURCE_DIR: &str = "assets/music/abstraction";
const CATALOG: &str = include_str!("../assets/music/abstraction/catalog.json");

#[derive(Deserialize)]
pub struct Track {
    pub title: String,
    file: String,
}

impl Track {
    pub fn bundle(&self) -> &str {
        self.file.split_once('/').map_or("", |(bundle, _)| bundle.trim_start_matches("music-loop-bundle-"))
    }

    pub fn copy_to_media(&self) -> Result<String> {
        let exe = std::env::current_exe()?;
        let install = exe.parent().ok_or_else(|| anyhow!("Could not locate the application folder"))?;
        let mut resource_root = install.join(RESOURCE_DIR);
        #[cfg(target_os = "linux")]
        if !resource_root.is_dir() && install.ends_with("usr/bin") {
            resource_root = install.join("../lib/openwx-broadcaster").join(RESOURCE_DIR);
        }
        if cfg!(debug_assertions) && !resource_root.is_dir() {
            resource_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(RESOURCE_DIR);
        }
        let path = resource_root.join(&self.file);
        let filename = path.file_name().and_then(|name| name.to_str())
            .ok_or_else(|| anyhow!("Invalid bundled track name"))?;
        let bytes = std::fs::read(&path).with_context(|| format!("Bundled track unavailable: {}", self.title))?;
        crate::library::import_builtin_track(filename, &bytes)
    }
}

pub fn tracks() -> &'static [Track] {
    static TRACKS: OnceLock<Vec<Track>> = OnceLock::new();
    TRACKS.get_or_init(|| serde_json::from_str(CATALOG).unwrap_or_else(|error| {
        log::error!("Could not read the bundled music catalog: {error}");
        Vec::new()
    }))
}
