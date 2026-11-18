//! Earthquake graphics (USGS): the latest quake near home with a distance line, the region's
//! quakes over 30 days, the U.S. over 7 days, the week's biggest quake anywhere, and
//! Drop, Cover, Hold On. Maps are drawn here from a JSON payload binding
//! (`set:quakes:{{set.quake.<scope>.payload}}`), like the tornado maps.
use crate::config::Settings;
use crate::data::WxSnapshot;
use crate::gfx::{
    bind::Bindings,
    model::{AnimKind, GraphicDoc, Layer},
    templates::{panel, DISPLAY},
    text::{draw_line, line_metrics, measure, Align, GlyphCache, TextStyle, Weight},
};
use serde::{Deserialize, Serialize};
use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, StrokeDash, Transform};

const SOURCE: &str = "Source: USGS Earthquake Hazards Program";
/// "Near home" for the latest-quake card, miles.
const NEAR_MILES: f64 = 500.0;
/// Region map radius, degrees of latitude either side of home.
const REGION_DEG: f64 = 5.0;

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub(super) struct Mark {
    pub id: String,
    pub lat: f64,
    pub lon: f64,
    pub mag: f32,
    /// Seconds since the quake.
    pub age: i64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Payload {
    /// Map bounds (west, south, east, north).
    pub bbox: [f64; 4],
    #[serde(default)]
    pub home: Option<(f64, f64)>,
    #[serde(default)]
    pub home_label: String,
    pub quakes: Vec<Mark>,
    /// The quake to highlight (rings + label) and, with `line`, connect to home.
    #[serde(default)]
    pub focus: Option<String>,
    #[serde(default)]
    pub line: bool,
    /// Shown when the list is empty ("No earthquakes M2.5+ this week").
    #[serde(default)]
    pub empty: String,
}

pub(super) fn parse(raw: &str) -> Option<Payload> {
    let p: Payload = serde_json::from_str(raw).ok()?;
    let [w, s, e, n] = p.bbox;
    if !(w.is_finite() && s.is_finite() && e.is_finite() && n.is_finite()) || e <= w || n <= s || p.quakes.len() > 5000 {
        return None;
    }
    if p.quakes.iter().any(|q| !q.lat.is_finite() || !q.lon.is_finite() || !q.mag.is_finite()) {
        return None;
    }
    Some(p)
}

// ---- Templates --------------------------------------------------------------------------------

struct Spec {
    key: &'static str,
    name: &'static str,
    description: &'static str,
}

const SPECS: &[Spec] = &[
    Spec { key: "set_quake_latest_near", name: "Latest Earthquake Near Us", description: "USGS: the newest earthquake within 500 miles (30 days) on a map with its distance from home, size, depth and what people felt." },
    Spec { key: "set_quake_region_month", name: "Earthquakes in Our Region: Past 30 Days", description: "USGS: every located earthquake around home over 30 days, sized by magnitude and coloured by age, with the count and largest." },
    Spec { key: "set_quake_us_week", name: "U.S. Earthquakes: Past 7 Days", description: "USGS: magnitude 2.5+ earthquakes across the lower 48 this week, with the count, largest and most recent." },
    Spec { key: "set_quake_biggest_week", name: "Biggest Earthquake This Week", description: "USGS: the largest earthquake anywhere in the past 7 days: magnitude, place, depth, tsunami and impact alerts." },
    Spec { key: "set_quake_safety", name: "Earthquake Safety: Drop, Cover, Hold On", description: "What to do when the ground shakes (USGS / FEMA Ready.gov). No live data." },
];

pub(super) fn infos() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    SPECS.iter().map(|s| (s.key, s.name, s.description))
}

fn label(d: &mut GraphicDoc, name: &str, box_: (f32, f32, f32, f32), value: &str, size: f32, color: &str, align: Align) {
    super::reference::label(d, name, box_, value, size, color, align);
}

fn map(d: &mut GraphicDoc, box_: (f32, f32, f32, f32), scope: &str) {
    d.layers.push(panel("Map panel", box_.0, box_.1, box_.2, box_.3).with_anim(AnimKind::Fade, 0.15, 0.5));
    d.layers.push(
        Layer::risk_map("Earthquake map", box_.0 + 14., box_.1 + 14., box_.2 - 28., box_.3 - 28., &format!("set:quakes:{{{{set.quake.{scope}.payload}}}}"))
            .with_anim(AnimKind::Fade, 0.3, 0.6),
    );
}

