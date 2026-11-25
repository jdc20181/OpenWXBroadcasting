//! Shared transparent map-and-legend overlay for source-backed county category products.
use super::board::{self, Layout, Scene};
use crate::gfx::{
    model::{AnimKind, Fill, GraphicDoc, Layer},
    text::{draw_text_box, Align, GlyphCache, TextStyle, Weight},
};
use tiny_skia::{
    Color, FillRule, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Stroke, Transform,
};

fn color(value: &str) -> Color {
    crate::gfx::parse_hex(value).unwrap_or(Color::WHITE)
}
fn paint(value: &str) -> Paint<'static> {
    let mut p = Paint::default();
    p.anti_alias = true;
    p.set_color(color(value));
    p
}
fn draw_text(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    font: &str,
    value: &str,
    rect: (f32, f32, f32, f32),
    size: f32,
    text_color: &str,
) {
    draw_text_box(
        pm,
        glyphs,
        value,
        rect,
        &TextStyle {
            family: font.to_string(),
            weight: Weight::SemiBold,
            size,
            color: color(text_color),
            letter_spacing: 0.,
        },
        Align::Left,
        true,
        true,
        true,
        false,
        1.,
    );
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

fn county_path(rings: &[Vec<(f64, f64)>], fit: Fit) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for ring in rings {
        let mut first = true;
        for &(lon, lat) in ring {
            if !lon.is_finite()
                || !lat.is_finite()
                || !(-180. ..=180.).contains(&lon)
                || !(-85. ..=85.).contains(&lat)
            {
                return None;
            }
            let (x, y) = fit.point(lon, lat);
            if first {
                builder.move_to(x, y);
                first = false;
            } else {
                builder.line_to(x, y);
            }
        }
        if !first {
            builder.close();
        }
    }
    builder.finish()
}

