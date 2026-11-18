//! Chart layers: line, area, bar, two-axis line and gauge, drawn from data series
//! (`hourly.temp`, `day.hi`, `river.stage`, …) or typed values ("Mon:3, Tue:5").

use tiny_skia::{Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

use crate::gfx::text::{draw_line, measure, GlyphCache, TextStyle};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChartKind {
    #[default]
    Line,
    Area,
    Bar,
    /// Two series on separate left / right scales (e.g. temperature and rain).
    TwoAxis,
    /// One value on a half-circle dial.
    Gauge,
    /// Series 1 (across) against series 2 (up), one dot per pair.
    Scatter,
    /// One value as a full ring filling toward the maximum.
    Radial,
    /// How the values are spread: histogram with the average and middle marked.
    Distribution,
    /// Every ensemble member as a thin line, the mean bold.
    Spaghetti,
    /// Ensemble percentile bands (10-90 and 25-75) with the median.
    Plume,
}

impl ChartKind {
    pub const ALL: [ChartKind; 10] = [ChartKind::Line, ChartKind::Area, ChartKind::Bar, ChartKind::TwoAxis, ChartKind::Gauge, ChartKind::Scatter, ChartKind::Radial, ChartKind::Distribution, ChartKind::Spaghetti, ChartKind::Plume];

    pub fn label(self) -> &'static str {
        match self {
            ChartKind::Line => "Line",
            ChartKind::Area => "Area",
            ChartKind::Bar => "Bar",
            ChartKind::TwoAxis => "Two-axis",
            ChartKind::Gauge => "Gauge",
            ChartKind::Scatter => "Scatter",
            ChartKind::Radial => "Ring",
            ChartKind::Distribution => "Distribution",
            ChartKind::Spaghetti => "Spaghetti",
            ChartKind::Plume => "Plume",
        }
    }
}

/// Built-in series the picker offers: (key, description).
pub const SERIES: &[(&str, &str)] = &[
    ("hourly.temp", "Temperature, next 24 h"),
    ("hourly.feels", "Feels like, next 24 h"),
    ("hourly.wind", "Wind, next 24 h"),
    ("hourly.gust", "Gusts, next 24 h"),
    ("hourly.rain", "Precipitation per hour, next 24 h"),
    ("hourly.cloud", "Cloud cover, next 24 h"),
    ("hourly.pressure", "Pressure, next 24 h"),
    ("day.hi", "Daily highs (7-day)"),
    ("day.lo", "Daily lows (7-day)"),
    ("day.precip", "Rain chance (7-day)"),
    ("daylight.next30", "Daylight hours, next 30 days"),
    ("daylight.year", "Daylight hours through the year"),
    ("lightning.rate", "Strikes per 5 min near home, last hour"),
    ("reports.kinds", "Storm reports by type, 24 h"),
    ("cities.temp", "Nearby city temperatures"),
    ("river.stage", "River hydrograph (worst gauge near home)"),
    ("river.flood", "Flood stage line for the hydrograph"),
    ("cell.dbz", "Strongest storm cell: max reflectivity history"),
    ("ten.hi", "10-day highs"),
    ("ten.lo", "10-day lows"),
    ("ten.pop", "10-day rain chances"),
    ("models.gfs", "GFS highs, 7 days"),
    ("models.ecmwf", "ECMWF highs, 7 days"),
    ("models.icon", "ICON highs, 7 days"),
];

/// Parse typed values: "3, 5, 8" or "Mon:3, Tue:5".
pub fn parse_inline(text: &str) -> Vec<(String, f32)> {
    text.split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            match part.rsplit_once(':') {
                Some((label, v)) => Some((label.trim().to_string(), v.trim().parse().ok()?)),
                None => Some((String::new(), part.parse().ok()?)),
            }
        })
        .collect()
}

fn paint(c: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(c);
    p.anti_alias = true;
    p
}

fn stroke(w: f32) -> Stroke {
    Stroke { width: w, line_cap: LineCap::Round, line_join: LineJoin::Round, ..Stroke::default() }
}

