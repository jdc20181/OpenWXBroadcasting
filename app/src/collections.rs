//! Graphics collections and asset management: a shared collection folder synced between
//! workstations, dependency tracking (media, fonts, bindings, template), asset license notes,
//! and template compatibility checks.

use std::{collections::HashMap, path::Path};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    config::LibraryPaths,
    gfx::model::{GraphicDoc, LayerKind},
};

/// Two-way sync of the graphics library with a shared folder: the newer copy of each graphic
/// wins (by its `updated` time) and media the graphics need is copied across. Returns
/// (pulled, pushed).
pub fn sync_shared(folder: &str) -> Result<(usize, usize)> {
    let shared = Path::new(folder.trim());
    anyhow::ensure!(!folder.trim().is_empty(), "set a shared collection folder in Settings first");
    let shared_gfx = shared.join("Graphics");
    let shared_media = shared.join("Media");
    std::fs::create_dir_all(&shared_gfx)?;
    std::fs::create_dir_all(&shared_media)?;
    let paths = LibraryPaths::get();
    std::fs::create_dir_all(&paths.graphics)?;
    let updated = |p: &Path| -> Option<i64> {
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()?;
        v.get("updated")?.as_i64()
    };
    let jsons = |dir: &Path| -> Vec<String> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect()
    };
    let (mut pulled, mut pushed) = (0, 0);
    let mut names = jsons(&paths.graphics);
    names.extend(jsons(&shared_gfx));
    names.sort();
    names.dedup();
    for name in names {
        let (local, remote) = (paths.graphics.join(&name), shared_gfx.join(&name));
        match (updated(&local), updated(&remote)) {
            (Some(a), Some(b)) if b > a => {
                std::fs::copy(&remote, &local)?;
                pulled += 1;
            }
            (Some(a), Some(b)) if a > b => {
                std::fs::copy(&local, &remote)?;
                pushed += 1;
            }
            (None, Some(_)) => {
                std::fs::copy(&remote, &local)?;
                pulled += 1;
            }
            (Some(_), None) => {
                std::fs::copy(&local, &remote)?;
                pushed += 1;
            }
            _ => {}
        }
    }
    // Media both ways (never overwriting).
    for (from, to) in [(&paths.media, &shared_media), (&shared_media.to_path_buf(), &paths.media)] {
        for e in std::fs::read_dir(from).into_iter().flatten().flatten() {
            let name = e.file_name();
            let target = to.join(&name);
            if e.path().is_file() && !target.exists() && !name.to_string_lossy().starts_with("cam-") {
                let _ = std::fs::copy(e.path(), target);
            }
        }
    }
    Ok((pulled, pushed))
}

/// What a graphic needs to render as designed.
#[derive(Default)]
pub struct Dependencies {
    pub media: Vec<String>,
    pub fonts: Vec<String>,
    pub bindings: Vec<String>,
    pub template: Option<String>,
}

pub fn dependencies(doc: &GraphicDoc) -> Dependencies {
    let mut d = Dependencies { template: doc.template.clone(), ..Default::default() };
    let json = serde_json::to_string(doc).unwrap_or_default();
    let mut rest = json.as_str();
    while let Some(start) = rest.find("{{") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("}}") else { break };
        let key = after[..end].trim().to_string();
        if !key.is_empty() && key.len() < 60 && !d.bindings.contains(&key) {
            d.bindings.push(key);
        }
        rest = &after[end + 2..];
    }
    for l in &doc.layers {
        match &l.kind {
            // Built-in pictures ("motif:leaf:#D9611E") are drawn by the app, not files in the
            // media library: they are not dependencies (were listed and flagged "missing").
            LayerKind::Image { media, .. } if !media.trim().is_empty() && !media.contains("{{") && !crate::gfx::bind::is_brand_image(media) && !media.trim().starts_with("motif:") => {
                if !d.media.contains(media) {
                    d.media.push(media.clone());
                }
            }
            LayerKind::Text { font, .. } | LayerKind::Ticker { font, .. } if !font.starts_with('$') => {
                if !d.fonts.contains(font) {
                    d.fonts.push(font.clone());
                }
            }
            _ => {}
        }
    }
    d.bindings.sort();
    d
}