fn render_scene(pm: &mut Pixmap, glyphs: &mut GlyphCache, font: &str, scene: &Scene) {
    let width = pm.width() as f32;
    let height = pm.height() as f32;
    let Some(panel) = scene.panels.first() else {
        return;
    };
    let scale = (width / 1132.).min(height / 240.).clamp(0.45, 1.5);
    let map_portion = width * 0.59;
    draw_text(
        pm,
        glyphs,
        font,
        &panel.title,
        (
            8. * scale,
            2. * scale,
            map_portion - 16. * scale,
            28. * scale,
        ),
        25. * scale,
        "#F4F8FC",
    );
    draw_text(
        pm,
        glyphs,
        font,
        &panel.valid,
        (
            8. * scale,
            29. * scale,
            map_portion - 16. * scale,
            24. * scale,
        ),
        18. * scale,
        "#B9C9D8",
    );

    let Some(all) = crate::map::boundaries::counties_named() else {
        draw_text(
            pm,
            glyphs,
            font,
            "County map data unavailable",
            (
                8. * scale,
                90. * scale,
                map_portion - 16. * scale,
                34. * scale,
            ),
            20. * scale,
            "#D5E1EC",
        );
        return;
    };
    let counties = board::selected(&all, &scene.scope);
    let bounds = counties.iter().flat_map(|c| c.rings.iter().flatten()).fold(
        [f64::MAX, f64::MAX, f64::MIN, f64::MIN],
        |b, &(lon, lat)| [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)],
    );
    let available = panel.counties.as_ref();
    let has_values = available.is_some_and(|m| counties.iter().any(|c| m.contains_key(&c.fips)))
        || panel.zones.as_ref().is_some_and(|zones| !zones.is_empty());
    let map_x = 4. * scale;
    let map_y = 58. * scale;
    let map_w = (map_portion - 8. * scale).max(1.);
    let status_space = if has_values { 4. } else { 34. };
    let map_h = (height - map_y - status_space * scale).max(1.);
    let Some(fit) = Fit::new(bounds, map_w, map_h, map_x, map_y) else {
        draw_text(
            pm,
            glyphs,
            font,
            "County map unavailable for this scope",
            (
                8. * scale,
                92. * scale,
                map_portion - 16. * scale,
                34. * scale,
            ),
            18. * scale,
            "#D5E1EC",
        );
        return;
    };
    let paths: Vec<_> = counties
        .iter()
        .map(|c| county_path(&c.rings, fit))
        .collect();
    for (county, path) in counties
        .iter()
        .zip(paths.iter())
        .filter_map(|(c, p)| p.as_ref().map(|p| (c, p)))
    {
        let Some(fill) = available
            .and_then(|m| m.get(&county.fips))
            .and_then(|i| scene.legend.get(*i))
            .map(|band| band.color.as_str())
        else {
            continue;
        };
        pm.fill_path(
            path,
            &paint(fill),
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }
    if let Some(zones)=&panel.zones {
        // Supplied polygons retain their actual shape; clip once to the county union.
        if let Some(mut clip)=tiny_skia::Mask::new(pm.width(),pm.height()) {
            for path in paths.iter().flatten(){clip.fill_path(path,FillRule::EvenOdd,true,Transform::identity());}
            for zone in zones {
                if let (Some(path),Some(band))=(county_path(std::slice::from_ref(&zone.ring),fit),scene.legend.get(zone.category)) {
                    pm.fill_path(&path,&paint(&band.color),FillRule::EvenOdd,Transform::identity(),Some(&clip));
                }
            }
        }
    }
    let edge = Stroke {
        width: 0.8 * scale,
        ..Default::default()
    };
    for path in paths.iter().flatten() {
        pm.stroke_path(path, &paint("#B8C7D5"), &edge, Transform::identity(), None);
    }

    if !has_values {
        draw_text(
            pm,
            glyphs,
            font,
            if available.is_some() {
                "No mapped values in this view"
            } else {
                "Data not available yet"
            },
            (
                8. * scale,
                height - 26. * scale,
                map_portion - 16. * scale,
                24. * scale,
            ),
            18. * scale,
            "#D5E1EC",
        );
    }

    let rail_x = map_portion + 18. * scale;
    let rail_w = (width - rail_x - 4. * scale).max(1.);
    let legend = &scene.legend;
    // The compact rail reserves enough vertical space for every legend entry.
    // Ranked values are optional; dense legends retain their full readable scale.
    let summary: &[(String, String)] = if legend.len() <= 4 {
        panel.summary.as_deref().unwrap_or_default()
    } else {
        &[]
    };
    let summary_start = if summary.is_empty() {
        height - 8. * scale
    } else {
        height * 0.57
    };
    let band_h = if legend.is_empty() {
        22. * scale
    } else {
        ((summary_start - 40. * scale) / legend.len() as f32).min(27. * scale)
    };
    draw_text(
        pm,
        glyphs,
        font,
        if summary.is_empty() {
            "CATEGORY LEGEND"
        } else {
            &panel.summary_title
        },
        (rail_x, 4. * scale, rail_w, 25. * scale),
        16. * scale,
        "#D5E1EC",
    );
    for (i, band) in legend.iter().enumerate() {
        let y = 34. * scale + i as f32 * band_h;
        if let Some(swatch) =
            tiny_skia::Rect::from_xywh(rail_x, y + 2. * scale, 18. * scale, 18. * scale)
        {
            pm.fill_rect(swatch, &paint(&band.color), Transform::identity(), None);
        }
        draw_text(
            pm,
            glyphs,
            font,
            &band.label,
            (rail_x + 27. * scale, y, rail_w - 28. * scale, 24. * scale),
            16. * scale,
            "#F2F6FA",
        );
    }
    if !summary.is_empty() {
        let y0 = (summary_start + 2. * scale).min(height - 52. * scale);
        for (i, (name, value)) in summary.iter().take(3).enumerate() {
            draw_text(
                pm,
                glyphs,
                font,
                &format!("{name}: {value}"),
                (
                    rail_x,
                    y0 + (21. + 21. * i as f32) * scale,
                    rail_w,
                    20. * scale,
                ),
                15. * scale,
                "#F2F6FA",
            );
        }
    }
}

