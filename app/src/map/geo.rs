//! Web Mercator math matching maplibre-rs (512px tiles, 1 world px = 1 screen px at pitch 0),
//! so overlays drawn by the compositor line up exactly with the rendered map.

use std::f64::consts::PI;

use crate::config::GeoView;

pub const TILE_SIZE: f64 = 512.0;
pub const MAX_LAT: f64 = 85.051_128_78;
pub const MIN_ZOOM: f64 = 2.0;
pub const MAX_ZOOM: f64 = 14.0;
const EARTH_RADIUS_KM: f64 = 6371.0;
pub const KM_PER_MILE: f64 = 1.609_344;

pub fn world_size(zoom: f64) -> f64 {
    TILE_SIZE * 2f64.powf(zoom)
}

/// Lat/lon → world pixels at `zoom` (same formula as maplibre-rs `WorldCoords::from_lat_lon`).
pub fn project(lat: f64, lon: f64, zoom: f64) -> (f64, f64) {
    let size = world_size(zoom);
    let lat = lat.clamp(-MAX_LAT, MAX_LAT);
    let x = (lon + 180.0) * (size / 360.0);
    let lat_rad = lat.to_radians();
    let merc_n = (PI / 4.0 + lat_rad / 2.0).tan().ln();
    let y = size / 2.0 - size * merc_n / (2.0 * PI);
    (x, y)
}

/// World pixels at `zoom` → lat/lon.
pub fn unproject(x: f64, y: f64, zoom: f64) -> (f64, f64) {
    let size = world_size(zoom);
    let lon = x / size * 360.0 - 180.0;
    let n = PI - 2.0 * PI * y / size;
    let lat = (0.5 * (n.exp() - (-n).exp())).atan().to_degrees();
    (lat, lon)
}

/// A camera over a frame of `width`×`height` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub view: GeoView,
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    pub fn new(view: GeoView, width: u32, height: u32) -> Self {
        Self {
            view,
            width: width as f64,
            height: height as f64,
        }
    }

    pub fn to_screen(&self, lat: f64, lon: f64) -> (f32, f32) {
        let (cx, cy) = project(self.view.lat, self.view.lon, self.view.zoom);
        let (x, y) = project(lat, lon, self.view.zoom);
        (
            (self.width / 2.0 + (x - cx)) as f32,
            (self.height / 2.0 + (y - cy)) as f32,
        )
    }

    pub fn to_geo(&self, sx: f64, sy: f64) -> (f64, f64) {
        let (cx, cy) = project(self.view.lat, self.view.lon, self.view.zoom);
        unproject(cx + sx - self.width / 2.0, cy + sy - self.height / 2.0, self.view.zoom)
    }

    /// Pan by a screen-pixel delta (drag direction: content follows the pointer).
    pub fn panned(&self, dx: f64, dy: f64) -> GeoView {
        let (cx, cy) = project(self.view.lat, self.view.lon, self.view.zoom);
        let (lat, lon) = unproject(cx - dx, cy - dy, self.view.zoom);
        GeoView {
            lat: lat.clamp(-80.0, 80.0),
            lon: wrap_lon(lon),
            zoom: self.view.zoom,
        }
    }

    /// Zoom by `delta` keeping the geographic point under (sx, sy) fixed, like MapLibre GL.
    pub fn zoomed_at(&self, delta: f64, sx: f64, sy: f64) -> GeoView {
        let new_zoom = (self.view.zoom + delta).clamp(MIN_ZOOM, MAX_ZOOM);
        let (anchor_lat, anchor_lon) = self.to_geo(sx, sy);
        let (ax, ay) = project(anchor_lat, anchor_lon, new_zoom);
        let cx = ax - (sx - self.width / 2.0);
        let cy = ay - (sy - self.height / 2.0);
        let (lat, lon) = unproject(cx, cy, new_zoom);
        GeoView {
            lat: lat.clamp(-80.0, 80.0),
            lon: wrap_lon(lon),
            zoom: new_zoom,
        }
    }

    /// Screen pixels per kilometre at the view center.
    pub fn px_per_km(&self) -> f64 {
        let meters_per_px =
            156_543.033_92 * self.view.lat.to_radians().cos() / 2f64.powf(self.view.zoom) / 2.0;
        1000.0 / meters_per_px
    }

    /// Geographic bounds (south, west, north, east) of the frame.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let (north, west) = self.to_geo(0.0, 0.0);
        let (south, east) = self.to_geo(self.width, self.height);
        (south, west, north, east)
    }
}

pub fn wrap_lon(lon: f64) -> f64 {
    let mut lon = lon;
    while lon > 180.0 {
        lon -= 360.0;
    }
    while lon < -180.0 {
        lon += 360.0;
    }
    lon
}

pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let d_lat = (lat2 - lat1).to_radians();
    let d_lon = (lon2 - lon1).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
    EARTH_RADIUS_KM * 2.0 * a.sqrt().asin()
}

