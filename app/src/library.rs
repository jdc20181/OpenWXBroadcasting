//! The on-disk library: shows, graphics, media and exports in Documents\OpenWXBroadcasting.
//! Deleting moves files to a `.trash` folder instead of removing them.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{de::DeserializeOwned, Serialize};

use crate::{config::LibraryPaths, gfx::model::GraphicDoc, show::Show};

static MEDIA_IMPORT: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn safe_id(id: &str) -> Result<&str> {
    if !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Ok(id)
    } else {
        Err(anyhow!("invalid document id"))
    }
}

fn write_json<T: Serialize>(dir: &Path, id: &str, value: &T) -> Result<PathBuf> {
    let id = safe_id(id)?;
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{id}.json"));
    let tmp = dir.join(format!("{id}.json.tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(value)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

fn read_all<T: DeserializeOwned>(dir: &Path) -> Vec<T> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .filter_map(|e| {
            let text = std::fs::read_to_string(e.path()).ok()?;
            match serde_json::from_str(&text) {
                Ok(value) => Some(value),
                Err(err) => {
                    log::warn!("skipping {}: {err}", e.path().display());
                    None
                }
            }
        })
        .collect()
}

fn trash(paths: &LibraryPaths, file: &Path) -> Result<()> {
    if !file.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&paths.trash)?;
    let name = file
        .file_name()
        .ok_or_else(|| anyhow!("bad file name"))?
        .to_string_lossy()
        .to_string();
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    std::fs::rename(file, paths.trash.join(format!("{stamp}_{name}")))?;
    Ok(())
}

pub fn list_graphics() -> Vec<GraphicDoc> {
    let mut docs: Vec<GraphicDoc> = read_all(&LibraryPaths::get().graphics);
    docs.sort_by(|a, b| b.updated.cmp(&a.updated));
    docs
}

pub fn save_graphic(doc: &mut GraphicDoc) -> Result<PathBuf> {
    doc.updated = chrono::Utc::now().timestamp();
    let path = write_json(&LibraryPaths::get().graphics, &doc.id, doc)?;
    save_version(doc);
    Ok(path)
}

fn versions_dir(id: &str) -> PathBuf {
    LibraryPaths::get().graphics.join(".versions").join(id.replace(['/', '\\', '.'], "_"))
}

/// Keep the last 25 saved versions of a graphic (Graphics Studio > Versions).
fn save_version(doc: &GraphicDoc) {
    let dir = versions_dir(&doc.id);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    if let Ok(json) = serde_json::to_string(doc) {
        let _ = std::fs::write(dir.join(format!("{}.json", doc.updated)), json);
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    files.sort();
    while files.len() > 25 {
        let _ = std::fs::remove_file(files.remove(0));
    }
}

/// Saved versions of a graphic, newest first: (unix time, file).
pub fn list_versions(id: &str) -> Vec<(i64, PathBuf)> {
    let mut out: Vec<(i64, PathBuf)> = std::fs::read_dir(versions_dir(id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            let t = p.file_stem()?.to_str()?.parse().ok()?;
            Some((t, p))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

pub fn load_version(path: &Path) -> Result<GraphicDoc> {
    Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
}

/// Thumbnail file for a graphic (written on save by Graphics Studio).
pub fn thumb_path(id: &str) -> PathBuf {
    LibraryPaths::get().graphics.join(".thumbs").join(format!("{}.png", id.replace(['/', '\\', '.'], "_")))
}

/// Media files referenced by any graphic, the brand or the sponsors.
pub fn media_in_use(graphics: &[GraphicDoc], settings: &crate::config::Settings) -> std::collections::HashSet<String> {
    use crate::gfx::model::LayerKind;
    let mut used = std::collections::HashSet::new();
    for g in graphics {
        used.insert(g.audio.trim().to_string());
        for l in &g.layers {
            if let LayerKind::Image { media, .. } = &l.kind {
                used.insert(media.trim().to_string());
            }
        }
    }
    used.extend(settings.brand.logo_media.clone());
    used.extend(settings.sponsor_logo_media.clone());
    used.extend(settings.sponsors.iter().filter_map(|s| s.logo_media.clone()));
    used
}

/// Move media nobody uses to the library trash. Returns how many files moved.
pub fn clean_unused_media(graphics: &[GraphicDoc], settings: &crate::config::Settings, current_show: &Show) -> Result<usize> {
    let paths = LibraryPaths::get();
    let mut used = media_in_use(graphics, settings);
    let shows = list_shows();
    for scene in shows.iter().chain(std::iter::once(current_show)).flat_map(|show| &show.scenes) {
        used.extend(scene.soundtrack.clone());
        match &scene.kind {
            crate::show::SceneKind::Clip { media } => { used.insert(media.clone()); }
            crate::show::SceneKind::Graphic { doc, .. } => { used.extend(media_in_use(std::slice::from_ref(doc), settings)); }
            crate::show::SceneKind::Map { .. } => {}
        }
    }
    let mut moved = 0;
    for e in std::fs::read_dir(&paths.media)?.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if e.path().is_file() && !used.contains(&name) && !name.starts_with('.') {
            trash(&paths, &e.path())?;
            moved += 1;
        }
    }
    Ok(moved)
}

/// Graphics whose images point at files that are not in the media library: (graphic, file).
pub fn missing_media(graphics: &[GraphicDoc]) -> Vec<(String, String)> {
    use crate::gfx::model::LayerKind;
    let dir = LibraryPaths::get().media;
    let mut out = Vec::new();
    for g in graphics {
        for l in &g.layers {
            if let LayerKind::Image { media, .. } = &l.kind {
                let m = media.trim();
                // Built-in motifs are drawn by the app, never files: not "missing".
                if !m.is_empty() && !m.contains("{{") && !crate::gfx::bind::is_brand_image(m) && !m.starts_with("motif:") && !dir.join(m).exists() {
                    out.push((g.name.clone(), m.to_string()));
                }
            }
        }
    }
    out
}

/// Zip graphics (and the media they use) into a package for another computer or station.
pub fn export_package(graphics: &[GraphicDoc], target: &Path) -> Result<usize> {
    use std::io::Write as _;
    use crate::gfx::model::LayerKind;
    let file = std::fs::File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let media_dir = LibraryPaths::get().media;
    let mut media = std::collections::BTreeSet::new();
    for g in graphics {
        zip.start_file(format!("graphics/{}.json", g.id), opts)?;
        zip.write_all(serde_json::to_string_pretty(g)?.as_bytes())?;
        for l in &g.layers {
            if let LayerKind::Image { media: m, .. } = &l.kind {
                if !m.trim().is_empty() && !m.contains("{{") && !crate::gfx::bind::is_brand_image(m) {
                    media.insert(m.trim().to_string());
                }
            }
        }
    }
    for m in media {
        if let Ok(bytes) = std::fs::read(media_dir.join(&m)) {
            zip.start_file(format!("media/{m}"), opts)?;
            zip.write_all(&bytes)?;
        }
    }
    zip.finish()?;
    Ok(graphics.len())
}

/// Import a package made by [`export_package`]. Graphics get new ids so nothing is overwritten;
/// media files keep their names (existing files are kept). Returns graphics imported.
pub fn import_package(source: &Path) -> Result<usize> {
    use std::io::Read as _;
    let paths = LibraryPaths::get();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(source)?)?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let name = entry.name().map(|n| n.into_owned()).unwrap_or_default();
        if name.contains("..") {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if let Some(file) = name.strip_prefix("media/") {
            let target = paths.media.join(file);
            if !target.exists() && !file.contains(['/', '\\']) {
                std::fs::write(target, &bytes)?;
            }
        } else if name.starts_with("graphics/") && name.ends_with(".json") {
            let mut doc: GraphicDoc = serde_json::from_slice(&bytes)?;
            doc.id = crate::gfx::model::new_id();
            if doc.folder.is_empty() {
                doc.folder = "Imported".into();
            }
            save_graphic(&mut doc)?;
            count += 1;
        }
    }
    Ok(count)
}

pub fn delete_graphic(id: &str) -> Result<()> {
    let paths = LibraryPaths::get();
    trash(&paths, &paths.graphics.join(format!("{}.json", safe_id(id)?)))
}

pub fn list_shows() -> Vec<Show> {
    let mut shows: Vec<Show> = read_all(&LibraryPaths::get().shows);
    shows.sort_by(|a, b| b.updated.cmp(&a.updated));
    shows
}

pub fn save_show(show: &mut Show) -> Result<PathBuf> {
    show.updated = chrono::Utc::now().timestamp();
    write_json(&LibraryPaths::get().shows, &show.id, show)
}

pub fn delete_show(id: &str) -> Result<()> {
    let paths = LibraryPaths::get();
    trash(&paths, &paths.shows.join(format!("{}.json", safe_id(id)?)))
}

#[derive(Clone, Debug)]
pub struct MediaItem {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub kind: MediaKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
    Other,
}

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];
/// Extensions the media import accepts (SVG is converted to PNG on import).
pub const IMPORT_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "svg"];

/// Render an SVG file to PNG bytes with its longest side at `size` px.
pub fn render_svg(path: &Path, size: u32) -> Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    let mut opt = resvg::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = resvg::usvg::Tree::from_data(&data, &opt).map_err(|e| anyhow!("SVG: {e}"))?;
    let sz = tree.size();
    let scale = size as f32 / sz.width().max(sz.height()).max(1.0);
    let (w, h) = ((sz.width() * scale).ceil() as u32, (sz.height() * scale).ceil() as u32);
    let mut pm = resvg::tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or_else(|| anyhow!("SVG too large"))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    pm.encode_png().map_err(|e| anyhow!("PNG: {e}"))
}
pub const VIDEO_EXTS: &[&str] = &["mp4", "mov", "webm", "mkv"];
pub const AUDIO_EXTS: &[&str] = &["wav", "mp3", "ogg", "flac"];