/// Draw source report points over a transparent county outline. The scene remains
/// the authority for scope, period, values and category; no county is filled or
/// interpolated between reports.
fn render_points(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    font: &str,
    scene: &Scene,
    panel: &super::board::Panel,
) {
    let width = pm.width() as f32;
    let height = pm.height() as f32;
    let scale = (width / 1132.).min(height / 300.).clamp(0.45, 1.5);
    let map_portion = width * 0.61;
    draw_text(
        pm,
        glyphs,
        font,
        &panel.title,
        (
            8. * scale,
            2. * scale,
            map_portion - 16. * scale,
            24. * scale,
        ),
        20. * scale,
        "#F4F8FC",
    );
    draw_text(
        pm,
        glyphs,
        font,
        &panel.valid,
        (
            8. * scale,
            25. * scale,
            map_portion - 16. * scale,
            20. * scale,
        ),
        14. * scale,
        "#B9C9D8",
    );

    let map_x = 6. * scale;
    let map_y = 48. * scale;
    let map_w = (map_portion - 12. * scale).max(1.);
    let map_h = (height - map_y - 27. * scale).max(1.);
    // Keep the Arc-backed geometry alive while `counties` borrows it.
    let all = crate::map::boundaries::counties_named();
    let counties = all
        .as_ref()
        .map(|all| board::selected(all, &scene.scope))
        .unwrap_or_default();
    let bounds = if !counties.is_empty() {
        counties.iter().flat_map(|c| c.rings.iter().flatten()).fold(
            [f64::MAX, f64::MAX, f64::MIN, f64::MIN],
            |b, &(lon, lat)| [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)],
        )
    } else {
        scene.scope.bbox.unwrap_or([0., 0., 0., 0.])
    };
    let Some(fit) = Fit::new(bounds, map_w, map_h, map_x, map_y) else {
        draw_text(
            pm,
            glyphs,
            font,
            "Point map scope unavailable",
            (
                8. * scale,
                70. * scale,
                map_portion - 16. * scale,
                24. * scale,
            ),
            15. * scale,
            "#D5E1EC",
        );
        return;
    };
    let paths: Vec<_> = counties
        .iter()
        .map(|c| county_path(&c.rings, fit))
        .collect();
    let edge = Stroke {
        width: 0.55 * scale,
        ..Default::default()
    };
    for path in paths.iter().flatten() {
        pm.stroke_path(path, &paint("#8FA5B8"), &edge, Transform::identity(), None);
    }

    let points = panel.points.as_deref().unwrap_or_default();
    let summary = panel.summary.as_deref().unwrap_or_default();
    let visible = visible_report_points(
        points,
        &counties,
        &scene.scope,
        fit,
        (map_x, map_y, map_w, map_h),
        scale,
    );
    let numbered = numbered_report_points(&visible, summary, (map_x, map_y, map_w, map_h), scale);
    for point in &visible {
        let selected_rank = numbered
            .iter()
            .position(|selected| selected.source_index == point.source_index);
        if selected_rank.is_some() {
            continue;
        }
        let (x, y) = (point.x, point.y);
        let r = 4.2 * scale;
        let c = report_color(scene, point.point.category, point.point.value);
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_circle(x, y, r);
        if let Some(circle) = builder.finish() {
            pm.fill_path(
                &circle,
                &paint(&c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    // Emphasize selected labels after plain dots so close-by reports cannot
    // obscure the numbered marker itself. Their data points remain plotted.
    for (rank, point) in numbered.iter().enumerate() {
        let (x, y) = (point.x, point.y);
        let r = 9.5 * scale;
        let c = report_color(scene, point.point.category, point.point.value);
        let mut builder = tiny_skia::PathBuilder::new();
        builder.push_circle(x, y, r);
        if let Some(circle) = builder.finish() {
            pm.fill_path(
                &circle,
                &paint(&c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
            pm.stroke_path(
                &circle,
                &paint("#F4F8FC"),
                &Stroke {
                    width: 0.8 * scale,
                    ..Default::default()
                },
                Transform::identity(),
                None,
            );
        }
        draw_centered_text(
            pm,
            glyphs,
            font,
            &(rank + 1).to_string(),
            (x - r, y - 7. * scale, r * 2., 14. * scale),
            11. * scale,
            "#FFFFFF",
        );
    }
    if let Some(status) = report_map_status(panel.points.as_deref(), visible.len(), numbered.len())
    {
        draw_text(
            pm,
            glyphs,
            font,
            &status,
            (
                8. * scale,
                height - 23. * scale,
                map_portion - 16. * scale,
                19. * scale,
            ),
            13. * scale,
            "#D5E1EC",
        );
    }

    let rail_x = map_portion + 12. * scale;
    let rail_w = (width - rail_x - 6. * scale).max(1.);
    draw_text(
        pm,
        glyphs,
        font,
        &scene.legend_title,
        (rail_x, 4. * scale, rail_w, 25. * scale),
        13. * scale,
        "#D5E1EC",
    );
    let legend = board::bands(scene);
    // Split dense precipitation scales into two columns so the named-value
    // rows below remain visible without shrinking or dropping them.
    let (columns, legend_rows, legend_row_h) = legend_layout(legend.len(), scale);
    let col_w = rail_w / columns as f32;
    for (i, band) in legend.iter().enumerate() {
        let column = i / legend_rows.max(1);
        let row = i % legend_rows.max(1);
        let x = rail_x + column as f32 * col_w;
        let y = 32. * scale + row as f32 * legend_row_h;
        if let Some(swatch) =
            tiny_skia::Rect::from_xywh(x, y + 2. * scale, 11. * scale, 11. * scale)
        {
            pm.fill_rect(swatch, &paint(&band.color), Transform::identity(), None);
        }
        draw_text(
            pm,
            glyphs,
            font,
            &band.label,
            (x + 15. * scale, y, col_w - 16. * scale, legend_row_h),
            if columns == 2 {
                14. * scale
            } else {
                15. * scale
            },
            "#F2F6FA",
        );
    }
    let rank_title_y = 32. * scale + legend_rows as f32 * legend_row_h + 4. * scale;
    // Only with numbered towns: an empty heading on the unavailable state reads as missing data.
    if !numbered.is_empty() {
        let heading = match numbered.len() {
            1 => "SELECTED TOWN LABEL • MARKER 1".to_string(),
            n => format!("SELECTED TOWN LABELS • MARKERS 1–{n}"),
        };
        draw_text(
            pm,
            glyphs,
            font,
            &heading,
            (rail_x, rank_title_y, rail_w, 18. * scale),
            14. * scale,
            "#D5E1EC",
        );
    }
    let list_y = rank_title_y + 19. * scale;
    let list_bottom = (height - 26. * scale).max(list_y);
    let item_h = ((list_bottom - list_y) / 3.).max(1.);
    for (rank, item) in numbered.iter().enumerate() {
        let Some((place, value)) = summary.get(item.source_index) else {
            continue;
        };
        let y = list_y + rank as f32 * item_h;
        let detail = if scene.legend_title.to_ascii_lowercase().contains("hail") {
            item.point
                .value
                .filter(|v| v.is_finite() && *v > 0.)
                .map(|v| format!("{value} • {}", hail_comparison(v)))
                .unwrap_or_else(|| value.clone())
        } else {
            value.clone()
        };
        draw_text(
            pm,
            glyphs,
            font,
            &format!("{}. {place}", rank + 1),
            (rail_x, y, rail_w, item_h * 0.48),
            15. * scale,
            "#F2F6FA",
        );
        draw_text(
            pm,
            glyphs,
            font,
            &detail,
            (
                rail_x + 13. * scale,
                y + item_h * 0.48,
                rail_w - 13. * scale,
                item_h * 0.48,
            ),
            14. * scale,
            "#D5E1EC",
        );
    }
}

struct RankedPoint<'a> {
    source_index: usize,
    point: &'a super::board::ReportPoint,
    x: f32,
    y: f32,
}

fn report_map_status(
    points: Option<&[super::board::ReportPoint]>,
    mapped: usize,
    numbered: usize,
) -> Option<String> {
    let Some(points) = points else {
        return Some("Data not available yet".into());
    };
    if points.is_empty() {
        return Some("No reports returned for this view".into());
    }
    if points.len() > 500 {
        return Some(format!(
            "Showing {mapped} mapped reports from first 500 source reports • {numbered} numbered"
        ));
    }
    if mapped == 0 {
        return Some("No reports inside this displayed scope".into());
    }
    Some(format!(
        "Showing {mapped} mapped report{} • {numbered} numbered town label{}",
        if mapped == 1 { "" } else { "s" },
        if numbered == 1 { "" } else { "s" }
    ))
}

fn visible_report_points<'a>(
    points: &'a [super::board::ReportPoint],
    counties: &[&crate::map::boundaries::County],
    scope: &super::board::Scope,
    fit: Fit,
    map: (f32, f32, f32, f32),
    scale: f32,
) -> Vec<RankedPoint<'a>> {
    let mut ranked = Vec::new();
    for (source_index, point) in points.iter().enumerate().take(500) {
        if !(-180. ..=180.).contains(&point.lon)
            || !(-85. ..=85.).contains(&point.lat)
            || !point_is_in_scene_scope(counties, scope, point.lon, point.lat)
        {
            continue;
        }
        let (x, y) = fit.point(point.lon, point.lat);
        let r = 4.2 * scale;
        let inset =
            map.0 <= x - r && map.1 <= y - r && x + r <= map.0 + map.2 && y + r <= map.1 + map.3;
        if inset {
            ranked.push(RankedPoint {
                source_index,
                point,
                x,
                y,
            });
        }
    }
    ranked
}

fn numbered_report_points<'a, 'p>(
    visible: &'a [RankedPoint<'p>],
    summaries: &[(String, String)],
    map: (f32, f32, f32, f32),
    scale: f32,
) -> Vec<&'a RankedPoint<'p>> {
    let mut selected = Vec::new();
    let radius = 9.5 * scale;
    for point in visible {
        if summaries.get(point.source_index).is_none() {
            continue;
        }
        if point.x - radius < map.0
            || point.y - radius < map.1
            || point.x + radius > map.0 + map.2
            || point.y + radius > map.1 + map.3
        {
            continue;
        }
        let label_collision = selected.iter().any(|placed: &&RankedPoint<'_>| {
            let dx = placed.x - point.x;
            let dy = placed.y - point.y;
            dx * dx + dy * dy < (2. * radius + 2. * scale).powi(2)
        });
        if !label_collision {
            selected.push(point);
            if selected.len() == 3 {
                break;
            }
        }
    }
    selected
}

