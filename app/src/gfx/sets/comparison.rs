//! Parameterised owner-reference layouts and their set-only bindings.
use super::board::Palette;
use super::{
    board::{self, Band, Layout, Scene, Scope, Zone},
    reference::{frame, label},
};
use crate::{
    config::Settings,
    data::WxSnapshot,
    gfx::{
        bind::Bindings,
        model::{AnimKind, GraphicDoc, Layer},
        text::Align,
    },
};

pub(super) const SPECS: &[(&str, &str, &str, &str, Layout)] = &[
    (
        "set_four_panel_timeline__6h_state_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__6h_state_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__6h_multi_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__6h_multi_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_region_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_region_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_state_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_state_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_multi_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_multi_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_region_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_region_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_state_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_state_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_multi_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_multi_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_region_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_region_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_state_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_state_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_multi_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_multi_square",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.multi",
        Layout::Timeline,
    ),
    (
        "set_county_compare__portrait",
        "County Comparison",
        "Comparisons",
        "compare",
        Layout::Compare,
    ),
    (
        "set_county_compare__square",
        "County Comparison",
        "Comparisons",
        "compare",
        Layout::Compare,
    ),
    (
        "set_winter_criteria_compare__portrait",
        "Winter Alert Criteria",
        "Winter",
        "criteria",
        Layout::Compare,
    ),
    (
        "set_winter_criteria_compare__square",
        "Winter Alert Criteria",
        "Winter",
        "criteria",
        Layout::Compare,
    ),
    (
        "set_cold_alert_compare__portrait",
        "Cold Weather Alerts",
        "Winter",
        "cold",
        Layout::Compare,
    ),
    (
        "set_cold_alert_compare__square",
        "Cold Weather Alerts",
        "Winter",
        "cold",
        Layout::Compare,
    ),
    (
        "set_cold_criteria_compare__portrait",
        "Cold Weather Criteria",
        "Winter",
        "coldcriteria",
        Layout::Compare,
    ),
    (
        "set_cold_criteria_compare__square",
        "Cold Weather Criteria",
        "Winter",
        "coldcriteria",
        Layout::Compare,
    ),
    (
        "set_four_panel_timeline__6h_region_portrait",
        "Weather Timeline",
        "Comparisons",
        "timeline",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__6h_region_square",
        "Weather Timeline",
        "Comparisons",
        "timeline",
        Layout::Timeline,
    ),
    (
        "set_forecast_zones__today_region_portrait",
        "Forecast Zones",
        "Winter",
        "zones.today.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_region_square",
        "Forecast Zones",
        "Winter",
        "zones.today.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_state_portrait",
        "Forecast Zones",
        "Winter",
        "zones.today.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_state_square",
        "Forecast Zones",
        "Winter",
        "zones.today.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_county_portrait",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_county_square",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_region_portrait",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_region_square",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_state_portrait",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_state_square",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_county_portrait",
        "Forecast Zones",
        "Winter",
        "zones.day3.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_county_square",
        "Forecast Zones",
        "Winter",
        "zones.day3.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_region_portrait",
        "Forecast Zones",
        "Winter",
        "zones.day3.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_region_square",
        "Forecast Zones",
        "Winter",
        "zones.day3.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_state_portrait",
        "Forecast Zones",
        "Winter",
        "zones.day3.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_state_square",
        "Forecast Zones",
        "Winter",
        "zones.day3.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_county_portrait",
        "Forecast Zones",
        "Winter",
        "zones",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_county_square",
        "Forecast Zones",
        "Winter",
        "zones",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_region",
        "Forecast Zones",
        "Winter",
        "zones.today.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__today_state",
        "Forecast Zones",
        "Winter",
        "zones.today.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_county",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_region",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__tomorrow_state",
        "Forecast Zones",
        "Winter",
        "zones.tomorrow.state",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_county",
        "Forecast Zones",
        "Winter",
        "zones.day3.county",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_region",
        "Forecast Zones",
        "Winter",
        "zones.day3.region",
        Layout::Zones,
    ),
    (
        "set_forecast_zones__day3_state",
        "Forecast Zones",
        "Winter",
        "zones.day3.state",
        Layout::Zones,
    ),
    (
        "set_four_panel_timeline__6h_state",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__6h_multi",
        "Weather Timeline",
        "Comparisons",
        "timeline.6h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_region",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_state",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__12h_multi",
        "Weather Timeline",
        "Comparisons",
        "timeline.12h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_region",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_state",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__24h_multi",
        "Weather Timeline",
        "Comparisons",
        "timeline.24h.multi",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_region",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.region",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_state",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.state",
        Layout::Timeline,
    ),
    (
        "set_four_panel_timeline__48h_multi",
        "Weather Timeline",
        "Comparisons",
        "timeline.48h.multi",
        Layout::Timeline,
    ),
    (
        "set_county_compare",
        "County Comparison",
        "Comparisons",
        "compare",
        Layout::Compare,
    ),
    (
        "set_winter_criteria_compare",
        "Winter Alert Criteria",
        "Winter",
        "criteria",
        Layout::Compare,
    ),
    (
        "set_cold_alert_compare",
        "Cold Weather Alerts",
        "Winter",
        "cold",
        Layout::Compare,
    ),
    (
        "set_cold_criteria_compare",
        "Cold Weather Criteria",
        "Winter",
        "coldcriteria",
        Layout::Compare,
    ),
    (
        "set_four_panel_timeline",
        "Weather Timeline",
        "Comparisons",
        "timeline",
        Layout::Timeline,
    ),
    (
        "set_forecast_zones",
        "Forecast Zones",
        "Winter",
        "zones",
        Layout::Zones,
    ),
];