fn nice_range(values: &[f32], min: Option<f32>, max: Option<f32>, bars: bool) -> (f32, f32) {
    let lo = values.iter().copied().fold(f32::MAX, f32::min);
    let hi = values.iter().copied().fold(f32::MIN, f32::max);
    let (mut lo, mut hi) = if values.is_empty() { (0.0, 1.0) } else { (lo, hi) };
    if bars {
        lo = lo.min(0.0);
    }
    if (hi - lo).abs() < 1e-3 {
        hi = lo + 1.0;
    }
    let pad = (hi - lo) * 0.12;
    (min.unwrap_or(if bars { lo } else { lo - pad }), max.unwrap_or(hi + pad))
}

fn fmt(v: f32) -> String {
    if v.abs() >= 10.0 || v.fract().abs() < 0.05 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    }
}

pub struct ChartStyle {
    pub color: Color,
    pub color2: Color,
    pub text: TextStyle,
    pub show_values: bool,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub unit: String,
    /// 0-1: how much of the chart has drawn in (animated time series; 1 = all).
    pub progress: f32,
    /// Highlight a point: "max", "min", "last", or a category label ("Tue").
    pub highlight: String,
    /// Annotation text at the highlighted point.
    pub note: String,
    /// Ensemble members (spaghetti) or the 10/25/50/75/90 percentiles (plume).
    pub members: Vec<Vec<f32>>,
}