fn legend_layout(legend_len: usize, scale: f32) -> (usize, usize, f32) {
    let columns = if legend_len > 4 { 2 } else { 1 };
    let rows = (legend_len + columns - 1) / columns;
    let row_height = 22. * scale;
    (columns, rows, row_height)
}

fn draw_centered_text(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    font: &str,
    value: &str,
    rect: (f32, f32, f32, f32),
    size: f32,
    text_color: &str,
) {
    draw_text_box(
        pm,
        glyphs,
        value,
        rect,
        &TextStyle {
            family: font.to_string(),
            weight: Weight::Bold,
            size,
            color: color(text_color),
            letter_spacing: 0.,
        },
        Align::Center,
        true,
        true,
        true,
        false,
        1.,
    );
}

fn report_color(scene: &Scene, category: usize, value: Option<f32>) -> String {
    if !scene.legend.is_empty() {
        return scene
            .legend
            .get(category)
            .map(|band| band.color.clone())
            .unwrap_or_else(|| "#D5E1EC".into());
    }
    let Some(value) = value.filter(|v| v.is_finite() && *v >= 0.) else {
        return "#D5E1EC".into();
    };
    let stops = if scene.palette == super::board::Palette::SnowTotal {
        crate::grid::SNOW_TOTAL_STOPS
    } else {
        crate::grid::RAIN_TOTAL_STOPS
    };
    let color = stops
        .iter()
        .rev()
        .find(|(cut, _)| value >= *cut)
        .or_else(|| stops.first())
        .map(|(_, rgb)| *rgb);
    color
        .map(|[r, g, b]| format!("#{r:02X}{g:02X}{b:02X}"))
        .unwrap_or_else(|| "#D5E1EC".into())
}