fn timeline_options(prefix: &str) -> Option<(i64, super::observed::Coverage)> {
    use super::observed::Coverage;
    if prefix == "timeline" {
        return Some((6, Coverage::Region));
    }
    let (hours, area) = prefix.strip_prefix("timeline.")?.split_once('.')?;
    let hours = hours
        .strip_suffix('h')?
        .parse::<i64>()
        .ok()
        .filter(|v| [6, 12, 24, 48].contains(v))?;
    let area = match area {
        "region" => Coverage::Region,
        "state" => Coverage::State,
        "multi" => Coverage::Multi,
        _ => return None,
    };
    Some((hours, area))
}

fn zone_options(prefix: &str) -> Option<(i64, super::observed::Coverage)> {
    use super::observed::Coverage;
    if prefix == "zones" {
        return Some((0, Coverage::County));
    }
    let (day, area) = prefix.strip_prefix("zones.")?.split_once('.')?;
    let day = match day {
        "today" => 0,
        "tomorrow" => 1,
        "day3" => 2,
        _ => return None,
    };
    let area = match area {
        "county" => Coverage::County,
        "region" => Coverage::Region,
        "state" => Coverage::State,
        _ => return None,
    };
    Some((day, area))
}
/// Distinct category colours for forecaster regions (legend order).
const REGION_COLORS: [&str; 8] = [
    "#38BDF8", "#F59E0B", "#22C55E", "#E879F9", "#F87171", "#A78BFA", "#FACC15", "#2DD4BF",
];

/// A forecaster region as a closed (lon, lat) ring, the order Winter Studio's zones use.
/// Circles become a 48-point polygon.
pub(super) fn region_ring(shape: &crate::forecast_edit::Shape) -> Vec<(f64, f64)> {
    use crate::forecast_edit::Shape;
    match shape {
        Shape::Circle { lat, lon, radius_mi } => {
            let r = radius_mi.max(0.5);
            let dlat = r / 69.0;
            let dlon = dlat / lat.to_radians().cos().max(0.05);
            (0..48)
                .map(|i| {
                    let a = i as f64 / 48.0 * std::f64::consts::TAU;
                    (lon + dlon * a.cos(), lat + dlat * a.sin())
                })
                .collect()
        }
        Shape::Polygon(pts) => pts.iter().map(|&(la, lo)| (lo, la)).collect(),
    }
}

fn signed(v: f32) -> String {
    let n = v.round() as i32;
    if n > 0 { format!("+{n}°") } else { format!("{n}°") }
}

/// Fill a Forecast Zones scene from the forecaster's published regions (Map Studio): one
/// reviewed zone and one legend entry per region (up to eight). Returns false with no
/// usable regions. Winter Studio's own zones still win: they arrive as document data
/// overrides, applied after these bindings.
pub(super) fn forecaster_zones(scene: &mut Scene, regions: &[crate::forecast_edit::Region]) -> bool {
    let mut legend = Vec::new();
    let mut zones = Vec::new();
    for region in regions.iter() {
        let ring = region_ring(&region.shape);
        let valid = ring.len() >= 3
            && ring.iter().all(|&(x, y)| x.is_finite() && y.is_finite() && (-180.0..=180.0).contains(&x) && (-90.0..=90.0).contains(&y));
        if !valid || legend.len() >= REGION_COLORS.len() {
            continue;
        }
        let name: String = match region.name.trim() {
            "" => format!("Region {}", legend.len() + 1),
            n => n.to_string(),
        };
        let mut label = name.clone();
        if region.hi_delta.round() != 0.0 || region.lo_delta.round() != 0.0 {
            label = format!("{name}: highs {}, lows {}", signed(region.hi_delta), signed(region.lo_delta));
        }
        if label.chars().count() > 55 {
            label = name.chars().take(55).collect();
        }
        let category = legend.len();
        legend.push(Band { label, color: REGION_COLORS[category].into() });
        zones.push(Zone { category, ring, label: name.chars().take(60).collect(), label_at: None });
    }
    if zones.is_empty() {
        return false;
    }
    scene.reviewed = true;
    scene.palette = Palette::Category;
    scene.legend_title = "Forecast regions".into();
    scene.legend = legend;
    if let Some(p) = scene.panels.first_mut() {
        p.title = "Forecaster forecast regions".into();
        p.zones = Some(zones);
    }
    true
}

fn area_label(coverage: super::observed::Coverage) -> &'static str {
    use super::observed::Coverage;
    match coverage {
        Coverage::County => "County",
        Coverage::Region => "Region",
        Coverage::State => "State",
        Coverage::Multi => "Multi-state",
    }
}

pub(super) fn build(key: &str) -> Option<GraphicDoc> {
    let (_, title, category, prefix, layout) = SPECS.iter().find(|s| s.0 == key)?;
    if *layout == Layout::Zones {
        if key.ends_with("_portrait") {
            return Some(portrait_template(title, category, prefix));
        }
        if key.ends_with("_square") {
            return Some(square_template(title, category, prefix));
        }
    }
    if key.ends_with("_portrait") || key.ends_with("_square") {
        return Some(multi_template(
            title,
            category,
            prefix,
            *layout,
            key.ends_with("_portrait"),
        ));
    }
    Some(template(title, category, prefix, *layout))
}

