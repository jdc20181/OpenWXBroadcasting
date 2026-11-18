//! Drawings for the `history:` specs (maps, charts, calendar, timelines). Everything reads the
//! history store; with no data yet each one shows a calm "Data not available yet" card.

use chrono::{Datelike, NaiveDate};
use tiny_skia::{Color, Mask, Pixmap, Transform};

use super::{
    data::{self, Hist},
    local_date,
    paint::{ellipse, hex, linear, mix, poly, rgba, rrect, solid, with_alpha, Al, Canvas, StateMap},
};
use crate::gfx::text::{GlyphCache, Weight};

pub const TO_C: &str = "#FF2D2D";
pub const SV_C: &str = "#FFD60A";
pub const FF_C: &str = "#2ED16B";

fn land() -> Color {
    hex("#22324A")
}

fn county_line() -> Color {
    rgba(190, 210, 235, 0.35)
}

fn phen_color(p: &str) -> Color {
    hex(match p {
        "TO" => TO_C,
        "SV" => SV_C,
        _ => FF_C,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, spec: &str, opacity: f32, font: &str) {
    let Some(mut cv) = Canvas::new(glyphs, w, h, local, font) else { return };
    let snap = data::snapshot();
    let home = super::home();
    match snap {
        None => cv.missing(label_for(spec)),
        Some(hs) => {
            let (kind, arg) = spec.split_once(':').unwrap_or((spec, ""));
            match kind {
                "polys" => polys(&mut cv, &hs, arg),
                "county" => county(&mut cv, &hs, arg),
                "months" => months(&mut cv, &hs, arg),
                "years" => years(&mut cv, &hs, arg),
                "countyyears" => county_years(&mut cv, &hs, false),
                "countyto" => county_years(&mut cv, &hs, true),
                "reports" => reports(&mut cv, &hs, arg),
                "tracks" => tracks(&mut cv, &hs),
                "strongest" => strongest(&mut cv, &hs, home),
                "calendar" => calendar(&mut cv, &hs),
                "freeze" => freeze(&mut cv, &hs),
                "wettest" => wettest(&mut cv, &hs),
                _ => cv.missing("Unknown history drawing"),
            }
        }
    }
    cv.finish(pm, local, mask, opacity);
}

fn label_for(spec: &str) -> &'static str {
    match spec.split(':').next().unwrap_or("") {
        "polys" => "Warning map",
        "county" => "County map",
        "months" | "years" | "countyyears" | "countyto" => "Warning history",
        "reports" => "Storm reports",
        "tracks" | "strongest" => "Tornado database",
        "calendar" => "Warning calendar",
        "freeze" | "wettest" => "Station climate",
        _ => "History",
    }
}

/// Map panel backdrop: rounded dark glass.
fn backdrop(cv: &mut Canvas, x: f32, y: f32, w: f32, h: f32) {
    cv.round(x, y, w, h, 24., &linear(0., y, 0., y + h, &[(0., rgba(10, 20, 36, 0.88)), (1., rgba(18, 32, 54, 0.88))]));
    cv.stroke(rrect(x, y, w, h, 24.), rgba(255, 255, 255, 0.10), 2.);
}

fn legend_chip(cv: &mut Canvas, x: f32, y: f32, c: Color, label: &str, outline: bool) {
    if outline {
        cv.stroke(rrect(x, y - 22., 34., 26., 4.), c, 4.);
    } else {
        cv.round(x, y - 22., 34., 26., 5., &solid(c));
    }
    cv.text(label, x + 46., y, 26., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
}

fn polys(cv: &mut Canvas, h: &Hist, which: &str) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    // Legend in the left column; the state fills the rest.
    let legend_w = 300.;
    let Some(m) = StateMap::new(&h.state_fips, legend_w, 24., w - legend_w - 30., hh - 48.) else {
        cv.missing("County map");
        return;
    };
    m.base(cv, land(), county_line());
    let day = if which == "busiest" { super::busiest_day(h).map(|d| d.0) } else { None };
    let list: Vec<&data::Poly> = h.polys.iter().filter(|p| day.is_none() || local_date(p.issue) == day).collect();
    // Clip to the state so polygons from neighbouring offices don't spill off the map.
    let mut pb = tiny_skia::PathBuilder::new();
    for (_, _, rings) in &m.counties {
        if let Some(p) = m.path(rings) {
            pb.push_path(&p);
        }
    }
    let state_path = pb.finish();
    cv.set_clip(state_path.as_ref());
    // Severe first, tornado on top; outlines only so overlapping polygons stay readable.
    for phen in ["SV", "TO"] {
        let c = phen_color(phen);
        for p in list.iter().filter(|p| p.phen == phen) {
            let path = m.path(&p.rings);
            cv.fill_c(path.clone(), with_alpha(c, 0.035));
            cv.stroke(path, c, if phen == "TO" { 2.6 } else { 1.6 });
        }
    }
    cv.set_clip(None);
    m.lines(cv, rgba(10, 16, 28, 0.35), 1.);
    let n_to = list.iter().filter(|p| p.phen == "TO").count();
    let n_sv = list.iter().filter(|p| p.phen == "SV").count();
    cv.text("POLYGONS", 30., 60., 26., Weight::Bold, rgba(255, 211, 77, 1.), Al::Left, 0.);
    legend_chip(cv, 30., 112., phen_color("TO"), "Tornado", true);
    cv.text(&n_to.to_string(), 76., 150., 26., Weight::SemiBold, rgba(220, 230, 245, 0.9), Al::Left, 0.);
    legend_chip(cv, 30., 202., phen_color("SV"), "Severe t-storm", true);
    cv.text(&n_sv.to_string(), 76., 240., 26., Weight::SemiBold, rgba(220, 230, 245, 0.9), Al::Left, 0.);
    cv.text("Flash flood", 30., hh - 70., 20., Weight::Regular, rgba(210, 220, 235, 0.8), Al::Left, legend_w - 40.);
    cv.text("polygons not shown", 30., hh - 44., 20., Weight::Regular, rgba(210, 220, 235, 0.8), Al::Left, legend_w - 40.);
    if list.is_empty() {
        cv.text("No warning polygons for this period", w / 2., hh / 2., 34., Weight::SemiBold, Color::WHITE, Al::Center, w * 0.8);
    }
}

/// Sequential ramp for counts (pale yellow → orange → red → deep purple).
pub fn ramp(t: f32) -> Color {
    let stops = [(0.0, hex("#FFF3B0")), (0.3, hex("#FFC24B")), (0.55, hex("#FF7A2F")), (0.78, hex("#E0262F")), (1.0, hex("#6A1B6E"))];
    let t = t.clamp(0., 1.);
    for win in stops.windows(2) {
        if t <= win[1].0 {
            return mix(win[0].1, win[1].1, (t - win[0].0) / (win[1].0 - win[0].0));
        }
    }
    stops[4].1
}

/// Legend bins for counts 0..=max: single numbers when few, else ranges. (lo, hi) inclusive.
pub fn bins(max: u32) -> Vec<(u32, u32)> {
    if max <= 9 {
        return (0..=max).map(|v| (v, v)).collect();
    }
    let step = max.div_ceil(9);
    let mut out = vec![(0, 0)];
    let mut lo = 1;
    while lo <= max {
        out.push((lo, (lo + step - 1).min(max)));
        lo += step;
    }
    out
}

fn county(cv: &mut Canvas, h: &Hist, phen: &str) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let legend_w = 360.;
    let Some(m) = StateMap::new(&h.state_fips, 30., 20., w - legend_w - 60., hh - 40.) else {
        cv.missing("County map");
        return;
    };
    let mut counts: std::collections::HashMap<String, u32> = Default::default();
    for wn in h.warns.iter().filter(|w| w.phen == phen) {
        for c in &wn.counties {
            *counts.entry(c.to_lowercase()).or_default() += 1;
        }
    }
    let max = counts.values().copied().max().unwrap_or(0);
    let bs = bins(max);
    let color_of = |n: u32| -> Color {
        if n == 0 {
            return hex("#2A3A52");
        }
        let i = bs.iter().position(|(lo, hi)| n >= *lo && n <= *hi).unwrap_or(0);
        ramp((i as f32 - 1.).max(0.) / (bs.len() as f32 - 2.).max(1.))
    };
    for (_, _, rings) in &m.counties {
        let p = m.path(rings).and_then(|p| p.transform(Transform::from_translate(6., 8.)));
        cv.fill_c(p, rgba(0, 0, 0, 0.35));
    }
    for (name, _, rings) in &m.counties {
        let n = counts.get(&name.to_lowercase()).copied().unwrap_or(0);
        cv.fill_c(m.path(rings), color_of(n));
    }
    m.lines(cv, rgba(20, 24, 32, 0.7), 1.6);
    // Numbers in each county (skipped where the map is too small to read them).
    let size = (m.to(0.25, 0.).0 - m.to(0., 0.).0).abs().clamp(0., 26.);
    if size >= 13. {
        for (name, _, rings) in &m.counties {
            let n = counts.get(&name.to_lowercase()).copied().unwrap_or(0);
            if n == 0 {
                continue;
            }
            let (lon, lat) = StateMap::centre(rings);
            let (x, y) = m.to(lon, lat);
            cv.text_halo(&n.to_string(), x, y + size * 0.35, size, Weight::Bold, Color::WHITE, Al::Center);
        }
    }
    // Numbered legend column.
    let lx = w - legend_w - 10.;
    cv.round(lx, 20., legend_w - 10., hh - 40., 18., &solid(rgba(255, 255, 255, 0.06)));
    cv.text("WARNINGS", lx + (legend_w - 10.) / 2., 66., 28., Weight::Bold, rgba(255, 211, 77, 1.), Al::Center, 0.);
    let rows = bs.len().max(1) as f32;
    let row_h = ((hh - 220.) / rows).min(62.);
    for (i, (lo, hi)) in bs.iter().enumerate() {
        let y = 96. + i as f32 * row_h;
        let c = color_of(*lo);
        cv.round(lx + 30., y, 70., row_h - 10., 8., &solid(c));
        cv.stroke(rrect(lx + 30., y, 70., row_h - 10., 8.), rgba(0, 0, 0, 0.5), 1.5);
        let label = if lo == hi { lo.to_string() } else { format!("{lo}–{hi}") };
        cv.text(&label, lx + 124., y + (row_h - 10.) * 0.72, 30., Weight::Bold, Color::WHITE, Al::Left, 200.);
    }
    let total = h.warns.iter().filter(|w| w.phen == phen).count();
    let note = format!("{total} so far in {}!", h.year);
    cv.text(&note, lx + (legend_w - 10.) / 2., hh - 50., 30., Weight::Bold, Color::WHITE, Al::Center, legend_w - 40.);
}