/// Keep the NWS report-size comparisons used by the existing observed table.
/// The measured diameter stays primary; these words are glossary context only.
fn hail_comparison(diameter_in: f32) -> String {
    let sizes = [
        (0.25, "pea"),
        (0.75, "penny"),
        (1.0, "quarter"),
        (1.75, "golf ball"),
        (2.75, "baseball"),
    ];
    if let Some((_, name)) = sizes
        .iter()
        .find(|(size, _)| (diameter_in - size).abs() < 0.02)
    {
        return format!("About {name} size");
    }
    let below = sizes.iter().rev().find(|(size, _)| diameter_in > *size);
    let above = sizes.iter().find(|(size, _)| diameter_in < *size);
    match (below, above) {
        (Some((_, a)), Some((_, b))) => format!("Between {a} / {b}"),
        (Some((_, a)), None) => format!("Larger than {a}"),
        (None, Some((_, a))) => format!("Smaller than {a}"),
        _ => String::new(),
    }
}

fn point_is_in_scene_scope(
    counties: &[&crate::map::boundaries::County],
    scope: &super::board::Scope,
    lon: f64,
    lat: f64,
) -> bool {
    scope.bbox.is_none_or(|[west, south, east, north]| {
        (west..=east).contains(&lon) && (south..=north).contains(&lat)
    }) && counties
        .iter()
        .any(|county| board::inside(county, lon, lat))
}

/// Build an original transparent over-map rail. Product source and note remain caller-bound.
pub(super) fn template(title: &str, category: &str, prefix: &str) -> GraphicDoc {
    template_fields(
        title,
        category,
        &format!("{{{{set.{prefix}.payload}}}}"),
        &format!("{{{{set.{prefix}.subtitle}}}}"),
        &format!("{{{{set.{prefix}.source}}}}"),
        &format!("{{{{set.{prefix}.note}}}}"),
    )
}

pub(super) fn observed_points_template(title: &str, category: &str, prefix: &str) -> GraphicDoc {
    let mut doc = template_fields(
        title,
        category,
        &format!("{{{{set.{prefix}.payload}}}}"),
        &format!("{{{{set.{prefix}.period}}}}"),
        "{{set.obs.source}}",
        &format!("{{{{set.{prefix}.note}}}}"),
    );
    if let Some(layer) = doc
        .layers
        .iter_mut()
        .find(|layer| layer.name == "Product overlay map and legend")
    {
        if let crate::gfx::model::LayerKind::RiskMap { spec } = &mut layer.kind {
            *spec = spec.replace("set:productoverlay:", "set:reportoverlay:");
        }
    }
    for layer in &mut doc.layers {
        match layer.name.as_str() {
            "Product overlay card" => {
                layer.y = 615.;
                layer.h = 414.;
            }
            "Product overlay accent" => {
                layer.y = 615.;
                layer.h = 414.;
            }
            "Product overlay map and legend" => {
                layer.y = 638.;
                layer.h = 300.;
            }
            "Product overlay station" => layer.y = 622.,
            _ => {}
        }
    }
    doc
}

fn template_fields(
    title: &str,
    category: &str,
    payload: &str,
    subtitle: &str,
    source: &str,
    note: &str,
) -> GraphicDoc {
    let mut doc = GraphicDoc::new(title, category, None);
    doc.layers.push(
        Layer::rect(
            "Product overlay card",
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
            "Product overlay accent",
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
            "Product overlay map and legend",
            82.,
            698.,
            1132.,
            240.,
            &format!("set:productoverlay:{payload}"),
        )
        .with_anim(AnimKind::Fade, 0.18, 0.4),
    );
    super::reference::label(
        &mut doc,
        "Product overlay subtitle",
        (86., 939., 1120., 25.),
        subtitle,
        19.,
        "#C7D4E0",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Product overlay source",
        (86., 964., 1120., 24.),
        source,
        17.,
        "#D5E1EC",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Product overlay caveat",
        (86., 988., 1120., 31.),
        note,
        16.,
        "#B9C9D8",
        Align::Left,
    );
    super::reference::label(
        &mut doc,
        "Product overlay station",
        (1000., 682., 210., 22.),
        "{{station}}",
        16.,
        "$muted",
        Align::Right,
    );
    doc
}

