//! GIS tools on custom data layers and warnings: buffers, intersection, clipping, union /
//! dissolve (geo boolean operations), coordinate conversion (decimal, DMS, Web Mercator, UTM),
//! projected exports, image overlay reprojection, and dataset validation. Results are written as
//! GeoJSON into Documents\OpenWX\GIS and load as new custom layers.

use anyhow::{anyhow, Result};
use geo::{BooleanOps, Coord, LineString, MultiPolygon, Polygon};

use crate::customdata::{load, CustomLayer};

fn out_dir() -> std::path::PathBuf {
    crate::config::LibraryPaths::get().root.join("GIS")
}

/// Polygons of a custom layer (lon, lat).
pub fn layer_polygons(layer: &CustomLayer) -> MultiPolygon<f64> {
    let Some(data) = load(std::path::Path::new(&layer.path)) else { return MultiPolygon::new(Vec::new()) };
    let mut polys = Vec::new();
    for f in &data.features {
        if let Some(outer) = f.rings.first().filter(|r| r.len() >= 3) {
            let holes = f.rings.iter().skip(1).filter(|r| r.len() >= 3).map(|r| LineString::from(r.clone())).collect();
            polys.push(Polygon::new(LineString::from(outer.clone()), holes));
        } else if let Some((lat, lon)) = f.point {
            // Points become tiny squares so they survive boolean operations.
            let d = 0.001;
            polys.push(Polygon::new(LineString::from(vec![(lon - d, lat - d), (lon + d, lat - d), (lon + d, lat + d), (lon - d, lat + d)]), Vec::new()));
        }
    }
    MultiPolygon::new(polys)
}

/// Warning polygons in effect as one multipolygon.
pub fn warning_polygons(alerts: &[crate::data::Alert]) -> MultiPolygon<f64> {
    MultiPolygon::new(alerts.iter().filter(|a| a.is_warning()).flat_map(|a| a.polygons.iter().filter(|r| r.len() >= 3).map(|r| Polygon::new(LineString::from(r.clone()), Vec::new()))).collect())
}

fn circle(lon: f64, lat: f64, miles: f64) -> Polygon<f64> {
    let pts: Vec<(f64, f64)> = (0..48)
        .map(|k| {
            let (la, lo) = crate::map::geo::destination(lat, lon, k as f64 * 7.5, miles * 1.609_344);
            (lo, la)
        })
        .collect();
    Polygon::new(LineString::from(pts), Vec::new())
}

/// Buffer: grow every shape by `miles` (union of the shape, circles at vertices and edge strips).
pub fn buffer(shapes: &MultiPolygon<f64>, miles: f64) -> MultiPolygon<f64> {
    let mut acc = shapes.clone();
    for poly in &shapes.0 {
        let ring: Vec<Coord<f64>> = poly.exterior().coords().copied().collect();
        // Thin the ring so huge polygons stay fast.
        let step = (ring.len() / 120).max(1);
        let pts: Vec<Coord<f64>> = ring.iter().step_by(step).copied().collect();
        let mut pieces = Vec::new();
        for w in pts.windows(2) {
            pieces.push(circle(w[0].x, w[0].y, miles));
            // Edge strip: the segment offset to both sides.
            let (dx, dy) = (w[1].x - w[0].x, w[1].y - w[0].y);
            let len = (dx * dx + dy * dy).sqrt().max(1e-9);
            let k = miles / 69.0;
            let (nx, ny) = (-dy / len * k / w[0].y.to_radians().cos().max(0.2), dx / len * k);
            pieces.push(Polygon::new(
                LineString::from(vec![(w[0].x + nx, w[0].y + ny), (w[1].x + nx, w[1].y + ny), (w[1].x - nx, w[1].y - ny), (w[0].x - nx, w[0].y - ny)]),
                Vec::new(),
            ));
        }
        for p in pieces {
            acc = acc.union(&MultiPolygon::new(vec![p]));
        }
    }
    acc
}

pub fn point_buffer(lat: f64, lon: f64, miles: f64) -> MultiPolygon<f64> {
    MultiPolygon::new(vec![circle(lon, lat, miles)])
}