pub(super) fn template(title: &str, category: &str, prefix: &str, layout: Layout) -> GraphicDoc {
    let mut d = frame(
        title,
        category,
        &format!("{{{{set.{prefix}.subtitle}}}}"),
        &format!("{{{{set.{prefix}.source}}}}"),
    );
    d.layers.push(
        Layer::risk_map(
            "Shared map board",
            70.,
            205.,
            1780.,
            715.,
            &format!("set:board:{}:{{{{set.{prefix}.payload}}}}", layout.key()),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.6),
    );
    label(
        &mut d,
        "Map context",
        (80., 925., 1760., 42.),
        &format!("{{{{set.{prefix}.note}}}}"),
        25.,
        "$muted",
        Align::Left,
    );
    d
}

/// Native vertical single-map layout, sharing data prefixes with its broadcast variant.
pub(super) fn portrait_template(title: &str, category: &str, prefix: &str) -> GraphicDoc {
    let mut d = super::reference::social_frame(
        title,
        category,
        &format!("{{{{set.{prefix}.subtitle}}}}"),
        &format!("{{{{set.{prefix}.source}}}}"),
        true,
    );
    d.layers.push(
        Layer::risk_map(
            "Portrait map board",
            54.,
            242.,
            972.,
            1400.,
            &format!("set:board:single:{{{{set.{prefix}.payload}}}}"),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.6),
    );
    label(
        &mut d,
        "Map context",
        (54., 1656., 972., 68.),
        &format!("{{{{set.{prefix}.note}}}}"),
        26.,
        "$muted",
        Align::Left,
    );
    d
}

pub(super) fn square_template(title: &str, category: &str, prefix: &str) -> GraphicDoc {
    let mut d = super::reference::social_frame(
        title,
        category,
        &format!("{{{{set.{prefix}.subtitle}}}}"),
        &format!("{{{{set.{prefix}.source}}}}"),
        false,
    );
    d.layers.push(
        Layer::risk_map(
            "Square map board",
            54.,
            242.,
            972.,
            650.,
            &format!("set:board:single:{{{{set.{prefix}.payload}}}}"),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.6),
    );
    label(
        &mut d,
        "Map context",
        (54., 902., 972., 64.),
        &format!("{{{{set.{prefix}.note}}}}"),
        25.,
        "$muted",
        Align::Left,
    );
    d
}

fn multi_template(
    title: &str,
    category: &str,
    prefix: &str,
    layout: Layout,
    portrait: bool,
) -> GraphicDoc {
    let mut d = super::reference::social_frame(
        title,
        category,
        &format!("{{{{set.{prefix}.subtitle}}}}"),
        &format!("{{{{set.{prefix}.source}}}}"),
        portrait,
    );
    d.layers.push(
        Layer::risk_map(
            "Social comparison board",
            54.,
            242.,
            972.,
            if portrait { 1400. } else { 650. },
            &format!("set:board:{}:{{{{set.{prefix}.payload}}}}", layout.key()),
        )
        .with_anim(AnimKind::Fade, 0.2, 0.6),
    );
    label(
        &mut d,
        "Map context",
        if portrait {
            (54., 1656., 972., 68.)
        } else {
            (54., 902., 972., 64.)
        },
        &format!("{{{{set.{prefix}.note}}}}"),
        25.,
        "$muted",
        Align::Left,
    );
    d
}

fn home_county(settings: &Settings) -> Option<(String, String)> {
    type Entry = ((u64, u64), (String, String));
    static CACHE: std::sync::OnceLock<std::sync::RwLock<Vec<Entry>>> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (settings.home.lon.to_bits(), settings.home.lat.to_bits());
    if let Some(value) = cache
        .read()
        .ok()
        .and_then(|v| v.iter().find(|v| v.0 == key).map(|v| v.1.clone()))
    {
        return Some(value);
    }
    let value = crate::map::boundaries::counties_named().and_then(|v| {
        v.iter()
            .find(|c| {
                c.rings.iter().fold(false, |v, r| {
                    v ^ crate::gfx::winter_map::inside_pub(r, settings.home.lon, settings.home.lat)
                })
            })
            .map(|c| (c.state.clone(), c.fips.clone()))
    });
    if let Some(value) = &value {
        if let Ok(mut v) = cache.write() {
            if v.len() >= 64 {
                v.remove(0);
            }
            v.push((key, value.clone()));
        }
    }
    value
}

pub(super) fn scope(settings: &Settings, layout: Layout) -> Scope {
    let county = home_county(settings);
    let states = county.as_ref().map(|v| v.0.clone()).into_iter().collect();
    Scope {
        states,
        county: if layout == Layout::Zones {
            county.map(|v| v.1)
        } else {
            None
        },
        bbox: if layout == Layout::Timeline {
            Some([
                settings.home.lon - 2.0,
                settings.home.lat - 1.2,
                settings.home.lon + 2.0,
                settings.home.lat + 1.2,
            ])
        } else {
            None
        },
    }
}