/// Bars with optional average markers. Values and averages share one scale.
#[allow(clippy::too_many_arguments)]
fn bars(cv: &mut Canvas, rect: (f32, f32, f32, f32), labels: &[String], values: &[f32], avg: Option<&[f32]>, color: Color, highlight: Option<usize>, legend: (&str, &str)) {
    let (x, y, w, h) = rect;
    let max = values.iter().chain(avg.unwrap_or(&[]).iter()).copied().fold(0f32, f32::max).max(1.);
    let step = nice_step(max);
    let top = (max / step).ceil() * step;
    let (px, py, pw, ph) = (x + 90., y + 70., w - 120., h - 140.);
    for k in 0..=4 {
        let v = top * k as f32 / 4.;
        let gy = py + ph - v / top * ph;
        cv.rect(px, gy, pw, 1.5, rgba(255, 255, 255, 0.14));
        cv.text(&fmt_n(v), px - 14., gy + 9., 24., Weight::Regular, rgba(220, 230, 245, 0.85), Al::Right, 0.);
    }
    let n = values.len().max(1) as f32;
    let slot = pw / n;
    let bw = slot * 0.62;
    for (i, v) in values.iter().enumerate() {
        let cx = px + slot * (i as f32 + 0.5);
        let bh = v / top * ph;
        let c = if highlight == Some(i) { mix(color, Color::WHITE, 0.25) } else { color };
        if bh > 0.5 {
            cv.round(cx - bw / 2., py + ph - bh, bw, bh, (bw * 0.18).min(10.), &linear(0., py + ph - bh, 0., py + ph, &[(0., mix(c, Color::WHITE, 0.18)), (1., mix(c, Color::BLACK, 0.25))]));
            // The value sits above the bar, or above the average mark when that is close.
            let mut top_y = py + ph - bh;
            if let Some(a) = avg.and_then(|a| a.get(i)) {
                let ay = py + ph - a / top * ph;
                if ay < top_y + 8. && ay > top_y - 40. {
                    top_y = top_y.min(ay - 6.);
                }
            }
            cv.text(&fmt_n(*v), cx, top_y - 12., 26., Weight::Bold, Color::WHITE, Al::Center, slot);
        }
        cv.text(&labels[i], cx, py + ph + 40., 26., Weight::SemiBold, rgba(220, 230, 245, 1.), Al::Center, slot * 0.95);
    }
    if let Some(a) = avg {
        for (i, v) in a.iter().enumerate().take(values.len()) {
            let cx = px + slot * (i as f32 + 0.5);
            let gy = py + ph - v / top * ph;
            cv.line((cx - bw * 0.62, gy), (cx + bw * 0.62, gy), rgba(0, 0, 0, 0.6), 7.);
            cv.line((cx - bw * 0.6, gy), (cx + bw * 0.6, gy), Color::WHITE, 4.);
        }
    }
    // Legend, top right.
    cv.round(x + w - 600., y + 10., 30., 24., 5., &solid(color));
    cv.text(legend.0, x + w - 560., y + 31., 26., Weight::SemiBold, Color::WHITE, Al::Left, 260.);
    if avg.is_some() || !legend.1.is_empty() {
        cv.line((x + w - 290., y + 22.), (x + w - 258., y + 22.), Color::WHITE, 4.);
        cv.text(legend.1, x + w - 246., y + 31., 26., Weight::SemiBold, Color::WHITE, Al::Left, 240.);
    }
}

