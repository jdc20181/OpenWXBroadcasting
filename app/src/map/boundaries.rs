//! County and state outlines drawn as crisp vector lines over the map (above the radar, under
//! warnings). Boundaries are US Census cartographic shapes (as GeoJSON), downloaded once and
//! cached in the app's cache folder.

use std::sync::{Arc, OnceLock, RwLock};

use tiny_skia::{Color, PathBuilder, Pixmap, Transform};

use crate::map::{
    geo::Viewport,
    overlays::{paint, stroke},
};

const COUNTIES_URL: &str = "https://raw.githubusercontent.com/plotly/datasets/master/geojson-counties-fips.json";
const STATES_URL: &str = "https://raw.githubusercontent.com/PublicaMundi/MappingAPI/master/data/geojson/us-states.json";

struct Ring {
    /// (west, south, east, north)
    bbox: (f64, f64, f64, f64),
    /// (lon, lat)
    points: Vec<(f64, f64)>,
}

struct Region {
    name: String,
    rings: Vec<Ring>,
}

#[derive(Default)]
pub struct Boundaries {
    counties: Vec<Region>,
    states: Vec<Region>,
}

pub struct SelectedRegion {
    pub name: String,
    pub rings: Vec<Vec<(f64, f64)>>,
}

fn store() -> &'static RwLock<Option<Arc<Boundaries>>> {
    static STORE: OnceLock<RwLock<Option<Arc<Boundaries>>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(None))
}

pub fn get() -> Option<Arc<Boundaries>> {
    store().read().ok()?.clone()
}

fn regions(value: &serde_json::Value) -> Vec<Region> {
    value.get("features").and_then(|features| features.as_array()).into_iter().flatten()
        .filter_map(|feature| {
            let name = feature.pointer("/properties/NAME").or_else(|| feature.pointer("/properties/name"))
                .and_then(|name| name.as_str()).unwrap_or_default().to_owned();
            let geometry = feature.get("geometry")?;
            let coordinates = geometry.get("coordinates")?.as_array()?;
            let polygons: Vec<_> = match geometry.get("type")?.as_str()? {
                "Polygon" => vec![coordinates],
                "MultiPolygon" => coordinates.iter().filter_map(|polygon| polygon.as_array()).collect(),
                _ => return None,
            };
            let rings: Vec<_> = polygons.into_iter().flatten().filter_map(|ring| {
                let points: Vec<_> = ring.as_array()?.iter().filter_map(|point| {
                    let point = point.as_array()?;
                    Some((point.first()?.as_f64()?, point.get(1)?.as_f64()?))
                }).collect();
                if points.len() < 3 { return None; }
                let bbox = points.iter().fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |(w, s, e, n), &(lon, lat)| {
                    (w.min(lon), s.min(lat), e.max(lon), n.max(lat))
                });
                Some(Ring { bbox, points })
            }).collect();
            (!rings.is_empty()).then_some(Region { name, rings })
        }).collect()
}

impl Region {
    fn contains(&self, lon: f64, lat: f64) -> bool {
        self.rings.iter().fold(false, |inside_region, ring| {
            let (west, south, east, north) = ring.bbox;
            inside_region ^ ((west..=east).contains(&lon) && (south..=north).contains(&lat) && inside(&ring.points, lon, lat))
        })
    }
}

/// Every state outline ring as (lon, lat) points, for county maps that draw state lines.
pub fn state_rings() -> Vec<Vec<(f64, f64)>> {
    if let Some(b) = get() {
        return b.states.iter().flat_map(|s| s.rings.iter().map(|r| r.points.clone())).collect();
    }
    // Graphics can render before the map has loaded its boundaries (a show at start-up,
    // tests): read the downloaded state file directly, once.
    static DISK: OnceLock<Vec<Vec<(f64, f64)>>> = OnceLock::new();
    DISK.get_or_init(|| {
        let path = crate::config::tile_cache_dir().join("..").join("boundaries").join("us-states.json");
        std::fs::read_to_string(path).ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .map(|v| regions(&v).into_iter().flat_map(|r| r.rings.into_iter().map(|ring| ring.points)).collect())
            .unwrap_or_default()
    }).clone()
}