/// Draw category or point overlays without painting their backgrounds.
pub(super) fn draw(
    pm: &mut Pixmap,
    glyphs: &mut GlyphCache,
    local: Transform,
    mask: Option<&tiny_skia::Mask>,
    w: f32,
    h: f32,
    spec: &str,
    opacity: f32,
    font: &str,
) -> bool {
    let (raw, point_mode) = if let Some(raw) = spec.strip_prefix("set:reportoverlay:") {
        (raw, true)
    } else if let Some(raw) = spec.strip_prefix("set:productoverlay:") {
        (raw, false)
    } else {
        return false;
    };
    let Some(scene) = board::parse(raw, Layout::Single) else {
        return true;
    };
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut overlay) = Pixmap::new(dw, dh) else {
        return true;
    };
    if point_mode {
        if let Some(panel) = scene.panels.first() {
            render_points(&mut overlay, glyphs, font, &scene, panel);
        }
    } else {
        render_scene(&mut overlay, glyphs, font, &scene);
    }
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
mod tests {
    use super::*;
    use crate::gfx::sets::board::{Band, Scope};

    #[test]
    fn polygon_overlay_draws_supplied_fill_and_preserves_transparent_corners() {
        let all=crate::map::boundaries::counties_named().expect("county geometry");
        let county=all.iter().find(|c|c.state=="18" && !c.rings.is_empty()).unwrap();
        let b=county.rings.iter().flatten().fold([f64::MAX,f64::MAX,f64::MIN,f64::MIN],|b,p|[b[0].min(p.0),b[1].min(p.1),b[2].max(p.0),b[3].max(p.1)]);
        let mut scene=board::empty(Layout::Single);scene.scope.county=Some(county.fips.clone());
        scene.legend=vec![Band{label:"Sample polygon".into(),color:"#FF0000".into()}];
        scene.panels[0].counties=None;scene.panels[0].zones=Some(vec![board::Zone{category:0,ring:vec![(b[0]-1.,b[1]-1.),(b[2]+1.,b[1]-1.),(b[2]+1.,b[3]+1.),(b[0]-1.,b[3]+1.)],label:String::new(),label_at:None}]);
        let mut pm=Pixmap::new(800,360).unwrap();render_scene(&mut pm,&mut GlyphCache::default(),"sans",&scene);
        assert!(pm.pixels().iter().filter(|p|p.red()==255 && p.green()==0 && p.blue()==0 && p.alpha()==255).count()>1000,"supplied polygon fill must render, not an unavailable map");
        assert_eq!(pm.pixel(0,0).unwrap().alpha(),0);
        assert_eq!(pm.pixel(799,359).unwrap().alpha(),0);
    }

    #[test]
    fn county_overlay_preserves_transparent_underlay() {
        let all = crate::map::boundaries::counties_for_tests();
        let county = all.iter().find(|c| !c.rings.is_empty()).expect("a county");
        let mut scene = board::empty(Layout::Single);
        scene.scope = Scope {
            states: vec![county.state.clone()],
            county: None,
            bbox: None,
        };
        scene.legend = vec![Band {
            label: "Sample category".into(),
            color: "#F69632".into(),
        }];
        scene.panels[0].valid = "SAMPLE • Valid date".into();
        scene.panels[0].counties = Some([(county.fips.clone(), 0)].into_iter().collect());
        let json = serde_json::to_string(&scene).unwrap();
        let mut underlay = Pixmap::new(800, 360).unwrap();
        underlay.fill(Color::from_rgba8(12, 75, 125, 255));
        let original_corner = underlay.data()[0..4].to_vec();
        let mut glyphs = GlyphCache::default();
        assert!(draw(
            &mut underlay,
            &mut glyphs,
            Transform::identity(),
            None,
            800.,
            360.,
            &format!("set:productoverlay:{json}"),
            1.,
            "sans",
        ));
        assert_eq!(
            &underlay.data()[0..4],
            original_corner.as_slice(),
            "outside the map/card remains transparent"
        );
        assert!(
            underlay
                .data()
                .chunks_exact(4)
                .any(|p| p != &[12, 75, 125, 255]),
            "county colors/text composite over the underlying map"
        );
    }

