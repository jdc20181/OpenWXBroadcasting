//! Transparent lower-screen variants for already reviewed forecaster-drawn polygons.
use super::board::{self, Layout, Scene, Zone};
use crate::gfx::{
    model::{AnimKind, Fill, GraphicDoc, Layer},
    text::{draw_text_box, Align, GlyphCache, TextStyle, Weight},
};
use tiny_skia::{
    Color, FillRule, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spec {
    key: &'static str,
    prefix: &'static str,
    base_key: &'static str,
}

const SPECS: [Spec; 9] = [
    Spec {
        key: "set_forecast_zones__today_county_overlay",
        prefix: "zones",
        base_key: "set_forecast_zones",
    },
    Spec {
        key: "set_forecast_zones__today_region_overlay",
        prefix: "zones.today.region",
        base_key: "set_forecast_zones__today_region",
    },
    Spec {
        key: "set_forecast_zones__today_state_overlay",
        prefix: "zones.today.state",
        base_key: "set_forecast_zones__today_state",
    },
    Spec {
        key: "set_forecast_zones__tomorrow_county_overlay",
        prefix: "zones.tomorrow.county",
        base_key: "set_forecast_zones__tomorrow_county",
    },
    Spec {
        key: "set_forecast_zones__tomorrow_region_overlay",
        prefix: "zones.tomorrow.region",
        base_key: "set_forecast_zones__tomorrow_region",
    },
    Spec {
        key: "set_forecast_zones__tomorrow_state_overlay",
        prefix: "zones.tomorrow.state",
        base_key: "set_forecast_zones__tomorrow_state",
    },
    Spec {
        key: "set_forecast_zones__day3_county_overlay",
        prefix: "zones.day3.county",
        base_key: "set_forecast_zones__day3_county",
    },
    Spec {
        key: "set_forecast_zones__day3_region_overlay",
        prefix: "zones.day3.region",
        base_key: "set_forecast_zones__day3_region",
    },
    Spec {
        key: "set_forecast_zones__day3_state_overlay",
        prefix: "zones.day3.state",
        base_key: "set_forecast_zones__day3_state",
    },
];

pub(super) const KEYS: [&str; 9] = [
    "set_forecast_zones__today_county_overlay",
    "set_forecast_zones__today_region_overlay",
    "set_forecast_zones__today_state_overlay",
    "set_forecast_zones__tomorrow_county_overlay",
    "set_forecast_zones__tomorrow_region_overlay",
    "set_forecast_zones__tomorrow_state_overlay",
    "set_forecast_zones__day3_county_overlay",
    "set_forecast_zones__day3_region_overlay",
    "set_forecast_zones__day3_state_overlay",
];

fn token(prefix: &str, field: &str) -> String {
    format!("{{{{set.{prefix}.{field}}}}}")
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let spec = *SPECS.iter().find(|s| s.key == key)?;
    let mut doc = GraphicDoc::new("Forecaster-drawn winter zones", "Winter", None);
    doc.template = Some(key.to_owned());
    doc.layers.push(
        Layer::rect(
            "Forecast zone overlay card",
            58.,
            675.,
            1180.,
            354.,
            Fill::solid("$panel"),
            20.,
        )
        .with_anim(AnimKind::Fade, 0.1, 0.35),
    );
    doc.layers.push(
        Layer::rect(
            "Forecast zone overlay accent",
            58.,
            675.,
            10.,
            354.,
            Fill::solid("$primary"),
            5.,
        )
        .with_anim(AnimKind::Wipe, 0.1, 0.35),
    );
    doc.layers.push(
        Layer::risk_map(
            "Reviewed forecaster polygons and legend",
            82.,
            698.,
            1132.,
            240.,
            &format!("set:zoneoverlay:{}", token(spec.prefix, "payload")),
        )
        .with_anim(AnimKind::Fade, 0.18, 0.4),
    );
    super::reference::label(
        &mut doc,
        "Forecast zone overlay subtitle",
        (86., 939., 1120., 25.),
        &token(spec.prefix, "subtitle"),
        19.,
        "#C7D4E0",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Forecast zone overlay source",
        (86., 964., 1120., 24.),
        &token(spec.prefix, "source"),
        17.,
        "#D5E1EC",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Forecast zone overlay note",
        (86., 988., 1120., 31.),
        &token(spec.prefix, "note"),
        16.,
        "#B9C9D8",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Forecast zone overlay station",
        (1000., 682., 210., 22.),
        "{{station}}",
        16.,
        "$muted",
        Align::Right,
    );
    Some(doc)
}

fn color(value: &str, alpha: u8) -> Color {
    crate::gfx::parse_hex(value)
        .and_then(|c| Color::from_rgba(c.red(), c.green(), c.blue(), alpha as f32 / 255.))
        .unwrap_or(Color::WHITE)
}

fn paint(value: &str, alpha: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.anti_alias = true;
    p.set_color(color(value, alpha));
    p
}

#[derive(Clone, Copy)]
struct Fit {
    west: f64,
    north: f64,
    cosine: f64,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Fit {
    fn new(
        bounds: [f64; 4],
        width: f32,
        height: f32,
        origin_x: f32,
        origin_y: f32,
    ) -> Option<Self> {
        let [west, south, east, north] = bounds;
        if ![west, south, east, north].iter().all(|v| v.is_finite())
            || west >= east
            || south >= north
            || width <= 16.
            || height <= 16.
        {
            return None;
        }
        let cosine = (((south + north) / 2.).to_radians().cos()).abs().max(0.15);
        let (geo_w, geo_h) = ((east - west) * cosine, north - south);
        if geo_w <= 0. || geo_h <= 0. {
            return None;
        }
        let scale = ((width - 16.) as f64 / geo_w).min((height - 16.) as f64 / geo_h);
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        Some(Self {
            west,
            north,
            cosine,
            scale,
            ox: origin_x as f64 + (width as f64 - geo_w * scale) / 2.,
            oy: origin_y as f64 + (height as f64 - geo_h * scale) / 2.,
        })
    }

    fn point(self, lon: f64, lat: f64) -> (f32, f32) {
        (
            (self.ox + (lon - self.west) * self.cosine * self.scale) as f32,
            (self.oy + (self.north - lat) * self.scale) as f32,
        )
    }
}

fn ring_path(rings: &[Vec<(f64, f64)>], fit: Fit) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for ring in rings {
        if ring.len() < 4 || ring.len() > 20_000 {
            return None;
        }
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            if !lon.is_finite()
                || !lat.is_finite()
                || !(-180. ..=180.).contains(&lon)
                || !(-85. ..=85.).contains(&lat)
            {
                return None;
            }
            let (x, y) = fit.point(lon, lat);
            if i == 0 {
                builder.move_to(x, y);
            } else {
                builder.line_to(x, y);
            }
        }
        builder.close();
    }
    builder.finish()
}