/// Right-hand fact card: a small heading, a big value, then lines.
fn card(d: &mut GraphicDoc, scope: &str, heading: &str, big: &str, lines: &[(&str, f32, &str)]) {
    let (x, y, w) = (1210., 200., 650.);
    d.layers.push(panel("Fact card", x, y, w, 740.).with_anim(AnimKind::Fade, 0.2, 0.5));
    label(d, "Card heading", (x + 34., y + 26., w - 68., 40.), heading, 26., "$muted", Align::Left);
    label(d, "Card value", (x + 34., y + 70., w - 68., 150.), &format!("{{{{set.quake.{scope}.{big}}}}}"), 120., "$text", Align::Left);
    let mut ly = y + 236.;
    for (i, (field, size, color)) in lines.iter().enumerate() {
        let h = size * 2.5;
        label(d, &format!("Card line {}", i + 1), (x + 34., ly, w - 68., h), &format!("{{{{set.quake.{scope}.{field}}}}}"), *size, color, Align::Left);
        ly += h + 10.;
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = SPECS.iter().find(|s| s.key == key)?;
    let frame = |subtitle: &str| super::reference::frame(spec.name, "Earthquakes", subtitle, "{{set.quake.source}}");
    let mut d = match key {
        "set_quake_latest_near" => {
            let mut d = frame("{{set.quake.near.period}}");
            map(&mut d, (60., 200., 1120., 740.), "near");
            card(&mut d, "near", "MAGNITUDE", "mag", &[
                ("class", 40., "$text"),
                ("place", 30., "$text"),
                ("when", 28., "$muted"),
                ("distance", 28., "$text"),
                ("depth", 26., "$muted"),
                ("felt", 26., "$muted"),
            ]);
            d
        }
        "set_quake_region_month" => {
            let mut d = frame("{{set.quake.region.period}}");
            map(&mut d, (60., 200., 1120., 740.), "region");
            card(&mut d, "region", "EARTHQUAKES", "count", &[
                ("count_note", 30., "$muted"),
                ("largest", 30., "$text"),
                ("latest", 28., "$text"),
                ("closest", 28., "$muted"),
            ]);
            d
        }
        "set_quake_us_week" => {
            let mut d = frame("{{set.quake.us.period}}");
            map(&mut d, (60., 200., 1120., 740.), "us");
            card(&mut d, "us", "EARTHQUAKES M2.5+", "count", &[
                ("count_note", 30., "$muted"),
                ("largest", 30., "$text"),
                ("latest", 28., "$text"),
                ("near", 28., "$muted"),
            ]);
            d
        }
        "set_quake_biggest_week" => {
            let mut d = frame("{{set.quake.big.period}}");
            d.layers.push(panel("Big panel", 60., 200., 1800., 740.).with_anim(AnimKind::Fade, 0.15, 0.5));
            label(&mut d, "Magnitude heading", (110., 240., 560., 44.), "MAGNITUDE", 30., "$muted", Align::Left);
            label(&mut d, "Magnitude", (110., 290., 560., 230.), "{{set.quake.big.mag}}", 200., "$text", Align::Left);
            label(&mut d, "Class", (110., 540., 560., 70.), "{{set.quake.big.class}}", 52., "$text", Align::Left);
            label(&mut d, "Place", (720., 250., 1090., 130.), "{{set.quake.big.place}}", 54., "$text", Align::Left);
            label(&mut d, "When", (720., 400., 1090., 60.), "{{set.quake.big.when}}", 34., "$muted", Align::Left);
            label(&mut d, "Depth", (720., 475., 1090., 60.), "{{set.quake.big.depth}}", 34., "$muted", Align::Left);
            label(&mut d, "Impact", (720., 560., 1090., 120.), "{{set.quake.big.impact}}", 34., "$text", Align::Left);
            label(&mut d, "Alerts", (720., 700., 1090., 120.), "{{set.quake.big.alerts}}", 32., "$text", Align::Left);
            d
        }
        "set_quake_safety" => {
            let mut d = super::reference::frame(spec.name, "Earthquakes", "When the ground starts shaking", "Source: USGS · FEMA Ready.gov · Great ShakeOut");
            let steps = [
                ("DROP", "Get down on your hands and knees before the shaking knocks you down."),
                ("COVER", "Cover your head and neck under a sturdy table or desk, or next to an interior wall away from windows."),
                ("HOLD ON", "Hold on to your shelter until the shaking stops. Expect aftershocks."),
            ];
            for (i, (word, text)) in steps.iter().enumerate() {
                let x = 60. + i as f32 * 610.;
                d.layers.push(panel(&format!("Step {} panel", i + 1), x, 210., 580., 560.).with_anim(AnimKind::Rise, 0.15 + i as f32 * 0.2, 0.5));
                label(&mut d, &format!("Step {} number", i + 1), (x + 40., 240., 500., 60.), &format!("STEP {}", i + 1), 30., "$muted", Align::Left);
                label(&mut d, &format!("Step {} word", i + 1), (x + 40., 300., 500., 120.), word, 92., "$text", Align::Left);
                label(&mut d, &format!("Step {} text", i + 1), (x + 40., 450., 500., 280.), text, 36., "$text", Align::Left);
            }
            label(&mut d, "Do not", (60., 800., 1800., 60.), "Don't run outside or stand in a doorway during shaking. Outdoors: move away from buildings, trees and power lines, then drop.", 30., "$muted", Align::Center);
            d.layers.push(panel("Tip strip", 60., 880., 1800., 70.).with_anim(AnimKind::Fade, 0.8, 0.5));
            label(&mut d, "Tip", (90., 885., 1740., 60.), "In bed? Stay there and cover your head with a pillow. Driving? Pull over, stop and stay buckled.", 28., "$text", Align::Center);
            d
        }
        _ => return None,
    };
    d.template = Some(key.to_owned());
    Some(d)
}

// ---- Bindings ---------------------------------------------------------------------------------

fn compass(from: (f64, f64), to: (f64, f64)) -> &'static str {
    let (la1, la2) = (from.0.to_radians(), to.0.to_radians());
    let dl = (to.1 - from.1).to_radians();
    let y = dl.sin() * la2.cos();
    let x = la1.cos() * la2.sin() - la1.sin() * la2.cos() * dl.cos();
    let deg = (y.atan2(x).to_degrees() + 360.) % 360.;
    const DIRS: [&str; 16] = ["north", "north-northeast", "northeast", "east-northeast", "east", "east-southeast", "southeast", "south-southeast", "south", "south-southwest", "southwest", "west-southwest", "west", "west-northwest", "northwest", "north-northwest"];
    DIRS[((deg + 11.25) / 22.5) as usize % 16]
}