/// Initial bearing from point 1 to point 2, degrees clockwise from north.
pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let (p1, p2) = (lat1.to_radians(), lat2.to_radians());
    let dl = (lon2 - lon1).to_radians();
    let y = dl.sin() * p2.cos();
    let x = p1.cos() * p2.sin() - p1.sin() * p2.cos() * dl.cos();
    (y.atan2(x).to_degrees() + 360.0) % 360.0
}

/// Point reached travelling `distance_km` from (lat, lon) on `bearing` degrees.
pub fn destination(lat: f64, lon: f64, bearing: f64, distance_km: f64) -> (f64, f64) {
    let delta = distance_km / EARTH_RADIUS_KM;
    let theta = bearing.to_radians();
    let p1 = lat.to_radians();
    let l1 = lon.to_radians();
    let p2 = (p1.sin() * delta.cos() + p1.cos() * delta.sin() * theta.cos()).asin();
    let l2 = l1 + (theta.sin() * delta.sin() * p1.cos()).atan2(delta.cos() - p1.sin() * p2.sin());
    (p2.to_degrees(), wrap_lon(l2.to_degrees()))
}

pub fn compass(deg: f64) -> &'static str {
    const POINTS: [&str; 16] = [
        "N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW",
        "NW", "NNW",
    ];
    POINTS[(((deg % 360.0 + 360.0) % 360.0) / 22.5).round() as usize % 16]
}

/// Smooth "fly to" between two views (zoom out, travel, zoom in), t in 0..=1.
pub fn fly_interpolate(from: GeoView, to: GeoView, t: f64) -> GeoView {
    fly_interpolate_with(from, to, t, "smooth", true)
}

/// Easing curves for camera moves (Settings > Camera moves).
pub fn ease(curve: &str, t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    match curve {
        "linear" => t,
        "ease-in" => t * t * t,
        "ease-out" => 1.0 - (1.0 - t).powi(3),
        "gentle" => -((std::f64::consts::PI * t).cos() - 1.0) / 2.0,
        _ => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
    }
}

pub fn fly_interpolate_with(from: GeoView, to: GeoView, t: f64, curve: &str, dip_on: bool) -> GeoView {
    let e = ease(curve, t);
    let distance_km = haversine_km(from.lat, from.lon, to.lat, to.lon);
    // Dip in zoom proportional to the distance travelled.
    let dip = if dip_on { (distance_km / 250.0).log2().clamp(0.0, 3.0) } else { 0.0 };
    let base_zoom = from.zoom + (to.zoom - from.zoom) * e;
    let zoom = base_zoom - dip * (std::f64::consts::PI * t).sin();
    GeoView {
        lat: from.lat + (to.lat - from.lat) * e,
        lon: from.lon + (to.lon - from.lon) * e,
        zoom: zoom.clamp(MIN_ZOOM, MAX_ZOOM),
    }
}

/// Length of a path of (lat, lon) points, in miles.
pub fn path_miles(points: &[(f64, f64)]) -> f64 {
    points.windows(2).map(|w| haversine_km(w[0].0, w[0].1, w[1].0, w[1].1)).sum::<f64>() / 1.609_344
}

/// Area of a (lat, lon) polygon in square miles (local equal-area projection; fine for
/// county-to-state sized shapes).
pub fn polygon_sq_mi(points: &[(f64, f64)]) -> f64 {
    if points.len() < 3 {
        return 0.0;
    }
    let lat0 = points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64;
    let k = lat0.to_radians().cos();
    let xy: Vec<(f64, f64)> = points.iter().map(|(lat, lon)| (lon * 111.32 * k, lat * 110.57)).collect();
    let mut twice = 0.0;
    for i in 0..xy.len() {
        let (a, b) = (xy[i], xy[(i + 1) % xy.len()]);
        twice += a.0 * b.1 - b.0 * a.1;
    }
    (twice.abs() / 2.0) / 2.589_988
}

/// Ground elevation in feet at each point (Open-Meteo elevation API, 90 m DEM).
pub async fn elevations_ft(points: Vec<(f64, f64)>) -> anyhow::Result<Vec<f64>> {
    let lats = points.iter().map(|p| format!("{:.4}", p.0)).collect::<Vec<_>>().join(",");
    let lons = points.iter().map(|p| format!("{:.4}", p.1)).collect::<Vec<_>>().join(",");
    let url = format!("https://api.open-meteo.com/v1/elevation?latitude={lats}&longitude={lons}");
    let v: serde_json::Value = crate::net::get_logged(&url, std::time::Duration::from_secs(10)).await?.json().await?;
    let list = v.get("elevation").and_then(|e| e.as_array()).ok_or_else(|| anyhow::anyhow!("no elevation in the answer"))?;
    Ok(list.iter().filter_map(|x| x.as_f64()).map(|m| m * 3.280_84).collect())
}