pub(super) fn bind(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    for (key, _, _, prefix, layout) in SPECS {
        if key.ends_with("_portrait") || key.ends_with("_square") {
            continue;
        }
        let mut scene = board::empty(*layout);
        scene.scope = scope(settings, *layout);
        if let Some((_, coverage)) = timeline_options(prefix) {
            scene.scope = super::observed::report_scope(settings, coverage);
        }
        if let Some((_, coverage)) = zone_options(prefix) {
            scene.scope = super::observed::report_scope(settings, coverage);
        }
        if *prefix == "coldcriteria" {
            scene.scope = scope(settings, Layout::Zones);
        }
        match *prefix {
            "criteria" | "coldcriteria" => {
                scene.panels[0].title = "Advisory criteria".into();
                scene.panels[1].title = "Warning criteria".into();
                if *prefix == "coldcriteria" {
                    scene.panels[0].title = "Cold Weather Advisory".into();
                    scene.panels[1].title = "Extreme Cold Warning".into();
                }
                for p in &mut scene.panels {
                    p.detail = "Local NWS office thresholds required".into();
                }
            }
            "cold" => {
                scene.panels[0].title = "Cold Weather Advisory".into();
                scene.panels[1].title = "Extreme Cold Warning".into();
            }
            "zones" => scene.panels[0].title = "Forecaster-drawn winter zones".into(),
            _ => {}
        }
        b.set(&format!("set.{prefix}.subtitle"), &settings.home_label);
        b.set(
            &format!("set.{prefix}.source"),
            "Source: NWS • Awaiting reviewed data",
        );
        if let Some((day, coverage)) = zone_options(prefix) {
            let date = chrono::Local::now().date_naive() + chrono::Duration::days(day);
            let period = match day {
                0 => "Today",
                1 => "Tomorrow",
                _ => "Day 3",
            };
            scene.panels[0].title = "Forecaster-drawn winter zones".into();
            scene.panels[0].valid = format!("Target date: {}", date.format("%b %-d, %Y"));
            b.set(
                &format!("set.{prefix}.subtitle"),
                &format!(
                    "{} • {period} • {}",
                    settings.home_label,
                    area_label(coverage)
                ),
            );
            b.set(
                &format!("set.{prefix}.source"),
                "Source: Forecaster analysis • Reviewed zones required",
            );
            // The forecaster's published regions (Map Studio) are reviewed forecast zones.
            if forecaster_zones(&mut scene, &data.forecast_edits.regions) {
                b.set(&format!("set.{prefix}.source"), "Source: Forecaster analysis");
            }
        } else if *prefix == "compare" {
            b.set(
                "set.compare.source",
                "Reviewed county field • Source attribution required",
            );
        }
        b.set(
            &format!("set.{prefix}.note"),
            match *prefix {
                "criteria" | "coldcriteria" => {
                    "Thresholds vary by NWS office. No advisory or warning threshold is assumed."
                }
                "timeline" => "One geographic view and one legend for every valid time.",
                "zones" => {
                    "Forecast zones express potential; conditions can vary within each zone."
                }
                _ => "Both panels use the same geographic view and shared legend.",
            },
        );
        if zone_options(prefix).is_some() {
            b.set(
                &format!("set.{prefix}.note"),
                "Forecast zones express potential; conditions can vary within each zone.",
            );
        }
        if *prefix == "cold" {
            scene = cold_scene(data, scene);
            b.set("set.cold.source", "Source: National Weather Service");
            b.set(
                "set.cold.note",
                "Available NWS alerts • Alert areas shown; no temperature thresholds inferred.",
            );
        }
        if let Some((hours, coverage)) = timeline_options(prefix) {
            scene = temperature_timeline(data, scene, hours);
            let area = match coverage {
                super::observed::Coverage::State => "State",
                super::observed::Coverage::Multi => "Multi-state",
                _ => "Region",
            };
            b.set(
                &format!("set.{prefix}.subtitle"),
                &format!("{} • Next {hours}h • {area}", settings.home_label),
            );
            b.set(
                &format!("set.{prefix}.source"),
                "Source: NOAA GFS • Temperature forecast",
            );
            b.set(&format!("set.{prefix}.note"), "County-centre forecast temperatures • Unfilled counties have no available grid value.");
        }
        if *prefix == "coldcriteria" && published_cold_criteria(&mut scene) {
            b.set(
                "set.coldcriteria.source",
                "Source: NWS Louisville • Published 2024 criteria",
            );
            b.set("set.coldcriteria.note", "Reference criteria for the selected county • Apparent temperature • Current NWS alerts govern; thresholds vary by office.");
        }
        b.set(
            &format!("set.{prefix}.payload"),
            &serde_json::to_string(&scene).unwrap_or_default(),
        );
    }
}

