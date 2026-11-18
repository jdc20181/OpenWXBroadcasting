//! Graphic checks for Graphics Studio: text fit and overflow, colour contrast, safe areas,
//! missing media, unresolved bindings; plus preview data modes (sample, missing, long text) for
//! testing a graphic before air.

use crate::{
    config::Brand,
    gfx::{
        bind::Bindings,
        model::{Fill, GraphicDoc, Layer, LayerKind},
        parse_color,
        render::resolve_font,
        text::{line_metrics, measure, wrap, TextStyle},
    },
};

/// Preview data for testing a graphic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PreviewData {
    /// Live feeds (what goes to air).
    #[default]
    Live,
    /// Live values, with plausible sample values wherever a feed has no data yet.
    Sample,
    /// Every value missing, to see how the graphic copes with a feed outage.
    Missing,
    /// Every value made very long, to find text that spills out of its box.
    Long,
}

impl PreviewData {
    pub const ALL: [PreviewData; 4] = [PreviewData::Live, PreviewData::Sample, PreviewData::Missing, PreviewData::Long];

    pub fn label(self) -> &'static str {
        match self {
            PreviewData::Live => "Live data",
            PreviewData::Sample => "Sample data",
            PreviewData::Missing => "Missing data",
            PreviewData::Long => "Long text",
        }
    }
}

const SAMPLE: &[(&str, &str)] = &[
    ("temp", "72°"),
    ("next.hi", "74°"),
    ("next.lo", "55°"),
    ("feels", "75°"),
    ("dewpoint", "61°"),
    ("humidity", "68%"),
    ("wind", "SW 12 mph"),
    ("gust", "24 mph"),
    ("pressure", "29.92 in"),
    ("visibility", "10 mi"),
    ("condition", "Partly Cloudy"),
    ("alert0.event", "Severe Thunderstorm Warning"),
    ("alert0.headline", "Severe Thunderstorm Warning until 5:45 PM"),
    ("alert0.area", "Dubois, Pike and Spencer Counties"),
    ("alert0.expires", "Until 5:45 PM"),
    ("alert0.instruction", "Move to an interior room on the lowest floor of a sturdy building."),
    ("spc.day1", "Slight"),
    ("spc.home", "Enhanced risk"),
    ("spc.home.level", "3"),
    ("spc.home.short", "ENH"),
    ("spc.home.number", "Level 3 of 5"),
    ("spc.home.tornado", "5%"),
    ("spc.home.tornado.level", "2"),
    ("spc.home.wind", "30%"),
    ("spc.home.wind.level", "3"),
    ("spc.home.wind.sig", "Significant"),
    ("spc.home.hail", "15%"),
    ("spc.home.hail.level", "2"),
    ("spc.home.flood.level", "1"),
    ("spc.home.flood.words", "Marginal"),
    ("spc.home.hazards", "Tornado 5%, Wind 30% (significant), Hail 15%"),
    ("spc.day1.home", "Enhanced risk"),
    ("spc.day2.home", "Slight risk"),
    ("spc.day2.home.level", "2"),
    ("spc.day2.home.number", "Level 2 of 5"),
    ("spc.day2.home.tornado", "2%"),
    ("spc.day2.home.tornado.level", "1"),
    ("spc.day2.home.wind", "15%"),
    ("spc.day2.home.wind.level", "2"),
    ("spc.day2.home.hail", "5%"),
    ("spc.day2.home.hail.level", "1"),
    ("spc.day2.home.flood.level", "0"),
    ("spc.day2.home.flood.words", "Low"),
    ("spc.day3.home", "Marginal risk"),
    ("spc.day1.weekday", "Today"),
    ("spc.day2.weekday", "Tomorrow"),
    ("spc.day1.date", "Wednesday, November 11"),
    ("spc.day2.date", "Thursday, November 12"),
    ("severe.window", "Storms most likely 4 PM to 10 PM"),
    ("cell0.summary", "Strong storm 14 mi SW, moving NE at 35 mph"),
];