pub fn is_audio(path: &Path) -> bool {
    path.extension().and_then(|ext| ext.to_str())
        .is_some_and(|ext| AUDIO_EXTS.iter().any(|audio| ext.eq_ignore_ascii_case(audio)))
}

fn kind_of(path: &Path) -> MediaKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if IMAGE_EXTS.contains(&ext.as_str()) {
        MediaKind::Image
    } else if VIDEO_EXTS.contains(&ext.as_str()) {
        MediaKind::Video
    } else {
        MediaKind::Other
    }
}

fn list_dir(dir: &Path) -> Vec<MediaItem> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut items: Vec<(MediaItem, std::time::SystemTime)> = entries
        .flatten()
        .filter(|e| e.path().is_file())
        .map(|e| {
            let meta = e.metadata().ok();
            (
                MediaItem {
                    name: e.file_name().to_string_lossy().to_string(),
                    path: e.path(),
                    bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                    kind: kind_of(&e.path()),
                },
                meta.and_then(|m| m.modified().ok())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH),
            )
        })
        .collect();
    items.sort_by(|a, b| b.1.cmp(&a.1));
    items.into_iter().map(|(item, _)| item).collect()
}

pub fn list_media() -> Vec<MediaItem> {
    list_dir(&LibraryPaths::get().media)
}

