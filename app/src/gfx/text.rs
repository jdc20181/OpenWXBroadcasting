//! Fonts and text rendering for broadcast graphics.
//!
//! Uses fonts that ship with Windows (no downloads): Bahnschrift (DIN, closest to the OpenWX
//! wordmark), Franklin Gothic (classic TV news), Segoe UI, Arial/Arial Narrow, Impact and more.
//! Custom .ttf/.otf files can be used by putting their path in the font name.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
};

use fontdue::Metrics;
use super::variable_font::LoadedFont as Font;
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, Pixmap};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Weight {
    Light,
    Regular,
    SemiBold,
    Bold,
    Black,
}

impl Weight {
    pub const ALL: [Weight; 5] = [
        Weight::Light,
        Weight::Regular,
        Weight::SemiBold,
        Weight::Bold,
        Weight::Black,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Weight::Light => "Light",
            Weight::Regular => "Regular",
            Weight::SemiBold => "Semibold",
            Weight::Bold => "Bold",
            Weight::Black => "Black",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Font families offered in the Graphics Studio, with their Windows font files by weight.
pub const FAMILIES: &[(&str, &[(Weight, &str)])] = &[
    ("Bahnschrift", &[(Weight::Regular, "bahnschrift.ttf")]),
    (
        "Franklin Gothic",
        &[(Weight::Regular, "framd.ttf"), (Weight::Black, "FRAHV.TTF"), (Weight::Bold, "FRAHV.TTF")],
    ),
    ("Franklin Gothic Condensed", &[(Weight::Regular, "FRAMDCN.TTF")]),
    (
        "Segoe UI",
        &[
            (Weight::Light, "segoeuil.ttf"),
            (Weight::Regular, "segoeui.ttf"),
            (Weight::SemiBold, "seguisb.ttf"),
            (Weight::Bold, "segoeuib.ttf"),
            (Weight::Black, "seguibl.ttf"),
        ],
    ),
    (
        "Arial",
        &[(Weight::Regular, "arial.ttf"), (Weight::Bold, "arialbd.ttf"), (Weight::Black, "ariblk.ttf")],
    ),
    ("Arial Narrow", &[(Weight::Regular, "ARIALN.TTF"), (Weight::Bold, "ARIALNB.TTF")]),
    ("Impact", &[(Weight::Regular, "impact.ttf")]),
    (
        "Calibri",
        &[(Weight::Light, "calibril.ttf"), (Weight::Regular, "calibri.ttf"), (Weight::Bold, "calibrib.ttf")],
    ),
    ("Verdana", &[(Weight::Regular, "verdana.ttf"), (Weight::Bold, "verdanab.ttf")]),
    ("Tahoma", &[(Weight::Regular, "tahoma.ttf"), (Weight::Bold, "tahomabd.ttf")]),
    ("Trebuchet MS", &[(Weight::Regular, "trebuc.ttf"), (Weight::Bold, "trebucbd.ttf")]),
    ("Georgia", &[(Weight::Regular, "georgia.ttf"), (Weight::Bold, "georgiab.ttf")]),
    ("Consolas", &[(Weight::Regular, "consola.ttf"), (Weight::Bold, "consolab.ttf")]),
    // macOS / Linux system fonts (used when the Windows families are not installed).
    ("Helvetica", &[(Weight::Regular, "Helvetica.ttc"), (Weight::Bold, "Helvetica.ttc")]),
    ("DejaVu Sans", &[(Weight::Regular, "DejaVuSans.ttf"), (Weight::Bold, "DejaVuSans-Bold.ttf")]),
    ("Liberation Sans", &[(Weight::Regular, "LiberationSans-Regular.ttf"), (Weight::Bold, "LiberationSans-Bold.ttf")]),
];

pub fn family_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = FAMILIES.iter().map(|(name, _)| *name).collect();
    names.extend(custom_families().iter().map(|(name, _)| *name));
    names
}

/// The station's own fonts folder (Documents\OpenWX\Fonts).
pub fn fonts_dir() -> PathBuf {
    crate::config::LibraryPaths::get().root.join("Fonts")
}

/// Weight from a font file name ("Montserrat-SemiBold.ttf" → SemiBold).
fn weight_from_name(stem: &str) -> (String, Weight) {
    let lower = stem.to_lowercase();
    let table: [(&str, Weight); 9] = [
        ("extrabold", Weight::Black),
        ("black", Weight::Black),
        ("heavy", Weight::Black),
        ("semibold", Weight::SemiBold),
        ("demibold", Weight::SemiBold),
        ("medium", Weight::SemiBold),
        ("bold", Weight::Bold),
        ("light", Weight::Light),
        ("thin", Weight::Light),
    ];
    for (key, w) in table {
        if let Some(pos) = lower.rfind(key) {
            let family = stem[..pos].trim_end_matches(['-', '_', ' ']).to_string();
            if !family.is_empty() {
                return (family, w);
            }
        }
    }
    let family = stem.trim_end_matches("-Regular").trim_end_matches("_Regular").trim_end_matches(" Regular").to_string();
    (family, Weight::Regular)
}

/// Custom families found in the fonts folder (scanned once; names live for the whole run).
pub fn custom_families() -> &'static Vec<(&'static str, Vec<(Weight, PathBuf)>)> {
    static CUSTOM: OnceLock<Vec<(&'static str, Vec<(Weight, PathBuf)>)>> = OnceLock::new();
    CUSTOM.get_or_init(|| {
        let mut map: Vec<(String, Vec<(Weight, PathBuf)>)> = Vec::new();
        for e in std::fs::read_dir(fonts_dir()).into_iter().flatten().flatten() {
            let path = e.path();
            let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_lowercase();
            if ext != "ttf" && ext != "otf" {
                continue;
            }
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let (family, weight) = weight_from_name(&stem);
            match map.iter_mut().find(|(f, _)| f.eq_ignore_ascii_case(&family)) {
                Some((_, files)) => files.push((weight, path)),
                None => map.push((family, vec![(weight, path)])),
            }
        }
        map.into_iter().map(|(f, files)| (&*Box::leak(f.into_boxed_str()), files)).collect()
    })
}

/// Copy font files into the fonts folder. They are available after the next start.
pub fn import_fonts(files: &[PathBuf]) -> anyhow::Result<usize> {
    let dir = fonts_dir();
    std::fs::create_dir_all(&dir)?;
    let mut n = 0;
    for f in files {
        if let Some(name) = f.file_name() {
            std::fs::copy(f, dir.join(name))?;
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(not(windows))]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec!["/System/Library/Fonts".into(), "/System/Library/Fonts/Supplemental".into(), "/Library/Fonts".into()]
    } else {
        vec!["/usr/share/fonts".into(), "/usr/local/share/fonts".into()]
    };
    if let Some(home) = dirs::home_dir() {
        dirs.push(home.join(if cfg!(target_os = "macos") { "Library/Fonts" } else { ".local/share/fonts" }));
    }
    // Linux keeps fonts in sub-folders (truetype/dejavu/…): include two levels.
    let mut all = Vec::new();
    for d in dirs {
        all.push(d.clone());
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
            all.push(e.path());
            for e2 in std::fs::read_dir(e.path()).into_iter().flatten().flatten().filter(|e| e.path().is_dir()) {
                all.push(e2.path());
            }
        }
    }
    all
}