/// Bindings for a preview mode, built from the live ones.
pub fn preview_bindings(live: &Bindings, mode: PreviewData) -> Bindings {
    match mode {
        PreviewData::Live => live.clone(),
        PreviewData::Sample => {
            let mut b = live.clone();
            for (k, v) in SAMPLE {
                if b.get(k).map(|s| s.trim().is_empty() || s.trim() == "--").unwrap_or(true) {
                    b.set(k, v);
                }
            }
            for i in 0..7 {
                let names = ["Today", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                let fill = [
                    (format!("day{i}.short"), names[i].to_string()),
                    (format!("day{i}.name"), names[i].to_string()),
                    (format!("day{i}.hi"), format!("{}°", 70 + i)),
                    (format!("day{i}.lo"), format!("{}°", 52 + i)),
                    (format!("day{i}.condition"), "Chance Showers".to_string()),
                    (format!("day{i}.precip"), format!("{}%", 10 * i)),
                ];
                for (k, v) in fill {
                    if b.get(&k).map(|s| s.trim().is_empty() || s.trim() == "--").unwrap_or(true) {
                        b.set(&k, &v);
                    }
                }
            }
            // Holiday cards: a sample forecast for any holiday still too far out.
            for h in crate::gfx::holidays::HOLIDAYS {
                for (field, v) in [("hi", "73°"), ("pop", "40%"), ("condition", "Breezy, PM Storms"), ("daypart", "day")] {
                    let k = format!("holiday.{}.{field}", h.id);
                    if b.get(&k).map(|s| s.trim().is_empty() || s.trim() == "--" || s.starts_with("Forecast coming")).unwrap_or(true) {
                        b.set(&k, v);
                    }
                }
            }
            for (k, v) in [("alertday1.when", "WEDNESDAY"), ("alertday1.line1", "Scattered strong storms possible"), ("alertday1.line2", "Damaging wind possible"), ("alertday1.line3", "A brief tornado possible")] {
                if b.get(k).map(|s| s.trim().is_empty()).unwrap_or(true) {
                    b.set(k, v);
                }
            }
            b
        }
        PreviewData::Missing => live.map_values(|_| String::new()),
        PreviewData::Long => live.map_values(|v| {
            if v.chars().count() < 3 {
                format!("{v}{v}{v}")
            } else {
                format!("{v} • {v} • {v}")
            }
        }),
    }
}

/// How a text layer fits its box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFit {
    /// Scale applied by Shrink to fit (1.0 = fits at the set size).
    pub scale: f32,
    /// True when the text does not fit and Shrink to fit is off.
    pub overflows: bool,
}

fn text_style(layer: &Layer, brand: &Brand) -> Option<(String, TextStyle, bool, bool)> {
    match &layer.kind {
        LayerKind::Text { text, font, size, weight, color, letter_spacing, uppercase, wrap, auto_fit, .. } => Some((
            if *uppercase { text.to_uppercase() } else { text.clone() },
            TextStyle {
                family: resolve_font(font, brand),
                weight: *weight,
                size: *size,
                color: parse_color(color, brand),
                letter_spacing: *letter_spacing,
            },
            *wrap,
            *auto_fit,
        )),
        _ => None,
    }
}

pub fn text_fit(layer: &Layer, bindings: &Bindings, brand: &Brand) -> Option<TextFit> {
    let (raw, mut style, wrap_lines, auto_fit) = text_style(layer, brand)?;
    let content = bindings.apply(&raw);
    let (w, h) = (layer.w, layer.h);
    let fits = |style: &TextStyle| {
        let lines: Vec<String> = if wrap_lines { wrap(&content, style, w) } else { content.split('\n').map(String::from).collect() };
        let widest = lines.iter().map(|l| measure(l, style)).fold(0.0, f32::max);
        let (a, d) = line_metrics(style);
        widest <= w + 0.5 && (a + d) * 1.08 * lines.len() as f32 <= h + 0.5
    };
    if fits(&style) {
        return Some(TextFit { scale: 1.0, overflows: false });
    }
    if !auto_fit {
        return Some(TextFit { scale: 1.0, overflows: true });
    }
    let start = style.size;
    for _ in 0..24 {
        if fits(&style) || style.size <= 8.0 {
            break;
        }
        style.size *= 0.92;
    }
    Some(TextFit { scale: style.size / start, overflows: false })
}