pub fn intersection(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> MultiPolygon<f64> {
    a.intersection(b)
}

pub fn union(a: &MultiPolygon<f64>, b: &MultiPolygon<f64>) -> MultiPolygon<f64> {
    a.union(b)
}

/// Dissolve: merge every polygon of a layer into one shape.
pub fn dissolve(a: &MultiPolygon<f64>) -> MultiPolygon<f64> {
    a.0.iter().fold(MultiPolygon::new(Vec::new()), |acc, p| acc.union(&MultiPolygon::new(vec![p.clone()])))
}

/// Area in square miles (local equal-area approximation per polygon).
pub fn area_sq_mi(m: &MultiPolygon<f64>) -> f64 {
    m.0.iter().map(|p| {
        let pts: Vec<(f64, f64)> = p.exterior().coords().map(|c| (c.y, c.x)).collect();
        crate::map::geo::polygon_sq_mi(&pts)
    }).sum()
}

/// Output projections for exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Projection {
    Wgs84,
    WebMercator,
    Utm,
}

/// Write a result as GeoJSON (in the chosen projection) and return a ready custom layer (always
/// loaded from the WGS84 copy so it can be drawn).
pub fn save(name: &str, shapes: &MultiPolygon<f64>, projection: Projection) -> Result<CustomLayer> {
    std::fs::create_dir_all(out_dir())?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '_' }).collect();
    let feature = |coords: &dyn Fn(f64, f64) -> (f64, f64)| -> serde_json::Value {
        let polys: Vec<serde_json::Value> = shapes
            .0
            .iter()
            .map(|p| {
                let mut rings = vec![p.exterior()];
                rings.extend(p.interiors().iter());
                serde_json::json!(rings.iter().map(|r| r.coords().map(|c| { let (x, y) = coords(c.x, c.y); vec![x, y] }).collect::<Vec<_>>()).collect::<Vec<_>>())
            })
            .collect();
        serde_json::json!({ "type": "FeatureCollection", "features": [{ "type": "Feature", "properties": { "name": name, "area_sq_mi": area_sq_mi(shapes) }, "geometry": { "type": "MultiPolygon", "coordinates": polys } }] })
    };
    let wgs = out_dir().join(format!("{clean} {stamp}.geojson"));
    std::fs::write(&wgs, feature(&|x, y| (x, y)).to_string())?;
    match projection {
        Projection::Wgs84 => {}
        Projection::WebMercator => {
            std::fs::write(out_dir().join(format!("{clean} {stamp} (Web Mercator).geojson")), feature(&|x, y| to_mercator(y, x)).to_string())?;
        }
        Projection::Utm => {
            let zone = shapes.0.first().and_then(|p| p.exterior().coords().next()).map(|c| utm_zone(c.x)).unwrap_or(16);
            std::fs::write(out_dir().join(format!("{clean} {stamp} (UTM {zone}N).geojson")), feature(&|x, y| { let (e, n, _) = to_utm(y, x, Some(zone)); (e, n) }).to_string())?;
        }
    }
    crate::customdata::new_layer(&wgs).map(|mut l| {
        l.name = name.to_string();
        l.mode = "choropleth".into();
        l
    }).ok_or_else(|| anyhow!("couldn't load the result"))
}

// ---- Coordinates ----

pub fn to_mercator(lat: f64, lon: f64) -> (f64, f64) {
    let r = 6_378_137.0;
    (lon.to_radians() * r, (std::f64::consts::FRAC_PI_4 + lat.to_radians() / 2.0).tan().ln() * r)
}

pub fn utm_zone(lon: f64) -> u32 {
    (((lon + 180.0) / 6.0).floor() as u32 + 1).clamp(1, 60)
}

/// WGS84 -> UTM (easting, northing, zone), standard series formulas.
pub fn to_utm(lat: f64, lon: f64, zone: Option<u32>) -> (f64, f64, u32) {
    let zone = zone.unwrap_or_else(|| utm_zone(lon));
    let (a, f) = (6_378_137.0f64, 1.0 / 298.257_223_563);
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);
    let k0 = 0.9996;
    let lon0 = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
    let (phi, lam) = (lat.to_radians(), lon.to_radians());
    let n = a / (1.0 - e2 * phi.sin().powi(2)).sqrt();
    let t = phi.tan().powi(2);
    let c = ep2 * phi.cos().powi(2);
    let aa = phi.cos() * (lam - lon0);
    let m = a * ((1.0 - e2 / 4.0 - 3.0 * e2 * e2 / 64.0 - 5.0 * e2.powi(3) / 256.0) * phi - (3.0 * e2 / 8.0 + 3.0 * e2 * e2 / 32.0 + 45.0 * e2.powi(3) / 1024.0) * (2.0 * phi).sin() + (15.0 * e2 * e2 / 256.0 + 45.0 * e2.powi(3) / 1024.0) * (4.0 * phi).sin() - (35.0 * e2.powi(3) / 3072.0) * (6.0 * phi).sin());
    let easting = k0 * n * (aa + (1.0 - t + c) * aa.powi(3) / 6.0 + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * ep2) * aa.powi(5) / 120.0) + 500_000.0;
    let mut northing = k0 * (m + n * phi.tan() * (aa * aa / 2.0 + (5.0 - t + 9.0 * c + 4.0 * c * c) * aa.powi(4) / 24.0 + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * ep2) * aa.powi(6) / 720.0));
    if lat < 0.0 {
        northing += 10_000_000.0;
    }
    (easting, northing, zone)
}