    #[test]
    fn taller_dense_legend_card_is_limited_to_observed_point_templates() {
        let category = template("Category", "Observed", "fixture");
        let points = observed_points_template("Reports", "Observed", "fixture");
        fn layer<'a>(doc: &'a GraphicDoc, name: &str) -> &'a crate::gfx::model::Layer {
            doc.layers.iter().find(|layer| layer.name == name).unwrap()
        }
        assert_eq!(
            (
                layer(&category, "Product overlay card").y,
                layer(&category, "Product overlay card").h
            ),
            (675., 354.)
        );
        assert_eq!(
            (
                layer(&category, "Product overlay map and legend").y,
                layer(&category, "Product overlay map and legend").h
            ),
            (698., 240.)
        );
        assert_eq!(
            (
                layer(&points, "Product overlay card").y,
                layer(&points, "Product overlay card").h
            ),
            (615., 414.)
        );
        assert_eq!(
            (
                layer(&points, "Product overlay map and legend").y,
                layer(&points, "Product overlay map and legend").h
            ),
            (638., 300.)
        );
        assert_eq!(
            layer(&category, "Product overlay source").y,
            layer(&points, "Product overlay source").y
        );
        assert_eq!(
            layer(&category, "Product overlay caveat").y,
            layer(&points, "Product overlay caveat").y
        );
    }

    #[test]
    fn point_colors_keep_the_scene_legend_color_without_an_allowlist() {
        let mut scene = board::empty(Layout::Single);
        scene.legend = vec![Band {
            label: "Custom report band".into(),
            color: "#A12BCD".into(),
        }];
        assert_eq!(report_color(&scene, 0, Some(1.)), "#A12BCD");
    }

    #[test]
    fn point_scope_requires_actual_selected_county_containment() {
        let all = crate::map::boundaries::counties_for_tests();
        let county = all.iter().find(|c| !c.rings.is_empty()).expect("a county");
        let selected = vec![county];
        let ring = county.rings.iter().find(|r| r.len() >= 3).expect("a ring");
        let bounds = ring
            .iter()
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                [b[0].min(p.0), b[1].min(p.1), b[2].max(p.0), b[3].max(p.1)]
            });
        let interior = (1..20)
            .flat_map(|x| (1..20).map(move |y| (x, y)))
            .map(|(x, y)| {
                (
                    bounds[0] + (bounds[2] - bounds[0]) * x as f64 / 20.,
                    bounds[1] + (bounds[3] - bounds[1]) * y as f64 / 20.,
                )
            })
            .find(|(lon, lat)| point_is_in_scene_scope(&selected, &Scope::default(), *lon, *lat))
            .expect("grid point inside county");
        assert!(point_is_in_scene_scope(
            &selected,
            &Scope::default(),
            interior.0,
            interior.1
        ));
        assert!(!point_is_in_scene_scope(
            &selected,
            &Scope::default(),
            0.,
            0.
        ));
        let outside_bbox = Scope {
            bbox: Some([
                interior.0 + 1.,
                interior.1 + 1.,
                interior.0 + 2.,
                interior.1 + 2.,
            ]),
            ..Scope::default()
        };
        assert!(!point_is_in_scene_scope(
            &selected,
            &outside_bbox,
            interior.0,
            interior.1
        ));
    }

    #[test]
    fn all_valid_dots_remain_while_numbered_rail_rows_stay_paired() {
        let all = crate::map::boundaries::counties_for_tests();
        let county = all.iter().find(|c| !c.rings.is_empty()).expect("a county");
        let scope = Scope {
            states: vec![county.state.clone()],
            county: Some(county.fips.clone()),
            bbox: None,
        };
        let selected = vec![county];
        let bounds = county.rings.iter().flatten().fold(
            [f64::MAX, f64::MAX, f64::MIN, f64::MIN],
            |b, &(lon, lat)| [b[0].min(lon), b[1].min(lat), b[2].max(lon), b[3].max(lat)],
        );
        let fit = Fit::new(bounds, 800., 360., 0., 0.).expect("county map fit");
        let interior = (1..20)
            .flat_map(|x| (1..20).map(move |y| (x, y)))
            .map(|(x, y)| {
                (
                    bounds[0] + (bounds[2] - bounds[0]) * x as f64 / 20.,
                    bounds[1] + (bounds[3] - bounds[1]) * y as f64 / 20.,
                )
            })
            .find(|(lon, lat)| {
                let (x, y) = fit.point(*lon, *lat);
                point_is_in_scene_scope(&selected, &scope, *lon, *lat)
                    && x > 12.
                    && x < 788.
                    && y > 12.
                    && y < 348.
            })
            .expect("interior point safely inside map");
        let inside2 = (1..20)
            .flat_map(|x| (1..20).map(move |y| (x, y)))
            .map(|(x, y)| {
                (
                    bounds[0] + (bounds[2] - bounds[0]) * x as f64 / 20.,
                    bounds[1] + (bounds[3] - bounds[1]) * y as f64 / 20.,
                )
            })
            .find(|(lon, lat)| {
                let (x, y) = fit.point(*lon, *lat);
                let (x0, y0) = fit.point(interior.0, interior.1);
                point_is_in_scene_scope(&selected, &scope, *lon, *lat)
                    && x > 12.
                    && x < 788.
                    && y > 12.
                    && y < 348.
                    && (x - x0).powi(2) + (y - y0).powi(2) > 2500.
            })
            .expect("second separated interior point");
        let points = vec![
            board::ReportPoint {
                id: "outside".into(),
                lon: 0.,
                lat: 0.,
                label: "2 in".into(),
                category: 0,
                value: Some(2.),
            },
            board::ReportPoint {
                id: "inside".into(),
                lon: interior.0,
                lat: interior.1,
                label: "1 in".into(),
                category: 0,
                value: Some(1.),
            },
            board::ReportPoint {
                id: "clustered".into(),
                lon: interior.0,
                lat: interior.1,
                label: "0.75 in".into(),
                category: 0,
                value: Some(0.75),
            },
            board::ReportPoint {
                id: "separated".into(),
                lon: inside2.0,
                lat: inside2.1,
                label: "0.5 in".into(),
                category: 0,
                value: Some(0.5),
            },
        ];
        let summaries = vec![
            ("Out-of-scope site".into(), "2 in".into()),
            ("In-scope site".into(), "1 in".into()),
            ("Overlapping site".into(), "0.75 in".into()),
            ("Separated site".into(), "0.5 in".into()),
        ];
        let visible =
            visible_report_points(&points, &selected, &scope, fit, (0., 0., 800., 360.), 1.);
        assert_eq!(
            visible.len(),
            3,
            "valid clustered reports remain plotted as dots"
        );
        let numbered = numbered_report_points(&visible, &summaries, (0., 0., 800., 360.), 1.);
        assert_eq!(
            numbered.len(),
            2,
            "only overlapping number labels are suppressed"
        );
        assert_eq!(
            numbered[0].source_index, 1,
            "first source representative in the cluster wins"
        );
        assert_eq!(summaries[numbered[0].source_index].0, "In-scope site");
        assert_eq!(numbered[1].source_index, 3);
        assert_eq!(summaries[numbered[1].source_index].0, "Separated site");
        assert_eq!(numbered[1].point.value, Some(0.5));
    }

    #[test]
    fn dense_total_legends_leave_room_for_three_numbered_value_rows() {
        for count in [7, 8] {
            let (columns, rows, row_height) = legend_layout(count, 1.);
            assert_eq!(columns, 2);
            assert_eq!(rows, 4);
            let title_y = 32. + rows as f32 * row_height + 4.;
            let value_rows_y = title_y + 19.;
            let available_each = (300. - 26. - value_rows_y) / 3.;
            assert!(
                available_each >= 40.,
                "{count} legend bins leave two readable lines per town"
            );
        }
        assert_eq!(hail_comparison(1.75), "About golf ball size");
        assert_eq!(hail_comparison(1.25), "Between quarter / golf ball");
    }

    #[test]
    fn mapped_count_and_first_500_source_cap_are_disclosed_truthfully() {
        assert_eq!(
            report_map_status(None, 0, 0).as_deref(),
            Some("Data not available yet")
        );
        assert_eq!(
            report_map_status(Some(&[]), 0, 0).as_deref(),
            Some("No reports returned for this view")
        );
        let report = || board::ReportPoint {
            id: "fixture".into(),
            lon: -86.,
            lat: 38.,
            label: "1 in".into(),
            category: 0,
            value: Some(1.),
        };
        let reports = vec![report(); 501];
        assert_eq!(
            report_map_status(Some(&reports), 417, 3).as_deref(),
            Some("Showing 417 mapped reports from first 500 source reports • 3 numbered")
        );
    }

    #[test]
    fn explicit_reportoverlay_mode_draws_unavailable_status_with_legend() {
        let all = crate::map::boundaries::counties_for_tests();
        let county = all.iter().find(|c| !c.rings.is_empty()).expect("a county");
        let mut scene = board::empty(Layout::Single);
        scene.scope = Scope {
            states: vec![county.state.clone()],
            county: Some(county.fips.clone()),
            bbox: None,
        };
        scene.legend_title = "Snow report points".into();
        scene.legend = vec![
            Band {
                label: "1–3 in".into(),
                color: "#01B414".into(),
            },
            Band {
                label: "3–6 in".into(),
                color: "#BF0C08".into(),
            },
        ];
        scene.panels[0].points = None;
        let raw = serde_json::to_string(&scene).unwrap();
        let mut pm = Pixmap::new(800, 360).unwrap();
        let mut glyphs = GlyphCache::default();
        assert!(draw(
            &mut pm,
            &mut glyphs,
            Transform::identity(),
            None,
            800.,
            360.,
            &format!("set:reportoverlay:{raw}"),
            1.,
            "sans"
        ));
        assert!(
            pm.data().chunks_exact(4).any(|px| px[3] != 0),
            "map, legend and unavailable status render on transparent canvas"
        );
    }
}