pub fn draw_chart(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    rect: (f32, f32, f32, f32),
    kind: ChartKind,
    a: &[(String, f32)],
    b: &[(String, f32)],
    st: &ChartStyle,
) {
    let (x, y, w, h) = rect;
    if a.is_empty() {
        let msg = "No data yet";
        let tw = measure(msg, &st.text);
        draw_line(pm, glyphs, msg, x + (w - tw) / 2.0, y + h / 2.0, &st.text, 0.7, None);
        return;
    }
    if kind == ChartKind::Gauge {
        draw_gauge(pm, glyphs, rect, a.last().map(|p| p.1).unwrap_or(0.0), st);
        return;
    }
    if kind == ChartKind::Radial {
        draw_radial(pm, glyphs, rect, a.last().map(|p| p.1).unwrap_or(0.0), st);
        return;
    }
    if kind == ChartKind::Scatter {
        draw_scatter(pm, glyphs, rect, a, b, st);
        return;
    }
    if kind == ChartKind::Distribution {
        draw_distribution(pm, glyphs, rect, a, st);
        return;
    }
    if matches!(kind, ChartKind::Spaghetti | ChartKind::Plume) {
        draw_ensemble(pm, glyphs, rect, kind, a, st);
        return;
    }
    // Animated time series: only the first part of the data draws in.
    let shown_n = ((a.len() as f32 * st.progress.clamp(0.0, 1.0)).ceil() as usize).clamp(1, a.len());
    let a_full = a;
    let a = &a_full[..shown_n];
    let b_shown = &b[..b.len().min(shown_n)];
    let label_h = st.text.size * 1.6;
    let axis_w = st.text.size * 2.8;
    let right_w = if kind == ChartKind::TwoAxis && !b.is_empty() { axis_w } else { st.text.size * 0.6 };
    let plot = (x + axis_w, y + st.text.size * 0.8, w - axis_w - right_w, h - label_h - st.text.size * 0.8);
    let (px, py, pw, ph) = plot;
    if pw <= 10.0 || ph <= 10.0 {
        return;
    }
    let values: Vec<f32> = a_full.iter().map(|p| p.1).collect();
    // Line / area charts with a second series on the same scale share the range.
    let shared: Vec<f32> = if kind != ChartKind::TwoAxis { values.iter().chain(b.iter().map(|p| &p.1)).copied().collect() } else { values.clone() };
    let (lo, hi) = nice_range(&shared, st.min, st.max, kind == ChartKind::Bar);
    let n = a_full.len();
    let sx = |i: usize| if kind == ChartKind::Bar { px + pw * (i as f32 + 0.5) / n as f32 } else { px + pw * i as f32 / (n.max(2) - 1) as f32 };
    let sy = |v: f32, lo: f32, hi: f32| py + ph - (v - lo) / (hi - lo) * ph;

    // Grid lines + left axis labels.
    let grid = paint(Color::from_rgba8(255, 255, 255, 40));
    for k in 0..=4 {
        let v = lo + (hi - lo) * k as f32 / 4.0;
        let gy = sy(v, lo, hi);
        if let Some(r) = Rect::from_xywh(px, gy, pw, 1.5) {
            pm.fill_rect(r, &grid, Transform::identity(), None);
        }
        let t = format!("{}{}", fmt(v), st.unit);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, px - tw - 8.0, gy + st.text.size * 0.35, &st.text, 0.8, None);
    }

    match kind {
        ChartKind::Bar => {
            let bw = pw / n as f32 * 0.66;
            for (i, (_, v)) in a.iter().enumerate() {
                let top = sy(v.max(lo), lo, hi);
                let base = sy(lo.max(0.0).min(hi), lo, hi);
                if let Some(r) = Rect::from_xywh(sx(i) - bw / 2.0, top.min(base), bw, (base - top).abs().max(1.0)) {
                    pm.fill_rect(r, &paint(st.color), Transform::identity(), None);
                }
                if st.show_values {
                    let t = fmt(*v);
                    let tw = measure(&t, &st.text);
                    draw_line(pm, glyphs, &t, sx(i) - tw / 2.0, top - 8.0, &st.text, 1.0, None);
                }
            }
        }
        _ => {
            let line = |pm: &mut Pixmap, pts: &[(String, f32)], lo: f32, hi: f32, color: Color, area: bool| {
                let mut pb = PathBuilder::new();
                for (i, (_, v)) in pts.iter().enumerate() {
                    let (lx, ly) = (px + pw * i as f32 / (n.max(2) - 1) as f32, sy(*v, lo, hi));
                    if i == 0 {
                        pb.move_to(lx, ly);
                    } else {
                        pb.line_to(lx, ly);
                    }
                }
                if area {
                    let mut fill = pb.clone();
                    fill.line_to(px + pw * (pts.len().max(1) - 1) as f32 / (n.max(2) - 1) as f32, py + ph);
                    fill.line_to(px, py + ph);
                    fill.close();
                    if let Some(path) = fill.finish() {
                        let mut c = color;
                        c.set_alpha(0.3);
                        pm.fill_path(&path, &paint(c), FillRule::Winding, Transform::identity(), None);
                    }
                }
                if let Some(path) = pb.finish() {
                    pm.stroke_path(&path, &paint(color), &stroke(5.0), Transform::identity(), None);
                }
            };
            line(pm, a, lo, hi, st.color, kind == ChartKind::Area);
            if !b.is_empty() {
                let (lo2, hi2) = if kind == ChartKind::TwoAxis { nice_range(&b.iter().map(|p| p.1).collect::<Vec<_>>(), None, None, false) } else { (lo, hi) };
                line(pm, b_shown, lo2, hi2, st.color2, false);
                if kind == ChartKind::TwoAxis {
                    for k in 0..=4 {
                        let v = lo2 + (hi2 - lo2) * k as f32 / 4.0;
                        draw_line(pm, glyphs, &fmt(v), px + pw + 8.0, sy(v, lo2, hi2) + st.text.size * 0.35, &st.text, 0.8, None);
                    }
                }
            }
            if st.show_values {
                for (i, (_, v)) in a.iter().enumerate() {
                    if n > 12 && i % 3 != 0 {
                        continue;
                    }
                    let (lx, ly) = (sx(i), sy(*v, lo, hi));
                    let mut pb = PathBuilder::new();
                    pb.push_circle(lx, ly, 6.0);
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(st.color), FillRule::Winding, Transform::identity(), None);
                    }
                    let t = fmt(*v);
                    let tw = measure(&t, &st.text);
                    // Keep end labels inside the plot so the first one can't run into the
                    // y-axis labels ("11 h9.8").
                    let tx = (lx - tw / 2.0).clamp(px + 4.0, (px + pw - tw).max(px + 4.0));
                    draw_line(pm, glyphs, &t, tx, ly - 14.0, &st.text, 1.0, None);
                }
            }
        }
    }

    // Highlight and annotation.
    if !st.highlight.is_empty() && shown_n == n {
        let pick = match st.highlight.as_str() {
            "max" => a.iter().enumerate().max_by(|x, y| x.1 .1.total_cmp(&y.1 .1)).map(|(i, _)| i),
            "min" => a.iter().enumerate().min_by(|x, y| x.1 .1.total_cmp(&y.1 .1)).map(|(i, _)| i),
            "last" => Some(n - 1),
            label => a.iter().position(|(l, _)| l.eq_ignore_ascii_case(label)),
        };
        if let Some(i) = pick {
            let v = a[i].1;
            let (hx, hy) = (sx(i), sy(v, lo, hi));
            let mut pb = PathBuilder::new();
            pb.push_circle(hx, hy, 14.0);
            if let Some(path) = pb.finish() {
                pm.stroke_path(&path, &paint(Color::WHITE), &stroke(4.0), Transform::identity(), None);
            }
            let text = if st.note.is_empty() { format!("{}{}", fmt(v), st.unit) } else { st.note.clone() };
            let mut big = st.text.clone();
            big.size *= 1.15;
            let tw = measure(&text, &big);
            let lx = (hx - tw / 2.0).clamp(px, px + pw - tw);
            let ly = (hy - 30.0).max(py + big.size);
            if let Some(r) = Rect::from_xywh(lx - 10.0, ly - big.size, tw + 20.0, big.size * 1.4) {
                pm.fill_rect(r, &paint(Color::from_rgba8(0, 0, 0, 170)), Transform::identity(), None);
            }
            draw_line(pm, glyphs, &text, lx, ly, &big, 1.0, None);
        }
    }

    // Category labels along the bottom (thinned to fit).
    let every = ((n as f32 * st.text.size * 3.2) / pw).ceil().max(1.0) as usize;
    for (i, (label, _)) in a_full.iter().enumerate() {
        if label.is_empty() || i % every != 0 {
            continue;
        }
        let tw = measure(label, &st.text);
        draw_line(pm, glyphs, label, sx(i) - tw / 2.0, py + ph + st.text.size * 1.3, &st.text, 0.9, None);
    }
}