fn dms(v: f64, pos: char, neg: char) -> String {
    let h = if v >= 0.0 { pos } else { neg };
    let v = v.abs();
    let d = v.floor();
    let m = ((v - d) * 60.0).floor();
    let s = ((v - d) * 60.0 - m) * 60.0;
    format!("{d:.0}°{m:02.0}'{s:04.1}\"{h}")
}

/// Every representation of a coordinate typed as "lat, lon" or DMS.
pub fn convert(input: &str) -> String {
    let Some((lat, lon)) = crate::places::parse_lat_lon(input).or_else(|| parse_dms(input)) else {
        return "Type a position like 38.39, -86.93 or 38°23'24\"N 86°55'48\"W".into();
    };
    let (x, y) = to_mercator(lat, lon);
    let (e, n, z) = to_utm(lat, lon, None);
    format!(
        "Decimal: {lat:.5}, {lon:.5}\nDMS: {} {}\nWeb Mercator: {x:.0} m, {y:.0} m\nUTM zone {z}{}: {e:.0} mE, {n:.0} mN",
        dms(lat, 'N', 'S'),
        dms(lon, 'E', 'W'),
        if lat >= 0.0 { "N" } else { "S" }
    )
}

fn parse_dms(s: &str) -> Option<(f64, f64)> {
    let nums: Vec<f64> = s.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter(|x| !x.is_empty()).filter_map(|x| x.parse().ok()).collect();
    if nums.len() < 6 {
        return None;
    }
    let lat = nums[0] + nums[1] / 60.0 + nums[2] / 3600.0;
    let lon = nums[3] + nums[4] / 60.0 + nums[5] / 3600.0;
    let up = s.to_uppercase();
    Some((if up.contains('S') { -lat } else { lat }, if up.contains('W') { -lon } else { lon }))
}

/// Geospatial dataset validation: empty or broken geometry, out-of-range coordinates, unclosed
/// rings, missing values in the chosen field, duplicates.
pub fn validate(layer: &CustomLayer) -> Vec<String> {
    let Some(data) = load(std::path::Path::new(&layer.path)) else { return vec!["The file can't be read".into()] };
    let mut out = Vec::new();
    let empty = data.features.iter().filter(|f| f.point.is_none() && f.rings.is_empty()).count();
    if empty > 0 {
        out.push(format!("{empty} feature(s) have no geometry"));
    }
    let bad_coords = data.features.iter().filter(|f| f.point.map(|(la, lo)| la.abs() > 90.0 || lo.abs() > 180.0).unwrap_or(false) || f.rings.iter().flatten().any(|(lo, la)| la.abs() > 90.0 || lo.abs() > 180.0)).count();
    if bad_coords > 0 {
        out.push(format!("{bad_coords} feature(s) have coordinates out of range (projected data? convert to WGS84)"));
    }
    let open = data.features.iter().flat_map(|f| f.rings.iter()).filter(|r| r.len() >= 3 && r.first() != r.last()).count();
    if open > 0 {
        out.push(format!("{open} polygon ring(s) are not closed (they will be closed automatically)"));
    }
    let tiny = data.features.iter().flat_map(|f| f.rings.iter()).filter(|r| r.len() < 4).count();
    if tiny > 0 {
        out.push(format!("{tiny} ring(s) have fewer than 3 points"));
    }
    if !layer.field.is_empty() {
        let missing = data.features.iter().filter(|f| f.props.get(&layer.field).map(|v| v.trim().parse::<f64>().is_err()).unwrap_or(true)).count();
        if missing > 0 {
            out.push(format!("{missing} feature(s) have no number in \"{}\"", layer.field));
        }
    }
    let mut seen = std::collections::HashSet::new();
    let dups = data.features.iter().filter_map(|f| f.point).filter(|(la, lo)| !seen.insert(((la * 1e5) as i64, (lo * 1e5) as i64))).count();
    if dups > 0 {
        out.push(format!("{dups} duplicate point(s)"));
    }
    if out.is_empty() {
        out.push(format!("Valid: {} features, {} numeric fields", data.features.len(), data.numeric_fields.len()));
    }
    out
}