/// Bounds of `half_lat` degrees either side of a centre, widened for the map's 1.48:1 shape.
fn bounds_around(lat: f64, lon: f64, half_lat: f64) -> [f64; 4] {
    let half_lon = half_lat * 1.48 / lat.to_radians().cos().max(0.2);
    [lon - half_lon, lat - half_lat, lon + half_lon, lat + half_lat]
}

fn local_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0)
        .map(|t| t.with_timezone(&chrono::Local).format("%a %b %-d, %-I:%M %p").to_string())
        .unwrap_or_default()
}

fn mark(q: &crate::quakes::Quake, now: i64) -> Mark {
    Mark { id: q.id.clone(), lat: q.lat, lon: q.lon, mag: q.mag, age: q.age_secs(now) }
}

fn json(p: &Payload) -> String {
    serde_json::to_string(p).unwrap_or_default()
}

pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let home = settings.home;
    let home_label = settings.station_name.clone();
    let now = chrono::Utc::now().timestamp();
    let quakes = &data.quakes;
    b.set("set.quake.source", SOURCE);
    if quakes.is_empty() {
        // No "--" placeholders on air: one clear line, everything else blank.
        let fields = ["mag", "count", "class", "count_note", "place", "when", "distance", "depth", "felt", "largest", "latest", "closest", "near", "impact", "alerts"];
        for scope in ["near", "region", "us", "big"] {
            for field in fields {
                b.set(&format!("set.quake.{scope}.{field}"), "");
            }
            b.set(&format!("set.quake.{scope}.period"), "Waiting for USGS earthquake data");
            b.set(&format!("set.quake.{scope}.class"), super::reference::MISSING);
            b.set(&format!("set.quake.{scope}.count_note"), super::reference::MISSING);
        }
        return;
    }
    let short = |q: &crate::quakes::Quake| format!("M{:.1} {}", q.mag, q.place);

    // Latest near home.
    let near = crate::quakes::select(quakes, now, 30, 0.0, Some((home.lat, home.lon, NEAR_MILES)));
    b.set("set.quake.near.period", &format!("Within {NEAR_MILES:.0} miles of {} · past 30 days", if home_label.is_empty() { "home" } else { home_label.as_str() }));
    match near.first() {
        Some(q) => {
            let miles = q.miles_from(home.lat, home.lon);
            b.set("set.quake.near.mag", &format!("{:.1}", q.mag));
            b.set("set.quake.near.class", &format!("{} earthquake", crate::quakes::class(q.mag)));
            b.set("set.quake.near.place", &q.place);
            b.set("set.quake.near.when", &format!("{} ({})", local_time(q.time), crate::quakes::age_label(q.age_secs(now))));
            b.set("set.quake.near.distance", &format!("{miles:.0} miles {} of {}", compass((home.lat, home.lon), (q.lat, q.lon)), if home_label.is_empty() { "us" } else { home_label.as_str() }));
            b.set("set.quake.near.depth", &format!("{:.0} km ({:.0} mi) deep", q.depth_km, q.depth_km / 1.609));
            b.set("set.quake.near.felt", &match q.felt {
                Some(n) if n > 0 => format!("{n} people reported feeling it · {}", crate::quakes::felt_words(q.mag)),
                _ => crate::quakes::felt_words(q.mag).to_string(),
            });
            // Frame home and the quake with room around both (about 1° each side at least).
            let span = (q.lat - home.lat).abs().max((q.lon - home.lon).abs() * home.lat.to_radians().cos() / 1.48);
            let half = (span * 0.75 + 0.7).clamp(1.0, 9.0);
            let (clat, clon) = ((q.lat + home.lat) / 2., (q.lon + home.lon) / 2.);
            let marks = near.iter().map(|x| mark(x, now)).collect();
            b.set("set.quake.near.payload", &json(&Payload { bbox: bounds_around(clat, clon, half), home: Some((home.lat, home.lon)), home_label: home_label.clone(), quakes: marks, focus: Some(q.id.clone()), line: true, empty: String::new() }));
        }
        None => {
            b.set("set.quake.near.mag", "None");
            b.set("set.quake.near.class", &format!("No earthquakes within {NEAR_MILES:.0} miles in 30 days"));
            b.set("set.quake.near.place", "");
            b.set("set.quake.near.payload", &json(&Payload { bbox: bounds_around(home.lat, home.lon, REGION_DEG), home: Some((home.lat, home.lon)), home_label: home_label.clone(), quakes: Vec::new(), focus: None, line: false, empty: "No earthquakes in the past 30 days".into() }));
        }
    }

    // Region, 30 days, all sizes.
    let bbox = bounds_around(home.lat, home.lon, REGION_DEG);
    let in_box = |q: &&crate::quakes::Quake| q.lon >= bbox[0] && q.lon <= bbox[2] && q.lat >= bbox[1] && q.lat <= bbox[3];
    let region: Vec<&crate::quakes::Quake> = crate::quakes::select(quakes, now, 30, 0.0, None).into_iter().filter(in_box).collect();
    b.set("set.quake.region.period", "Every located earthquake · past 30 days");
    b.set("set.quake.region.count", &region.len().to_string());
    b.set("set.quake.region.count_note", if region.len() == 1 { "earthquake on this map in 30 days" } else { "earthquakes on this map in 30 days" });
    let largest = region.iter().max_by(|a, b| a.mag.total_cmp(&b.mag));
    b.set("set.quake.region.largest", &largest.map(|q| format!("Largest: {}", short(q))).unwrap_or_else(|| "None in 30 days".into()));
    b.set("set.quake.region.latest", &region.first().map(|q| format!("Latest: M{:.1}, {}", q.mag, crate::quakes::age_label(q.age_secs(now)))).unwrap_or_default());
    let closest = region.iter().min_by(|a, b| a.miles_from(home.lat, home.lon).total_cmp(&b.miles_from(home.lat, home.lon)));
    b.set("set.quake.region.closest", &closest.map(|q| format!("Closest: M{:.1}, {:.0} miles away", q.mag, q.miles_from(home.lat, home.lon))).unwrap_or_default());
    b.set("set.quake.region.payload", &json(&Payload { bbox, home: Some((home.lat, home.lon)), home_label: home_label.clone(), quakes: region.iter().map(|q| mark(q, now)).collect(), focus: largest.map(|q| q.id.clone()), line: false, empty: "No earthquakes in the past 30 days".into() }));

    // Lower 48, 7 days, M2.5+.
    let us_box = [-125.5, 23.5, -66.0, 50.0];
    let us: Vec<&crate::quakes::Quake> = crate::quakes::select(quakes, now, 7, 2.5, None).into_iter()
        .filter(|q| q.lon >= us_box[0] && q.lon <= us_box[2] && q.lat >= us_box[1] && q.lat <= us_box[3]).collect();
    b.set("set.quake.us.period", "Lower 48 · magnitude 2.5 and larger · past 7 days");
    b.set("set.quake.us.count", &us.len().to_string());
    b.set("set.quake.us.count_note", "earthquakes this week");
    let us_largest = us.iter().max_by(|a, b| a.mag.total_cmp(&b.mag));
    b.set("set.quake.us.largest", &us_largest.map(|q| format!("Largest: {}", short(q))).unwrap_or_else(|| "None this week".into()));
    b.set("set.quake.us.latest", &us.first().map(|q| format!("Latest: M{:.1}, {}", q.mag, crate::quakes::age_label(q.age_secs(now)))).unwrap_or_default());
    let near_count = us.iter().filter(|q| q.miles_from(home.lat, home.lon) <= NEAR_MILES).count();
    b.set("set.quake.us.near", &format!("{near_count} within {NEAR_MILES:.0} miles of us"));
    b.set("set.quake.us.payload", &json(&Payload { bbox: us_box, home: Some((home.lat, home.lon)), home_label: home_label.clone(), quakes: us.iter().map(|q| mark(q, now)).collect(), focus: us_largest.map(|q| q.id.clone()), line: false, empty: "No magnitude 2.5+ earthquakes this week".into() }));

    // Biggest anywhere, 7 days.
    let week = crate::quakes::select(quakes, now, 7, 0.0, None);
    b.set("set.quake.big.period", "Anywhere in the world · past 7 days");
    if let Some(q) = week.iter().max_by(|a, b| a.mag.total_cmp(&b.mag)) {
        b.set("set.quake.big.mag", &format!("{:.1}", q.mag));
        b.set("set.quake.big.class", &format!("{} earthquake", crate::quakes::class(q.mag)));
        b.set("set.quake.big.place", &q.place);
        b.set("set.quake.big.when", &format!("{} our time ({})", local_time(q.time), crate::quakes::age_label(q.age_secs(now))));
        b.set("set.quake.big.depth", &format!("{:.0} km ({:.0} mi) deep · {:.0} miles from us", q.depth_km, q.depth_km / 1.609, q.miles_from(home.lat, home.lon)));
        b.set("set.quake.big.impact", crate::quakes::felt_words(q.mag));
        let mut alerts = Vec::new();
        if q.tsunami {
            alerts.push("Tsunami information issued: check NOAA's Tsunami Warning Centers".to_string());
        }
        match q.alert.as_str() {
            "yellow" => alerts.push("USGS PAGER: yellow alert (some casualties or damage possible)".into()),
            "orange" => alerts.push("USGS PAGER: orange alert (significant casualties or damage likely)".into()),
            "red" => alerts.push("USGS PAGER: red alert (high casualties and extensive damage probable)".into()),
            _ => {}
        }
        b.set("set.quake.big.alerts", &alerts.join(" · "));
    } else {
        b.set("set.quake.big.mag", "None");
        b.set("set.quake.big.class", "No earthquakes recorded this week");
    }
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    if !doc.template.as_deref().is_some_and(|k| k.starts_with("set_quake_")) {
        return;
    }
    let mut put = |key: &str, value: &str| doc.data_overrides.push((key.into(), value.into()));
    put("set.quake.source", "SAMPLE ONLY • Not for broadcast");
    let marks = vec![
        Mark { id: "s1".into(), lat: 38.45, lon: -87.75, mag: 3.4, age: 3_000 },
        Mark { id: "s2".into(), lat: 36.55, lon: -89.55, mag: 2.6, age: 90_000 },
        Mark { id: "s3".into(), lat: 36.2, lon: -89.4, mag: 1.9, age: 400_000 },
        Mark { id: "s4".into(), lat: 37.8, lon: -88.2, mag: 4.6, age: 1_500_000 },
    ];
    let payload = |bbox: [f64; 4], focus: &str, line: bool| json(&Payload { bbox, home: Some((38.39, -86.93)), home_label: "SAMPLE".into(), quakes: marks.clone(), focus: Some(focus.into()), line, empty: String::new() });
    for (scope, bbox, focus, line) in [("near", bounds_around(38.42, -87.34, 1.2), "s1", true), ("region", bounds_around(38.39, -86.93, REGION_DEG), "s4", false), ("us", [-125.5, 23.5, -66.0, 50.0], "s4", false)] {
        put(&format!("set.quake.{scope}.payload"), &payload(bbox, focus, line));
        put(&format!("set.quake.{scope}.period"), "SAMPLE period");
    }
    for (k, v) in [
        ("near.mag", "3.4"), ("near.class", "Minor earthquake"), ("near.place", "SAMPLE • 5 km NE of a sample town"),
        ("near.when", "SAMPLE • Mon Nov 16, 2:10 PM (50 min ago)"), ("near.distance", "45 miles west of SAMPLE"),
        ("near.depth", "11 km (7 mi) deep"), ("near.felt", "SAMPLE • 120 people reported feeling it"),
        ("region.count", "4"), ("region.count_note", "earthquakes on this map in 30 days"), ("region.largest", "Largest: M4.6 SAMPLE"),
        ("region.latest", "Latest: M3.4, 50 min ago"), ("region.closest", "Closest: M3.4, 45 miles away"),
        ("us.count", "4"), ("us.count_note", "earthquakes this week"), ("us.largest", "Largest: M4.6 SAMPLE"),
        ("us.latest", "Latest: M3.4, 50 min ago"), ("us.near", "2 within 500 miles of us"),
        ("big.period", "SAMPLE period"), ("big.mag", "6.8"), ("big.class", "Strong earthquake"), ("big.place", "SAMPLE • 80 km S of a sample coast"),
        ("big.when", "SAMPLE • Sun Nov 15, 9:41 PM our time"), ("big.depth", "35 km (22 mi) deep · 4,100 miles from us"),
        ("big.impact", "Damage likely near the center"), ("big.alerts", "SAMPLE • USGS PAGER: yellow alert"),
    ] {
        put(&format!("set.quake.{k}"), v);
    }
}