// Published reference thresholds, not weather observations or computed alerts.
// Verified 2026-11-13 against the 2024 LMK criteria sheet and LMK county warning area.
// https://www.weather.gov/media/lmk/pdf/LMK_Warning_Advisory_Criteria_2018_public.pdf
// https://www.weather.gov/lmk/lmkwarningarea
fn published_cold_criteria(scene: &mut Scene) -> bool {
    let Some(fips) = scene.scope.county.as_ref().filter(|f| {
        [
            "18019", "18025", "18037", "18043", "18061", "18077", "18117", "18123", "18143",
            "18175",
        ]
        .contains(&f.as_str())
    }) else {
        return false;
    };
    scene.legend_title =
        "Published apparent-temperature criteria • °F • NWS Louisville Indiana counties only"
            .into();
    scene.legend = vec![
        Band {
            label: "Advisory: -5°F to -15°F".into(),
            color: "#55D5F9".into(),
        },
        Band {
            label: "Warning: -15°F or colder".into(),
            color: "#8A5AFF".into(),
        },
    ];
    for (i, p) in scene.panels.iter_mut().enumerate() {
        p.title = if i == 0 {
            "Cold Weather Advisory"
        } else {
            "Extreme Cold Warning"
        }
        .into();
        p.headline = if i == 0 {
            "-5°F to -15°F"
        } else {
            "-15°F or colder"
        }
        .into();
        p.valid = "NWS Louisville • Published 2024".into();
        p.detail = "Apparent temperature • Reference criteria".into();
        p.counties = Some([(fips.clone(), i)].into_iter().collect());
    }
    scene.reviewed = true;
    true
}

// Only the existing NOAA GFS snapshot is eligible. Never consume the other grid
// providers, extrapolate outside the model extent, or relabel clamped times.
fn temperature_timeline(data: &WxSnapshot, mut scene: Scene, hours: i64) -> Scene {
    scene.legend_title = "Forecast temperature • °F".into();
    scene.legend = [
        ("Below 0", "#8454DC"),
        ("0–19", "#425BDE"),
        ("20–31", "#46B9E4"),
        ("32–49", "#55CBAD"),
        ("50–69", "#EBD64C"),
        ("70–89", "#F49A3B"),
        ("90+", "#EC4A54"),
    ]
    .into_iter()
    .map(|(label, color)| Band {
        label: label.into(),
        color: color.into(),
    })
    .collect();
    let Some(grid) = data.grid.as_ref().filter(|g| {
        g.model == crate::grid::GridModel::Gfs
            && g.model.is_noaa()
            && g.nx >= 2
            && g.ny >= 2
            && g.times.len() >= 2
            && g.times.windows(2).all(|p| p[1] - p[0] == 3600)
            && g.north > g.south
            && g.east > g.west
    }) else {
        return scene;
    };
    let Some(all) = crate::map::boundaries::counties_named() else {
        return scene;
    };
    let counties = board::selected(&all, &scene.scope);
    let now = chrono::Utc::now().timestamp();
    let start = now;
    for (i, p) in scene.panels.iter_mut().enumerate() {
        let offset = i as i64 * hours / 3;
        let valid = start + offset * 3600;
        p.title = format!("Temperature • +{offset}h");
        // Source and sampling context are printed once below the board. Keep each
        // frame's full valid timestamp readable in the narrower state columns.
        p.detail.clear();
        if valid < grid.times[0] || valid > *grid.times.last().unwrap() {
            continue;
        }
        p.valid = chrono::DateTime::from_timestamp(valid, 0)
            .unwrap()
            .format("%b %-d, %Y %H:%M UTC")
            .to_string();
        let tf = (valid - grid.times[0]) as f32 / 3600.;
        let mut values = std::collections::BTreeMap::new();
        for c in &counties {
            let (lon, lat) = board::centre(c);
            if lon < grid.west || lon > grid.east || lat < grid.south || lat > grid.north {
                continue;
            }
            if let Some(t) = grid
                .sample(crate::grid::Var::Temp, tf, lat, lon)
                .filter(|t| t.is_finite() && (-150. ..=150.).contains(t))
            {
                let index = [0., 20., 32., 50., 70., 90.]
                    .iter()
                    .filter(|v| t >= **v)
                    .count();
                values.insert(c.fips.clone(), index);
            }
        }
        if !values.is_empty() {
            p.counties = Some(values);
            scene.reviewed = true;
        }
    }
    scene
}

fn cold_scene(data: &WxSnapshot, mut scene: Scene) -> Scene {
    let now = chrono::Utc::now();
    scene.legend_title = "NWS cold alert category".into();
    scene.legend = vec![
        Band {
            label: "Cold Weather Advisory".into(),
            color: "#55D5F9".into(),
        },
        Band {
            label: "Extreme Cold Warning".into(),
            color: "#8A5AFF".into(),
        },
    ];
    let mut found = false;
    let all = crate::map::boundaries::counties_named();
    let counties = all
        .as_ref()
        .map(|v| board::selected(v, &scene.scope))
        .unwrap_or_default();
    let mut next_expiry: [Option<i64>; 2] = [None, None];
    for a in super::winter_alerts::active(data, now.timestamp()) {
        let index = match a.event.as_str() {
            "Cold Weather Advisory" => 0,
            "Extreme Cold Warning" => 1,
            _ => continue,
        };
        let Some(expires) = a
            .expires
            .as_ref()
            .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        else {
            continue;
        };
        if expires.with_timezone(&chrono::Utc) <= now {
            continue;
        }
        if a.fips.is_empty() && a.polygons.is_empty() {
            continue;
        }
        if !counties.iter().any(|c| {
            a.fips.iter().any(|f| f == &c.fips)
                || a.polygons.iter().any(|ring| {
                    let (x, y) = board::centre(c);
                    crate::gfx::winter_map::inside_pub(ring, x, y)
                        || ring.iter().any(|&(x, y)| board::inside(c, x, y))
                })
        }) {
            continue;
        }
        found = true;
        next_expiry[index] = Some(
            next_expiry[index]
                .map(|v| v.min(expires.timestamp()))
                .unwrap_or(expires.timestamp()),
        );
        let p = &mut scene.panels[index];
        p.detail = if a.sender.trim().is_empty() {
            "NWS alert areas".into()
        } else {
            a.sender.clone()
        };
        let values = p.counties.get_or_insert_with(Default::default);
        for f in &a.fips {
            if f.len() == 5 && f.bytes().all(|v| v.is_ascii_digit()) {
                values.insert(f.clone(), index);
            }
        }
        let zones = p.zones.get_or_insert_with(Default::default);
        for ring in &a.polygons {
            if ring.len() >= 3 {
                zones.push(Zone {
                    category: index,
                    ring: ring.clone(),
                    label: String::new(),
                    label_at: None,
                });
            }
        }
    }
    for (p, t) in scene.panels.iter_mut().zip(next_expiry) {
        if let Some(t) = t.and_then(|v| chrono::DateTime::from_timestamp(v, 0)) {
            p.valid = format!("Next expiry {}", t.format("%b %-d %H:%M UTC"));
        }
    }
    // A missing feed never becomes "no alerts". Do not invent the absent panel's coverage.
    scene.reviewed = found;
    scene
}