fn draw_gauge(pm: &mut Pixmap, glyphs: &mut GlyphCache, rect: (f32, f32, f32, f32), value: f32, st: &ChartStyle) {
    let (x, y, w, h) = rect;
    let (lo, hi) = (st.min.unwrap_or(0.0), st.max.unwrap_or(100.0));
    let r = (w / 2.0).min(h * 0.85) - 12.0;
    let (cx, cy) = (x + w / 2.0, y + h * 0.85);
    let arc = |from: f32, to: f32| {
        let mut pb = PathBuilder::new();
        let steps = 48;
        for i in 0..=steps {
            let t = from + (to - from) * i as f32 / steps as f32;
            let ang = std::f32::consts::PI * (1.0 - t);
            let (px, py) = (cx + r * ang.cos(), cy - r * ang.sin());
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        pb.finish()
    };
    let thick = (r * 0.18).max(10.0);
    if let Some(path) = arc(0.0, 1.0) {
        pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 50)), &stroke(thick), Transform::identity(), None);
    }
    let p = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
    if p > 0.0 {
        if let Some(path) = arc(0.0, p) {
            pm.stroke_path(&path, &paint(st.color), &stroke(thick), Transform::identity(), None);
        }
    }
    let mut big = st.text.clone();
    big.size *= 2.6;
    let t = format!("{}{}", fmt(value), st.unit);
    let tw = measure(&t, &big);
    draw_line(pm, glyphs, &t, cx - tw / 2.0, cy - r * 0.15, &big, 1.0, None);
    for (v, align) in [(lo, -1.0f32), (hi, 1.0)] {
        let t = fmt(v);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, cx + align * r - tw / 2.0, cy + st.text.size * 1.3, &st.text, 0.8, None);
    }
}