pub fn list_exports() -> Vec<MediaItem> {
    list_dir(&LibraryPaths::get().exports)
}

/// Copy files into the media library (unique names). Returns the stored names.
pub fn import_media(files: &[PathBuf]) -> Result<Vec<String>> {
    let _import = MEDIA_IMPORT.lock().map_err(|_| anyhow!("Media import is unavailable"))?;
    let dir = LibraryPaths::get().media;
    std::fs::create_dir_all(&dir)?;
    let mut names = Vec::new();
    for file in files {
        let Some(stem) = file.file_stem().map(|s| s.to_string_lossy().to_string()) else {
            continue;
        };
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let clean: String = stem
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        // SVG logos and artwork are rendered to a sharp 2048 px PNG (keeps transparency).
        let out_ext = if ext == "svg" { "png".to_string() } else { ext.clone() };
        let mut name = format!("{clean}.{out_ext}");
        let mut n = 1;
        while dir.join(&name).exists() {
            n += 1;
            name = format!("{clean}-{n}.{out_ext}");
        }
        if ext == "svg" {
            let png = render_svg(file, 2048)?;
            std::fs::write(dir.join(&name), png)?;
        } else {
            std::fs::copy(file, dir.join(&name))?;
        }
        names.push(name);
    }
    Ok(names)
}