#[cfg(windows)]
fn font_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![PathBuf::from(
        std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()),
    )
    .join("Fonts")];
    if let Some(local) = dirs::data_local_dir() {
        dirs.push(local.join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// A loaded face plus whether bold must be synthesized (family has no bold file).
#[derive(Clone)]
pub struct Face {
    pub font: Arc<Font>,
    pub synthetic_bold: bool,
}

#[derive(Default)]
pub struct FontBook {
    loaded: Mutex<HashMap<(String, Weight), Option<Face>>>,
    files: Mutex<HashMap<PathBuf, Option<Arc<Font>>>>,
}

impl FontBook {
    pub fn global() -> &'static FontBook {
        static BOOK: OnceLock<FontBook> = OnceLock::new();
        BOOK.get_or_init(FontBook::default)
    }

    fn load_file(&self, path: PathBuf) -> Option<Arc<Font>> {
        let mut files = self.files.lock().ok()?;
        if let Some(font) = files.get(&path) {
            return font.clone();
        }
        let font = std::fs::read(&path)
            .ok()
            .and_then(|bytes| Font::from_bytes(bytes))
            .map(Arc::new);
        if font.is_none() {
            log::warn!("font not available: {}", path.display());
        }
        files.insert(path, font.clone());
        font
    }

    fn find_file(&self, file: &str) -> Option<Arc<Font>> {
        for dir in font_dirs() {
            let path = dir.join(file);
            if path.exists() {
                return self.load_file(path);
            }
        }
        None
    }

    /// Resolve `family` at `weight`, falling back to the nearest weight, then to Segoe UI/Arial.
    pub fn face(&self, family: &str, weight: Weight) -> Option<Face> {
        let key = (family.to_string(), weight);
        if let Some(face) = self.loaded.lock().ok()?.get(&key) {
            return face.clone();
        }

        let face = self.resolve(family, weight).or_else(|| {
            self.resolve("Segoe UI", weight)
                .or_else(|| self.resolve("Arial", weight))
                // macOS and Linux fallbacks.
                .or_else(|| self.resolve("Helvetica", weight))
                .or_else(|| self.resolve("DejaVu Sans", weight))
                .or_else(|| self.resolve("Liberation Sans", weight))
        });
        self.loaded.lock().ok()?.insert(key, face.clone());
        face
    }

    fn resolve(&self, family: &str, weight: Weight) -> Option<Face> {
        // A path to a custom font file.
        let lower = family.to_lowercase();
        if lower.ends_with(".ttf") || lower.ends_with(".otf") {
            let path = PathBuf::from(family);
            let font = if path.is_absolute() {
                self.load_file(path)
            } else {
                self.find_file(family)
            }?;
            return Some(Face {
                font,
                synthetic_bold: matches!(weight, Weight::Bold | Weight::Black),
            });
        }

        let rank = |w: Weight| match w {
            Weight::Light => 0i32,
            Weight::Regular => 1,
            Weight::SemiBold => 2,
            Weight::Bold => 3,
            Weight::Black => 4,
        };
        // The station's own fonts.
        if let Some((_, files)) = custom_families().iter().find(|(name, _)| name.eq_ignore_ascii_case(family)) {
            let mut candidates: Vec<&(Weight, PathBuf)> = files.iter().collect();
            candidates.sort_by_key(|(w, _)| (rank(*w) - rank(weight)).abs());
            for (w, path) in candidates {
                if let Some(font) = self.load_file(path.clone()) {
                    return Some(Face { font, synthetic_bold: rank(weight) >= 3 && rank(*w) < 3 });
                }
            }
        }
        let (_, files) = FAMILIES
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(family))?;
        let rank = |w: Weight| match w {
            Weight::Light => 0i32,
            Weight::Regular => 1,
            Weight::SemiBold => 2,
            Weight::Bold => 3,
            Weight::Black => 4,
        };
        let mut candidates: Vec<&(Weight, &str)> = files.iter().collect();
        candidates.sort_by_key(|(w, _)| (rank(*w) - rank(weight)).abs());
        for (w, file) in candidates {
            if let Some(font) = self.find_file(file) {
                let wants_heavy = rank(weight) >= 3;
                let got_heavy = rank(*w) >= 3;
                return Some(Face {
                    font,
                    synthetic_bold: wants_heavy && !got_heavy,
                });
            }
        }
        None
    }
}