// ---- Map drawing ------------------------------------------------------------------------------

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

fn rgb((r, g, b): (u8, u8, u8), a: u8) -> Color {
    Color::from_rgba8(r, g, b, a)
}

fn text_style(font: &str, size: f32, weight: Weight) -> TextStyle {
    TextStyle { family: font.into(), weight, size, color: Color::WHITE, letter_spacing: 0. }
}

/// Text on a dark plate, kept inside the map; returns the plate box.
fn plate(pm: &mut Pixmap, g: &mut GlyphCache, text: &str, x: f32, y: f32, st: &TextStyle, s: f32) -> (f32, f32, f32, f32) {
    let (asc, desc) = line_metrics(st);
    let (tw, th) = (measure(text, st) + 16. * s, asc + desc + 10. * s);
    let bx = x.clamp(4. * s, (pm.width() as f32 - tw - 4. * s).max(4. * s));
    let by = y.clamp(4. * s, (pm.height() as f32 - th - 4. * s).max(4. * s));
    if let Some(r) = Rect::from_xywh(bx, by, tw, th) {
        pm.fill_rect(r, &paint(Color::from_rgba8(8, 22, 38, 225)), Transform::identity(), None);
    }
    draw_line(pm, g, text, bx + 8. * s, by + 5. * s + asc, st, 1., None);
    (bx, by, tw, th)
}