#[cfg(test)]
pub(super) fn sample(doc: &mut GraphicDoc) {
    let Some((_, _, _, prefix, layout)) =
        SPECS.iter().find(|s| Some(s.0) == doc.template.as_deref())
    else {
        return;
    };
    let mut scene = board::empty(*layout);
    if *prefix == "coldcriteria" {
        scene.scope = scope(&Settings::default(), Layout::Zones);
    }
    scene.reviewed = true;
    scene.legend_title = "SAMPLE • shared categories".into();
    scene.legend = vec![
        Band {
            label: "Lower range".into(),
            color: "#56D3EF".into(),
        },
        Band {
            label: "Middle range".into(),
            color: "#4A88F0".into(),
        },
        Band {
            label: "Upper range".into(),
            color: "#8D5BEE".into(),
        },
    ];
    if *layout == Layout::Timeline {
        scene.scope = super::observed::report_scope(
            &Settings::default(),
            timeline_options(prefix).unwrap().1,
        );
        scene.palette = Palette::SnowTotal;
        scene.legend_title = "SAMPLE snowfall forecast • inches • TWC palette 4".into();
    }
    if matches!(*prefix, "cold" | "coldcriteria") {
        scene.panels[0].title = "Cold Weather Advisory".into();
        scene.panels[1].title = "Extreme Cold Warning".into();
        scene.legend = vec![
            Band {
                label: if *prefix == "coldcriteria" {
                    "-10°F • SAMPLE threshold"
                } else {
                    "Advisory"
                }
                .into(),
                color: "#55D5F9".into(),
            },
            Band {
                label: if *prefix == "coldcriteria" {
                    "-25°F • SAMPLE threshold"
                } else {
                    "Warning"
                }
                .into(),
                color: "#8A5AFF".into(),
            },
        ];
    }
    if matches!(*prefix, "criteria" | "coldcriteria") {
        scene.panels[0].title = "Advisory threshold".into();
        scene.panels[1].title = "Warning threshold".into();
        for p in &mut scene.panels {
            p.detail = "SAMPLE criteria • not operational thresholds".into();
        }
        if *prefix == "coldcriteria" {
            scene.panels[0].title = "Cold Weather Advisory".into();
            scene.panels[1].title = "Extreme Cold Warning".into();
            scene.panels[0].detail = "SAMPLE threshold: wind chill -10°F • not operational".into();
            scene.panels[1].detail = "SAMPLE threshold: wind chill -25°F • not operational".into();
        }
    }
    let all = crate::map::boundaries::counties_named();
    let band_count = board::bands(&scene).len();
    for (i, p) in scene.panels.iter_mut().enumerate() {
        let time = chrono::NaiveDate::from_ymd_opt(2026, 11, 13)
            .unwrap()
            .and_hms_opt(18, 0, 0)
            .unwrap()
            .and_utc()
            + chrono::Duration::hours(
                i as i64 * timeline_options(prefix).map(|v| v.0 / 3).unwrap_or(2),
            )
            + chrono::Duration::days(zone_options(prefix).map(|v| v.0).unwrap_or(0));
        p.valid = format!("SAMPLE • {}", time.format("%b %-d, %Y %H:%M UTC"));
        if *layout == Layout::Timeline {
            p.title = format!("Forecast frame {}", i + 1);
        }
        if let Some(all) = &all {
            p.counties = Some(
                all.iter()
                    .filter(|c| c.state == "18" || scene.scope.bbox.is_some())
                    .enumerate()
                    .filter(|(j, _)| (j + i) % 4 != 0)
                    .map(|(j, c)| {
                        (
                            c.fips.clone(),
                            if matches!(*prefix, "cold" | "coldcriteria") {
                                i
                            } else {
                                (j + i) % band_count
                            },
                        )
                    })
                    .collect(),
            );
        }
    }
    if *layout == Layout::Zones {
        scene.scope =
            super::observed::report_scope(&Settings::default(), zone_options(prefix).unwrap().1);
        scene.legend_title = "SAMPLE • forecaster-drawn potential".into();
        let a = crate::grid::SNOW_TOTAL_STOPS[0].1;
        let b = crate::grid::SNOW_TOTAL_STOPS[3].1;
        scene.legend = vec![
            Band {
                label: "Flakes possible".into(),
                color: format!("#{:02X}{:02X}{:02X}", a[0], a[1], a[2]),
            },
            Band {
                label: "Accumulation possible".into(),
                color: format!("#{:02X}{:02X}{:02X}", b[0], b[1], b[2]),
            },
        ];
        let bbox = scene.scope.bbox.or_else(|| {
            all.as_ref().and_then(|all| {
                let counties = board::selected(all, &scene.scope);
                if counties.is_empty() {
                    return None;
                }
                Some(
                    counties
                        .iter()
                        .flat_map(|c| c.rings.iter().flatten())
                        .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, &(x, y)| {
                            [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
                        }),
                )
            })
        });
        if let Some([w, s, e, n]) = bbox {
            let mid = (s + n) / 2.;
            let x = (w + e) / 2.;
            scene.panels[0].counties = None;
            scene.panels[0].title = format!(
                "{} forecast zones • SAMPLE",
                area_label(zone_options(prefix).unwrap().1)
            );
            scene.panels[0].zones = Some(vec![
                Zone {
                    category: 0,
                    ring: vec![
                        (w - 0.1, mid),
                        (x - 0.05, mid + 0.02),
                        (x + 0.07, mid - 0.02),
                        (e + 0.1, mid + 0.01),
                        (e + 0.1, n + 0.1),
                        (w - 0.1, n + 0.1),
                    ],
                    label: "Flakes possible".into(),
                    label_at: Some((x, (mid + n) / 2.)),
                },
                Zone {
                    category: 1,
                    ring: vec![
                        (w - 0.1, s - 0.1),
                        (e + 0.1, s - 0.1),
                        (e + 0.1, mid + 0.01),
                        (x + 0.07, mid - 0.02),
                        (x - 0.05, mid + 0.02),
                        (w - 0.1, mid),
                    ],
                    label: "Accumulation possible".into(),
                    label_at: Some((x, (s + mid) / 2.)),
                },
            ]);
        }
    }
    if *prefix == "coldcriteria" {
        published_cold_criteria(&mut scene);
    }
    doc.data_overrides.push((
        format!("set.{prefix}.payload"),
        serde_json::to_string(&scene).unwrap(),
    ));
    for (field, value) in [
        ("source", "SAMPLE ONLY • Not for broadcast"),
        ("subtitle", "SAMPLE • layout verification"),
        (
            "note",
            "SAMPLE • Reviewed layout fixture; values and zones are synthetic.",
        ),
    ] {
        doc.data_overrides
            .push((format!("set.{prefix}.{field}"), value.into()));
    }
    if let Some((hours, coverage)) = timeline_options(prefix) {
        let area = match coverage {
            super::observed::Coverage::State => "State",
            super::observed::Coverage::Multi => "Multi-state",
            _ => "Region",
        };
        doc.data_overrides.push((
            format!("set.{prefix}.subtitle"),
            format!("SAMPLE • Next {hours}h • {area}"),
        ));
    }
    if let Some((day, coverage)) = zone_options(prefix) {
        doc.data_overrides.push((
            format!("set.{prefix}.subtitle"),
            format!(
                "SAMPLE • {} • {}",
                match day {
                    0 => "Today",
                    1 => "Tomorrow",
                    _ => "Day 3",
                },
                area_label(coverage)
            ),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Forecaster regions (Map Studio) fill every Forecast Zones prefix as reviewed zones,
    /// one legend entry per region, with (lon, lat) rings like Winter Studio's zones.
    #[test]
    fn forecaster_regions_fill_every_forecast_zones_prefix() {
        use crate::forecast_edit::{Region, Shape};
        let settings = Settings::default();
        let mut data = WxSnapshot::default();
        let zone_specs: Vec<_> = SPECS
            .iter()
            .filter(|s| zone_options(s.3).is_some() && !s.0.ends_with("_portrait") && !s.0.ends_with("_square"))
            .collect();
        assert!(zone_specs.len() >= 9, "today/tomorrow/day3 × county/region/state");

        let mut b = Bindings::default();
        bind(&mut b, &data, &settings);
        for (_, _, _, prefix, layout) in &zone_specs {
            let scene = board::parse(b.get(&format!("set.{prefix}.payload")).unwrap(), *layout).unwrap();
            assert!(!scene.reviewed, "{prefix}: no regions = placeholder");
        }

        data.forecast_edits.regions = vec![
            Region { name: "Hills".into(), shape: Shape::Circle { lat: 38.4, lon: -86.9, radius_mi: 12.0 }, hi_delta: -3.0, lo_delta: 2.0, note: String::new() },
            Region { name: "River valley".into(), shape: Shape::Polygon(vec![(38.0, -87.0), (38.0, -86.5), (38.3, -86.5)]), hi_delta: 0.0, lo_delta: 0.0, note: String::new() },
        ];
        let mut b = Bindings::default();
        bind(&mut b, &data, &settings);
        for (_, _, _, prefix, layout) in &zone_specs {
            let scene = board::parse(b.get(&format!("set.{prefix}.payload")).unwrap(), *layout).unwrap();
            assert!(scene.reviewed, "{prefix}");
            assert_eq!(scene.legend.len(), 2);
            assert_eq!(scene.legend[0].label, "Hills: highs -3°, lows +2°");
            assert_eq!(scene.legend[1].label, "River valley");
            let zones = scene.panels[0].zones.as_ref().unwrap();
            assert_eq!(zones.len(), 2);
            assert_eq!(zones[0].ring.len(), 48);
            assert_eq!(zones[1].ring[0], (-87.0, 38.0), "lon, lat order");
            assert_eq!(b.get(&format!("set.{prefix}.source")), Some("Source: Forecaster analysis"));
        }
    }

    /// `DCWX_OUT=<dir>`: Forecast Zones county/region/state with forecaster regions, both looks.
    #[test]
    #[ignore]
    fn render_forecaster_zone_samples() {
        use crate::forecast_edit::{Region, Shape};
        let Ok(dir) = std::env::var("DCWX_OUT") else { return };
        let mut settings = Settings::default();
        settings.home_label = "Dubois County, IN".into();
        settings.home.lat = 38.3914;
        settings.home.lon = -86.9311;
        let mut data = WxSnapshot::default();
        data.forecast_edits.regions = vec![
            Region { name: "Patoka hills".into(), shape: Shape::Circle { lat: 38.45, lon: -86.75, radius_mi: 9.0 }, hi_delta: -3.0, lo_delta: -2.0, note: String::new() },
            Region { name: "Ohio valley".into(), shape: Shape::Polygon(vec![(38.05, -87.2), (38.2, -86.95), (38.15, -86.6), (37.95, -86.55), (37.85, -86.95)]), hi_delta: 2.0, lo_delta: 3.0, note: String::new() },
            Region { name: "Jasper".into(), shape: Shape::Circle { lat: 38.39, lon: -86.93, radius_mi: 4.0 }, hi_delta: 0.0, lo_delta: 0.0, note: String::new() },
        ];
        for scope in ["county", "region", "state"] {
            for look in ["modern", "classic"] {
                let key = if scope == "county" { "set_forecast_zones".to_string() } else { format!("set_forecast_zones__today_{scope}") };
                let mut doc = super::super::build(&key).unwrap();
                crate::gfx::look::apply(&mut doc, look);
                let px = crate::program::render_graphic_mode(&doc, &settings, &data, doc.intro_length() + 1.0, 1920, 1080, None, crate::gfx::check::PreviewData::Live);
                px.save_png(std::path::Path::new(&dir).join(format!("zones-{scope}-{look}.png"))).unwrap();
            }
        }
    }

    #[test]
    fn timeline_parameters_preserve_period_scope_and_exact_end_frame() {
        for (key, _, _, prefix, layout) in SPECS.iter().filter(|s| s.4 == Layout::Timeline) {
            let (hours, coverage) = timeline_options(prefix).unwrap();
            let mut doc = super::super::build(key).unwrap();
            sample(&mut doc);
            let payload = doc
                .data_overrides
                .iter()
                .find(|(k, _)| k == &format!("set.{prefix}.payload"))
                .unwrap();
            let scene = board::parse(&payload.1, Layout::Timeline).unwrap();
            let time = |p: &board::Panel| {
                chrono::NaiveDateTime::parse_from_str(
                    p.valid.strip_prefix("SAMPLE • ").unwrap(),
                    "%b %e, %Y %H:%M UTC",
                )
                .unwrap()
            };
            assert_eq!(
                (time(&scene.panels[3]) - time(&scene.panels[0])).num_hours(),
                hours
            );
            let scope = scene.scope;
            if coverage == super::super::observed::Coverage::State {
                assert!(scope.bbox.is_none());
            } else {
                assert!(scope.bbox.is_some());
            }
            assert_eq!(*layout, Layout::Timeline);
        }
        assert!(timeline_options("timeline.9h.state").is_none());
    }
    #[test]
    fn invalid_or_unreviewed_data_is_not_drawn_as_weather() {
        let mut s = board::empty(Layout::Compare);
        s.panels[0].counties = Some(Default::default());
        assert!(
            !board::parse(&serde_json::to_string(&s).unwrap(), Layout::Compare)
                .unwrap()
                .panels[0]
                .available()
        );
        s.reviewed = true;
        s.panels[0]
            .counties
            .as_mut()
            .unwrap()
            .insert("18037".into(), 2);
        assert!(board::parse(&serde_json::to_string(&s).unwrap(), Layout::Compare).is_none());
        assert!(board::parse("{}", Layout::Timeline).is_none());
    }
    #[test]
    fn unknown_home_does_not_silently_become_indiana() {
        let mut settings = Settings::default();
        settings.home.lat = 0.;
        settings.home.lon = 0.;
        let s = scope(&settings, Layout::Zones);
        assert!(s.county.is_none());
        assert!(s.states.is_empty());
    }
    #[test]
    fn criteria_are_limited_to_the_verified_office_and_are_not_alerts() {
        let mut scene = board::empty(Layout::Compare);
        scene.scope.county = Some("18037".into());
        assert!(published_cold_criteria(&mut scene));
        assert_eq!(scene.panels[1].headline, "-15°F or colder");
        assert!(board::parse(&serde_json::to_string(&scene).unwrap(), Layout::Compare).is_some());
        scene.scope.county = Some("18097".into());
        assert!(!published_cold_criteria(&mut scene));
    }
}