fn nice_step(max: f32) -> f32 {
    let raw = max / 4.;
    let mag = 10f32.powf(raw.log10().floor());
    let n = raw / mag;
    mag * if n <= 1. { 1. } else if n <= 2. { 2. } else if n <= 2.5 { 2.5 } else if n <= 5. { 5. } else { 10. }
}

fn fmt_n(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 { format!("{}", v.round() as i64) } else { format!("{v:.1}") }
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn months(cv: &mut Canvas, h: &Hist, phen: &str) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let Some(pi) = data::PHENS.iter().position(|p| *p == phen) else { return };
    let this = data::month_counts(&h.warns, h.year);
    let values: Vec<f32> = this[pi].iter().map(|v| *v as f32).collect();
    let n = h.past.len().max(1) as f32;
    let avg: Vec<f32> = (0..12).map(|m| h.past.iter().map(|y| y.months[pi][m] as f32).sum::<f32>() / n).collect();
    let labels: Vec<String> = MONTHS.iter().map(|m| m.to_string()).collect();
    let cur = chrono::Local::now().month0() as usize;
    bars(cv, (10., 0., w - 20., hh), &labels, &values, if h.past.is_empty() { None } else { Some(&avg) }, phen_color(phen), Some(cur), (&format!("{}", h.year), "10-year average"));
}