/// Copy a built-in track on first use, preserving a user's file if the name is taken.
pub fn import_builtin_track(filename: &str, bytes: &[u8]) -> Result<String> {
    use std::io::{ErrorKind, Write};

    let file = Path::new(filename);
    if file.file_name() != Some(file.as_os_str()) || !is_audio(file) {
        return Err(anyhow!("Invalid bundled track name"));
    }
    let stem = file.file_stem().and_then(|name| name.to_str()).ok_or_else(|| anyhow!("Invalid track filename"))?;
    let extension = file.extension().and_then(|ext| ext.to_str()).ok_or_else(|| anyhow!("Invalid track extension"))?;
    let _import = MEDIA_IMPORT.lock().map_err(|_| anyhow!("Media import is unavailable"))?;
    let dir = LibraryPaths::get().media;
    let mut index = 1usize;
    loop {
        let name = if index == 1 { filename.to_string() } else { format!("{stem}-{index}.{extension}") };
        let path = dir.join(&name);
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    drop(file);
                    let _ = std::fs::remove_file(&path);
                    return Err(error.into());
                }
                return Ok(name);
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                if std::fs::read(&path).is_ok_and(|existing| existing == bytes) {
                    return Ok(name);
                }
            }
            Err(error) => return Err(error.into()),
        }
        index += 1;
    }
}

pub fn delete_media(name: &str) -> Result<()> {
    let paths = LibraryPaths::get();
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(anyhow!("invalid media name"));
    }
    trash(&paths, &paths.media.join(name))
}

pub fn delete_export(name: &str) -> Result<()> {
    let paths = LibraryPaths::get();
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(anyhow!("invalid file name"));
    }
    trash(&paths, &paths.exports.join(name))
}

/// Save a premultiplied pixmap as PNG in Exports (or `target` when given).
pub fn export_png(pixmap: &tiny_skia::Pixmap, base_name: &str, target: Option<PathBuf>) -> Result<PathBuf> {
    let path = match target {
        Some(path) => path,
        None => {
            let dir = LibraryPaths::get().exports;
            std::fs::create_dir_all(&dir)?;
            let clean: String = base_name
                .chars()
                .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                .collect();
            dir.join(format!(
                "{clean}_{}.png",
                chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")
            ))
        }
    };
    pixmap.save_png(&path)?;
    Ok(path)
}

/// The Exports folder (created if missing).
pub fn exports_dir() -> PathBuf {
    let dir = LibraryPaths::get().exports;
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Save as JPEG (quality 0-100) in Exports. JPEG has no transparency: see-through areas become
/// black, which is what a web page or social post expects from a full-frame graphic.
pub fn export_jpeg(pixmap: &tiny_skia::Pixmap, base_name: &str, quality: u8) -> Result<PathBuf> {
    let dir = LibraryPaths::get().exports;
    std::fs::create_dir_all(&dir)?;
    let clean: String = base_name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let path = dir.join(format!("{clean}_{}.jpg", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S")));
    // Premultiplied RGBA over black is just the premultiplied RGB.
    let rgb: Vec<u8> = pixmap.data().chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let file = std::fs::File::create(&path)?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality.clamp(1, 100));
    image::ImageEncoder::write_image(encoder, &rgb, pixmap.width(), pixmap.height(), image::ExtendedColorType::Rgb8)
        .map_err(|e| anyhow!("JPEG: {e}"))?;
    Ok(path)
}

/// Show a file in the system file browser (Explorer, Finder, or the Linux file manager).
pub fn reveal(path: &Path) {
    #[cfg(windows)]
    let _ = std::process::Command::new("explorer").arg("/select,").arg(path).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = open::that(path.parent().unwrap_or(path));
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