/// Draw a plate at the first candidate top-left that clears `occupied` (else the first one).
#[allow(clippy::too_many_arguments)]
fn place_plate(pm: &mut Pixmap, g: &mut GlyphCache, text: &str, candidates: &[(f32, f32)], st: &TextStyle, s: f32, occupied: &mut Vec<(f32, f32, f32, f32)>) {
    let (asc, desc) = line_metrics(st);
    let (tw, th) = (measure(text, st) + 16. * s, asc + desc + 10. * s);
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let fits = |&(x, y): &(f32, f32)| {
        let (x, y) = (x.clamp(4. * s, (w - tw - 4. * s).max(4. * s)), y.clamp(4. * s, (h - th - 4. * s).max(4. * s)));
        !occupied.iter().any(|o| x < o.0 + o.2 + 4. * s && x + tw + 4. * s > o.0 && y < o.1 + o.3 + 4. * s && y + th + 4. * s > o.1)
    };
    let at = candidates.iter().copied().find(fits).or_else(|| candidates.first().copied()).unwrap_or((0., 0.));
    let placed = plate(pm, g, text, at.0, at.1, st, s);
    occupied.push(placed);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(pm: &mut Pixmap, glyphs: &mut GlyphCache, local: Transform, mask: Option<&Mask>, w: f32, h: f32, spec: &str, opacity: f32, font: &str) -> bool {
    let Some(raw) = spec.strip_prefix("set:quakes:") else { return false };
    let dw = (w * local.sx.abs()).round().clamp(1., 4096.) as u32;
    let dh = (h * local.sy.abs()).round().clamp(1., 4096.) as u32;
    let Some(mut off) = Pixmap::new(dw, dh) else { return true };
    let s = dw as f32 / w.max(1.);
    // Water first: the whole map box.
    off.fill(Color::from_rgba8(9, 24, 41, 255));
    let Some(p) = parse(raw) else {
        let st = text_style(font, 30. * s, Weight::SemiBold);
        let t = super::reference::MISSING;
        draw_line(&mut off, glyphs, t, (dw as f32 - measure(t, &st)) / 2., dh as f32 / 2., &st, 1., None);
        composite(pm, &off, local, mask, opacity);
        return true;
    };
    let [west, south, east, north] = p.bbox;
    let k = ((south + north) / 2.).to_radians().cos();
    let (gw, gh) = ((east - west) * k, north - south);
    let sc = (dw as f64 / gw).min(dh as f64 / gh);
    let (ox, oy) = ((dw as f64 - gw * sc) / 2., (dh as f64 - gh * sc) / 2.);
    let to = |lon: f64, lat: f64| ((ox + (lon - west) * k * sc) as f32, (oy + (north - lat) * sc) as f32);

    // Land: state outlines that touch the view, filled, then the lines.
    let (pad_lon, pad_lat) = ((east - west) * 0.3, (north - south) * 0.3);
    let mut land = PathBuilder::new();
    let mut any = false;
    for ring in crate::map::boundaries::state_rings() {
        if !ring.iter().any(|&(lon, lat)| lon >= west - pad_lon && lon <= east + pad_lon && lat >= south - pad_lat && lat <= north + pad_lat) {
            continue;
        }
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            let (x, y) = to(lon, lat);
            if i == 0 { land.move_to(x, y) } else { land.line_to(x, y) }
        }
        land.close();
        any = true;
    }
    if let Some(path) = any.then(|| land.finish()).flatten() {
        off.fill_path(&path, &paint(Color::from_rgba8(34, 56, 80, 255)), FillRule::Winding, Transform::identity(), None);
        off.stroke_path(&path, &paint(Color::from_rgba8(122, 146, 172, 255)), &Stroke { width: 1.4 * s, line_join: tiny_skia::LineJoin::Round, ..Default::default() }, Transform::identity(), None);
    }

    let now_marks = &p.quakes;
    let mut occupied: Vec<(f32, f32, f32, f32)> = Vec::new();
    let focus = p.focus.as_ref().and_then(|id| now_marks.iter().find(|q| &q.id == id));
    // Home and the distance line.
    let home_xy = p.home.map(|(lat, lon)| to(lon, lat));
    if let (Some((hx, hy)), Some(f), true) = (home_xy, focus, p.line) {
        let (fx, fy) = to(f.lon, f.lat);
        let mut line = PathBuilder::new();
        line.move_to(hx, hy);
        line.line_to(fx, fy);
        if let Some(path) = line.finish() {
            let stroke = Stroke { width: 3. * s, dash: StrokeDash::new(vec![14. * s, 10. * s], 0.), ..Default::default() };
            off.stroke_path(&path, &paint(Color::from_rgba8(255, 255, 255, 220)), &stroke, Transform::identity(), None);
        }
        for i in 0..=16 {
            let t = i as f32 / 16.;
            let (x, y) = (hx + (fx - hx) * t, hy + (fy - hy) * t);
            occupied.push((x - 6. * s, y - 6. * s, 12. * s, 12. * s));
        }
    }

    // Quakes: oldest and smallest first.
    let mut order: Vec<&Mark> = now_marks.iter().collect();
    order.sort_by(|a, b| b.age.cmp(&a.age).then(a.mag.total_cmp(&b.mag)));
    let scale_k = (dh as f32 / 700.).clamp(0.6, 2.0);
    for q in order {
        let (x, y) = to(q.lon, q.lat);
        let r = crate::quakes::radius(q.mag) * scale_k;
        if x < -r || y < -r || x > dw as f32 + r || y > dh as f32 + r {
            continue;
        }
        let colour = crate::quakes::age_rgb(q.age);
        if let Some(path) = PathBuilder::from_circle(x, y, r) {
            off.fill_path(&path, &paint(rgb(colour, 215)), FillRule::Winding, Transform::identity(), None);
            off.stroke_path(&path, &paint(Color::from_rgba8(8, 16, 26, 240)), &Stroke { width: 1.8 * s, ..Default::default() }, Transform::identity(), None);
        }
        occupied.push((x - r, y - r, 2. * r, 2. * r));
    }
    // Focus: seismic rings and a label.
    if let Some(f) = focus {
        let (x, y) = to(f.lon, f.lat);
        let r = crate::quakes::radius(f.mag) * scale_k;
        let colour = crate::quakes::age_rgb(f.age);
        for (i, alpha) in [(1.0f32, 230u8), (2.0, 150), (3.0, 80)] {
            if let Some(path) = PathBuilder::from_circle(x, y, r + i * 12. * s) {
                off.stroke_path(&path, &paint(rgb(colour, alpha)), &Stroke { width: 3. * s, ..Default::default() }, Transform::identity(), None);
            }
        }
        let st = text_style(font, 30. * s, Weight::Black);
        let text = format!("M{:.1}", f.mag);
        let tw = measure(&text, &st) + 16. * s;
        let gap = r + 40. * s;
        // Away from home first, so the label never sits on the distance line.
        let west_of_home = home_xy.is_some_and(|(hx, _)| x < hx);
        let (away, toward) = if west_of_home { (x - gap - tw, x + gap) } else { (x + gap, x - gap - tw) };
        place_plate(&mut off, glyphs, &text, &[(away, y - 22. * s), (x - tw / 2., y - gap - 44. * s), (x - tw / 2., y + gap), (toward, y - 22. * s)], &st, s, &mut occupied);
        // Distance along the line, beside its midpoint.
        if let (Some((hx, hy)), Some((hl, ho)), true) = (home_xy, p.home, p.line) {
            let miles = crate::map::geo::haversine_km(hl, ho, f.lat, f.lon) / 1.609;
            let st = text_style(font, 24. * s, Weight::Bold);
            let label = format!("{miles:.0} mi");
            let (mx, my) = ((hx + x) / 2., (hy + y) / 2.);
            let lw = measure(&label, &st) + 16. * s;
            place_plate(&mut off, glyphs, &label, &[(mx - lw / 2., my + 14. * s), (mx - lw / 2., my - 50. * s), (mx + 14. * s, my - 18. * s), (mx - lw - 14. * s, my - 18. * s)], &st, s, &mut occupied);
        }
    }
    // Home marker.
    if let Some((hx, hy)) = home_xy {
        if let Some(path) = PathBuilder::from_circle(hx, hy, 9. * s) {
            off.fill_path(&path, &paint(Color::WHITE), FillRule::Winding, Transform::identity(), None);
            off.stroke_path(&path, &paint(Color::from_rgba8(8, 22, 38, 255)), &Stroke { width: 3. * s, ..Default::default() }, Transform::identity(), None);
        }
        if !p.home_label.is_empty() {
            let st = text_style(font, 22. * s, Weight::Bold);
            let lw = measure(&p.home_label, &st) + 16. * s;
            place_plate(&mut off, glyphs, &p.home_label, &[(hx + 14. * s, hy + 8. * s), (hx - lw - 14. * s, hy + 8. * s), (hx - lw / 2., hy - 50. * s), (hx - lw / 2., hy + 18. * s)], &st, s, &mut occupied);
        }
    }
    // A few big towns for orientation, where they fit.
    let mut towns = crate::places::in_box(south, north, west, east);
    towns.sort_by(|a, b| b.population.cmp(&a.population));
    let st = text_style(font, 18. * s, Weight::SemiBold);
    let (asc, desc) = line_metrics(&st);
    let mut placed = 0;
    for town in towns {
        if placed >= 6 {
            break;
        }
        let (x, y) = to(town.lon, town.lat);
        let (tw, th) = (measure(&town.name, &st) + 12. * s, asc + desc + 6. * s);
        let bx = (x - tw / 2., y + 6. * s, tw, th);
        if bx.0 < 0. || bx.1 < 0. || bx.0 + tw > dw as f32 || bx.1 + th > dh as f32 {
            continue;
        }
        if occupied.iter().any(|o| bx.0 < o.0 + o.2 + 6. * s && bx.0 + tw + 6. * s > o.0 && bx.1 < o.1 + o.3 + 6. * s && bx.1 + th + 6. * s > o.1) {
            continue;
        }
        if let Some(path) = PathBuilder::from_circle(x, y, 3.5 * s) {
            off.fill_path(&path, &paint(Color::from_rgba8(230, 236, 244, 255)), FillRule::Winding, Transform::identity(), None);
        }
        let mut light = st.clone();
        light.color = Color::from_rgba8(225, 232, 241, 255);
        draw_line(&mut off, glyphs, &town.name, bx.0 + 6. * s, bx.1 + 3. * s + asc, &light, 1., None);
        occupied.push(bx);
        placed += 1;
    }
    if now_marks.is_empty() && !p.empty.is_empty() {
        let st = text_style(font, 30. * s, Weight::SemiBold);
        plate(&mut off, glyphs, &p.empty, (dw as f32 - measure(&p.empty, &st)) / 2. - 8. * s, dh as f32 * 0.82, &st, s);
    }
    // Age key, bottom right.
    let key_st = text_style(font, 17. * s, Weight::SemiBold);
    let items = [("Past hour", 60i64), ("Past day", 7_200), ("Past week", 2 * 86_400), ("Older", 9 * 86_400)];
    let key_w: f32 = items.iter().map(|(t, _)| measure(t, &key_st) + 34. * s).sum::<f32>() + 12. * s;
    let (kx, ky) = (dw as f32 - key_w - 12. * s, dh as f32 - 40. * s);
    if let Some(r) = Rect::from_xywh(kx, ky, key_w, 30. * s) {
        off.fill_rect(r, &paint(Color::from_rgba8(8, 22, 38, 220)), Transform::identity(), None);
    }
    let mut x = kx + 10. * s;
    for (t, age) in items {
        if let Some(path) = PathBuilder::from_circle(x + 7. * s, ky + 15. * s, 7. * s) {
            off.fill_path(&path, &paint(rgb(crate::quakes::age_rgb(age), 255)), FillRule::Winding, Transform::identity(), None);
        }
        draw_line(&mut off, glyphs, t, x + 18. * s, ky + 21. * s, &key_st, 1., None);
        x += measure(t, &key_st) + 34. * s;
    }
    composite(pm, &off, local, mask, opacity);
    true
}