/// Full ring that fills clockwise from the top toward the maximum, value in the middle.
fn draw_radial(pm: &mut Pixmap, glyphs: &mut GlyphCache, rect: (f32, f32, f32, f32), value: f32, st: &ChartStyle) {
    let (x, y, w, h) = rect;
    let (lo, hi) = (st.min.unwrap_or(0.0), st.max.unwrap_or(100.0));
    let r = (w.min(h) / 2.0) - 16.0;
    let (cx, cy) = (x + w / 2.0, y + h / 2.0);
    let thick = (r * 0.16).max(10.0);
    let ring = |from: f32, to: f32| {
        let mut pb = PathBuilder::new();
        let steps = 72;
        for i in 0..=steps {
            let t = from + (to - from) * i as f32 / steps as f32;
            let ang = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU;
            let (px, py) = (cx + r * ang.cos(), cy + r * ang.sin());
            if i == 0 {
                pb.move_to(px, py);
            } else {
                pb.line_to(px, py);
            }
        }
        pb.finish()
    };
    if let Some(path) = ring(0.0, 1.0) {
        pm.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 45)), &stroke(thick), Transform::identity(), None);
    }
    let p = ((value - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0) * st.progress.clamp(0.0, 1.0);
    if p > 0.001 {
        if let Some(path) = ring(0.0, p) {
            pm.stroke_path(&path, &paint(st.color), &stroke(thick), Transform::identity(), None);
        }
    }
    let mut big = st.text.clone();
    big.size *= 2.4;
    let t = format!("{}{}", fmt(value), st.unit);
    let tw = measure(&t, &big);
    draw_line(pm, glyphs, &t, cx - tw / 2.0, cy + big.size * 0.35, &big, 1.0, None);
    if !st.note.is_empty() {
        let tw = measure(&st.note, &st.text);
        draw_line(pm, glyphs, &st.note, cx - tw / 2.0, cy + big.size * 0.35 + st.text.size * 1.4, &st.text, 0.85, None);
    }
}

/// Scatter: series 1 across, series 2 up; dots appear in order with the progress.
fn draw_scatter(pm: &mut Pixmap, glyphs: &mut GlyphCache, rect: (f32, f32, f32, f32), a: &[(String, f32)], b: &[(String, f32)], st: &ChartStyle) {
    let (x, y, w, h) = rect;
    let n = a.len().min(b.len());
    if n == 0 {
        let msg = "Scatter needs two series";
        let tw = measure(msg, &st.text);
        draw_line(pm, glyphs, msg, x + (w - tw) / 2.0, y + h / 2.0, &st.text, 0.7, None);
        return;
    }
    let xs: Vec<f32> = a[..n].iter().map(|p| p.1).collect();
    let ys: Vec<f32> = b[..n].iter().map(|p| p.1).collect();
    let (x0, x1) = nice_range(&xs, None, None, false);
    let (y0, y1) = nice_range(&ys, st.min, st.max, false);
    let axis = st.text.size * 2.8;
    let (px, py, pw, ph) = (x + axis, y + st.text.size, w - axis - st.text.size, h - st.text.size * 2.6);
    let grid = paint(Color::from_rgba8(255, 255, 255, 40));
    for k in 0..=4 {
        let gy = py + ph - ph * k as f32 / 4.0;
        if let Some(r) = Rect::from_xywh(px, gy, pw, 1.5) {
            pm.fill_rect(r, &grid, Transform::identity(), None);
        }
        let t = fmt(y0 + (y1 - y0) * k as f32 / 4.0);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, px - tw - 8.0, gy + st.text.size * 0.35, &st.text, 0.8, None);
        let tx = fmt(x0 + (x1 - x0) * k as f32 / 4.0);
        let txw = measure(&tx, &st.text);
        draw_line(pm, glyphs, &tx, px + pw * k as f32 / 4.0 - txw / 2.0, py + ph + st.text.size * 1.4, &st.text, 0.8, None);
    }
    let shown = ((n as f32 * st.progress.clamp(0.0, 1.0)).ceil() as usize).min(n);
    for i in 0..shown {
        let dx = px + (xs[i] - x0) / (x1 - x0) * pw;
        let dy = py + ph - (ys[i] - y0) / (y1 - y0) * ph;
        let mut pb = PathBuilder::new();
        pb.push_circle(dx, dy, 8.0);
        if let Some(path) = pb.finish() {
            pm.fill_path(&path, &paint(st.color), FillRule::Winding, Transform::identity(), None);
            pm.stroke_path(&path, &paint(Color::from_rgba8(0, 0, 0, 120)), &stroke(1.5), Transform::identity(), None);
        }
        if st.show_values && !a[i].0.is_empty() && n <= 14 {
            draw_line(pm, glyphs, &a[i].0, dx + 12.0, dy - 8.0, &st.text, 0.9, None);
        }
    }
}

