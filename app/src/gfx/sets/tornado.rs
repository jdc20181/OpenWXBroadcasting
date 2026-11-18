//! Reviewed-event map payload, resolved through normal per-document data overrides.
//! A null event list means unavailable; an empty reviewed list means zero tornadoes.
use crate::gfx::text::{draw_line, line_metrics, measure, GlyphCache, TextStyle, Weight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tiny_skia::{
    Color, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub(super) struct Event {
    pub id: String,
    pub lon: f64,
    pub lat: f64,
    pub ef: Option<u8>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Payload {
    pub state: String,
    #[serde(default)]
    pub reviewed: bool,
    pub events: Option<Vec<Event>>,
    #[serde(default)]
    pub annual_average: Option<f64>,
    #[serde(default)]
    pub baseline: String,
}

pub(super) fn parse(raw: &str) -> Option<Payload> {
    let mut p: Payload = serde_json::from_str(raw).ok()?;
    if p.annual_average.is_some_and(|n| !n.is_finite() || n < 0.) || p.baseline.chars().count() > 60
    {
        return None;
    }
    if p.state.len() != 2 || !p.state.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if let Some(events) = p.events.as_mut() {
        if !p.reviewed || events.len() > 10000 {
            return None;
        }
        let mut ids = HashMap::new();
        if events.iter().any(|e| {
            e.id.trim().is_empty()
                || !e.lat.is_finite()
                || !e.lon.is_finite()
                || !(-90.0..=90.0).contains(&e.lat)
                || !(-180.0..=180.0).contains(&e.lon)
                || e.ef.is_some_and(|v| v > 5)
        }) {
            return None;
        }
        for e in events.iter() {
            if ids.get(&e.id).is_some_and(|prior| prior != e) {
                // Conflicting revisions require review; never silently choose a rating.
                return None;
            }
            ids.insert(e.id.clone(), e.clone());
        }
        let mut seen = std::collections::HashSet::new();
        events.retain(|e| seen.insert(e.id.clone()));
    }
    Some(p)
}

fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}

fn ink(hex: &str) -> Color {
    crate::gfx::parse_hex(hex).unwrap_or(Color::WHITE)
}

fn caption(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    value: &str,
    x: f32,
    y: f32,
    max_width: f32,
    size: f32,
) {
    let mut st = TextStyle {
        family: font.into(),
        weight: Weight::SemiBold,
        size,
        color: Color::WHITE,
        letter_spacing: 0.,
    };
    let width = measure(value, &st);
    if width > max_width {
        st.size *= max_width / width;
    }
    let (a, d) = line_metrics(&st);
    let clip = Some((0, 0, pm.width() as i32, pm.height() as i32));
    draw_line(pm, g, value, x, y + (a - d) / 2., &st, 1., clip);
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
    font: &str,
) -> bool {
    if let Some(raw) = spec.strip_prefix("set:tornado-summary:") {
        summary(pm, glyphs, local, mask, w, h, raw, opacity, font);
        return true;
    }
    let Some(raw) = spec.strip_prefix("set:tornado:") else {
        return false;
    };
    let dw = (w * local.sx.abs()).round().clamp(1., 4096.) as u32;
    let dh = (h * local.sy.abs()).round().clamp(1., 4096.) as u32;
    let Some(mut off) = Pixmap::new(dw, dh) else {
        return true;
    };
    let scale = dw as f32 / w.max(1.);
    let payload = parse(raw);
    let events = payload.as_ref().and_then(|p| p.events.as_ref());
    // The unavailable strip owns its own space; never paint over the state or placards.
    let map_h = if events.is_none() {
        (dh as f32 - 70. * scale).max(1.)
    } else {
        dh as f32
    };
    let state = payload.as_ref().map(|p| p.state.as_str()).unwrap_or("18");
    let all = crate::map::boundaries::counties_named();
    let counties: Vec<_> = all
        .as_ref()
        .map(|v| v.iter().filter(|c| c.state == state).collect())
        .unwrap_or_default();
    let mut bbox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in &counties {
        for &(lon, lat) in c.rings.iter().flatten() {
            bbox = (
                bbox.0.min(lon),
                bbox.1.min(lat),
                bbox.2.max(lon),
                bbox.3.max(lat),
            );
        }
    }
    if counties.is_empty() {
        caption(
            &mut off,
            glyphs,
            font,
            "County map not available yet",
            20. * scale,
            dh as f32 * 0.43,
            dw as f32 - 40. * scale,
            30. * scale,
        );
        caption(
            &mut off,
            glyphs,
            font,
            super::reference::MISSING,
            20. * scale,
            dh as f32 * 0.57,
            dw as f32 - 40. * scale,
            25. * scale,
        );
    } else {
        let k = ((bbox.1 + bbox.3) / 2.).to_radians().cos();
        let (gw, gh) = ((bbox.2 - bbox.0) * k, bbox.3 - bbox.1);
        let s = ((dw as f64 * 0.9) / gw).min((map_h as f64 * 0.9) / gh);
        let (ox, oy) = ((dw as f64 - gw * s) / 2., (map_h as f64 - gh * s) / 2.);
        let to = |lon: f64, lat: f64| {
            (
                (ox + (lon - bbox.0) * k * s) as f32,
                (oy + (bbox.3 - lat) * s) as f32,
            )
        };
        let paths: Vec<_> = counties
            .iter()
            .filter_map(|c| {
                let mut b = PathBuilder::new();
                for ring in &c.rings {
                    for (i, &(lon, lat)) in ring.iter().enumerate() {
                        let (x, y) = to(lon, lat);
                        if i == 0 {
                            b.move_to(x, y)
                        } else {
                            b.line_to(x, y)
                        }
                    }
                    b.close();
                }
                b.finish()
            })
            .collect();
        // Rim first, then solid counties, then county lines. Blend the finished map once.
        let rim = Stroke {
            width: 5. * scale,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        for path in &paths {
            off.stroke_path(
                path,
                &paint(ink("#AFC5DE")),
                &rim,
                Transform::identity(),
                None,
            );
        }
        for path in &paths {
            off.fill_path(
                path,
                &paint(ink("#273F59")),
                FillRule::EvenOdd,
                Transform::identity(),
                None,
            );
        }
        let edge = Stroke {
            width: 0.8 * scale,
            ..Default::default()
        };
        for path in &paths {
            off.stroke_path(
                path,
                &paint(ink("#6C849B")),
                &edge,
                Transform::identity(),
                None,
            );
        }
        let mut occupied: Vec<(f32, f32, f32, f32)> = Vec::new();
        if let Some(events) = events {
            for e in events {
                // Reviewed payloads must be in the selected state's actual polygon.
                if !in_counties(&counties, e) {
                    continue;
                }
                let (x, y) = to(e.lon, e.lat);
                let index = e.ef.map(usize::from).unwrap_or(6);
                let radius = 5.5 * scale;
                if let Some(circle) = PathBuilder::from_circle(x, y, radius + 1.5 * scale) {
                    off.fill_path(
                        &circle,
                        &paint(ink("#081626")),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                if let Some(circle) = PathBuilder::from_circle(x, y, radius) {
                    off.fill_path(
                        &circle,
                        &paint(ink(super::reference::EF[index].1)),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
                occupied.push((x - radius * 2., y - radius * 2., radius * 4., radius * 4.));
            }
        }
        let mut towns = crate::places::in_box(bbox.1, bbox.3, bbox.0, bbox.2);
        towns.sort_by(|a, b| b.population.cmp(&a.population));
        let st = TextStyle {
            family: font.into(),
            weight: Weight::SemiBold,
            size: 17. * scale,
            color: Color::WHITE,
            letter_spacing: 0.,
        };
        let (asc, desc) = line_metrics(&st);
        let mut count = 0;
        for town in towns {
            if count >= 4 {
                break;
            }
            if !counties.iter().any(|c| {
                c.rings.iter().fold(false, |inside, ring| {
                    inside ^ crate::gfx::winter_map::inside_pub(ring, town.lon, town.lat)
                })
            }) {
                continue;
            }
            let (x, y) = to(town.lon, town.lat);
            let (tw, th) = (
                measure(&town.name, &st) + 16. * scale,
                asc + desc + 10. * scale,
            );
            let box_ = (x - tw / 2., y + 10. * scale, tw, th);
            if box_.0 < 0. || box_.1 + th > map_h || box_.0 + tw > dw as f32 {
                continue;
            }
            if occupied.iter().any(|b| {
                box_.0 < b.0 + b.2 + 8. * scale
                    && box_.0 + tw + 8. * scale > b.0
                    && box_.1 < b.1 + b.3 + 8. * scale
                    && box_.1 + th + 8. * scale > b.1
            }) {
                continue;
            }
            if let Some(rect) = Rect::from_xywh(box_.0, box_.1, tw, th) {
                off.fill_rect(rect, &paint(ink("#081626")), Transform::identity(), None);
            }
            draw_line(
                &mut off,
                glyphs,
                &town.name,
                box_.0 + 8. * scale,
                box_.1 + 5. * scale + asc,
                &st,
                1.,
                None,
            );
            occupied.push(box_);
            count += 1;
        }
        if events.is_none() {
            let bar_h = 58. * scale;
            if let Some(rect) = Rect::from_xywh(0., dh as f32 - bar_h, dw as f32, bar_h) {
                off.fill_rect(rect, &paint(ink("#081626")), Transform::identity(), None);
            }
            caption(
                &mut off,
                glyphs,
                font,
                super::reference::MISSING,
                20. * scale,
                dh as f32 - bar_h / 2.,
                dw as f32 - 40. * scale,
                28. * scale,
            );
        }
    }
    pm.draw_pixmap(
        0,
        0,
        off.as_ref(),
        &PixmapPaint {
            opacity: opacity.clamp(0., 1.),
            ..Default::default()
        },
        Transform::from_translate(local.tx, local.ty),
        mask,
    );
    true
}

/// Uses the exact same polygon inclusion and unique-event list as the map.
fn in_counties(counties: &[&crate::map::boundaries::County], e: &Event) -> bool {
    counties.iter().any(|c| {
        c.rings.iter().fold(false, |inside, ring| {
            inside ^ crate::gfx::winter_map::inside_pub(ring, e.lon, e.lat)
        })
    })
}

pub(super) fn category_counts(events: &[Event]) -> [usize; 7] {
    let mut counts = [0; 7];
    for e in events {
        counts[e.ef.map(usize::from).unwrap_or(6)] += 1;
    }
    counts
}

#[allow(clippy::too_many_arguments)]
fn summary(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    local: Transform,
    mask: Option<&Mask>,
    w: f32,
    h: f32,
    raw: &str,
    opacity: f32,
    font: &str,
) {
    let dw = (w * local.sx.abs()).round().clamp(1., 4096.) as u32;
    let dh = (h * local.sy.abs()).round().clamp(1., 4096.) as u32;
    let Some(mut off) = Pixmap::new(dw, dh) else {
        return;
    };
    let (sx, sy) = (dw as f32 / 580., dh as f32 / 659.);
    let payload = parse(raw);
    let all = crate::map::boundaries::counties_named();
    let counties: Vec<_> = all
        .as_ref()
        .map(|v| {
            v.iter()
                .filter(|c| Some(c.state.as_str()) == payload.as_ref().map(|p| p.state.as_str()))
                .collect()
        })
        .unwrap_or_default();
    let events = payload
        .as_ref()
        .and_then(|p| p.events.as_ref())
        .filter(|_| !counties.is_empty());
    caption(
        &mut off,
        glyphs,
        font,
        "Confirmed tornadoes",
        30. * sx,
        50. * sy,
        520. * sx,
        30. * sx,
    );
    if let Some(events) = events {
        let events: Vec<_> = events
            .iter()
            .filter(|e| in_counties(&counties, e))
            .cloned()
            .collect();
        let counts = category_counts(&events);
        caption(
            &mut off,
            glyphs,
            font,
            &events.len().to_string(),
            30. * sx,
            127. * sy,
            520. * sx,
            82. * sx,
        );
        for (i, (ef, color)) in super::reference::EF.iter().enumerate() {
            let y = (223. + i as f32 * 48.) * sy;
            if let Some(rect) = Rect::from_xywh(30. * sx, y - 12. * sy, 10. * sx, 25. * sy) {
                off.fill_rect(rect, &paint(ink(color)), Transform::identity(), None);
            }
            caption(&mut off, glyphs, font, ef, 55. * sx, y, 200. * sx, 29. * sx);
            let value = counts[i].to_string();
            let st = TextStyle {
                family: font.into(),
                weight: Weight::SemiBold,
                size: 30. * sx,
                color: Color::WHITE,
                letter_spacing: 0.,
            };
            caption(
                &mut off,
                glyphs,
                font,
                &value,
                550. * sx - measure(&value, &st),
                y,
                200. * sx,
                30. * sx,
            );
        }
        let p = payload.as_ref().unwrap();
        let avg = match p.annual_average.filter(|_| !p.baseline.trim().is_empty()) {
            Some(value) => format!("Annual average: {value:.1}"),
            None => "Annual average not available yet".into(),
        };
        caption(
            &mut off,
            glyphs,
            font,
            &avg,
            30. * sx,
            584. * sy,
            520. * sx,
            26. * sx,
        );
        if p.annual_average.is_some() && !p.baseline.trim().is_empty() {
            caption(
                &mut off,
                glyphs,
                font,
                &p.baseline,
                30. * sx,
                623. * sy,
                520. * sx,
                23. * sx,
            );
        }
    } else {
        caption(
            &mut off,
            glyphs,
            font,
            super::reference::MISSING,
            30. * sx,
            280. * sy,
            520. * sx,
            32. * sx,
        );
        caption(
            &mut off,
            glyphs,
            font,
            "Awaiting reviewed records",
            30. * sx,
            335. * sy,
            520. * sx,
            26. * sx,
        );
        caption(
            &mut off,
            glyphs,
            font,
            "Annual average not available yet",
            30. * sx,
            584. * sy,
            520. * sx,
            26. * sx,
        );
    }
    pm.draw_pixmap(
        0,
        0,
        off.as_ref(),
        &PixmapPaint {
            opacity: opacity.clamp(0., 1.),
            ..Default::default()
        },
        Transform::from_translate(local.tx, local.ty),
        mask,
    );
}