fn luminance(c: tiny_skia::Color) -> f32 {
    let lin = |v: f32| if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(c.red()) + 0.7152 * lin(c.green()) + 0.0722 * lin(c.blue())
}

/// WCAG contrast ratio between two colours (1 to 21).
pub fn contrast_ratio(a: tiny_skia::Color, b: tiny_skia::Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn fill_color(fill: &Fill, brand: &Brand) -> tiny_skia::Color {
    parse_color(fill.primary_color(), brand)
}

/// Colour behind a layer's centre: the topmost filled shape under it, else the background.
fn color_behind(doc: &GraphicDoc, index: usize, brand: &Brand) -> Option<tiny_skia::Color> {
    let layer = &doc.layers[index];
    let (cx, cy) = (layer.x + layer.w / 2.0, layer.y + layer.h / 2.0);
    for below in doc.layers[..index].iter().rev() {
        if !below.visible || !below.contains(cx, cy) {
            continue;
        }
        if let LayerKind::Rect { fill, .. } | LayerKind::Ellipse { fill, .. } = &below.kind {
            if below.opacity >= 0.6 {
                return Some(fill_color(fill, brand));
            }
        }
    }
    doc.background.as_ref().map(|f| fill_color(f, brand))
}

/// One problem found in a graphic.
#[derive(Clone, Debug)]
pub struct Issue {
    pub layer_id: String,
    pub layer: String,
    pub serious: bool,
    pub text: String,
}

/// Everything worth fixing before this graphic goes to air.
pub fn check_doc(doc: &GraphicDoc, bindings: &Bindings, brand: &Brand, media_dir: &std::path::Path) -> Vec<Issue> {
    let mut out = Vec::new();
    // Template validation rules.
    for (name, rule) in &doc.rules {
        let Some(layer) = doc.layers.iter().find(|l| &l.name == name) else { continue };
        let text = match &layer.kind {
            crate::gfx::model::LayerKind::Text { text, .. } | crate::gfx::model::LayerKind::Ticker { text, .. } => bindings.apply(text),
            _ => continue,
        };
        let plain = crate::gfx::text::strip_markup(&text);
        let problem = if rule == "required" && plain.trim().is_empty() {
            Some("Must not be empty (template rule)".to_string())
        } else if let Some(n) = rule.strip_prefix("max_chars:").and_then(|n| n.trim().parse::<usize>().ok()) {
            (plain.chars().count() > n).then(|| format!("Longer than {n} characters (template rule)"))
        } else if rule == "fits" {
            text_fit(layer, bindings, brand).filter(|f| f.overflows).map(|_| "Must fit its box (template rule)".to_string())
        } else {
            None
        };
        if let Some(text) = problem {
            out.push(Issue { layer_id: layer.id.clone(), layer: layer.name.clone(), serious: true, text });
        }
    }
    // Sponsor graphics with no sponsor set render as an empty "brought to you by" box.
    let uses_sponsor = doc.layers.iter().any(|l| match &l.kind {
        LayerKind::Text { text, .. } => text.contains("{{sponsor"),
        LayerKind::Image { media, .. } => media.contains("{{sponsor"),
        _ => false,
    });
    if uses_sponsor && bindings.apply("{{sponsor}}{{sponsor_logo}}").trim().is_empty() {
        if let Some(l) = doc.layers.iter().find(|l| l.name.to_lowercase().contains("sponsor")).or(doc.layers.first()) {
            out.push(Issue {
                layer_id: l.id.clone(),
                layer: l.name.clone(),
                serious: true,
                text: "No sponsor is set, so this shows an empty box on air. Add one in Branding & sponsors.".into(),
            });
        }
    }
    let (dw, dh) = (doc.width as f32, doc.height as f32);
    // Title-safe area: 90% of the frame (SMPTE RP 2046-2 style).
    let (sx, sy) = (dw * 0.05, dh * 0.05);
    for (i, layer) in doc.layers.iter().enumerate() {
        if !layer.visible {
            continue;
        }
        let mut push = |serious: bool, text: String| out.push(Issue { layer_id: layer.id.clone(), layer: layer.name.clone(), serious, text });
        let outside = layer.x + layer.w < 0.0 || layer.y + layer.h < 0.0 || layer.x > dw || layer.y > dh;
        if outside {
            push(false, "Is entirely off the canvas".into());
            continue;
        }
        if let Some(fit) = text_fit(layer, bindings, brand) {
            if fit.overflows {
                push(true, "Text spills out of its box (turn on Shrink to fit or make the box bigger)".into());
            } else if fit.scale < 0.7 {
                push(false, format!("Text is shrunk to {:.0}% to fit", fit.scale * 100.0));
            }
            let partly_outside = layer.x < sx || layer.y < sy || layer.x + layer.w > dw - sx || layer.y + layer.h > dh - sy;
            // The standard footer band (station / source line every template gets) is designed to
            // sit low in the frame, like a broadcast lower band: warning on it flagged nearly every
            // graphic and taught people to ignore the check (CWXB-102).
            let standard_footer = matches!(layer.name.as_str(), "Footer left" | "Footer right" | "Station" | "Source") && layer.y >= dh * 0.88;
            if partly_outside && !doc.is_overlay() && !standard_footer {
                push(false, "Text reaches outside the title-safe area".into());
            }
            if let (LayerKind::Text { color, size, .. }, Some(bg)) = (&layer.kind, color_behind(doc, i, brand)) {
                let ratio = contrast_ratio(parse_color(color, brand), bg);
                let needed = if *size >= 48.0 { 3.0 } else { 4.5 };
                if ratio < needed {
                    push(true, format!("Low contrast ({ratio:.1}:1, needs {needed}:1) - hard to read on air"));
                }
            }
        }
        let text = match &layer.kind {
            LayerKind::Text { text, .. } | LayerKind::Ticker { text, .. } => Some(text.as_str()),
            LayerKind::Image { media, .. } if !super::bind::is_brand_image(media) => Some(media.as_str()),
            _ => None,
        };
        if let Some(t) = text {
            let unknown = bindings.unknown_keys(t);
            if !unknown.is_empty() {
                push(true, format!("Unknown live data {} (check the spelling, or the feed has no data yet)", unknown.iter().map(|k| format!("{{{{{k}}}}}")).collect::<Vec<_>>().join(", ")));
            }
        }
        if let LayerKind::Image { media, .. } = &layer.kind {
            if super::bind::is_brand_image(media) { continue; }
            let name = bindings.apply(media);
            if name.trim().is_empty() {
                push(false, "Image has no picture chosen".into());
            } else if name.trim().starts_with("motif:") {
                // Built-in pictures (holiday motifs) are drawn by the app, not read from the
                // media library: only flag a motif name the app doesn't know.
                if crate::gfx::motifs::raster(name.trim(), 16.0).is_none() {
                    push(true, format!("Built-in picture \"{}\" doesn't exist", name.trim().trim_start_matches("motif:")));
                }
            } else if !media_dir.join(name.trim()).exists() {
                push(true, format!("Image file \"{}\" is missing from the media library", name.trim()));
            }
        }
    }
    out
}

#[cfg(test)]
mod motif_tests {
    /// Holiday cards use built-in motifs; the checker must not call them missing (CWXB-55).
    #[test]
    fn holiday_motifs_are_not_missing() {
        let brand = crate::config::Brand::default();
        let bindings = super::Bindings::default();
        let empty = std::env::temp_dir().join("cwx-no-media-here");
        for h in crate::gfx::holidays::HOLIDAYS {
            let doc = crate::gfx::templates::more::holiday(h.id).unwrap();
            let bad: Vec<_> = super::check_doc(&doc, &bindings, &brand, &empty)
                .into_iter()
                .filter(|i| i.text.contains("missing from the media library") || i.text.contains("doesn't exist"))
                .map(|i| i.text)
                .collect();
            assert!(bad.is_empty(), "{}: {bad:?}", h.id);
        }
    }
}