/// Rasterized glyph cache (per renderer/thread).
#[derive(Default)]
pub struct GlyphCache {
    glyphs: HashMap<(usize, u16, u32, Weight), (Metrics, Arc<Vec<u8>>)>,
}

impl GlyphCache {
    fn get(&mut self, font: &Arc<Font>, index: u16, px: f32, weight: Weight) -> (Metrics, Arc<Vec<u8>>) {
        let key = (Arc::as_ptr(font) as usize, index, px.to_bits(), weight);
        if let Some(found) = self.glyphs.get(&key) {
            return found.clone();
        }
        if self.glyphs.len() > 6000 {
            self.glyphs.clear();
        }
        let (metrics, bitmap) = font.raster(index, px, weight);
        let entry = (metrics, Arc::new(bitmap));
        self.glyphs.insert(key, entry.clone());
        entry
    }
}

#[derive(Clone, Debug)]
pub struct TextStyle {
    pub family: String,
    pub weight: Weight,
    pub size: f32,
    pub color: Color,
    pub letter_spacing: f32,
}

/// Width of a single line of text.
pub fn measure(text: &str, style: &TextStyle) -> f32 {
    let Some(face) = FontBook::global().face(&style.family, style.weight) else {
        return text.chars().count() as f32 * style.size * 0.55;
    };
    let font = &face.font;
    let mut width = 0.0;
    let mut prev: Option<u16> = None;
    let count = text.chars().count();
    for (i, ch) in text.chars().enumerate() {
        let index = font.lookup_glyph_index(ch);
        if let Some(prev) = prev {
            width += font.horizontal_kern_indexed(prev, index, style.size).unwrap_or(0.0);
        }
        width += font.glyph_metrics(index, style.size, style.weight).advance_width;
        if i + 1 < count {
            width += style.letter_spacing;
        }
        prev = Some(index);
    }
    if face.synthetic_bold && !face.font.variable {
        width += style.size * 0.04;
    }
    width
}

