//! Producer-reviewed flood road-closure lists. No automatic INDOT feed or all-clear inference.
use super::{
    arcgis_geometry, board,
    observed::{self, Coverage},
    reference::{frame, label, lowerthird_frame, social_frame, MISSING},
};
use crate::{
    config::Settings,
    gfx::{
        bind::Bindings,
        model::{AnimKind, Fill, GraphicDoc, Layer},
        text::{draw_text_box, Align, GlyphCache, TextStyle, Weight},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, OnceLock},
};
use tiny_skia::{Color, Mask, Paint, Pixmap, PixmapPaint, Rect, Transform};
#[derive(Clone, Copy)]
struct Spec {
    key: &'static str,
    county: bool,
    format: u8,
}
include!("road_closure_specs.rs");
#[derive(Clone, Serialize, Deserialize)]
struct Scope {
    counties: BTreeMap<String, String>,
    panel: String,
    secondary: String,
    text: String,
    muted: String,
    primary: String,
}
#[derive(Default)]
struct Geography {
    county: BTreeMap<String, String>,
    region: BTreeMap<String, String>,
    area: String,
}
fn geography(settings: &Settings) -> Arc<Geography> {
    static CACHE: OnceLock<Mutex<BTreeMap<[u64; 2], Arc<Geography>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    let key = [settings.home.lon.to_bits(), settings.home.lat.to_bits()];
    if let Some(found) = cache.lock().ok().and_then(|s| s.get(&key).cloned()) {
        return found;
    }
    let Some(all) = crate::map::boundaries::counties_named() else {
        return Arc::new(Geography::default());
    };
    let name = |c: &crate::map::boundaries::County| {
        format!(
            "{}, {}",
            c.name,
            crate::winter::STATES
                .iter()
                .find(|s| s.0 == c.state)
                .map(|s| s.1)
                .unwrap_or("US")
        )
    };
    let county: Vec<_> = all
        .iter()
        .filter(|c| {
            arcgis_geometry::contains(&c.rings, settings.home.lon, settings.home.lat) == Some(true)
        })
        .collect();
    let mut geo = Geography::default();
    if county.len() == 1 {
        geo.area = name(county[0]);
        geo.county.insert(county[0].fips.clone(), geo.area.clone());
    }
    for c in board::selected(&all, &observed::report_scope(settings, Coverage::Region)) {
        geo.region.insert(c.fips.clone(), name(c));
    }
    let geo = Arc::new(geo);
    if let Ok(mut s) = cache.lock() {
        if s.len() >= 4 {
            s.clear();
        }
        s.insert(key, Arc::clone(&geo));
    }
    geo
}
#[derive(Deserialize)]
struct Envelope {
    scope: Scope,
    payload: Payload,
}
#[derive(Deserialize)]
struct Payload {
    #[serde(default)]
    reviewed: bool,
    #[serde(default)]
    checked_at: String,
    #[serde(default)]
    expires_at: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    reports: Vec<Input>,
}
#[derive(Clone, Deserialize)]
struct Input {
    id: String,
    county: String,
    road: String,
    segment: String,
    status: String,
    cause: String,
    verified_at: String,
}
#[derive(Clone, PartialEq, Eq)]
struct Record {
    id: String,
    county: String,
    road: String,
    segment: String,
    closed: bool,
    verified: i64,
}
struct Reports {
    source: String,
    checked: i64,
    records: Vec<Record>,
}
fn text_ok(s: &str, n: usize) -> bool {
    !s.trim().is_empty() && s.chars().count() <= n && !s.chars().any(char::is_control)
}
fn stamp(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|t| t.timestamp())
}
fn parse(raw: &str, now: i64) -> Option<(Scope, Reports)> {
    if raw.len() > 131072 {
        return None;
    }
    let input: Envelope = serde_json::from_str(raw).ok()?;
    let scope = input.scope;
    let p = input.payload;
    if !p.reviewed
        || !text_ok(&p.source, 120)
        || p.reports.len() > 100
        || scope.counties.is_empty()
        || scope.counties.len() > 512
        || scope.counties.iter().any(|(id, name)| {
            id.len() != 5 || !id.bytes().all(|b| b.is_ascii_digit()) || !text_ok(name, 100)
        })
    {
        return None;
    }
    for c in [
        &scope.panel,
        &scope.secondary,
        &scope.text,
        &scope.muted,
        &scope.primary,
    ] {
        crate::gfx::parse_hex(c)?;
    }
    let checked = stamp(&p.checked_at)?;
    let expires = stamp(&p.expires_at)?;
    if !(0..=21600).contains(&(now - checked))
        || expires <= now
        || !(1..=21600).contains(&(expires - checked))
    {
        return None;
    }
    let mut latest: BTreeMap<String, Record> = BTreeMap::new();
    for input in p.reports {
        if input.cause != "flooding"
            || !text_ok(&input.id, 80)
            || input.county.len() != 5
            || !input.county.bytes().all(|b| b.is_ascii_digit())
            || !text_ok(&input.road, 100)
            || !text_ok(&input.segment, 120)
        {
            return None;
        }
        let verified = stamp(&input.verified_at)?;
        if verified > checked || verified > now {
            return None;
        }
        let closed = match input.status.as_str() {
            "closed" => true,
            "reopened" => false,
            _ => return None,
        };
        let record = Record {
            id: input.id,
            county: input.county,
            road: input.road,
            segment: input.segment,
            closed,
            verified,
        };
        if let Some(old) = latest.get(&record.id) {
            if old.verified == record.verified && old != &record {
                return None;
            }
            if old.verified >= record.verified {
                continue;
            }
        }
        latest.insert(record.id.clone(), record);
    }
    let mut records: Vec<_> = latest
        .into_values()
        .filter(|r| {
            r.closed
                && (0..=21600).contains(&(now - r.verified))
                && scope.counties.contains_key(&r.county)
        })
        .collect();
    records.sort_by(|a, b| {
        b.verified
            .cmp(&a.verified)
            .then_with(|| a.road.cmp(&b.road))
    });
    Some((
        scope,
        Reports {
            source: p.source,
            checked,
            records,
        },
    ))
}
pub(super) fn bind(b: &mut Bindings, settings: &Settings) {
    let geo = geography(settings);
    for county in [true, false] {
        let prefix = if county { "county" } else { "region" };
        let scope = Scope {
            counties: if county {
                geo.county.clone()
            } else {
                geo.region.clone()
            },
            panel: settings.brand.panel.clone(),
            secondary: settings.brand.secondary.clone(),
            text: settings.brand.text.clone(),
            muted: settings.brand.muted.clone(),
            primary: settings.brand.primary.clone(),
        };
        b.set(
            &format!("set.roadclosure.{prefix}.scope"),
            &serde_json::to_string(&scope).unwrap_or_default(),
        );
        b.set(
            &format!("set.roadclosure.{prefix}.area"),
            if county {
                if geo.area.is_empty() {
                    "Home county not available"
                } else {
                    &geo.area
                }
            } else {
                "Regional reported closures"
            },
        );
    }
    b.set("set.roadclosure.payload", r#"{"reviewed":false}"#);
    b.set(
        "set.roadclosure.credit",
        "Source: Producer-verified road reports",
    );
}
pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let s = *SPECS.iter().find(|s| s.key == key)?;
    let prefix = if s.county { "county" } else { "region" };
    let title = "Reported Road Closures";
    let area = format!("{{{{set.roadclosure.{prefix}.area}}}}");
    let source = "{{set.roadclosure.credit}}";
    if s.format == 4 {
        let mut doc = GraphicDoc::new(title, "Flooding", None);
        doc.layers.push(
            Layer::rect(
                "Closure overlay card",
                54.,
                500.,
                1260.,
                530.,
                Fill::solid("$panel"),
                18.,
            )
            .with_anim(AnimKind::Fade, 0.1, 0.3),
        );
        doc.layers.push(
            Layer::rect(
                "Closure overlay accent",
                54.,
                500.,
                8.,
                530.,
                Fill::solid("$primary"),
                4.,
            )
            .with_anim(AnimKind::Wipe, 0.1, 0.3),
        );
        label(
            &mut doc,
            "Closure overlay title",
            (80., 516., 960., 46.),
            title,
            37.,
            "$text",
            Align::Left,
        );
        label(
            &mut doc,
            "Closure overlay area",
            (80., 564., 1200., 30.),
            &area,
            23.,
            "$muted",
            Align::Left,
        );
        doc.layers.push(Layer::risk_map("Verified closure list", 80., 604., 1200., 365., &format!("set:roadclosures:4:{{\"scope\":{{{{set.roadclosure.{prefix}.scope}}}},\"payload\":{{{{set.roadclosure.payload}}}}}}" )).with_anim(AnimKind::Fade, 0.15, 0.3));
        label(
            &mut doc,
            "Closure overlay source",
            (80., 982., 900., 26.),
            source,
            19.,
            "$muted",
            Align::Left,
        );
        label(
            &mut doc,
            "Closure overlay station",
            (1000., 982., 285., 26.),
            "{{station}}",
            19.,
            "$muted",
            Align::Right,
        );
        doc.template = Some(key.to_owned());
        return Some(doc);
    }
    let mut doc = match s.format {
        1 | 2 => social_frame(title, "Flooding", &area, source, s.format == 1),
        3 => lowerthird_frame(title, "Flooding", &area, source),
        _ => frame(title, "Flooding", &area, source),
    };
    let (x, y, w, h) = match s.format {
        1 => (54., 260., 972., 1400.),
        2 => (54., 210., 972., 710.),
        3 => (0., 865., 1920., 131.),
        _ => (80., 205., 1760., 715.),
    };
    doc.layers.push(Layer::risk_map("Verified closure list",x,y,w,h,&format!("set:roadclosures:{}:{{\"scope\":{{{{set.roadclosure.{prefix}.scope}}}},\"payload\":{{{{set.roadclosure.payload}}}}}}",s.format)).with_anim(AnimKind::Fade,0.2,0.4));
    if s.format != 3 {
        label(
            &mut doc,
            "Closure context",
            (
                x,
                if s.format == 1 {
                    1670.
                } else if s.format == 2 {
                    930.
                } else {
                    930.
                },
                w,
                40.,
            ),
            "Selected reports • Follow posted closures • Check with the road agency",
            if s.format == 2 { 21. } else { 25. },
            "$muted",
            Align::Left,
        );
    }
    Some(doc)
}
fn color(s: &str) -> Color {
    crate::gfx::parse_hex(s).unwrap_or(Color::WHITE)
}
pub(super) fn draw(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
    font: &str,
) -> bool {
    let Some(rest) = spec.strip_prefix("set:roadclosures:") else {
        return false;
    };
    let Some((kind, raw)) = rest.split_once(':') else {
        return true;
    };
    let Some(format) = kind.parse::<u8>().ok().filter(|f| *f <= 4) else {
        return true;
    };
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut out) = Pixmap::new(dw, dh) else {
        return true;
    };
    let sx = dw as f32 / w.max(1.);
    let sy = dh as f32 / h.max(1.);
    let u = sx.min(sy);
    let parsed = parse(raw, chrono::Utc::now().timestamp());
    // Even a missing/unreviewed payload uses the current station's supplied chrome colors.
    let style: Option<Scope> = serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|v| serde_json::from_value(v.get("scope")?.clone()).ok());
    let default = Settings::default().brand;
    let panel = style
        .as_ref()
        .map(|s| s.panel.as_str())
        .unwrap_or(&default.panel);
    let text = style
        .as_ref()
        .map(|s| s.text.as_str())
        .unwrap_or(&default.text);
    let muted = style
        .as_ref()
        .map(|s| s.muted.as_str())
        .unwrap_or(&default.muted);
    let secondary = style
        .as_ref()
        .map(|s| s.secondary.as_str())
        .unwrap_or(&default.secondary);
    let primary = style
        .as_ref()
        .map(|s| s.primary.as_str())
        .unwrap_or(&default.primary);
    out.fill(color(panel));
    let words = |out: &mut Pixmap,
                 g: &mut GlyphCache,
                 value: &str,
                 box_: (f32, f32, f32, f32),
                 size: f32,
                 c: &str,
                 align: Align| {
        let style = TextStyle {
            family: font.into(),
            weight: Weight::SemiBold,
            size: size * u,
            color: color(c),
            letter_spacing: 0.,
        };
        draw_text_box(
            out,
            g,
            value,
            (box_.0 * sx, box_.1 * sy, box_.2 * sx, box_.3 * sy),
            &style,
            align,
            true,
            true,
            true,
            false,
            1.,
        );
    };
    let rect = |out: &mut Pixmap, x: f32, y: f32, rw: f32, rh: f32, c: &str| {
        if let Some(r) = Rect::from_xywh(x * sx, y * sy, rw * sx, rh * sy) {
            let mut paint = Paint::default();
            paint.set_color(color(c));
            out.fill_rect(r, &paint, Transform::identity(), None);
        }
    };
    if let Some((scope, reports)) = parsed {
        let clock = |t| {
            chrono::DateTime::from_timestamp(t, 0)
                .unwrap()
                .format("%b %d %H:%M UTC")
                .to_string()
        };
        let max = if format == 3 || format == 4 {
            2
        } else if format == 2 {
            3
        } else {
            4
        };
        let n = reports.records.len().min(max);
        let checked = format!(
            "Producer verified {} • {}",
            clock(reports.checked),
            reports.source
        );
        if format == 3 {
            words(
                &mut out,
                g,
                &format!("{checked} • Showing {n} selected reports"),
                (30., 2., w - 60., 29.),
                20.,
                muted,
                Align::Left,
            );
            for (i, r) in reports.records.iter().take(max).enumerate() {
                let x = 40. + i as f32 * (w - 80.) / 2.;
                let cw = (w - 80.) / 2. - 24.;
                words(
                    &mut out,
                    g,
                    &format!("{} • {}", r.road, scope.counties[&r.county]),
                    (x, 34., cw, 39.),
                    31.,
                    text,
                    Align::Left,
                );
                words(
                    &mut out,
                    g,
                    &format!("Closed • {}", r.segment),
                    (x, 77., cw, 45.),
                    23.,
                    text,
                    Align::Left,
                );
            }
            if n == 0 {
                words(
                    &mut out,
                    g,
                    "No current verified closure reports entered for this area",
                    (40., 35., w - 80., 80.),
                    30.,
                    text,
                    Align::Left,
                );
            }
        } else {
            let (meta, foot, gap, title_size, body_size, time_size) = if format == 4 {
                (62., 48., 14., 30., 23., 17.)
            } else if format == 1 {
                (145., 110., 24., 44., 33., 24.)
            } else if format == 2 {
                (100., 65., 14., 29., 23., 18.)
            } else {
                (100., 65., 14., 35., 25., 20.)
            };
            words(
                &mut out,
                g,
                &checked,
                (24., 5., w - 48., meta - 15.),
                if format == 1 { 27. } else { 24. },
                muted,
                Align::Left,
            );
            let row_h = (h - meta - foot - gap * (max - 1) as f32) / max as f32;
            for (i, r) in reports.records.iter().take(max).enumerate() {
                let y = meta + i as f32 * (row_h + gap);
                rect(&mut out, 0., y, w, row_h, secondary);
                rect(&mut out, 0., y, 6., row_h, primary);
                let title_h = row_h * 0.32;
                words(
                    &mut out,
                    g,
                    &r.road,
                    (24., y + 8., w - 220., title_h),
                    title_size,
                    text,
                    Align::Left,
                );
                rect(&mut out, w - 180., y + 16., 156., 34., "#E5383B");
                words(
                    &mut out,
                    g,
                    "CLOSED",
                    (w - 174., y + 16., 144., 34.),
                    22.,
                    "#FFFFFF",
                    Align::Center,
                );
                let time_h = if format == 1 { 36. } else { 26. };
                let time_y = y + row_h - time_h - 6.;
                words(
                    &mut out,
                    g,
                    &format!("{} • {}", scope.counties[&r.county], r.segment),
                    (
                        24.,
                        y + title_h + 16.,
                        w - 48.,
                        (time_y - y - title_h - 20.).max(16.),
                    ),
                    body_size,
                    text,
                    Align::Left,
                );
                words(
                    &mut out,
                    g,
                    &format!("Report verified {}", clock(r.verified)),
                    (24., time_y, w - 48., time_h),
                    time_size,
                    muted,
                    Align::Left,
                );
            }
            if n == 0 {
                words(
                    &mut out,
                    g,
                    "No current verified closure reports entered for this area",
                    (24., meta, w - 48., h - meta - foot),
                    38.,
                    text,
                    Align::Center,
                );
            }
            words(
                &mut out,
                g,
                &format!(
                    "Showing {n} of {} selected reports • Sections only • No all-clear implied",
                    reports.records.len()
                ),
                (24., h - foot + 10., w - 48., foot - 15.),
                if format == 1 { 25. } else { 21. },
                muted,
                Align::Left,
            );
        }
    } else {
        words(
            &mut out,
            g,
            MISSING,
            (24., 0., w - 48., h),
            if format == 3 { 36. } else { 42. },
            text,
            Align::Center,
        );
    }
    pm.draw_pixmap(
        0,
        0,
        out.as_ref(),
        &PixmapPaint {
            opacity: opacity.clamp(0., 1.),
            ..Default::default()
        },
        Transform::from_translate(local.tx, local.ty),
        mask,
    );
    true
}
#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some(s) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let prefix = if s.county { "county" } else { "region" };
    let brand = Settings::default().brand;
    let scope = Scope {
        counties: BTreeMap::from([("18019".into(), "Clark, IN".into())]),
        panel: brand.panel,
        secondary: brand.secondary,
        text: brand.text,
        muted: brand.muted,
        primary: brand.primary,
    };
    let now = chrono::Utc::now();
    let checked = now.to_rfc3339();
    let expires = (now + chrono::Duration::hours(2)).to_rfc3339();
    let roads = [
        (
            "SAMPLE • Jeffersonville River Access Road",
            "Between the east entrance and the river access intersection",
        ),
        (
            "SAMPLE • County Road 100 West",
            "Between Sample Bridge Road and Sample School Road",
        ),
        (
            "SAMPLE • Riverside Drive",
            "Between Sample Mill Road and the north access gate",
        ),
        (
            "SAMPLE • Old Highway Connector",
            "Between the west junction and Sample Park Road",
        ),
    ];
    let reports:Vec<_>=roads.iter().enumerate().map(|(i,(road,segment))|serde_json::json!({"id":format!("sample-{i}"),"county":"18019","road":road,"segment":segment,"status":"closed","cause":"flooding","verified_at":checked})).collect();
    for (key,value) in [(format!("set.roadclosure.{prefix}.scope"),serde_json::to_string(&scope).unwrap()),(format!("set.roadclosure.{prefix}.area"),"SAMPLE • Clark County, Indiana".into()),("set.roadclosure.payload".into(),serde_json::json!({"reviewed":true,"checked_at":checked,"expires_at":expires,"source":"SAMPLE road agency • Not for broadcast","reports":reports}).to_string()),("set.roadclosure.credit".into(),"SAMPLE ONLY • Not for broadcast".into())]{doc.data_overrides.push((key,value));}
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fixture() -> serde_json::Value {
        json!({"scope":{"counties":{"18019":"Clark, IN"},"panel":"#161A26","secondary":"#222635","text":"#FFFFFF","muted":"#AABBCC","primary":"#2ED3F3"},"payload":{"reviewed":true,"checked_at":"2026-11-14T15:00:00Z","expires_at":"2026-11-14T17:00:00Z","source":"Producer-verified road agency","reports":[{"id":"a","county":"18019","road":"River Road","segment":"Between A and B","status":"closed","cause":"flooding","verified_at":"2026-11-14T14:30:00Z"}]}})
    }
    #[test]
    fn manual_closures_require_review_fresh_verification_expiry_and_known_county() {
        let now = stamp("2026-11-14T15:15:00Z").unwrap();
        let v = fixture();
        assert_eq!(parse(&v.to_string(), now).unwrap().1.records.len(), 1);
        let mut bad = v.clone();
        bad["payload"]["reviewed"] = json!(false);
        assert!(parse(&bad.to_string(), now).is_none());
        assert!(parse(&v.to_string(), now + 3 * 3600).is_none());
        assert!(parse(&v.to_string(), now - 3600).is_none());
        let mut bad = v.clone();
        bad["payload"]["reports"][0]["status"] = json!("unknown");
        assert!(parse(&bad.to_string(), now).is_none());
        let mut outside = v.clone();
        outside["payload"]["reports"][0]["county"] = json!("18037");
        assert!(parse(&outside.to_string(), now)
            .unwrap()
            .1
            .records
            .is_empty());
        let mut stale = v.clone();
        stale["payload"]["reports"][0]["verified_at"] = json!("2026-11-13T12:00:00Z");
        assert!(parse(&stale.to_string(), now).unwrap().1.records.is_empty());
    }
    #[test]
    fn reopened_latest_record_removes_closure_and_equal_time_conflicts_are_withheld() {
        let now = stamp("2026-11-14T15:15:00Z").unwrap();
        let mut v = fixture();
        let mut reopened = v["payload"]["reports"][0].clone();
        reopened["status"] = json!("reopened");
        reopened["verified_at"] = json!("2026-11-14T14:45:00Z");
        v["payload"]["reports"]
            .as_array_mut()
            .unwrap()
            .push(reopened);
        assert!(parse(&v.to_string(), now).unwrap().1.records.is_empty());
        let mut v = fixture();
        let mut conflict = v["payload"]["reports"][0].clone();
        conflict["status"] = json!("reopened");
        v["payload"]["reports"]
            .as_array_mut()
            .unwrap()
            .push(conflict);
        assert!(parse(&v.to_string(), now).is_none());
    }
    #[test]
    fn closure_overlays_leave_program_video_visible_in_both_looks() {
        let settings = Settings::default();
        let data = crate::data::WxSnapshot::default();
        for key in [
            "set_road_closures__county_overlay",
            "set_road_closures__region_overlay",
        ] {
            for look in ["modern", "classic"] {
                let mut doc = build(key).unwrap();
                doc.look = look.into();
                sample(&mut doc);
                assert!(doc.background.is_none());
                let image = crate::program::render_graphic_mode(
                    &doc,
                    &settings,
                    &data,
                    10_000.,
                    1920,
                    1080,
                    None,
                    crate::gfx::check::PreviewData::Sample,
                );
                for (x, y) in [(0, 0), (1800, 900), (1919, 1079)] {
                    assert_eq!(
                        image.pixel(x, y).unwrap().alpha(),
                        0,
                        "{key}/{look} must preserve program picture"
                    );
                }
                assert!(
                    image.pixel(80, 515).unwrap().alpha() > 0,
                    "closure card must remain visible"
                );
            }
        }
    }
}
