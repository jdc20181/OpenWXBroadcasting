//! WPC QPF source-polygon renderer. Geometry rings stay nested so holes survive even-odd fill.
use super::board::{self, Scope};
use crate::gfx::text::{Align, GlyphCache, TextStyle, Weight, draw_text_box};
use serde::{Deserialize, Serialize};
use tiny_skia::{
    Color, FillRule, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Polygon {
    pub qpf: f32,
    pub rings: Vec<Vec<(f64, f64)>>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Payload {
    pub scope: Scope,
    pub title: String,
    pub valid: String,
    pub issue: String,
    pub home: [u64; 2],
    pub generation: u64,
    pub layer: u8,
    pub sample: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_polygons: Option<Vec<Polygon>>,
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
    fn new(bounds: [f64; 4], rect: [f32; 4]) -> Option<Self> {
        let [west, south, east, north] = bounds;
        let [x, y, w, h] = rect;
        if ![west, south, east, north].iter().all(|v| v.is_finite())
            || west >= east
            || south >= north
            || w < 10.
            || h < 10.
        {
            return None;
        }
        let cosine = (((south + north) / 2.).to_radians().cos()).abs().max(0.15);
        let (gw, gh) = ((east - west) * cosine, north - south);
        let scale = ((w as f64 - 8.) / gw).min((h as f64 - 8.) / gh);
        if !scale.is_finite() || scale <= 0. {
            return None;
        }
        Some(Self {
            west,
            north,
            cosine,
            scale,
            ox: x as f64 + (w as f64 - gw * scale) / 2.,
            oy: y as f64 + (h as f64 - gh * scale) / 2.,
        })
    }
    fn point(self, lon: f64, lat: f64) -> (f32, f32) {
        (
            (self.ox + (lon - self.west) * self.cosine * self.scale) as f32,
            (self.oy + (self.north - lat) * self.scale) as f32,
        )
    }
}
fn color(rgb: [u8; 3], alpha: u8) -> Color {
    Color::from_rgba8(rgb[0], rgb[1], rgb[2], alpha)
}
fn paint(color: Color) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    p
}
fn text(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    s: &str,
    rect: (f32, f32, f32, f32),
    size: f32,
    color: Color,
    align: Align,
) {
    draw_text_box(
        pm,
        g,
        s,
        rect,
        &TextStyle {
            family: font.to_string(),
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
fn path(rings: &[Vec<(f64, f64)>], fit: Fit) -> Option<Path> {
    let mut b = PathBuilder::new();
    for ring in rings {
        if ring.len() < 4 || ring.len() > 100_000 {
            return None;
        }
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            if !lon.is_finite()
                || !lat.is_finite()
                || !(-180. ..=180.).contains(&lon)
                || !(-90. ..=90.).contains(&lat)
            {
                return None;
            }
            let (x, y) = fit.point(lon, lat);
            if i == 0 {
                b.move_to(x, y);
            } else {
                b.line_to(x, y);
            }
        }
        if ring.first() != ring.last() {
            return None;
        }
        b.close();
    }
    b.finish()
}
fn qpf_color(v: f32) -> Option<Color> {
    const STOPS: &[(f32, [u8; 3])] = crate::grid::RAIN_TOTAL_STOPS;
    if !v.is_finite() || v < 0. {
        return None;
    }
    let mut chosen = None;
    for &(cut, rgb) in STOPS {
        if v >= cut {
            chosen = Some(rgb);
        } else {
            break;
        }
    }
    chosen.map(|c| color(c, 255))
}
fn render(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    p: &Payload,
    polygons: &[Polygon],
    available: bool,
) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let compact = h < w * 0.34;
    let scale = if compact {
        (w / 1132.).clamp(0.65, 1.8)
    } else {
        (w / 1132.).min(h / 715.).clamp(0.45, 1.8)
    };
    let ink = Color::from_rgba8(245, 248, 252, 255);
    let muted = Color::from_rgba8(205, 218, 231, 255);
    let title = if p.sample {
        format!("SAMPLE • {}", p.title)
    } else {
        p.title.clone()
    };
    text(
        pm,
        g,
        font,
        &title,
        (8. * scale, 2. * scale, w - 16. * scale, 26. * scale),
        22. * scale,
        ink,
        Align::Left,
    );
    let issue = if !available {
        "WPC issue unavailable".to_string()
    } else if p.issue.is_empty() {
        "WPC issue unavailable".to_string()
    } else {
        format!("Issued {0}", p.issue)
    };
    let valid = if !available {
        "Forecast period unavailable".to_string()
    } else if p.valid.is_empty() {
        "Valid period unavailable".to_string()
    } else {
        p.valid.clone()
    };
    text(
        pm,
        g,
        font,
        &format!("{valid}   •   {issue}"),
        (8. * scale, 27. * scale, w - 16. * scale, 23. * scale),
        15. * scale,
        muted,
        Align::Left,
    );

    let compact = h < w * 0.34;
    let vertical = h > w * 1.15;
    let map_rect = if compact {
        [4. * scale, 54. * scale, w * 0.68, h - 58. * scale]
    } else if vertical {
        [8. * scale, 58. * scale, w - 16. * scale, h * 0.60]
    } else {
        [8. * scale, 56. * scale, w - 16. * scale, h * 0.63]
    };
    let legend_rect = if compact {
        [w * 0.72, 54. * scale, w * 0.27, h - 58. * scale]
    } else if vertical {
        [8. * scale, h * 0.71, w - 16. * scale, h * 0.27]
    } else {
        [8. * scale, h * 0.72, w - 16. * scale, h * 0.24]
    };

    let Some(all) = crate::map::boundaries::counties_named() else {
        text(
            pm,
            g,
            font,
            "Geographic boundaries unavailable",
            (8. * scale, 90. * scale, w - 16. * scale, 32. * scale),
            17. * scale,
            muted,
            Align::Left,
        );
        legend(pm, g, font, legend_rect, scale);
        return;
    };
    let counties = board::selected(&all, &p.scope);
    let county_bounds = counties
        .iter()
        .flat_map(|c| c.rings.iter().flatten())
        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, &(x, y)| {
            [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
        });
    let bounds = p.scope.bbox.unwrap_or(county_bounds);
    let Some(fit) = Fit::new(bounds, map_rect) else {
        text(
            pm,
            g,
            font,
            "Selected geography unavailable",
            (8. * scale, 90. * scale, w - 16. * scale, 32. * scale),
            17. * scale,
            muted,
            Align::Left,
        );
        legend(pm, g, font, legend_rect, scale);
        return;
    };

    // Local scopes are exact requested boxes; state/national scopes use the actual
    // selected county union. Even-odd filling preserves holes and detached islands.
    let Some(mut clip) = Mask::new(pm.width(), pm.height()) else {
        return;
    };
    if let Some([west, south, east, north]) = p.scope.bbox {
        if let Some(boundary) = path(
            &[vec![
                (west, south),
                (east, south),
                (east, north),
                (west, north),
                (west, south),
            ]],
            fit,
        ) {
            clip.fill_path(&boundary, FillRule::EvenOdd, true, Transform::identity());
        }
    } else {
        let mut geography = PathBuilder::new();
        for c in &counties {
            for ring in &c.rings {
                if ring.len() < 4 {
                    continue;
                }
                for (i, &(lon, lat)) in ring.iter().enumerate() {
                    let (x, y) = fit.point(lon, lat);
                    if i == 0 {
                        geography.move_to(x, y);
                    } else {
                        geography.line_to(x, y);
                    }
                }
                geography.close();
            }
        }
        if let Some(boundary) = geography.finish() {
            clip.fill_path(&boundary, FillRule::EvenOdd, true, Transform::identity());
        }
    }
    for a in polygons {
        let Some(c) = qpf_color(a.qpf) else {
            continue;
        };
        if let Some(shape) = path(&a.rings, fit) {
            pm.fill_path(
                &shape,
                &paint(c),
                FillRule::EvenOdd,
                Transform::identity(),
                Some(&clip),
            );
        }
    }
    // Coast/state/county edges remain visible over filled source polygons.
    let edge = Stroke {
        width: (0.8 * scale).clamp(0.35, 1.7),
        ..Default::default()
    };
    for c in &counties {
        if let Some(shape) = path(&c.rings, fit) {
            pm.stroke_path(
                &shape,
                &paint(Color::from_rgba8(230, 238, 247, 190)),
                &edge,
                Transform::identity(),
                Some(&clip),
            );
        }
    }
    if !available {
        text(
            pm,
            g,
            font,
            "WPC QPF product unavailable",
            (
                map_rect[0] + 8. * scale,
                map_rect[1] + map_rect[3] * 0.42,
                map_rect[2] - 16. * scale,
                30. * scale,
            ),
            16. * scale,
            muted,
            Align::Center,
        );
    }
    legend(pm, g, font, legend_rect, scale);
    if p.sample {
        text(
            pm,
            g,
            font,
            "SAMPLE ONLY • Synthetic geometry • Not for broadcast",
            (8. * scale, h - 21. * scale, w - 16. * scale, 18. * scale),
            12. * scale,
            Color::from_rgba8(255, 210, 120, 255),
            Align::Center,
        );
    }
}

fn valid_payload(p: &Payload) -> bool {
    if ![1, 2, 3, 4, 9, 10, 11].contains(&p.layer)
        || p.title.chars().count() > 85
        || p.valid.chars().count() > 60
        || p.issue.chars().count() > 48
        || p.scope.states.len() > 49
        || p.scope
            .states
            .iter()
            .any(|s| s.len() != 2 || !s.bytes().all(|b| b.is_ascii_digit()))
        || p.scope
            .county
            .as_ref()
            .is_some_and(|s| s.len() != 5 || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return false;
    }
    if let Some([west, south, east, north]) = p.scope.bbox {
        if ![west, south, east, north].iter().all(|v| v.is_finite())
            || !(-180.0..=180.0).contains(&west)
            || !(-180.0..=180.0).contains(&east)
            || !(-90.0..=90.0).contains(&south)
            || !(-90.0..=90.0).contains(&north)
            || west >= east
            || south >= north
            || east - west > 70.0
            || north - south > 55.0
        {
            return false;
        }
    }
    if !p.sample {
        return p.sample_polygons.is_none();
    }
    if !p.issue.starts_with("SAMPLE") || !p.valid.starts_with("SAMPLE") {
        return false;
    }
    let Some(polygons) = p.sample_polygons.as_ref() else {
        return false;
    };
    if polygons.len() > 40 {
        return false;
    }
    let mut vertices = 0usize;
    let mut prior = f32::NEG_INFINITY;
    polygons.iter().all(|a| {
        if !a.qpf.is_finite()
            || !(0.0..=100.0).contains(&a.qpf)
            || a.qpf < prior
            || a.rings.is_empty()
            || a.rings.len() > 16
        {
            return false;
        }
        prior = a.qpf;
        a.rings.iter().all(|r| {
            vertices = vertices.saturating_add(r.len());
            r.len() >= 4
                && r.len() <= 256
                && vertices <= 2000
                && r.first() == r.last()
                && r.iter().all(|(x, y)| {
                    x.is_finite()
                        && y.is_finite()
                        && (-180.0..=180.0).contains(x)
                        && (-90.0..=90.0).contains(y)
                })
        })
    })
}
fn legend(pm: &mut Pixmap, g: &mut GlyphCache, font: &str, r: [f32; 4], s: f32) {
    const STOPS: &[(f32, [u8; 3])] = crate::grid::RAIN_TOTAL_STOPS;
    let compact = r[2] < 350. * s;
    text(
        pm,
        g,
        font,
        "QPF • inches",
        (r[0], r[1], r[2], 20. * s),
        15. * s,
        Color::from_rgba8(226, 235, 244, 255),
        Align::Left,
    );
    if compact {
        let item_h = ((r[3] - 22. * s) / 8.).max(12. * s);
        for (i, (v, rgb)) in STOPS.iter().enumerate() {
            let y = r[1] + 22. * s + i as f32 * item_h;
            let sw = Rect::from_xywh(r[0], y + 2. * s, 12. * s, 12. * s);
            if let Some(sw) = sw {
                pm.fill_rect(sw, &paint(color(*rgb, 235)), Transform::identity(), None);
            }
            text(
                pm,
                g,
                font,
                if i + 1 == STOPS.len() {
                    "5+".to_string()
                } else {
                    format!("{v:.2}")
                }
                .as_str(),
                (r[0] + 16. * s, y, r[2] - 16. * s, item_h),
                12. * s,
                Color::from_rgba8(245, 248, 252, 255),
                Align::Left,
            );
        }
    } else {
        let item_w = r[2] / 8.;
        for (i, (v, rgb)) in STOPS.iter().enumerate() {
            let x = r[0] + i as f32 * item_w;
            if let Some(sw) = Rect::from_xywh(x, r[1] + 22. * s, 15. * s, 12. * s) {
                pm.fill_rect(sw, &paint(color(*rgb, 235)), Transform::identity(), None);
            }
            text(
                pm,
                g,
                font,
                if i + 1 == STOPS.len() {
                    "5+".to_string()
                } else {
                    format!("{v:.2}")
                }
                .as_str(),
                (
                    x + 19. * s,
                    r[1] + 19. * s,
                    (item_w - 19. * s).max(18. * s),
                    18. * s,
                ),
                11. * s,
                Color::from_rgba8(245, 248, 252, 255),
                Align::Left,
            );
        }
    }
}

/// Draws a source-backed QPF graphic. Canvas and overlays remain transparent outside data/boundaries.
#[allow(clippy::too_many_arguments)]
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
    let Some(raw) = spec.strip_prefix("set:qpfcanvas:") else {
        return false;
    };
    if raw.len() > 32 * 1024 {
        return true;
    }
    let Some(payload) = serde_json::from_str::<Payload>(raw).ok() else {
        return true;
    };
    if !valid_payload(&payload) {
        return true;
    }
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut out) = Pixmap::new(dw, dh) else {
        return true;
    };
    let live = if payload.sample {
        None
    } else {
        super::qpf::live_polygons(payload.home, payload.generation, payload.layer)
    };
    let polygons = if payload.sample {
        payload.sample_polygons.as_deref().unwrap_or_default()
    } else {
        live.as_deref().map(Vec::as_slice).unwrap_or_default()
    };
    let available = payload.sample || live.is_some();
    render(&mut out, g, font, &payload, polygons, available);
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
mod tests {
    use super::*;
    #[test]
    fn even_odd_fill_keeps_source_hole_transparent() {
        let fit = Fit::new([0., 0., 10., 10.], [0., 0., 100., 100.]).unwrap();
        let rings = vec![
            vec![(1., 1.), (9., 1.), (9., 9.), (1., 9.), (1., 1.)],
            vec![(3., 3.), (3., 7.), (7., 7.), (7., 3.), (3., 3.)],
        ];
        let shape = path(&rings, fit).unwrap();
        let mut pm = Pixmap::new(100, 100).unwrap();
        pm.fill_path(
            &shape,
            &paint(color([20, 180, 40], 255)),
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
        assert_eq!(pm.data()[(50 * 100 + 50) * 4 + 3], 0);
        assert!(pm.data()[(20 * 100 + 20) * 4 + 3] > 0);
    }
    #[test]
    fn empty_canvas_does_not_paint_over_map_underlay() {
        let p = Payload {
            scope: Scope {
                states: vec!["18".into()],
                county: None,
                bbox: None,
            },
            title: "Day 1 QPF".into(),
            valid: "SAMPLE • Valid period".into(),
            issue: "SAMPLE • Issue".into(),
            home: [0, 0],
            generation: 0,
            layer: 1,
            sample: true,
            sample_polygons: Some(Vec::new()),
        };
        let mut pm = Pixmap::new(500, 200).unwrap();
        let mut glyphs = GlyphCache::default();
        render(&mut pm, &mut glyphs, "Arial", &p, &[], true);
        assert_eq!(
            pm.data()[3],
            0,
            "the top-left underlay pixel stays transparent"
        );
    }
    #[test]
    fn twc_palette_and_unavailable_are_explicit() {
        assert_eq!(qpf_color(0.005), None);
        assert_eq!(
            qpf_color(0.01),
            Some(color(crate::grid::RAIN_TOTAL_STOPS[0].1, 255))
        );
        assert_eq!(
            qpf_color(10.),
            Some(color(crate::grid::RAIN_TOTAL_STOPS[7].1, 255))
        );
        assert_eq!(qpf_color(20.), qpf_color(5.));
        let p = Payload {
            scope: Scope {
                states: vec!["18".into()],
                county: None,
                bbox: None,
            },
            title: "Day 1 QPF".into(),
            valid: String::new(),
            issue: String::new(),
            home: [0, 0],
            generation: 0,
            layer: 1,
            sample: false,
            sample_polygons: None,
        };
        assert!(serde_json::to_string(&p).is_ok());
    }
}