fn label_point(zone: &Zone) -> Option<(f64, f64)> {
    let (x, y) = zone.label_at.unwrap_or_else(|| {
        let n = zone.ring.len() as f64;
        let sum = zone
            .ring
            .iter()
            .fold((0., 0.), |a, p| (a.0 + p.0, a.1 + p.1));
        (sum.0 / n, sum.1 / n)
    });
    (!zone.label.trim().is_empty() && crate::gfx::winter_map::inside_pub(&zone.ring, x, y))
        .then_some((x, y))
}

fn text(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    font: &str,
    value: &str,
    rect: (f32, f32, f32, f32),
    size: f32,
    color: Color,
    align: Align,
) {
    draw_text_box(
        pm,
        glyphs,
        value,
        rect,
        &TextStyle {
            family: font.to_owned(),
            weight: Weight::SemiBold,
            size,
            color,
            letter_spacing: 0.,
        },
        align,
        true,
        true,
        true,
        false,
        1.,
    );
}

fn render(pm: &mut Pixmap, glyphs: &mut GlyphCache, font: &str, scene: &Scene) {
    let Some(panel) = scene.panels.first() else {
        return;
    };
    let (width, height) = (pm.width() as f32, pm.height() as f32);
    let unit = (width / 1132.).min(height / 240.).clamp(0.45, 1.5);
    let map_right = width * 0.61;
    text(
        pm,
        glyphs,
        font,
        &panel.title,
        (8. * unit, 1. * unit, map_right - 16. * unit, 27. * unit),
        23. * unit,
        color("#F4F8FC", 255),
        Align::Left,
    );
    text(
        pm,
        glyphs,
        font,
        &panel.valid,
        (8. * unit, 28. * unit, map_right - 16. * unit, 24. * unit),
        17. * unit,
        color("#C6D2DF", 255),
        Align::Left,
    );

    let Some(all) = crate::map::boundaries::counties_named() else {
        text(
            pm,
            glyphs,
            font,
            "County boundaries unavailable",
            (8. * unit, 95. * unit, map_right - 16. * unit, 32. * unit),
            18. * unit,
            color("#E1EAF2", 255),
            Align::Left,
        );
        return;
    };
    let counties = board::selected(&all, &scene.scope);
    let bounds = counties
        .iter()
        .flat_map(|c| c.rings.iter().flatten())
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, &(x, y)| {
            [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
        });
    let map_x = 4. * unit;
    let map_y = 57. * unit;
    let map_w = map_right - 8. * unit;
    let zones = panel.zones.as_deref().unwrap_or_default();
    let status_message = if !scene.reviewed || panel.zones.is_none() {
        Some("Forecaster review required")
    } else if zones.is_empty() {
        Some("No reviewed zones in this view • Not an all-clear")
    } else {
        None
    };
    // Reserve a dedicated status rail so unavailable/empty-state copy never overlaps fills.
    let status_reserve = if status_message.is_some() {
        38. * unit
    } else {
        8. * unit
    };
    let map_h = height - map_y - status_reserve;
    let Some(fit) = Fit::new(bounds, map_w, map_h, map_x, map_y) else {
        text(
            pm,
            glyphs,
            font,
            "Selected geography unavailable",
            (8. * unit, 95. * unit, map_right - 16. * unit, 32. * unit),
            18. * unit,
            color("#E1EAF2", 255),
            Align::Left,
        );
        return;
    };

    // County union is the clip, so source polygons outside this scope never paint the map.
    let mut clip = match Mask::new(pm.width(), pm.height()) {
        Some(v) => v,
        None => return,
    };
    let county_rings = counties
        .iter()
        .flat_map(|c| c.rings.iter().cloned())
        .collect::<Vec<_>>();
    if let Some(geography) = ring_path(&county_rings, fit) {
        clip.fill_path(&geography, FillRule::EvenOdd, true, Transform::identity());
    } else {
        return;
    }
    draw_zone_fills(pm, scene, fit, &clip);

    // Draw zone contours over the fills, then restore county boundaries above both.
    for zone in zones {
        if let Some(shape) = ring_path(std::slice::from_ref(&zone.ring), fit) {
            let border = Stroke {
                width: (1.0 * unit).clamp(0.5, 1.5),
                ..Default::default()
            };
            pm.stroke_path(
                &shape,
                &paint("#FFFFFF", 230),
                &border,
                Transform::identity(),
                Some(&clip),
            );
        }
    }
    let county_edge = Stroke {
        width: (1.0 * unit).clamp(0.6, 1.4),
        ..Default::default()
    };
    for county in &counties {
        if let Some(path) = ring_path(&county.rings, fit) {
            pm.stroke_path(
                &path,
                &paint("#D7E2EC", 255),
                &county_edge,
                Transform::identity(),
                None,
            );
        }
    }

    if let Some(message) = status_message {
        text(
            pm,
            glyphs,
            font,
            message,
            (
                8. * unit,
                height - 29. * unit,
                map_right - 16. * unit,
                24. * unit,
            ),
            16. * unit,
            color("#E1EAF2", 255),
            Align::Left,
        );
    }
    let mut placed_labels = Vec::new();
    for zone in zones {
        let Some((lon, lat)) = label_point(zone) else {
            continue;
        };
        if !counties.iter().any(|c| board::inside(c, lon, lat)) {
            continue;
        }
        let (x, y) = fit.point(lon, lat);
        // At tiny preview sizes the minimum legible label can exceed the map.
        // Keep the polygons visible instead of giving f32::clamp reversed bounds.
        if map_w < 140. * unit {
            continue;
        }
        let bw = (zone.label.chars().count() as f32 * 8.2 * unit + 18. * unit)
            .clamp(130. * unit, map_w - 10. * unit);
        let box_ = tiny_skia::Rect::from_xywh(
            (x - bw / 2.).clamp(map_x + 4. * unit, map_x + map_w - bw - 4. * unit),
            (y - 13. * unit).clamp(map_y + 3. * unit, map_y + map_h - 26. * unit),
            bw,
            26. * unit,
        );
        let Some(box_) = box_ else {
            continue;
        };
        if placed_labels
            .iter()
            .copied()
            .any(|old| rects_overlap(box_, old))
        {
            continue;
        }
        placed_labels.push(box_);
        let mut fill = Paint::default();
        fill.set_color(Color::from_rgba8(8, 22, 36, 235));
        let mut back_builder = PathBuilder::new();
        back_builder.move_to(box_.x(), box_.y());
        back_builder.line_to(box_.right(), box_.y());
        back_builder.line_to(box_.right(), box_.bottom());
        back_builder.line_to(box_.x(), box_.bottom());
        back_builder.close();
        if let Some(back) = back_builder.finish() {
            pm.fill_path(
                &back,
                &fill,
                FillRule::Winding,
                Transform::identity(),
                Some(&clip),
            );
        }
        text(
            pm,
            glyphs,
            font,
            &zone.label,
            (box_.x(), box_.y(), box_.width(), box_.height()),
            16. * unit,
            color("#FFFFFF", 255),
            Align::Center,
        );
    }

    // Shared legend comes from the same reviewed scene and stays outside the clipped map.
    let rail_x = map_right + 12. * unit;
    let rail_w = width - rail_x - 8. * unit;
    text(
        pm,
        glyphs,
        font,
        &scene.legend_title,
        (rail_x, 5. * unit, rail_w, 30. * unit),
        15. * unit,
        color("#D5E1EC", 255),
        Align::Left,
    );
    for (i, band) in scene.legend.iter().enumerate() {
        let y = 45. * unit + i as f32 * 36. * unit;
        if let Some(swatch) =
            tiny_skia::Rect::from_xywh(rail_x, y + 4. * unit, 16. * unit, 16. * unit)
        {
            pm.fill_rect(
                swatch,
                &paint(&band.color, 255),
                Transform::identity(),
                None,
            );
        }
        text(
            pm,
            glyphs,
            font,
            &band.label,
            (rail_x + 24. * unit, y, rail_w - 24. * unit, 25. * unit),
            15. * unit,
            color("#F2F6FA", 255),
            Align::Left,
        );
    }
}

