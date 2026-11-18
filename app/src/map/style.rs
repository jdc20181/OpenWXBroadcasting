//! Builds the MapLibre style (style-spec JSON) for a map scene.
//!
//! Layer stack, bottom to top - the classic broadcast order:
//!   background → basemap → satellite → MRMS rain totals → radar → place labels.
//! Composite radar: every LibreWXR frame is its own raster source + layer; only the current frame
//! is visible (`raster-opacity` > 0), the others stay at 0 so they are pre-loaded, as MapLibre GL
//! radar loops do. LibreWXR radar and IR satellite come through the site's /api/radar proxy.
//! Single-site NEXRAD products (velocity, storm-relative velocity, super-res reflectivity, echo
//! tops), GOES-East channels and MRMS totals come from the Iowa Environmental Mesonet tile cache.
//!
//! Layer ids include a hash of their tile URL. maplibre-rs keys tile textures by layer id, so a
//! layer whose URL changes (new palette, smoothing, site, product…) must get a new id, or the
//! old tiles stay on screen.

use serde_json::{json, Value};

use crate::{
    config::Basemap,
    data::{nearest_site, RadarIndex},
    map::scene::{MapScene, RadarSource, SatChannel},
};

const IEM: &str = "https://mesonet.agron.iastate.edu/cache/tile.py/1.0.0";

/// Which radar frames the scene animates, oldest first. Single-site radar is latest-only.
pub fn radar_frames(scene: &MapScene, radar: Option<&RadarIndex>) -> Vec<i64> {
    if let Some(replay) = &scene.site_replay { return replay.scans.clone(); }
    if let Some(replay) = scene.radar_replay {
        return replay.frames();
    }
    if scene.history_time.is_some() {
        return Vec::new();
    }
    let Some(radar) = radar else {
        return Vec::new();
    };
    if scene.radar.source == RadarSource::Site {
        return Vec::new();
    }
    let mut frames: Vec<i64> = radar.radar.past.iter().map(|f| f.time).collect();
    // Adaptive tile loading: a slow radar server gets a shorter loop (the newest frames).
    if let Some(cap) = scene.frame_cap {
        if frames.len() > cap {
            frames.drain(..frames.len() - cap);
        }
    }
    if scene.radar.include_nowcast {
        frames.extend(radar.radar.nowcast.iter().map(|f| f.time));
    }
    frames
}

// Basemaps are Esri's keyless tile services. (CARTO's basemap CDN now answers non-browser
// clients with "API KEY REQUIRED" tiles.)
fn basemap_tiles(basemap: Basemap) -> (&'static str, u8, &'static str) {
    match basemap {
        Basemap::BroadcastDark => (
            "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Base/MapServer/tile/{z}/{y}/{x}",
            16,
            "#0b0f18",
        ),
        Basemap::Light => (
            "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Base/MapServer/tile/{z}/{y}/{x}",
            16,
            "#e9edf2",
        ),
        Basemap::Voyager => (
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Street_Map/MapServer/tile/{z}/{y}/{x}",
            19,
            "#e9e5dc",
        ),
        Basemap::Satellite => (
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
            19,
            "#0a0e12",
        ),
        Basemap::Terrain => (
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Shaded_Relief/MapServer/tile/{z}/{y}/{x}",
            13,
            "#cfd6cf",
        ),
    }
}

/// Place-label overlay drawn above the radar; None when the basemap has its labels baked in.
fn label_tiles(basemap: Basemap) -> Option<&'static str> {
    match basemap {
        Basemap::Light | Basemap::Terrain => Some(
            "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Light_Gray_Reference/MapServer/tile/{z}/{y}/{x}",
        ),
        Basemap::Voyager => None,
        Basemap::BroadcastDark | Basemap::Satellite => Some(
            "https://server.arcgisonline.com/ArcGIS/rest/services/Canvas/World_Dark_Gray_Reference/MapServer/tile/{z}/{y}/{x}",
        ),
    }
}

/// FNV-1a, for stable layer ids derived from tile URLs.
fn short_hash(text: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in text.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", hash as u32)
}

struct StyleBuilder {
    sources: serde_json::Map<String, Value>,
    layers: Vec<Value>,
}

impl StyleBuilder {
    /// Add a raster source + layer; returns the id (name + URL hash).
    fn raster(&mut self, name: &str, tiles: &str, maxzoom: u8, opacity: f32, attribution: &str) -> String {
        let id = format!("{name}-{}", short_hash(tiles));
        self.sources.insert(
            id.clone(),
            json!({
                "type": "raster",
                "tiles": [tiles],
                "tileSize": 512,
                "minzoom": 0,
                "maxzoom": maxzoom,
                "attribution": attribution,
            }),
        );
        self.layers.push(json!({
            "id": id,
            "type": "raster",
            "source": id,
            "source-layer": id,
            "paint": { "raster-opacity": opacity }
        }));
        id
    }