/// Histogram of the values (up to 10 bins) with the average (solid) and middle (dashed) lines.
fn draw_distribution(pm: &mut Pixmap, glyphs: &mut GlyphCache, rect: (f32, f32, f32, f32), a: &[(String, f32)], st: &ChartStyle) {
    let (x, y, w, h) = rect;
    let mut vals: Vec<f32> = a.iter().map(|p| p.1).filter(|v| v.is_finite()).collect();
    if vals.is_empty() {
        return;
    }
    vals.sort_by(|p, q| p.total_cmp(q));
    let (lo, hi) = (vals[0], vals[vals.len() - 1].max(vals[0] + 1e-3));
    let bins = (vals.len() as f32).sqrt().ceil().clamp(3.0, 10.0) as usize;
    let mut counts = vec![0u32; bins];
    for v in &vals {
        let i = (((v - lo) / (hi - lo)) * bins as f32).floor() as usize;
        counts[i.min(bins - 1)] += 1;
    }
    let top = *counts.iter().max().unwrap_or(&1) as f32;
    let axis = st.text.size * 1.2;
    let (px, py, pw, ph) = (x + axis, y + st.text.size * 2.0, w - axis * 2.0, h - st.text.size * 4.4);
    let bw = pw / bins as f32;
    let grow = st.progress.clamp(0.0, 1.0);
    for (i, c) in counts.iter().enumerate() {
        let bh = ph * (*c as f32 / top) * grow;
        if let Some(r) = Rect::from_xywh(px + i as f32 * bw + 2.0, py + ph - bh, bw - 4.0, bh.max(1.0)) {
            pm.fill_rect(r, &paint(st.color), Transform::identity(), None);
        }
        if st.show_values && *c > 0 {
            let t = c.to_string();
            let tw = measure(&t, &st.text);
            draw_line(pm, glyphs, &t, px + i as f32 * bw + (bw - tw) / 2.0, py + ph - bh - 8.0, &st.text, 1.0, None);
        }
    }
    let mean = vals.iter().sum::<f32>() / vals.len() as f32;
    let median = vals[vals.len() / 2];
    for (v, label, dashed) in [(mean, "Average", false), (median, "Middle", true)] {
        let lx = px + (v - lo) / (hi - lo) * pw;
        let mut pb = PathBuilder::new();
        pb.move_to(lx, py - st.text.size * 0.4);
        pb.line_to(lx, py + ph);
        if let Some(path) = pb.finish() {
            let mut s = stroke(3.0);
            if dashed {
                s.dash = tiny_skia::StrokeDash::new(vec![10.0, 7.0], 0.0);
            }
            pm.stroke_path(&path, &paint(st.color2), &s, Transform::identity(), None);
        }
        let t = format!("{label} {}{}", fmt(v), st.unit);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, (lx - tw / 2.0).clamp(x, x + w - tw), if dashed { py - st.text.size * 0.6 } else { py - st.text.size * 1.7 }, &st.text, 1.0, None);
    }
    for (v, align) in [(lo, 0.0f32), (hi, 1.0)] {
        let t = format!("{}{}", fmt(v), st.unit);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, px + pw * align - tw * align, py + ph + st.text.size * 1.4, &st.text, 0.8, None);
    }
}