fn years(cv: &mut Canvas, h: &Hist, phen: &str) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let Some(pi) = data::PHENS.iter().position(|p| *p == phen) else { return };
    let mut labels: Vec<String> = h.past.iter().map(|y| y.year.to_string()).collect();
    let mut values: Vec<f32> = h.past.iter().map(|y| y.total(pi) as f32).collect();
    labels.push(format!("{}*", h.year));
    values.push(h.warns.iter().filter(|w| w.phen == phen).count() as f32);
    let avg = if h.past.is_empty() { 0. } else { values[..values.len() - 1].iter().sum::<f32>() / (values.len() - 1) as f32 };
    let avgs = vec![avg; values.len()];
    let hl = Some(values.len() - 1);
    bars(cv, (10., 0., w - 20., hh - 30.), &labels, &values, Some(&avgs), phen_color(phen), hl, ("Warnings issued", "Average"));
    cv.text("* this year to date", w - 30., hh - 18., 22., Weight::Regular, rgba(220, 230, 245, 0.8), Al::Right, 0.);
}

fn county_years(cv: &mut Canvas, h: &Hist, tornado_only: bool) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    if h.county_events.is_empty() {
        cv.missing("County warning history");
        return;
    }
    let first = 2005;
    let years: Vec<i32> = (first..=h.year).collect();
    let count = |p: &str, y: i32| h.county_events.iter().filter(|e| e.phen == p && e.sig == "W" && local_date(e.issue).map(|d| d.year()) == Some(y)).count() as f32;
    if tornado_only {
        let labels: Vec<String> = years.iter().map(|y| format!("'{:02}", y % 100)).collect();
        let values: Vec<f32> = years.iter().map(|y| count("TO", *y)).collect();
        bars(cv, (10., 0., w - 20., hh), &labels, &values, None, phen_color("TO"), Some(values.len() - 1), ("Tornado warnings", ""));
        return;
    }
    // Grouped: three slim bars a year.
    let series: Vec<(&str, Vec<f32>)> = ["TO", "SV", "FF"].iter().map(|p| (*p, years.iter().map(|y| count(p, *y)).collect())).collect();
    let max = series.iter().flat_map(|s| s.1.iter()).copied().fold(1f32, f32::max);
    let step = nice_step(max);
    let top = (max / step).ceil() * step;
    let (px, py, pw, ph) = (100., 80., w - 140., hh - 160.);
    for k in 0..=4 {
        let v = top * k as f32 / 4.;
        let gy = py + ph - v / top * ph;
        cv.rect(px, gy, pw, 1.5, rgba(255, 255, 255, 0.14));
        cv.text(&fmt_n(v), px - 14., gy + 9., 24., Weight::Regular, rgba(220, 230, 245, 0.85), Al::Right, 0.);
    }
    let slot = pw / years.len() as f32;
    let bw = slot * 0.26;
    for (yi, y) in years.iter().enumerate() {
        let x0 = px + slot * yi as f32 + slot * 0.11;
        for (si, (p, vals)) in series.iter().enumerate() {
            let v = vals[yi];
            let bh = v / top * ph;
            if bh > 0.5 {
                cv.round(x0 + si as f32 * bw, py + ph - bh, bw - 2., bh, 3., &solid(phen_color(p)));
            }
        }
        cv.text(&format!("'{:02}", y % 100), px + slot * (yi as f32 + 0.5), py + ph + 36., 24., Weight::SemiBold, rgba(220, 230, 245, 1.), Al::Center, slot);
    }
    for (i, (p, label)) in [("TO", "Tornado"), ("SV", "Severe thunderstorm"), ("FF", "Flash flood")].iter().enumerate() {
        let x = 120. + i as f32 * 330.;
        cv.round(x, 22., 30., 24., 5., &solid(phen_color(p)));
        cv.text(label, x + 42., 43., 26., Weight::SemiBold, Color::WHITE, Al::Left, 280.);
    }
}

fn report_style(kind: &str) -> Option<(Color, u8, &'static str)> {
    // SPC report colours: tornado red, hail green, wind blue; flooding teal.
    Some(match kind {
        "TORNADO" => (hex("#FF2D2D"), 0, "Tornado"),
        "FUNNEL CLOUD" | "WALL CLOUD" => (hex("#FF8A8A"), 0, "Funnel / wall cloud"),
        "HAIL" => (hex("#2ED16B"), 1, "Hail"),
        "TSTM WND DMG" | "TSTM WND GST" | "NON-TSTM WND DMG" | "NON-TSTM WND GST" => (hex("#3D8BFF"), 2, "Wind"),
        "FLASH FLOOD" => (hex("#2FD3D3"), 3, "Flash flood"),
        _ => return None,
    })
}