/// Ascent and descent (positive) for a style.
pub fn line_metrics(style: &TextStyle) -> (f32, f32) {
    FontBook::global()
        .face(&style.family, style.weight)
        .and_then(|face| face.font.horizontal_line_metrics(style.size))
        .map(|m| (m.ascent, -m.descent))
        .unwrap_or((style.size * 0.8, style.size * 0.2))
}

/// Greedy word wrap to `max_width`.
pub fn wrap(text: &str, style: &TextStyle, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate, style) > max_width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Draw one line with its baseline at `baseline_y`, starting at `x`. Blends straight into the
/// premultiplied pixmap. `opacity` multiplies the color alpha. `clip` limits drawing to a rect.
pub fn draw_line(
    pixmap: &mut Pixmap,
    cache: &mut GlyphCache,
    text: &str,
    x: f32,
    baseline_y: f32,
    style: &TextStyle,
    opacity: f32,
    clip: Option<(i32, i32, i32, i32)>,
) {
    let Some(face) = FontBook::global().face(&style.family, style.weight) else {
        return;
    };
    let font = face.font.clone();
    let mut pen_x = x;
    let mut prev: Option<u16> = None;
    let passes: &[f32] = if face.synthetic_bold && !face.font.variable {
        &[0.0, 0.035]
    } else {
        &[0.0]
    };
    for ch in text.chars() {
        let index = font.lookup_glyph_index(ch);
        if let Some(prev) = prev {
            pen_x += font.horizontal_kern_indexed(prev, index, style.size).unwrap_or(0.0);
        }
        let (metrics, bitmap) = cache.get(&font, index, style.size, style.weight);
        for pass in passes {
            let gx = (pen_x + metrics.xmin as f32 + pass * style.size).round() as i32;
            let gy = (baseline_y - metrics.ymin as f32 - metrics.height as f32).round() as i32;
            blend_coverage(
                pixmap,
                gx,
                gy,
                metrics.width,
                metrics.height,
                &bitmap,
                style.color,
                opacity,
                clip,
            );
        }
        pen_x += metrics.advance_width + style.letter_spacing;
        prev = Some(index);
    }
}