fn rects_overlap(a: tiny_skia::Rect, b: tiny_skia::Rect) -> bool {
    a.x() < b.right() && b.x() < a.right() && a.y() < b.bottom() && b.y() < a.bottom()
}

fn draw_zone_fills(pm: &mut Pixmap, scene: &Scene, fit: Fit, clip: &Mask) {
    // Higher categories paint after lower categories, matching the shared board map.
    let Some(panel) = scene.panels.first() else {
        return;
    };
    let zones = panel.zones.as_deref().unwrap_or_default();
    for (category, band) in scene.legend.iter().enumerate() {
        for zone in zones.iter().filter(|zone| zone.category == category) {
            if let Some(shape) = ring_path(std::slice::from_ref(&zone.ring), fit) {
                pm.fill_path(
                    &shape,
                    &paint(&band.color, 255),
                    FillRule::EvenOdd,
                    Transform::identity(),
                    Some(clip),
                );
            }
        }
    }
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
    let Some(raw) = spec.strip_prefix("set:zoneoverlay:") else {
        return false;
    };
    if raw.len() > 8_000_000 {
        return true;
    }
    let Some(scene) = board::parse(raw, Layout::Zones) else {
        return true;
    };
    let Some(panel) = scene.panels.first() else {
        return true;
    };
    let vertices = panel
        .zones
        .as_deref()
        .unwrap_or_default()
        .iter()
        .fold(0usize, |n, z| n.saturating_add(z.ring.len()));
    if panel.zones.as_ref().is_some_and(|zones| zones.len() > 512) || vertices > 250_000 {
        return true;
    }
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut overlay) = Pixmap::new(dw, dh) else {
        return true;
    };
    render(&mut overlay, glyphs, font, &scene);
    pm.draw_pixmap(
        0,
        0,
        overlay.as_ref(),
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
    let Some(spec) = SPECS
        .iter()
        .find(|s| Some(s.key) == doc.template.as_deref())
    else {
        return;
    };
    let Some(mut base) = super::comparison::build(spec.base_key) else {
        return;
    };
    base.template = Some(spec.base_key.to_owned());
    super::comparison::sample(&mut base);
    doc.data_overrides.extend(base.data_overrides);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_overlay_keys_preserve_period_and_scope_binding_identity() {
        let mut keys = HashSet::new();
        for spec in SPECS {
            let mut doc = build(spec.key).unwrap();
            assert_eq!(doc.template.as_deref(), Some(spec.key));
            assert!(keys.insert(doc.template.clone().unwrap()));
            assert!(doc
                .layers
                .iter()
                .any(|l| l.name == "Reviewed forecaster polygons and legend"));
            sample(&mut doc);
            assert!(
                doc.data_overrides
                    .iter()
                    .any(|(k, v)| k == &format!("set.{}.payload", spec.prefix)
                        && v.contains("SAMPLE"))
            );
        }
        assert_eq!(keys.len(), 9);
    }

    #[test]
    fn overlay_keeps_map_outside_pixels_transparent_and_draws_reviewed_sample() {
        let mut doc = build("set_forecast_zones__today_county_overlay").unwrap();
        sample(&mut doc);
        let raw = doc
            .data_overrides
            .iter()
            .find(|(k, _)| k == "set.zones.payload")
            .unwrap()
            .1
            .clone();
        let mut pm = Pixmap::new(1132, 240).unwrap();
        let before = pm.data().to_vec();
        let mut glyphs = GlyphCache::default();
        assert!(draw(
            &mut pm,
            &mut glyphs,
            Transform::identity(),
            None,
            1132.,
            240.,
            &format!("set:zoneoverlay:{raw}"),
            1.,
            "sans"
        ));
        assert_eq!(
            &pm.data()[0..4],
            &before[0..4],
            "outside card map/legend content is transparent"
        );
        assert!(
            pm.data().chunks_exact(4).any(|p| p[3] != 0),
            "reviewed sample should paint content"
        );
    }

    #[test]
    fn unreviewed_polygon_payload_is_removed_before_overlay_drawing() {
        let mut scene = board::empty(Layout::Zones);
        scene.legend.push(board::Band {
            label: "Flakes possible".into(),
            color: "#4AADE0".into(),
        });
        scene.panels[0].zones = Some(vec![Zone {
            category: 0,
            ring: vec![(-87.0, 38.0), (-86.0, 38.0), (-86.0, 39.0), (-87.0, 38.0)],
            label: "SAMPLE polygon".into(),
            label_at: Some((-86.5, 38.5)),
        }]);
        scene.reviewed = false;
        let payload = serde_json::to_string(&scene).unwrap();
        let checked = board::parse(&payload, Layout::Zones).unwrap();
        assert!(checked.panels[0].zones.is_none());
        assert!(!checked.reviewed);
    }

    #[test]
    fn opaque_zone_fills_follow_category_order_and_respect_scope_clip() {
        let mut scene = board::empty(Layout::Zones);
        scene.legend = vec![
            board::Band {
                label: "Lower".into(),
                color: "#FF0000".into(),
            },
            board::Band {
                label: "Higher".into(),
                color: "#00FF00".into(),
            },
        ];
        scene.panels[0].zones = Some(vec![
            Zone {
                category: 0,
                ring: vec![(1., 1.), (8., 1.), (8., 8.), (1., 8.), (1., 1.)],
                label: "Lower zone".into(),
                label_at: Some((2., 2.)),
            },
            Zone {
                category: 1,
                ring: vec![(4., 4.), (9., 4.), (9., 9.), (4., 9.), (4., 4.)],
                label: "Higher zone".into(),
                label_at: Some((5., 5.)),
            },
        ]);
        let mut pm = Pixmap::new(100, 100).unwrap();
        let mut clip = Mask::new(100, 100).unwrap();
        let mut clip_path = PathBuilder::new();
        clip_path.move_to(0., 0.);
        clip_path.line_to(60., 0.);
        clip_path.line_to(60., 100.);
        clip_path.line_to(0., 100.);
        clip_path.close();
        clip.fill_path(
            &clip_path.finish().unwrap(),
            FillRule::Winding,
            true,
            Transform::identity(),
        );
        let fit = Fit::new([0., 0., 10., 10.], 100., 100., 0., 0.).unwrap();
        draw_zone_fills(&mut pm, &scene, fit, &clip);

        let overlap = pm.pixel(50, 50).unwrap();
        assert!(overlap.green() > 250 && overlap.red() < 5);
        assert_eq!(overlap.alpha(), 255);
        let clipped = pm.pixel(80, 50).unwrap();
        assert_eq!(clipped.alpha(), 0);
    }
    #[test]
    fn narrow_previews_keep_polygons_without_panicking_on_label_width() {
        for spec in SPECS {
            let mut doc = build(spec.key).unwrap();
            sample(&mut doc);
            let raw = &doc
                .data_overrides
                .iter()
                .find(|(key, _)| key == &format!("set.{}.payload", spec.prefix))
                .unwrap()
                .1;
            let mut pm = Pixmap::new(96, 80).unwrap();
            let mut glyphs = GlyphCache::default();
            assert!(draw(
                &mut pm,
                &mut glyphs,
                Transform::identity(),
                None,
                96.,
                80.,
                &format!("set:zoneoverlay:{raw}"),
                1.,
                "sans"
            ));
        }
    }
}