fn marker(cv: &mut Canvas, x: f32, y: f32, r: f32, c: Color, shape: u8) {
    let edge = rgba(0, 0, 0, 0.85);
    match shape {
        0 => {
            let p = poly(&[(x, y - r * 1.2), (x + r * 1.1, y + r * 0.8), (x - r * 1.1, y + r * 0.8)], true);
            cv.fill_c(p.clone(), c);
            cv.stroke(p, edge, 1.6);
        }
        2 => {
            cv.fill_c(rrect(x - r * 0.85, y - r * 0.85, r * 1.7, r * 1.7, 2.), c);
            cv.stroke(rrect(x - r * 0.85, y - r * 0.85, r * 1.7, r * 1.7, 2.), edge, 1.6);
        }
        3 => {
            let p = poly(&[(x, y - r), (x + r, y), (x, y + r), (x - r, y)], true);
            cv.fill_c(p.clone(), c);
            cv.stroke(p, edge, 1.6);
        }
        _ => {
            cv.fill_c(ellipse(x, y, r, r), c);
            cv.stroke(ellipse(x, y, r, r), edge, 1.6);
        }
    }
}

fn reports(cv: &mut Canvas, h: &Hist, which: &str) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let big = which == "big";
    let map_w = if big { w - 40. } else { w - 420. };
    let Some(m) = StateMap::new(&h.state_fips, 20., 20., map_w, hh - if big { 90. } else { 40. }) else {
        cv.missing("County map");
        return;
    };
    m.base(cv, land(), county_line());
    if big {
        let mut hail: Vec<&data::Report> = h.reports.iter().filter(|r| r.kind == "HAIL" && r.mag.unwrap_or(0.) >= 1.75).collect();
        let mut wind: Vec<&data::Report> = h.reports.iter().filter(|r| r.kind == "TSTM WND GST" && r.mag.unwrap_or(0.) >= 65.).collect();
        hail.sort_by(|a, b| a.mag.partial_cmp(&b.mag).unwrap_or(std::cmp::Ordering::Equal));
        wind.sort_by(|a, b| a.mag.partial_cmp(&b.mag).unwrap_or(std::cmp::Ordering::Equal));
        for r in &wind {
            let (x, y) = m.to(r.lon, r.lat);
            marker(cv, x, y, 7. + (r.mag.unwrap_or(65.) as f32 - 65.) * 0.25, hex("#FFB340"), 2);
        }
        for r in &hail {
            let (x, y) = m.to(r.lon, r.lat);
            marker(cv, x, y, 6. + r.mag.unwrap_or(1.75) as f32 * 3.5, hex("#7FD1FF"), 1);
        }
        legend_marker(cv, 30., hh - 30., hex("#7FD1FF"), 1, &format!("Hail 1.75\" or larger ({})", hail.len()));
        legend_marker(cv, w / 2., hh - 30., hex("#FFB340"), 2, &format!("Gusts 65 mph or more ({})", wind.len()));
        return;
    }
    let Some((day, _)) = super::report_day(h) else {
        cv.text("No storm reports this year", map_w / 2., hh / 2., 34., Weight::SemiBold, Color::WHITE, Al::Center, map_w);
        return;
    };
    let list: Vec<&data::Report> = h.reports.iter().filter(|r| super::is_storm_report(&r.kind) && local_date(r.valid) == Some(day)).collect();
    let mut counts = [0usize; 4];
    // Draw wind, then hail, then floods, tornadoes on top.
    for order in [2u8, 1, 3, 0] {
        for r in &list {
            if let Some((c, shape, _)) = report_style(&r.kind) {
                if shape == order {
                    let (x, y) = m.to(r.lon, r.lat);
                    marker(cv, x, y, 9., c, shape);
                    counts[shape as usize] += 1;
                }
            }
        }
    }
    let lx = w - 390.;
    cv.round(lx, 20., 370., hh - 40., 18., &solid(rgba(255, 255, 255, 0.06)));
    cv.text("REPORTS", lx + 185., 70., 30., Weight::Bold, rgba(255, 211, 77, 1.), Al::Center, 0.);
    for (i, (shape, label)) in [(0u8, "Tornado"), (1, "Hail"), (2, "Wind"), (3, "Flash flood")].iter().enumerate() {
        let y = 140. + i as f32 * 90.;
        let c = match shape {
            0 => hex("#FF2D2D"),
            1 => hex("#2ED16B"),
            2 => hex("#3D8BFF"),
            _ => hex("#2FD3D3"),
        };
        marker(cv, lx + 50., y - 9., 15., c, *shape);
        cv.text(label, lx + 82., y, 30., Weight::SemiBold, Color::WHITE, Al::Left, 180.);
        cv.text(&counts[*shape as usize].to_string(), lx + 340., y, 36., Weight::Bold, Color::WHITE, Al::Right, 0.);
    }
    cv.text("Times are reports, not", lx + 185., hh - 90., 22., Weight::Regular, rgba(210, 220, 235, 0.85), Al::Center, 340.);
    cv.text("confirmed damage surveys", lx + 185., hh - 60., 22., Weight::Regular, rgba(210, 220, 235, 0.85), Al::Center, 340.);
}