/// Alpha-blend a coverage mask in `color` into a premultiplied RGBA pixmap.
#[allow(clippy::too_many_arguments)]
pub fn blend_coverage(
    pixmap: &mut Pixmap,
    x0: i32,
    y0: i32,
    width: usize,
    height: usize,
    coverage: &[u8],
    color: Color,
    opacity: f32,
    clip: Option<(i32, i32, i32, i32)>,
) {
    let pw = pixmap.width() as i32;
    let ph = pixmap.height() as i32;
    let (cx0, cy0, cx1, cy1) = clip.unwrap_or((0, 0, pw, ph));
    let (cx0, cy0, cx1, cy1) = (cx0.max(0), cy0.max(0), cx1.min(pw), cy1.min(ph));
    let (r, g, b) = (color.red(), color.green(), color.blue());
    let base_alpha = color.alpha() * opacity;
    if base_alpha <= 0.0 {
        return;
    }
    let data = pixmap.data_mut();
    for row in 0..height as i32 {
        let y = y0 + row;
        if y < cy0 || y >= cy1 {
            continue;
        }
        for col in 0..width as i32 {
            let x = x0 + col;
            if x < cx0 || x >= cx1 {
                continue;
            }
            let cov = coverage[(row as usize) * width + col as usize];
            if cov == 0 {
                continue;
            }
            let a = cov as f32 / 255.0 * base_alpha;
            let idx = ((y * pw + x) * 4) as usize;
            let inv = 1.0 - a;
            data[idx] = (r * a * 255.0 + data[idx] as f32 * inv).min(255.0) as u8;
            data[idx + 1] = (g * a * 255.0 + data[idx + 1] as f32 * inv).min(255.0) as u8;
            data[idx + 2] = (b * a * 255.0 + data[idx + 2] as f32 * inv).min(255.0) as u8;
            data[idx + 3] = (a * 255.0 + data[idx + 3] as f32 * inv).min(255.0) as u8;
        }
    }
}

/// Draw text inside a box: alignment, optional wrapping, shrink-to-fit, optional drop shadow.
#[allow(clippy::too_many_arguments)]
pub fn draw_text_box(
    pixmap: &mut Pixmap,
    cache: &mut GlyphCache,
    text: &str,
    rect: (f32, f32, f32, f32),
    style: &TextStyle,
    align: Align,
    vcenter: bool,
    wrap_lines: bool,
    auto_fit: bool,
    shadow: bool,
    opacity: f32,
) {
    let (x, y, w, h) = rect;
    if has_markup(text) {
        draw_rich(pixmap, cache, text, rect, style, align, vcenter, shadow, opacity);
        return;
    }
    let mut style = style.clone();

    let mut lines = if wrap_lines {
        wrap(text, &style, w)
    } else {
        text.split('\n').map(String::from).collect()
    };

    if auto_fit {
        // Shrink until every line fits the width and the block fits the height.
        for _ in 0..24 {
            let widest = lines.iter().map(|l| measure(l, &style)).fold(0.0, f32::max);
            let (ascent, descent) = line_metrics(&style);
            let block = (ascent + descent) * 1.08 * lines.len() as f32;
            if (widest <= w + 0.5 && block <= h + 0.5) || style.size <= 8.0 {
                break;
            }
            style.size *= 0.92;
            if wrap_lines {
                lines = wrap(text, &style, w);
            }
        }
    }

    let (ascent, descent) = line_metrics(&style);
    let line_height = (ascent + descent) * 1.08;
    let block_height = line_height * lines.len() as f32;
    let mut top = if vcenter { y + (h - block_height) / 2.0 } else { y };
    let clip = (
        (x - 2.0).floor() as i32,
        (y - 2.0).floor() as i32,
        (x + w + 2.0).ceil() as i32,
        (y + h + 2.0).ceil() as i32,
    );

    for line in &lines {
        let line_w = measure(line, &style);
        let lx = match align {
            Align::Left => x,
            Align::Center => x + (w - line_w) / 2.0,
            Align::Right => x + w - line_w,
        };
        let baseline = top + ascent;
        if shadow {
            let mut shadow_style = style.clone();
            shadow_style.color = Color::from_rgba8(0, 0, 0, 150);
            let offset = (style.size * 0.05).max(1.5);
            draw_line(
                pixmap,
                cache,
                line,
                lx + offset,
                baseline + offset,
                &shadow_style,
                opacity,
                Some(clip),
            );
        }
        draw_line(pixmap, cache, line, lx, baseline, &style, opacity, Some(clip));
        top += line_height;
    }
}