/// Return the entire county or state, including detached parts and interior holes.
pub fn region_at(lat: f64, lon: f64, states: bool) -> Option<SelectedRegion> {
    let boundaries = get()?;
    let regions = if states { &boundaries.states } else { &boundaries.counties };
    let region = regions.iter().find(|region| region.contains(lon, lat))?;
    Some(SelectedRegion {
        name: region.name.clone(),
        rings: region.rings.iter().map(|ring| ring.points.iter().map(|&(lon, lat)| (lat, lon)).collect()).collect(),
    })
}

/// A county with its name and state FIPS (outage maps, facility lookup).
pub struct County {
    pub name: String,
    pub state: String,
    /// 5-digit county FIPS code.
    pub fips: String,
    /// (lon, lat) rings.
    pub rings: Vec<Vec<(f64, f64)>>,
    bbox: (f64, f64, f64, f64),
}

/// Named counties, read from the cached county file once it has been downloaded.
pub fn counties_named() -> Option<Arc<Vec<County>>> {
    static NAMED: OnceLock<RwLock<Option<Arc<Vec<County>>>>> = OnceLock::new();
    let slot = NAMED.get_or_init(|| RwLock::new(None));
    if let Some(c) = slot.read().ok()?.clone() {
        return Some(c);
    }
    let path = crate::config::tile_cache_dir().join("..").join("boundaries").join("us-counties.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let mut out = Vec::new();
    for f in v.get("features").and_then(|f| f.as_array()).into_iter().flatten() {
        let name = f.pointer("/properties/NAME").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let state = f.pointer("/properties/STATE").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let mut rings = Vec::new();
        if let Some(g) = f.get("geometry") {
            crate::data::collect_rings(g, &mut rings);
        }
        let mut bbox = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for (lon, lat) in rings.iter().flatten() {
            bbox = (bbox.0.min(*lon), bbox.1.min(*lat), bbox.2.max(*lon), bbox.3.max(*lat));
        }
        let fips = f.get("id").and_then(|n| n.as_str().map(String::from).or_else(|| n.as_u64().map(|n| format!("{n:05}")))).unwrap_or_default();
        out.push(County { name, state, fips, rings, bbox });
    }
    let arc = Arc::new(out);
    if let Ok(mut w) = slot.write() {
        *w = Some(arc.clone());
    }
    Some(arc)
}

/// Outlines (lon, lat rings) of the counties with these FIPS codes; empty until the county file
/// has been downloaded.
pub fn county_rings(fips: &[String]) -> Vec<Vec<(f64, f64)>> {
    let Some(counties) = counties_named() else { return Vec::new() };
    counties.iter().filter(|c| fips.iter().any(|f| *f == c.fips)).flat_map(|c| c.rings.iter().cloned()).collect()
}

/// The county containing a point.
pub fn county_at(counties: &[County], lat: f64, lon: f64) -> Option<String> {
    counties
        .iter()
        .filter(|c| lon >= c.bbox.0 && lon <= c.bbox.2 && lat >= c.bbox.1 && lat <= c.bbox.3)
        .find(|c| c.rings.iter().any(|r| inside(r, lon, lat)))
        .map(|c| c.name.clone())
}

fn inside(ring: &[(f64, f64)], x: f64, y: f64) -> bool {
    let mut c = false;
    let n = ring.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let (xi, yi) = ring[i];
        let (xj, yj) = ring[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            c = !c;
        }
        j = i;
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_includes_detached_parts_and_excludes_holes() {
        let fixture = serde_json::json!({
            "features": [{
                "properties": {"NAME": "Example County"},
                "geometry": {"type": "MultiPolygon", "coordinates": [
                    [
                        [[0,0],[5,0],[5,5],[0,5],[0,0]],
                        [[1,1],[2,1],[2,2],[1,2],[1,1]]
                    ],
                    [[[8,0],[9,0],[9,1],[8,1],[8,0]]]
                ]}
            }]
        });
        let regions = regions(&fixture);
        assert_eq!(regions[0].name, "Example County");
        assert_eq!(regions[0].rings.len(), 3);
        assert!(regions[0].contains(3.0, 3.0));
        assert!(regions[0].contains(8.5, 0.5));
        assert!(!regions[0].contains(1.5, 1.5));
        assert!(!regions[0].contains(7.0, 0.5));
    }
}

async fn load_json(url: &str, file: &str) -> anyhow::Result<serde_json::Value> {
    let dir = crate::config::tile_cache_dir().join("..").join("boundaries");
    let path = dir.join(file);
    if let Ok(text) = tokio::fs::read_to_string(&path).await {
        if let Ok(v) = serde_json::from_str(&text) {
            return Ok(v);
        }
    }
    let bytes = crate::net::get_logged(url, std::time::Duration::from_secs(120)).await?.bytes().await?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let _ = tokio::fs::create_dir_all(&dir).await;
    let _ = tokio::fs::write(&path, &bytes).await;
    Ok(value)
}

/// Load (from cache or the web) once; later calls return immediately.
pub async fn ensure_loaded() -> anyhow::Result<()> {
    if get().is_some() {
        return Ok(());
    }
    let counties = load_json(COUNTIES_URL, "us-counties.json").await?;
    let states = load_json(STATES_URL, "us-states.json").await?;
    let b = Boundaries {
        counties: regions(&counties),
        states: regions(&states),
    };
    if let Ok(mut slot) = store().write() {
        *slot = Some(Arc::new(b));
    }
    Ok(())
}

fn draw_rings(pm: &mut Pixmap, vp: &Viewport, regions: &[Region], color: Color, width: f32) {
    let (n, w) = vp.to_geo(0.0, 0.0);
    let (s, e) = vp.to_geo(vp.width, vp.height);
    let mut pb = PathBuilder::new();
    for ring in regions.iter().flat_map(|region| &region.rings) {
        let (bw, bs, be, bn) = ring.bbox;
        if be < w || bw > e || bn < s || bs > n {
            continue;
        }
        for (i, (lon, lat)) in ring.points.iter().enumerate() {
            let (x, y) = vp.to_screen(*lat, *lon);
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        pb.close();
    }
    if let Some(path) = pb.finish() {
        pm.stroke_path(&path, &paint(color), &stroke(width), Transform::identity(), None);
    }
}

/// County lines (from zoom ~5.5) and state lines. `light` = for light basemaps.
pub fn draw(pm: &mut Pixmap, vp: &Viewport, counties: bool, states: bool, light: bool, opacity: f32, theme: Option<&crate::maptheme::Theme>) {
    let Some(b) = get() else { return };
    let zoom = vp.view.zoom;
    let opacity = opacity.clamp(0.05, 1.0);
    if counties && zoom >= 5.5 {
        let fade = ((((zoom - 5.5) / 1.5).clamp(0.0, 1.0) * 150.0 + 40.0) / 190.0) as f32;
        let alpha = (fade * 190.0 * opacity) as u8;
        let (color, width) = match theme {
            Some(t) => {
                let mut c = crate::maptheme::color(&t.county_color, Color::WHITE);
                c.set_alpha(c.alpha() * fade * opacity);
                (c, t.county_width.max(0.3) * if zoom > 8.0 { 1.3 } else { 1.0 })
            }
            None => (if light { Color::from_rgba8(60, 60, 70, alpha) } else { Color::from_rgba8(200, 210, 225, alpha) }, if zoom > 8.0 { 1.6 } else { 1.1 }),
        };
        draw_rings(pm, vp, &b.counties, color, width);
    }
    if states {
        let (la, da) = ((220.0 * opacity) as u8, (210.0 * opacity) as u8);
        let (color, width) = match theme {
            Some(t) => {
                let mut c = crate::maptheme::color(&t.state_color, Color::WHITE);
                c.set_alpha(c.alpha() * opacity);
                (c, t.state_width.max(0.5) * if zoom > 7.0 { 1.2 } else { 1.0 })
            }
            None => (if light { Color::from_rgba8(30, 30, 40, la) } else { Color::from_rgba8(235, 240, 250, da) }, if zoom > 7.0 { 2.6 } else { 2.0 }),
        };
        draw_rings(pm, vp, &b.states, color, width);
    }
}

/// Tests that draw counties: the county file, downloaded the way the app does on first run when
/// this machine has not fetched it yet (a fresh profile or a CI runner).
#[cfg(test)]
pub fn counties_for_tests() -> Arc<Vec<County>> {
    if let Some(c) = counties_named() {
        return c;
    }
    if let Err(e) = crate::net::runtime().block_on(ensure_loaded()) {
        panic!("county geometry: download failed ({e})");
    }
    counties_named().expect("county geometry")
}