fn legend_marker(cv: &mut Canvas, x: f32, y: f32, c: Color, shape: u8, label: &str) {
    marker(cv, x + 14., y - 9., 11., c, shape);
    cv.text(label, x + 38., y, 26., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
}

pub fn ef_color(ef: i32) -> Color {
    hex(match ef {
        0 => "#7FD8FF",
        1 => "#4FE070",
        2 => "#FFE14D",
        3 => "#FF9A2E",
        4 => "#FF3030",
        5 => "#E040FB",
        _ => "#B0B8C8",
    })
}

fn tracks(cv: &mut Canvas, h: &Hist) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let legend_w = 340.;
    let Some(m) = StateMap::new(&h.state_fips, 20., 20., w - legend_w - 40., hh - 40.) else {
        cv.missing("County map");
        return;
    };
    m.base(cv, hex("#1C2A40"), rgba(190, 210, 235, 0.22));
    if h.tornadoes.is_empty() {
        cv.text("Tornado database not downloaded yet", (w - legend_w) / 2., hh / 2., 32., Weight::SemiBold, Color::WHITE, Al::Center, w - legend_w - 80.);
        return;
    }
    let mut counts = [0usize; 7];
    for ef in [-9, 0, 1, 2, 3, 4, 5] {
        for t in h.tornadoes.iter().filter(|t| t.ef.max(-9) == ef || (ef == -9 && t.ef < 0)) {
            if ef == -9 && t.ef >= 0 {
                continue;
            }
            let a = m.to(t.start.1, t.start.0);
            let b = m.to(t.end.1, t.end.0);
            let width = 1.6 + t.ef.max(0) as f32 * 0.9;
            let c = ef_color(t.ef);
            if (a.0 - b.0).abs() + (a.1 - b.1).abs() < 1.5 {
                cv.fill_c(ellipse(a.0, a.1, width, width), c);
            } else {
                cv.line(a, b, c, width);
            }
            counts[if t.ef < 0 { 6 } else { t.ef as usize }] += 1;
        }
    }
    let lx = w - legend_w - 10.;
    cv.round(lx, 20., legend_w, hh - 40., 18., &solid(rgba(255, 255, 255, 0.06)));
    cv.text("RATING", lx + legend_w / 2., 70., 30., Weight::Bold, rgba(255, 211, 77, 1.), Al::Center, 0.);
    for ef in 0..6 {
        let y = 130. + ef as f32 * 78.;
        cv.line((lx + 30., y - 10.), (lx + 100., y - 10.), ef_color(ef), 3. + ef as f32 * 1.4);
        cv.text(&format!("(E)F{ef}"), lx + 120., y, 30., Weight::Bold, Color::WHITE, Al::Left, 0.);
        cv.text(&counts[ef as usize].to_string(), lx + legend_w - 30., y, 30., Weight::SemiBold, Color::WHITE, Al::Right, 0.);
    }
    if counts[6] > 0 {
        let y = 130. + 6. * 78.;
        cv.line((lx + 30., y - 10.), (lx + 100., y - 10.), ef_color(-1), 3.);
        cv.text("Unrated", lx + 120., y, 28., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
        cv.text(&counts[6].to_string(), lx + legend_w - 30., y, 30., Weight::SemiBold, Color::WHITE, Al::Right, 0.);
    }
    cv.text("F scale before 2007, EF after", lx + legend_w / 2., hh - 50., 22., Weight::Regular, rgba(210, 220, 235, 0.85), Al::Center, legend_w - 30.);
}

fn strongest(cv: &mut Canvas, h: &Hist, home: (f64, f64)) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let Some((t, _)) = super::strongest(h, home) else {
        cv.missing("Tornado database");
        return;
    };
    // Window around the track and home.
    let lats = [t.start.0, t.end.0, home.0];
    let lons = [t.start.1, t.end.1, home.1];
    let (mut s, mut n) = (lats.iter().copied().fold(f64::MAX, f64::min), lats.iter().copied().fold(f64::MIN, f64::max));
    let (mut west, mut east) = (lons.iter().copied().fold(f64::MAX, f64::min), lons.iter().copied().fold(f64::MIN, f64::max));
    let pad = 0.25;
    s -= pad;
    n += pad;
    west -= pad * 1.3;
    east += pad * 1.3;
    let k = ((s + n) / 2.).to_radians().cos();
    let (bw, bh) = (w as f64 - 40., hh as f64 - 40.);
    // Grow the window to the box's aspect.
    let (gw, gh) = ((east - west) * k, n - s);
    if gw / gh < bw / bh {
        let extra = (gh * bw / bh / k - (east - west)) / 2.;
        west -= extra;
        east += extra;
    } else {
        let extra = ((east - west) * k * bh / bw - gh) / 2.;
        s -= extra;
        n += extra;
    }
    let scale = bw / ((east - west) * k);
    let to = |lon: f64, lat: f64| ((20. + (lon - west) * k * scale) as f32, (20. + (n - lat) * scale) as f32);
    let Some(all) = crate::map::boundaries::counties_named() else {
        cv.missing("County map");
        return;
    };
    let path_of = |rings: &[Vec<(f64, f64)>]| {
        let mut pb = tiny_skia::PathBuilder::new();
        for ring in rings {
            for (i, &(lon, lat)) in ring.iter().enumerate() {
                let (x, y) = to(lon, lat);
                if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
            }
            pb.close();
        }
        pb.finish()
    };
    let near: Vec<_> = all.iter().filter(|c| c.rings.iter().flatten().any(|&(lon, lat)| lon > west - 0.3 && lon < east + 0.3 && lat > s - 0.3 && lat < n + 0.3)).collect();
    for c in &near {
        let home_c = c.rings.iter().fold(false, |v, r| v ^ crate::gfx::winter_map::inside_pub(r, home.1, home.0));
        cv.fill_c(path_of(&c.rings), if home_c { hex("#2E4A70") } else { land() });
    }
    for c in &near {
        cv.stroke(path_of(&c.rings), rgba(190, 210, 235, 0.4), 1.6);
    }
    // County names, small, for orientation (kept clear of the start, end and home marks).
    let marks = [to(t.start.1, t.start.0), to(t.end.1, t.end.0), to(home.1, home.0)];
    for c in &near {
        let (lon, lat) = StateMap::centre(&c.rings);
        let (x, y) = to(lon, lat);
        let clear = marks.iter().all(|m| (m.0 - x).abs() > 90. || (m.1 - y).abs() > 60.);
        if clear && x > 60. && x < w - 60. && y > 40. && y < hh - 30. {
            cv.text_halo(&c.name, x, y, 20., Weight::SemiBold, rgba(220, 230, 245, 0.85), Al::Center);
        }
    }
    let a = to(t.start.1, t.start.0);
    let b = to(t.end.1, t.end.0);
    let c = ef_color(t.ef);
    cv.line(a, b, rgba(0, 0, 0, 0.7), 18.);
    cv.line(a, b, c, 11.);
    cv.fill_c(ellipse(a.0, a.1, 12., 12.), Color::WHITE);
    cv.fill_c(ellipse(b.0, b.1, 12., 12.), c);
    cv.text_halo("Start", a.0, a.1 - 22., 24., Weight::Bold, Color::WHITE, Al::Center);
    cv.text_halo("End", b.0, b.1 - 22., 24., Weight::Bold, Color::WHITE, Al::Center);
    let hp = to(home.1, home.0);
    let star: Vec<(f32, f32)> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { 16. } else { 7. };
            let ang = std::f32::consts::PI * (i as f32 / 5. - 0.5);
            (hp.0 + r * ang.cos(), hp.1 + r * ang.sin())
        })
        .collect();
    cv.fill_c(poly(&star, true), hex("#FFD34D"));
    cv.stroke(poly(&star, true), rgba(0, 0, 0, 0.8), 1.6);
    cv.text_halo("Home", hp.0, hp.1 + 40., 24., Weight::Bold, hex("#FFD34D"), Al::Center);
}