fn composite(pm: &mut Pixmap, off: &Pixmap, local: Transform, mask: Option<&Mask>, opacity: f32) {
    pm.draw_pixmap(0, 0, off.as_ref(), &PixmapPaint { opacity: opacity.clamp(0., 1.), ..Default::default() }, Transform::from_translate(local.tx, local.ty), mask);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_round_trips_and_rejects_bad_bounds() {
        let p = Payload { bbox: [-90., 36., -84., 40.], quakes: vec![Mark { id: "a".into(), lat: 38., lon: -87., mag: 3., age: 10 }], ..Default::default() };
        assert!(parse(&json(&p)).is_some());
        let bad = Payload { bbox: [-84., 36., -90., 40.], ..Default::default() };
        assert!(parse(&json(&bad)).is_none());
    }

    #[test]
    fn compass_names_directions() {
        assert_eq!(compass((38.0, -87.0), (40.0, -87.0)), "north");
        assert_eq!(compass((38.0, -87.0), (38.0, -90.0)), "west");
    }

    /// Renders every quake graphic (sample data, Modern) to `CWX_QA_OUT` for review.
    #[test]
    #[ignore]
    fn qa_quake_pngs() {
        let out = std::path::PathBuf::from(std::env::var("CWX_QA_OUT").expect("set CWX_QA_OUT"));
        std::fs::create_dir_all(&out).unwrap();
        let settings = Settings::default();
        let data = WxSnapshot::default();
        for (key, _, _) in infos() {
            for fixture in ["sample", "unavailable"] {
                let mut doc = crate::gfx::templates::build_in(key, "modern").unwrap();
                if fixture == "sample" {
                    sample(&mut doc);
                }
                let pm = crate::program::render_graphic_mode(&doc, &settings, &data, 10_000., doc.width, doc.height, None, crate::gfx::check::PreviewData::Sample);
                pm.save_png(out.join(format!("{key}-{fixture}.png"))).unwrap();
            }
        }
    }

    #[test]
    fn every_quake_template_builds() {
        for (key, _, _) in infos() {
            let d = build(key).expect(key);
            assert_eq!(d.template.as_deref(), Some(key));
        }
    }
}