/// Spaghetti (every member, mean bold) or plume (percentile bands) over the labels of `a`.
fn draw_ensemble(pm: &mut Pixmap, glyphs: &mut GlyphCache, rect: (f32, f32, f32, f32), kind: ChartKind, a: &[(String, f32)], st: &ChartStyle) {
    let (x, y, w, h) = rect;
    let all: Vec<f32> = st.members.iter().flatten().copied().filter(|v| v.is_finite()).collect();
    if all.is_empty() {
        let msg = "Ensemble loads with the next refresh";
        let tw = measure(msg, &st.text);
        draw_line(pm, glyphs, msg, x + (w - tw) / 2.0, y + h / 2.0, &st.text, 0.7, None);
        return;
    }
    let (lo, hi) = nice_range(&all, st.min, st.max, false);
    let axis = st.text.size * 2.8;
    let (px, py, pw, ph) = (x + axis, y + st.text.size * 0.8, w - axis - st.text.size, h - st.text.size * 2.6);
    let n = st.members.iter().map(|m| m.len()).max().unwrap_or(0).max(2);
    let sx = |i: usize| px + pw * i as f32 / (n - 1) as f32;
    let sy = |v: f32| py + ph - (v - lo) / (hi - lo) * ph;
    for k in 0..=4 {
        let v = lo + (hi - lo) * k as f32 / 4.0;
        if let Some(r) = Rect::from_xywh(px, sy(v), pw, 1.5) {
            pm.fill_rect(r, &paint(Color::from_rgba8(255, 255, 255, 40)), Transform::identity(), None);
        }
        let t = format!("{}{}", fmt(v), st.unit);
        let tw = measure(&t, &st.text);
        draw_line(pm, glyphs, &t, px - tw - 8.0, sy(v) + st.text.size * 0.35, &st.text, 0.8, None);
    }
    let shown = ((n as f32 * st.progress.clamp(0.0, 1.0)).ceil() as usize).clamp(2, n);
    let path_of = |m: &[f32]| {
        let mut pb = PathBuilder::new();
        let mut started = false;
        for (i, v) in m.iter().enumerate().take(shown) {
            if !v.is_finite() {
                continue;
            }
            if started {
                pb.line_to(sx(i), sy(*v));
            } else {
                pb.move_to(sx(i), sy(*v));
                started = true;
            }
        }
        pb.finish()
    };
    if kind == ChartKind::Spaghetti {
        let mut thin = st.color;
        thin.set_alpha(0.35);
        for m in &st.members {
            if let Some(p) = path_of(m) {
                pm.stroke_path(&p, &paint(thin), &stroke(2.0), Transform::identity(), None);
            }
        }
        let mean: Vec<f32> = a.iter().map(|p| p.1).collect();
        if let Some(p) = path_of(&mean) {
            pm.stroke_path(&p, &paint(st.color2), &stroke(6.0), Transform::identity(), None);
        }
    } else if st.members.len() >= 5 {
        let band = |pm: &mut Pixmap, low: &[f32], high: &[f32], alpha: f32| {
            let mut pb = PathBuilder::new();
            for (i, v) in high.iter().enumerate().take(shown) {
                if i == 0 { pb.move_to(sx(i), sy(*v)); } else { pb.line_to(sx(i), sy(*v)); }
            }
            for i in (0..shown.min(low.len())).rev() {
                pb.line_to(sx(i), sy(low[i]));
            }
            pb.close();
            if let Some(p) = pb.finish() {
                let mut c = st.color;
                c.set_alpha(alpha);
                pm.fill_path(&p, &paint(c), FillRule::Winding, Transform::identity(), None);
            }
        };
        band(pm, &st.members[0], &st.members[4], 0.25);
        band(pm, &st.members[1], &st.members[3], 0.45);
        if let Some(p) = path_of(&st.members[2]) {
            pm.stroke_path(&p, &paint(st.color2), &stroke(5.0), Transform::identity(), None);
        }
    }
    let every = ((n as f32 * st.text.size * 3.2) / pw).ceil().max(1.0) as usize;
    for (i, (label, _)) in a.iter().enumerate() {
        if label.is_empty() || i % every != 0 {
            continue;
        }
        let tw = measure(label, &st.text);
        draw_line(pm, glyphs, label, sx(i) - tw / 2.0, py + ph + st.text.size * 1.3, &st.text, 0.9, None);
    }
}