/// Rich text markup: **bold**, [c=#FF3B3B]colour[/c], [s=1.4]size[/s].
pub fn has_markup(text: &str) -> bool {
    text.contains("**") || text.contains("[c=") || text.contains("[s=")
}

/// Text with the markup removed (for measuring, reveal counts and checks).
pub fn strip_markup(text: &str) -> String {
    rich_runs(text, &TextStyle { family: String::new(), weight: Weight::Regular, size: 10.0, color: Color::WHITE, letter_spacing: 0.0 })
        .into_iter()
        .map(|(t, _)| t)
        .collect()
}

fn rich_runs(text: &str, base: &TextStyle) -> Vec<(String, TextStyle)> {
    let mut runs: Vec<(String, TextStyle)> = Vec::new();
    let (mut bold, mut colors, mut sizes): (bool, Vec<Color>, Vec<f32>) = (false, Vec::new(), Vec::new());
    let mut cur = String::new();
    let style_now = |bold: bool, colors: &[Color], sizes: &[f32]| {
        let mut s = base.clone();
        if bold {
            s.weight = match base.weight {
                Weight::Light | Weight::Regular => Weight::Bold,
                _ => Weight::Black,
            };
        }
        if let Some(c) = colors.last() {
            s.color = *c;
        }
        s.size = base.size * sizes.iter().product::<f32>();
        s
    };
    let mut rest = text;
    while !rest.is_empty() {
        let tag = if rest.starts_with("**") {
            Some(("**", 2))
        } else if rest.starts_with("[/c]") {
            Some(("/c", 4))
        } else if rest.starts_with("[/s]") {
            Some(("/s", 4))
        } else if rest.starts_with("[c=") || rest.starts_with("[s=") {
            rest.find(']').map(|end| (if rest.starts_with("[c=") { "c" } else { "s" }, end + 1))
        } else {
            None
        };
        match tag {
            Some((kind, len)) => {
                if !cur.is_empty() {
                    runs.push((std::mem::take(&mut cur), style_now(bold, &colors, &sizes)));
                }
                match kind {
                    "**" => bold = !bold,
                    "/c" => {
                        colors.pop();
                    }
                    "/s" => {
                        sizes.pop();
                    }
                    "c" => {
                        let v = &rest[3..len - 1];
                        colors.push(crate::gfx::parse_hex(v.trim()).unwrap_or(base.color));
                    }
                    _ => {
                        let v = rest[3..len - 1].trim().parse::<f32>().unwrap_or(1.0).clamp(0.3, 4.0);
                        sizes.push(v);
                    }
                }
                rest = &rest[len..];
            }
            None => {
                let ch = rest.chars().next().unwrap_or(' ');
                cur.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
        }
    }
    if !cur.is_empty() {
        runs.push((cur, style_now(bold, &colors, &sizes)));
    }
    runs
}

/// Rich text: words keep their run's style, wrapped to the box width, aligned per line.
#[allow(clippy::too_many_arguments)]
fn draw_rich(pixmap: &mut Pixmap, cache: &mut GlyphCache, text: &str, rect: (f32, f32, f32, f32), base: &TextStyle, align: Align, vcenter: bool, shadow: bool, opacity: f32) {
    let (x, y, w, h) = rect;
    // Tokens: (text, style, is_space, forced break)
    let mut tokens: Vec<(String, TextStyle)> = Vec::new();
    for (t, s) in rich_runs(text, base) {
        let mut word = String::new();
        for ch in t.chars() {
            if ch == ' ' || ch == '\n' {
                if !word.is_empty() {
                    tokens.push((std::mem::take(&mut word), s.clone()));
                }
                tokens.push((ch.to_string(), s.clone()));
            } else {
                word.push(ch);
            }
        }
        if !word.is_empty() {
            tokens.push((word, s.clone()));
        }
    }
    let mut lines: Vec<Vec<(String, TextStyle)>> = vec![Vec::new()];
    let mut line_w = 0.0;
    for (t, s) in tokens {
        if t == "\n" {
            lines.push(Vec::new());
            line_w = 0.0;
            continue;
        }
        let tw = measure(&t, &s);
        if t != " " && line_w + tw > w && line_w > 0.0 {
            lines.push(Vec::new());
            line_w = 0.0;
        }
        if t == " " && line_w == 0.0 {
            continue;
        }
        line_w += tw;
        if let Some(l) = lines.last_mut() {
            l.push((t, s));
        }
    }
    let metrics: Vec<(f32, f32)> = lines
        .iter()
        .map(|l| l.iter().map(|(_, s)| line_metrics(s)).fold(line_metrics(base), |a, b| (a.0.max(b.0), a.1.max(b.1))))
        .collect();
    let block: f32 = metrics.iter().map(|(a, d)| (a + d) * 1.08).sum();
    let mut top = if vcenter { y + (h - block) / 2.0 } else { y };
    let clip = ((x - 2.0).floor() as i32, (y - 2.0).floor() as i32, (x + w + 2.0).ceil() as i32, (y + h + 2.0).ceil() as i32);
    for (line, (ascent, descent)) in lines.iter().zip(metrics) {
        let width: f32 = line.iter().map(|(t, s)| measure(t, s)).sum::<f32>();
        let mut pen = match align {
            Align::Left => x,
            Align::Center => x + (w - width) / 2.0,
            Align::Right => x + w - width,
        };
        let baseline = top + ascent;
        for (t, s) in line {
            if shadow {
                let mut sh = s.clone();
                sh.color = Color::from_rgba8(0, 0, 0, 150);
                let o = (s.size * 0.05).max(1.5);
                draw_line(pixmap, cache, t, pen + o, baseline + o, &sh, opacity, Some(clip));
            }
            draw_line(pixmap, cache, t, pen, baseline, s, opacity, Some(clip));
            pen += measure(t, s);
        }
        top += (ascent + descent) * 1.08;
    }
}

/// Baselines (y, in the box's coordinates) of each line as draw_text_box lays them out:
/// for the baseline guides in Graphics Studio.
pub fn baselines(text: &str, rect: (f32, f32, f32, f32), style: &TextStyle, vcenter: bool, wrap_lines: bool) -> Vec<f32> {
    let (_, y, w, h) = rect;
    let plain = strip_markup(text);
    let lines = if wrap_lines { wrap(&plain, style, w) } else { plain.split('\n').map(String::from).collect() };
    let (ascent, descent) = line_metrics(style);
    let lh = (ascent + descent) * 1.08;
    let top = if vcenter { y + (h - lh * lines.len() as f32) / 2.0 } else { y };
    (0..lines.len()).map(|i| top + ascent + lh * i as f32).collect()
}