fn calendar(cv: &mut Canvas, h: &Hist) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let mut by: std::collections::HashMap<NaiveDate, u32> = Default::default();
    for wn in &h.warns {
        if let Some(d) = local_date(wn.issue) {
            *by.entry(d).or_default() += 1;
        }
    }
    let max = by.values().copied().max().unwrap_or(1).max(1);
    let today = chrono::Local::now().date_naive();
    let (gx, gy) = (150., 70.);
    let cw = (w - gx - 40.) / 31.;
    let ch = (hh - gy - 90.) / 12.;
    for d in 1..=31 {
        if d == 1 || d % 5 == 0 {
            cv.text(&d.to_string(), gx + cw * (d as f32 - 0.5), gy - 16., 22., Weight::SemiBold, rgba(220, 230, 245, 0.85), Al::Center, 0.);
        }
    }
    for m in 0..12u32 {
        let y = gy + ch * m as f32;
        cv.text(MONTHS[m as usize], gx - 20., y + ch * 0.66, 28., Weight::Bold, Color::WHITE, Al::Right, 0.);
        for d in 1..=31u32 {
            let Some(date) = NaiveDate::from_ymd_opt(h.year, m + 1, d) else { continue };
            let x = gx + cw * (d - 1) as f32;
            let n = by.get(&date).copied().unwrap_or(0);
            let future = date > today;
            let c = if future {
                rgba(255, 255, 255, 0.03)
            } else if n == 0 {
                rgba(255, 255, 255, 0.09)
            } else {
                ramp(((n as f32).ln_1p() / (max as f32).ln_1p()).max(0.08))
            };
            cv.round(x + 2., y + 2., cw - 4., ch - 4., 5., &solid(c));
            if n > 0 && cw > 34. {
                cv.text(&n.to_string(), x + cw / 2., y + ch * 0.66, (ch * 0.42).min(24.), Weight::Bold, if n as f32 > max as f32 * 0.4 { Color::WHITE } else { rgba(20, 20, 30, 1.) }, Al::Center, cw - 6.);
            }
            if date == today {
                cv.stroke(rrect(x + 1., y + 1., cw - 2., ch - 2., 6.), Color::WHITE, 3.);
            }
        }
    }
    let ly = hh - 34.;
    cv.text("Fewer", gx, ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    for i in 0..8 {
        cv.round(gx + 80. + i as f32 * 44., ly - 22., 40., 26., 4., &solid(ramp(0.08 + i as f32 / 7. * 0.92)));
    }
    cv.text(&format!("More (busiest day: {max})"), gx + 80. + 8. * 44. + 10., ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    let total = h.warns.len();
    let days = by.len();
    cv.text(&format!("{total} warnings on {days} days"), w - 40., ly, 26., Weight::Bold, hex("#FFD34D"), Al::Right, 0.);
}

fn freeze(cv: &mut Canvas, h: &Hist) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let fz = super::freezes(h);
    if fz.is_empty() {
        cv.missing("Station climate");
        return;
    }
    let (gx, gy) = (110., 70.);
    let gw = w - gx - 40.;
    let rows = fz.len().max(1) as f32;
    let rh = (hh - gy - 70.) / rows;
    let xof = |doy: f32| gx + gw * (doy - 1.) / 365.;
    for (m, name) in MONTHS.iter().enumerate() {
        let doy = NaiveDate::from_ymd_opt(2001, m as u32 + 1, 1).map(|d| d.ordinal()).unwrap_or(1) as f32;
        let x = xof(doy);
        cv.rect(x, gy - 10., 1.5, rows * rh + 10., rgba(255, 255, 255, 0.10));
        cv.text(name, x + gw / 24., gy - 22., 22., Weight::SemiBold, rgba(220, 230, 245, 0.85), Al::Center, 0.);
    }
    for (i, (y, last, first)) in fz.iter().enumerate() {
        let ry = gy + rh * i as f32;
        cv.text(&y.to_string(), gx - 14., ry + rh * 0.72, (rh * 0.62).min(24.), Weight::SemiBold, if *y == h.year { hex("#FFD34D") } else { Color::WHITE }, Al::Right, 0.);
        let x0 = last.map(|d| xof(d.ordinal() as f32)).unwrap_or(xof(1.));
        let x1 = first.map(|d| xof(d.ordinal() as f32)).unwrap_or_else(|| if *y == h.year { xof(chrono::Local::now().ordinal() as f32) } else { xof(365.) });
        cv.round(x0, ry + rh * 0.22, (x1 - x0).max(2.), rh * 0.56, rh * 0.28, &linear(0., ry, 0., ry + rh, &[(0., hex("#5BE08A")), (1., hex("#1E9A4E"))]));
        for d in [last, first].into_iter().flatten() {
            let x = xof(d.ordinal() as f32);
            cv.fill_c(ellipse(x, ry + rh * 0.5, rh * 0.32, rh * 0.32), hex("#7FD1FF"));
            cv.stroke(ellipse(x, ry + rh * 0.5, rh * 0.32, rh * 0.32), rgba(0, 0, 0, 0.7), 1.4);
        }
    }
    let ly = hh - 26.;
    cv.round(gx, ly - 20., 40., 20., 10., &solid(hex("#3BC46E")));
    cv.text("Growing season (no freeze)", gx + 52., ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
    cv.fill_c(ellipse(gx + 470., ly - 9., 10., 10.), hex("#7FD1FF"));
    cv.text("Last / first 32°F or colder", gx + 490., ly, 24., Weight::SemiBold, Color::WHITE, Al::Left, 0.);
}

fn wettest(cv: &mut Canvas, h: &Hist) {
    let (w, hh) = (cv.w, cv.h);
    backdrop(cv, 0., 0., w, hh);
    let mut months = super::month_rain(h);
    if months.is_empty() {
        cv.missing("Station climate");
        return;
    }
    months.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    months.truncate(10);
    let max = months[0].2.max(0.1);
    let (lx, top) = (330., 50.);
    let bar_w = w - lx - 220.;
    let rh = (hh - top - 40.) / 10.;
    for (i, (y, m, p)) in months.iter().enumerate() {
        let ry = top + rh * i as f32;
        cv.text(&format!("{}", i + 1), 50., ry + rh * 0.66, 34., Weight::Bold, hex("#FFD34D"), Al::Center, 0.);
        let name = NaiveDate::from_ymd_opt(*y, *m, 1).map(|d| d.format("%B %Y").to_string()).unwrap_or_default();
        cv.text(&name, lx - 24., ry + rh * 0.66, 32., Weight::SemiBold, Color::WHITE, Al::Right, 240.);
        let bw = bar_w * p / max;
        // TWC rain-total colour for the amount.
        let c = rain_color(*p);
        cv.round(lx, ry + rh * 0.16, bw, rh * 0.68, 10., &linear(0., ry, 0., ry + rh, &[(0., mix(c, Color::WHITE, 0.15)), (1., mix(c, Color::BLACK, 0.25))]));
        cv.text(&format!("{p:.2} in"), lx + bw + 18., ry + rh * 0.66, 32., Weight::Bold, Color::WHITE, Al::Left, 0.);
    }
}

fn rain_color(inches: f32) -> Color {
    let stops = crate::grid::RAIN_TOTAL_STOPS;
    let mut c = stops.first().map(|s| s.1).unwrap_or([60, 160, 255]);
    for s in stops.iter() {
        if inches as f64 >= s.0 as f64 {
            c = s.1;
        }
    }
    Color::from_rgba8(c[0], c[1], c[2], 255)
}