/// Graphics that use a media file.
pub fn used_by(media: &str, graphics: &[GraphicDoc]) -> Vec<String> {
    graphics
        .iter()
        .filter(|g| g.layers.iter().any(|l| matches!(&l.kind, LayerKind::Image { media: m, .. } if m.trim() == media)))
        .map(|g| g.name.clone())
        .collect()
}

/// License notes for a media file (where it came from and what may be done with it).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AssetMeta {
    pub source: String,
    pub license: String,
    pub credit: String,
    /// "YYYY-MM-DD" when the right to use it ends (empty = no end).
    pub expires: String,
}

fn meta_path() -> std::path::PathBuf {
    LibraryPaths::get().media.join(".asset-meta.json")
}

pub fn load_meta() -> HashMap<String, AssetMeta> {
    std::fs::read_to_string(meta_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_meta(name: &str, meta: AssetMeta) -> Result<()> {
    let mut all = load_meta();
    all.insert(name.to_string(), meta);
    std::fs::write(meta_path(), serde_json::to_string_pretty(&all)?)?;
    Ok(())
}

/// True when the license end date has passed.
pub fn expired(meta: &AssetMeta) -> bool {
    chrono::NaiveDate::parse_from_str(meta.expires.trim(), "%Y-%m-%d").map(|d| d < chrono::Local::now().date_naive()).unwrap_or(false)
}

/// Template compatibility: bindings the app no longer provides, missing media or fonts, a
/// template that no longer exists, an unusual canvas, and media whose license has ended.
pub fn validate(doc: &GraphicDoc, live: &crate::gfx::bind::Bindings) -> Vec<String> {
    let mut out = Vec::new();
    let deps = dependencies(doc);
    let known: Vec<String> = crate::gfx::bind::CATALOG
        .iter()
        .chain(crate::gfx::bind_ext::CATALOG_EXT.iter())
        .map(|(k, _)| normalize(k))
        .collect();
    for b in &deps.bindings {
        let n = normalize(b);
        // Families like day3.high, hour7.temp, threat2.town are catalogued by their first member.
        if live.get(b.split('|').next().unwrap_or(b).trim()).is_none() && !known.iter().any(|k| *k == n) {
            out.push(format!("Binding {{{{{b}}}}} is not provided by this version"));
        }
    }
    let media_dir = LibraryPaths::get().media;
    let meta = load_meta();
    for m in &deps.media {
        if !media_dir.join(m).exists() {
            out.push(format!("Media file \"{m}\" is missing"));
        }
        if let Some(info) = meta.get(m).filter(|i| expired(i)) {
            out.push(format!("License for \"{m}\" ended {}", info.expires));
        }
    }
    let families = crate::gfx::text::family_names();
    for f in &deps.fonts {
        if !families.iter().any(|x| x.eq_ignore_ascii_case(f)) {
            out.push(format!("Font \"{f}\" is not installed"));
        }
    }
    if let Some(t) = &deps.template {
        let exists = crate::gfx::templates::all().any(|i| i.key == t);
        if !exists {
            out.push(format!("Made from template \"{t}\", which this version no longer has"));
        }
    }
    if !((doc.width == 1920 && doc.height == 1080) || (doc.width == 1080 && doc.height == 1920)) {
        out.push(format!("Canvas is {}x{} (outputs are 1920x1080)", doc.width, doc.height));
    }
    out
}

/// Numbers in binding keys become "#" so day3.high matches day1.high in the catalogue.
fn normalize(key: &str) -> String {
    let key = key.split('|').next().unwrap_or(key).trim();
    let mut out = String::new();
    let mut last_digit = false;
    for c in key.chars() {
        if c.is_ascii_digit() {
            if !last_digit {
                out.push('#');
            }
            last_digit = true;
        } else {
            out.push(c);
            last_digit = false;
        }
    }
    out
}