    /// A raster layer with full paint (map themes: brightness, contrast, saturation, hue).
    fn raster_paint(&mut self, name: &str, tiles: &str, maxzoom: u8, paint: Value, attribution: &str) {
        let id = self.raster(name, tiles, maxzoom, 1.0, attribution);
        if let Some(layer) = self.layers.iter_mut().rev().find(|l| l.get("id").and_then(|v| v.as_str()) == Some(id.as_str())) {
            layer["paint"] = paint;
        }
    }
}

/// Satellite opacity for new scenes (and scenes saved with the old faint 0.55).
pub const SAT_OPACITY: f32 = 0.9;

/// The full style for `scene`, showing radar frame `radar_time` (when the radar is on).
/// `refresh` changes whenever "latest" products (single-site radar, GOES, MRMS) should be
/// fetched again; it is part of their URLs.
pub fn build_style(
    scene: &MapScene,
    radar: Option<&RadarIndex>,
    radar_time: Option<i64>,
    api_base: &str,
    refresh: i64,
) -> Value {
    let api = api_base.trim_end_matches('/');
    let (base_tiles, base_max, background) = basemap_tiles(scene.basemap);
    let theme = crate::maptheme::get(&scene.theme);

    let mut b = StyleBuilder {
        sources: serde_json::Map::new(),
        layers: vec![json!({
            "id": "background",
            "type": "background",
            "paint": { "background-color": theme.as_ref().map(|t| t.background.clone()).unwrap_or_else(|| background.to_string()) }
        })],
    };

    match &theme {
        // Custom map theme: base, terrain and water under the weather, each with its own colours.
        Some(t) => {
            if let (true, Some(tiles)) = (t.base_adjust.on, crate::maptheme::base_tiles(&t.base)) {
                b.raster_paint("theme-base", tiles, 16, t.base_adjust.paint(), "Esri, HERE, Garmin, © OpenStreetMap contributors");
            }
            if t.terrain.on {
                b.raster_paint("theme-terrain", crate::maptheme::TERRAIN_TILES, 16, t.terrain.paint(), "Esri");
            }
            if t.water.on {
                b.raster_paint("theme-water", crate::maptheme::WATER_TILES, 16, t.water.paint(), "Esri");
            }
        }
        None => {
            b.raster("basemap", base_tiles, base_max, 1.0, "Esri, HERE, Garmin, © OpenStreetMap contributors");
        }
    }

    // Event replay: archived satellite and radar for the replay time instead of the live loop.
    let replay_time = scene.history_time.or_else(|| scene.is_radar_replay().then_some(radar_time).flatten());
    if let Some(t) = replay_time {
        if scene.satellite.enabled {
            b.raster(&format!("hist-sat-{}", t - t.rem_euclid(600)), &crate::history::satellite_tiles(t), 7, scene.satellite.opacity, "NASA GIBS GOES-East");
        }
        if scene.radar.enabled {
            if let Some(replay) = &scene.site_replay {
                let tiles = format!("{api}/api/weatherbroadcaster/archive-velocity-tile?site={}&time={t}&z={{z}}&x={{x}}&y={{y}}", replay.site);
                b.raster(&format!("hist-velocity-{t}"), &tiles, 10, scene.radar.opacity, "NEXRAD velocity archive via IEM");
            } else {
                b.raster(&format!("hist-radar-{}", t - t.rem_euclid(300)), &crate::history::radar_tiles(t), 8, scene.radar.opacity, "NEXRAD archive via Iowa Environmental Mesonet");
            }
        }
    }
    // Satellite, under the radar. Greyscale IR at half opacity all but vanished on the dark
    // base maps (CWXB-51): draw it strong, with the contrast stretched so cold cloud tops read
    // bright white and warm ground stays dark. Scenes saved with the old 0.55 default get the
    // new one.
    if scene.satellite.enabled && replay_time.is_none() {
        let opacity = if (scene.satellite.opacity - 0.55).abs() < 1e-3 { SAT_OPACITY } else { scene.satellite.opacity };
        let paint = json!({
            "raster-opacity": opacity,
            "raster-contrast": 0.35,
            "raster-brightness-min": 0.0,
            "raster-brightness-max": 1.0,
        });
        match scene.satellite.channel {
            SatChannel::Infrared => {
                if let Some(frame) = radar.and_then(|r| r.satellite.infrared.last()) {
                    let tiles = format!(
                        "{api}/api/radar?path=v2/satellite/{}/512/{{z}}/{{x}}/{{y}}/0/0_0.webp",
                        frame.time
                    );
                    b.raster_paint("satellite", &tiles, 12, paint, "LibreWXR");
                }
            }
            channel => {
                let tiles = format!("{IEM}/{}/{{z}}/{{x}}/{{y}}.png?r={refresh}", channel.iem_layer());
                b.raster_paint("satellite", &tiles, 9, paint, "GOES-East via Iowa Environmental Mesonet");
            }
        }
    }

    // MRMS multi-sensor rainfall totals.
    if let Some(window) = scene.precip_total {
        let tiles = format!("{IEM}/{}/{{z}}/{{x}}/{{y}}.png?r={refresh}", window.iem_layer());
        b.raster("mrms", &tiles, 10, 0.85, "NOAA MRMS via Iowa Environmental Mesonet");
    }

    if scene.radar.enabled && replay_time.is_none() {
        match scene.radar.source {
            RadarSource::Composite => {
                // One source/layer per frame; the visible one gets the scene opacity.
                let palette = scene.radar.palette.trim();
                let smoothing = if scene.radar.smoothing { "1" } else { "0" };
                // The palette is part of the layer name: the renderer keeps a layer's tiles across
                // style changes, so without it a palette switch left old frames in old colours.
                let winter = if palette == "4" && crate::map::winter_radar::enabled() { "w" } else { "" };
                let tag = format!("{palette}{winter}{smoothing}");
                let frames = radar_frames(scene, radar);
                let visible = radar_time.or_else(|| frames.last().copied());
                for time in &frames {
                    let tiles = format!(
                        "{api}/api/radar?path=v2/radar/{time}/512/{{z}}/{{x}}/{{y}}/{palette}/{smoothing}_1.webp"
                    );
                    let opacity = if Some(*time) == visible { scene.radar.opacity } else { 0.0 };
                    // Adaptive resolution: under load, radar tiles come from a lower zoom (fewer,
                    // upscaled tiles). The zoom is in the layer name so the change takes effect.
                    match scene.radar_maxzoom_cap {
                        Some(z) if z < 12 => b.raster(&format!("radar-z{z}-{tag}-{time}"), &tiles, z, opacity, "LibreWXR"),
                        _ => b.raster(&format!("radar-{tag}-{time}"), &tiles, 12, opacity, "LibreWXR"),
                    };
                }
            }
            RadarSource::Site => {
                let site = scene.radar.resolved_site(scene.view.lat, scene.view.lon);
                let tiles = format!(
                    "{IEM}/ridge::{site}-{}-0/{{z}}/{{x}}/{{y}}.png?r={refresh}",
                    scene.radar.product.code()
                );
                // Reflectivity: clear-air filter and (Modern HD) TWC colours via the tile filter;
                // the settings are in the layer id so a change reloads the tiles (CWXB-88).
                let id = match scene.radar.product {
                    crate::map::scene::SiteProduct::SuperRes | crate::map::scene::SiteProduct::Reflectivity => crate::map::site_radar::layer_id(scene.radar.palette.trim() == "4", scene.radar.site_min_dbz),
                    _ => "nexrad".to_string(),
                };
                b.raster(&id, &tiles, 12, scene.radar.opacity, "NWS NEXRAD via Iowa Environmental Mesonet");
            }
        }
    }

    if let Some(t) = &theme {
        if t.roads.on {
            b.raster_paint("theme-roads", crate::maptheme::ROAD_TILES, 16, t.roads.paint(), "Esri");
        }
        if scene.labels && t.labels.on {
            b.raster_paint("theme-labels", crate::maptheme::label_tiles(&t.labels_style), 16, t.labels.paint(), "Esri");
        }
    } else if scene.highways {
        b.raster(
            "highways",
            "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Transportation/MapServer/tile/{z}/{y}/{x}",
            16,
            1.0,
            "Esri",
        );
    }
    if let (true, Some(tiles), true) = (scene.labels, label_tiles(scene.basemap), theme.is_none()) {
        b.raster("labels", tiles, 16, 1.0, "Esri");
    }

    for source in b.sources.values_mut() {
        if let Some(tiles) = source.get_mut("tiles").and_then(Value::as_array_mut) {
            for tile in tiles {
                if let Some(url) = tile.as_str().filter(|url| url.starts_with("https://mesonet.agron.iastate.edu/") || url.starts_with("https://gibs.earthdata.nasa.gov/")) {
                    *tile = Value::String(crate::net::proxy_url(api, url));
                }
            }
        }
    }

    json!({
        "version": 8,
        "name": "OpenWX Broadcast",
        "center": [scene.view.lat, scene.view.lon],
        "zoom": scene.view.zoom,
        "pitch": 0.0,
        "sources": Value::Object(b.sources),
        "layers": b.layers,
    })
}

/// Site id actually shown ("AUTO" resolves to the radar nearest the map centre).
pub fn site_for(site: &str, lat: f64, lon: f64) -> String {
    if site.eq_ignore_ascii_case("AUTO") || site.trim().is_empty() {
        nearest_site(lat, lon).0.to_string()
    } else {
        site.to_uppercase()
    }
}
