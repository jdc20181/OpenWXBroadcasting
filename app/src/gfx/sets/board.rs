//! County comparison, timeline and manual-zone canvases. One payload owns the shared
//! projection and legend; panels cannot accidentally use different bounds or colour scales.
use crate::gfx::text::{Align, GlyphCache, TextStyle, Weight, draw_text_box, measure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    hash::{Hash, Hasher},
    sync::{Arc, Mutex, OnceLock},
};
use tiny_skia::{
    Color, FillRule, Mask, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform,
};

// Preserve native placard sizing while scaling the complete canvas for preview/4K.
thread_local! {static DEVICE_SCALE: std::cell::Cell<f32> = const {std::cell::Cell::new(1.)};}
struct BoardDeviceScale(f32);
impl BoardDeviceScale {fn new(value:f32)->Self {Self(DEVICE_SCALE.with(|s|s.replace(value)))}}
impl Drop for BoardDeviceScale {fn drop(&mut self){DEVICE_SCALE.with(|s|s.set(self.0));}}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Layout {
    Compare,
    Timeline,
    Zones,
    Single,
}
impl Layout {
    pub fn key(self) -> &'static str {
        match self {
            Self::Compare => "compare",
            Self::Timeline => "timeline",
            Self::Zones => "zones",
            Self::Single => "single",
        }
    }
    pub fn panels(self) -> usize {
        match self {
            Self::Compare => 2,
            Self::Timeline => 4,
            _ => 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Scope {
    pub states: Vec<String>,
    pub county: Option<String>,
    /// West, south, east, north; when present, a regional view crossing state lines.
    pub bbox: Option<[f64; 4]>,
}
impl Default for Scope {
    fn default() -> Self {
        Self {
            states: vec!["18".into()],
            county: None,
            bbox: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum Palette {
    #[default]
    Category,
    RainTotal,
    SnowTotal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Band {
    pub label: String,
    pub color: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Zone {
    pub category: usize,
    pub ring: Vec<(f64, f64)>,
    #[serde(default)]
    pub label: String,
    pub label_at: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct ReportPoint {
    pub id: String,
    pub lon: f64,
    pub lat: f64,
    pub label: String,
    #[serde(default)]
    pub category: usize,
    pub value: Option<f32>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Panel {
    pub title: String,
    pub headline: String,
    pub valid: String,
    pub detail: String,
    /// None is missing. Some(empty) is an explicitly reviewed empty result.
    pub counties: Option<BTreeMap<String, usize>>,
    pub zones: Option<Vec<Zone>>,
    pub points: Option<Vec<ReportPoint>>,
    /// Optional ranked place/value sidebar for point-report and cold maps.
    pub summary: Option<Vec<(String, String)>>,
    pub summary_title: String,
}
impl Panel {
    pub fn available(&self) -> bool {
        self.counties.is_some() || self.zones.is_some() || self.points.is_some()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Scene {
    pub reviewed: bool,
    pub scope: Scope,
    pub palette: Palette,
    pub legend_title: String,
    pub legend: Vec<Band>,
    pub panels: Vec<Panel>,
}

pub(super) fn bands(scene: &Scene) -> Vec<Band> {
    let stops = match scene.palette {
        Palette::Category => return scene.legend.clone(),
        Palette::RainTotal => crate::grid::RAIN_TOTAL_STOPS,
        Palette::SnowTotal => crate::grid::SNOW_TOTAL_STOPS,
    };
    stops
        .iter()
        .map(|(n, c)| Band {
            label: format!("{n} in"),
            color: format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
        })
        .collect()
}

fn geo(lon: f64, lat: f64) -> bool {
    lon.is_finite()
        && lat.is_finite()
        && (-180.0..=180.).contains(&lon)
        && (-85.0..=85.).contains(&lat)
}
fn digits(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|c| c.is_ascii_digit())
}

pub(super) fn parse(raw: &str, layout: Layout) -> Option<Scene> {
    let mut s: Scene = serde_json::from_str(raw).ok()?;
    if s.panels.len() != layout.panels()
        || s.scope.states.len() > 8
        || s.scope.states.iter().any(|v| !digits(v, 2))
        || s.scope.county.as_ref().is_some_and(|v| !digits(v, 5))
    {
        return None;
    }
    if let Some([w, so, e, n]) = s.scope.bbox {
        if !geo(w, so) || !geo(e, n) || w >= e || so >= n || e - w > 70. || n - so > 55. {
            return None;
        }
    }
    let legend = bands(&s);
    if legend.len() > 8
        || legend
            .iter()
            .any(|b| b.label.chars().count() > 55 || crate::gfx::parse_hex(&b.color).is_none())
    {
        return None;
    }
    if !s.reviewed {
        for p in &mut s.panels {
            p.counties = None;
            p.zones = None;
            p.points = None;
            p.headline.clear();
            if p.summary.is_some() {
                p.summary = Some(Vec::new());
            }
        }
    }
    for p in &mut s.panels {
        if p.title.chars().count() > 85
            || p.headline.chars().count() > 60
            || p.summary_title.chars().count() > 70
            || p.summary.as_ref().is_some_and(|rows| {
                rows.len() > 7
                    || rows.iter().any(|(place, value)| {
                        place.chars().count() > 100 || value.chars().count() > 40
                    })
            })
            || p.valid.chars().count() > 90
            || p.detail.chars().count() > 130
        {
            return None;
        }
        if s.reviewed && p.available() && layout == Layout::Timeline && p.valid.trim().is_empty() {
            return None;
        }
        if let Some(values) = &p.counties {
            if values.len() > 3500
                || values
                    .iter()
                    .any(|(f, i)| !digits(f, 5) || *i >= legend.len())
            {
                return None;
            }
        }
        if let Some(zones) = &p.zones {
            if zones.len() > 512
                || zones.iter().any(|z| {
                    z.category >= legend.len()
                        || z.ring.len() < 3
                        || z.ring.len() > 4096
                        || z.ring.iter().any(|&(x, y)| !geo(x, y))
                        || z.label.chars().count() > 60
                        || z.label_at.is_some_and(|(x, y)| !geo(x, y))
                })
            {
                return None;
            }
        }
        if let Some(points) = &mut p.points {
            if points.len() > 10000
                || points.iter().any(|r| {
                    r.id.is_empty()
                        || !geo(r.lon, r.lat)
                        || r.label.chars().count() > 90
                        || r.category >= legend.len()
                        || r.value.is_some_and(|v| !v.is_finite() || v < 0.)
                })
            {
                return None;
            }
            let mut seen = std::collections::HashSet::new();
            points.retain(|r| seen.insert(r.id.clone()));
        }
    }
    Some(s)
}

pub(super) fn empty(layout: Layout) -> Scene {
    Scene {
        panels: (0..layout.panels())
            .map(|i| Panel {
                title: match layout {
                    Layout::Compare => if i == 0 {
                        "First field"
                    } else {
                        "Second field"
                    }
                    .into(),
                    Layout::Timeline => format!("Frame {}", i + 1),
                    Layout::Zones => "Forecast zones".into(),
                    Layout::Single => "Reported observations".into(),
                },
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    }
}

#[derive(Clone, Copy)]
struct Box2 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Box2 {
    fn overlaps(self, b: Self, gap: f32) -> bool {
        self.x < b.x + b.w + gap
            && self.x + self.w + gap > b.x
            && self.y < b.y + b.h + gap
            && self.y + self.h + gap > b.y
    }
}
fn color(hex: &str) -> Color {
    crate::gfx::parse_hex(hex).unwrap_or(Color::WHITE)
}
fn paint(hex: &str) -> Paint<'static> {
    let mut p = Paint::default();
    p.anti_alias = true;
    p.set_color(color(hex));
    p
}
fn rect(pm: &mut Pixmap, b: Box2, c: &str) {
    if let Some(r) = Rect::from_xywh(b.x, b.y, b.w, b.h) {
        pm.fill_rect(r, &paint(c), Transform::identity(), None);
    }
}
fn words(
    pm: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    text: &str,
    b: Box2,
    size: f32,
    c: &str,
    align: Align,
) {
    let mut st = TextStyle {
        family: font.into(),
        weight: Weight::SemiBold,
        size,
        color: color(c),
        letter_spacing: 0.,
    };
    // The shared text fitter has an 8px native floor. Scale that floor too,
    // otherwise preview labels stop shrinking before their boxes fit.
    let device_scale=DEVICE_SCALE.with(|s|s.get());
    for _ in 0..24 {
        let lines=crate::gfx::text::wrap(text,&st,b.w);
        let widest=lines.iter().map(|line|measure(line,&st)).fold(0.,f32::max);
        let (ascent,descent)=crate::gfx::text::line_metrics(&st);
        let block=(ascent+descent)*1.08*lines.len() as f32;
        if (widest<=b.w+0.5*device_scale && block<=b.h+0.5*device_scale)||st.size<=8.*device_scale {break;}
        st.size*=0.92;
    }
    draw_text_box(
        pm,
        g,
        text,
        (b.x, b.y, b.w, b.h),
        &st,
        align,
        true,
        true,
        false,
        false,
        1.,
    );
}

pub(super) fn centre(c: &crate::map::boundaries::County) -> (f64, f64) {
    let n = c.rings.iter().map(Vec::len).sum::<usize>().max(1) as f64;
    let v = c
        .rings
        .iter()
        .flatten()
        .fold((0., 0.), |a, p| (a.0 + p.0, a.1 + p.1));
    (v.0 / n, v.1 / n)
}
pub(super) fn inside(c: &crate::map::boundaries::County, lon: f64, lat: f64) -> bool {
    c.rings.iter().fold(false, |v, r| {
        v ^ crate::gfx::winter_map::inside_pub(r, lon, lat)
    })
}

pub(super) fn selected<'a>(
    all: &'a [crate::map::boundaries::County],
    scope: &Scope,
) -> Vec<&'a crate::map::boundaries::County> {
    all.iter()
        .filter(|c| {
            if let Some(f) = &scope.county {
                return &c.fips == f;
            }
            if let Some([w, s, e, n]) = scope.bbox {
                let (x, y) = centre(c);
                return (w..=e).contains(&x) && (s..=n).contains(&y);
            }
            scope.states.iter().any(|v| v == &c.state)
        })
        .collect()
}

#[derive(Clone, Copy)]
struct Fit {
    west: f64,
    north: f64,
    k: f64,
    s: f64,
    ox: f64,
    oy: f64,
}
impl Fit {
    fn new(b: [f64; 4], w: f32, h: f32) -> Self {
        let [west, south, east, north] = b;
        let k = ((south + north) / 2.).to_radians().cos();
        let (gw, gh) = ((east - west) * k, north - south);
        let s = (w as f64 * 0.9 / gw).min(h as f64 * 0.9 / gh);
        Self {
            west,
            north,
            k,
            s,
            ox: (w as f64 - gw * s) / 2.,
            oy: (h as f64 - gh * s) / 2.,
        }
    }
    fn to(self, lon: f64, lat: f64) -> (f32, f32) {
        (
            (self.ox + (lon - self.west) * self.k * self.s) as f32,
            (self.oy + (self.north - lat) * self.s) as f32,
        )
    }
}

#[derive(Clone, Eq)]
struct MapCacheKey {
    boundary_identity: usize,
    states: Vec<String>,
    county: Option<String>,
    bbox: Option<[u64; 4]>,
    width: u32,
    height: u32,
    reserve: bool,
}
impl PartialEq for MapCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.boundary_identity == other.boundary_identity
            && self.states == other.states
            && self.county == other.county
            && self.bbox == other.bbox
            && self.width == other.width
            && self.height == other.height
            && self.reserve == other.reserve
    }
}
impl Hash for MapCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.boundary_identity.hash(state);
        self.states.hash(state);
        self.county.hash(state);
        self.bbox.hash(state);
        self.width.hash(state);
        self.height.hash(state);
        self.reserve.hash(state);
    }
}
impl MapCacheKey {
    fn new(
        all: &Arc<Vec<crate::map::boundaries::County>>,
        scope: &Scope,
        width: u32,
        height: u32,
        reserve: bool,
    ) -> Self {
        Self {
            boundary_identity: Arc::as_ptr(all) as usize,
            states: scope.states.clone(),
            county: scope.county.clone(),
            bbox: scope.bbox.map(|b| b.map(f64::to_bits)),
            width,
            height,
            reserve,
        }
    }
}

struct MapGeometry {
    // Retain the source Arc so its pointer identity cannot be reused while cached.
    boundaries: Arc<Vec<crate::map::boundaries::County>>,
    county_indices: Vec<usize>,
    bbox: [f64; 4],
    fit: Fit,
    paths: Vec<Option<tiny_skia::Path>>,
    clip: Option<Mask>,
    base: Pixmap,
    estimated_bytes: usize,
}
#[derive(Default)]
struct MapGeometryCache {
    entries: HashMap<MapCacheKey, Arc<MapGeometry>>,
    order: VecDeque<MapCacheKey>,
    bytes: usize,
}
const MAP_CACHE_MAX_ENTRIES: usize = 16;
const MAP_CACHE_MAX_BYTES: usize = 128 * 1024 * 1024;
fn map_geometry_cache() -> &'static Mutex<MapGeometryCache> {
    static CACHE: OnceLock<Mutex<MapGeometryCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(MapGeometryCache::default()))
}
fn prepare_map_geometry(
    all: &Arc<Vec<crate::map::boundaries::County>>,
    scope: &Scope,
    width: u32,
    height: u32,
    reserve: bool,
    u: f32,
) -> Option<MapGeometry> {
    let counties = selected(all, scope);
    if counties.is_empty() {
        return None;
    }
    let indices = all
        .iter()
        .enumerate()
        .map(|(index, county)| (county as *const _ as usize, index))
        .collect::<HashMap<_, _>>();
    let county_indices = counties
        .iter()
        .filter_map(|county| indices.get(&(*county as *const _ as usize)).copied())
        .collect::<Vec<_>>();
    let bbox = scope.bbox.unwrap_or_else(|| {
        counties
            .iter()
            .flat_map(|c| c.rings.iter().flatten())
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, &(x, y)| {
                [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)]
            })
    });
    let map_h = (height as f32 - if reserve { 52. * u } else { 0. }).max(1.);
    let fit = Fit::new(bbox, width as f32, map_h);
    let paths: Vec<_> = counties.iter().map(|c| path(&c.rings, fit)).collect();
    let mut base = Pixmap::new(width, height)?;
    base.fill(color("#10243A"));
    let rim = Stroke {
        width: 3. * u,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    for p in paths.iter().flatten() {
        base.stroke_path(p, &paint("#A7BED5"), &rim, Transform::identity(), None);
    }
    let mut shape = PathBuilder::new();
    for p in &paths {
        if let Some(p) = p {
            base.fill_path(
                p,
                &paint("#2B435B"),
                FillRule::EvenOdd,
                Transform::identity(),
                None,
            );
            shape.push_path(p);
        }
    }
    let clip = shape.finish().and_then(|p| {
        let mut mask = Mask::new(width, height)?;
        mask.fill_path(&p, FillRule::Winding, true, Transform::identity());
        Some(mask)
    });
    // Includes RGBA base, 1-byte mask, and a conservative path-point allowance.
    let path_points = counties
        .iter()
        .flat_map(|c| &c.rings)
        .map(Vec::len)
        .sum::<usize>();
    let estimated_bytes = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(5)
        .saturating_add(path_points.saturating_mul(32));
    Some(MapGeometry {
        boundaries: Arc::clone(all),
        county_indices,
        bbox,
        fit,
        paths,
        clip,
        base,
        estimated_bytes,
    })
}
fn cached_map_geometry(
    all: &Arc<Vec<crate::map::boundaries::County>>,
    scope: &Scope,
    width: u32,
    height: u32,
    reserve: bool,
    u: f32,
    cache: bool,
) -> Option<Arc<MapGeometry>> {
    if !cache {
        return prepare_map_geometry(all, scope, width, height, reserve, u).map(Arc::new);
    }
    let key = MapCacheKey::new(all, scope, width, height, reserve);
    if let Ok(mut cache) = map_geometry_cache().lock() {
        if let Some(value) = cache.entries.get(&key).cloned() {
            cache.order.retain(|entry| entry != &key);
            cache.order.push_back(key);
            return Some(value);
        }
    }
    let value = Arc::new(prepare_map_geometry(all, scope, width, height, reserve, u)?);
    if value.estimated_bytes > MAP_CACHE_MAX_BYTES {
        return Some(value);
    }
    if let Ok(mut cache) = map_geometry_cache().lock() {
        if let Some(existing) = cache.entries.get(&key).cloned() {
            cache.order.retain(|entry| entry != &key);
            cache.order.push_back(key);
            return Some(existing);
        }
        cache.bytes = cache.bytes.saturating_add(value.estimated_bytes);
        cache.entries.insert(key.clone(), Arc::clone(&value));
        cache.order.push_back(key);
        while cache.entries.len() > MAP_CACHE_MAX_ENTRIES || cache.bytes > MAP_CACHE_MAX_BYTES {
            let Some(expired) = cache.order.pop_front() else {
                break;
            };
            if let Some(old) = cache.entries.remove(&expired) {
                cache.bytes = cache.bytes.saturating_sub(old.estimated_bytes);
            }
        }
    }
    Some(value)
}
fn path(rings: &[Vec<(f64, f64)>], fit: Fit) -> Option<tiny_skia::Path> {
    let mut b = PathBuilder::new();
    for ring in rings {
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            let (x, y) = fit.to(lon, lat);
            if i == 0 {
                b.move_to(x, y)
            } else {
                b.line_to(x, y)
            }
        }
        b.close();
    }
    b.finish()
}
fn value_band(scene: &Scene, value: f32, fallback: usize) -> usize {
    let stops = match scene.palette {
        Palette::Category => return fallback,
        Palette::RainTotal => crate::grid::RAIN_TOTAL_STOPS,
        Palette::SnowTotal => crate::grid::SNOW_TOTAL_STOPS,
    };
    stops
        .iter()
        .rposition(|(min, _)| value >= *min)
        .unwrap_or(0)
}

fn map(
    g: &mut GlyphCache,
    font: &str,
    scene: &Scene,
    panel: &Panel,
    legend: &[Band],
    width: u32,
    height: u32,
) -> Option<Pixmap> {
    map_impl(g, font, scene, panel, legend, width, height, true)
}
fn map_impl(
    g: &mut GlyphCache,
    font: &str,
    scene: &Scene,
    panel: &Panel,
    legend: &[Band],
    width: u32,
    height: u32,
    use_cache: bool,
) -> Option<Pixmap> {
    let all = crate::map::boundaries::counties_named();
    let device_scale=DEVICE_SCALE.with(|s|s.get());
    let native_width=width as f32/device_scale;
    let native_height=height as f32/device_scale;
    let width_base = if native_width < 600. && native_height > native_width {400.}else{850.};
    let u=(native_width/width_base).min(native_height/475.).clamp(0.8,1.3)*device_scale;
    let reserve = !panel.available();
    let reserve_px = if reserve { 52. * u } else { 0. };
    let geometry = all.as_ref().and_then(|all| {
        cached_map_geometry(all, &scene.scope, width, height, reserve, u, use_cache)
    });
    let Some(geometry) = geometry else {
        let mut out = Pixmap::new(width, height)?;
        out.fill(color("#10243A"));
        words(
            &mut out,
            g,
            font,
            "County map not available yet",
            Box2 {
                x: 20. * u,
                y: 0.,
                w: width as f32 - 40. * u,
                h: height as f32,
            },
            28. * u,
            "#D9E6F4",
            Align::Center,
        );
        return Some(out);
    };
    let counties = geometry
        .county_indices
        .iter()
        .filter_map(|&index| geometry.boundaries.get(index))
        .collect::<Vec<_>>();
    let paths = &geometry.paths;
    let bbox = geometry.bbox;
    let fit = geometry.fit;
    let map_h = (height as f32 - reserve_px).max(1.);
    let clip = geometry.clip.as_ref();
    let mut out = geometry.base.clone();
    let mut occupied = Vec::new();
    // Render categories in order across BOTH county and polygon shapes. A lower
    // alert polygon must never paint over a higher-level county warning.
    for (i, band) in legend.iter().enumerate() {
        for (c, p) in counties.iter().zip(paths.iter()) {
            if panel.counties.as_ref().and_then(|v| v.get(&c.fips)) == Some(&i) {
                if let Some(p) = p {
                    out.fill_path(
                        p,
                        &paint(&band.color),
                        FillRule::EvenOdd,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        for z in panel.zones.iter().flatten().filter(|z| z.category == i) {
            if let Some(p) = path(std::slice::from_ref(&z.ring), fit) {
                out.fill_path(
                    &p,
                    &paint(&band.color),
                    FillRule::Winding,
                    Transform::identity(),
                    clip,
                );
            }
        }
    }
    let edge = Stroke {
        width: (0.8 * u).max(0.5),
        ..Default::default()
    };
    for p in paths.iter().flatten() {
        out.stroke_path(p, &paint("#9FB1C2"), &edge, Transform::identity(), None);
    }
    for z in panel.zones.iter().flatten() {
        if z.label.trim().is_empty() {
            continue;
        }
        let (lon, lat) = z.label_at.unwrap_or_else(|| {
            let n = z.ring.len() as f64;
            let a = z.ring.iter().fold((0., 0.), |a, p| (a.0 + p.0, a.1 + p.1));
            (a.0 / n, a.1 / n)
        });
        let (x, y) = fit.to(lon, lat);
        let bw = (measure(
            &z.label,
            &TextStyle {
                family: font.into(),
                weight: Weight::SemiBold,
                size: 25. * u,
                color: Color::WHITE,
                letter_spacing: 0.,
            },
        ) + 24. * u)
            .min(width as f32 - 20. * u);
        // A zone smaller on screen than its label would vanish under it: let the legend
        // name it instead.
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(lon, lat) in &z.ring {
            let (px, py) = fit.to(lon, lat);
            (x0, y0, x1, y1) = (x0.min(px), y0.min(py), x1.max(px), y1.max(py));
        }
        if x1 - x0 < bw * 0.9 || y1 - y0 < 48. * u * 1.5 {
            continue;
        }
        // Nor label a zone whose centre lies off the map (it would sit on the edge).
        if !(0.0..=width as f32).contains(&x) || !(0.0..=map_h).contains(&y) {
            continue;
        }
        let b = Box2 {
            x: (x - bw / 2.).clamp(8. * u, (width as f32 - bw - 8. * u).max(8. * u)),
            y: (y - 24. * u).clamp(0., (map_h - 48. * u).max(0.)),
            w: bw,
            h: 48. * u,
        };
        if occupied.iter().any(|&v| b.overlaps(v, 6. * u)) {
            continue;
        }
        rect(&mut out, b, "#091727");
        words(
            &mut out,
            g,
            font,
            &z.label,
            b,
            25. * u,
            "#FFFFFF",
            Align::Center,
        );
        occupied.push(b);
    }
    let points: Vec<_> = panel
        .points
        .iter()
        .flatten()
        .filter(|r| counties.iter().any(|c| inside(c, r.lon, r.lat)))
        .collect();
    // Reserve every marker before placing labels, so later markers cannot cover text.
    for r in &points {
        let (x, y) = fit.to(r.lon, r.lat);
        let i = r
            .value
            .map(|v| value_band(scene, v, r.category))
            .unwrap_or(r.category);
        if let Some(p) = PathBuilder::from_circle(x, y, 7. * u) {
            out.fill_path(
                &p,
                &paint("#091727"),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        if let Some(p) = PathBuilder::from_circle(x, y, 5. * u) {
            out.fill_path(
                &p,
                &paint(&legend[i].color),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        occupied.push(Box2 {
            x: x - 9. * u,
            y: y - 9. * u,
            w: 18. * u,
            h: 18. * u,
        });
    }
    let point_style = TextStyle {
        family: font.into(),
        weight: Weight::SemiBold,
        size: 22. * u,
        color: Color::WHITE,
        letter_spacing: 0.,
    };
    for r in points {
        if !r.label.is_empty() {
            let (x, y) = fit.to(r.lon, r.lat);
            let w = (measure(&r.label, &point_style) + 16. * u).clamp(80. * u, 280. * u);
            let h = 34. * u;
            for (bx, by) in [
                (x + 14. * u, y - h / 2.),
                (x - w - 14. * u, y - h / 2.),
                (x - w / 2., y - h - 14. * u),
                (x - w / 2., y + 14. * u),
            ] {
                let b = Box2 { x: bx, y: by, w, h };
                if b.x < 0.
                    || b.y < 0.
                    || b.x + b.w > width as f32
                    || b.y + b.h > map_h
                    || occupied.iter().any(|&v| b.overlaps(v, 3. * u))
                {
                    continue;
                }
                rect(&mut out, b, "#091727");
                words(
                    &mut out,
                    g,
                    font,
                    &r.label,
                    b,
                    22. * u,
                    "#FFFFFF",
                    Align::Center,
                );
                occupied.push(b);
                break;
            }
        }
    }
    let mut towns = crate::places::in_box(bbox[1], bbox[3], bbox[0], bbox[2]);
    towns.sort_by(|a, b| b.population.cmp(&a.population));
    let style = TextStyle {
        family: font.into(),
        weight: Weight::SemiBold,
        size: 18. * u,
        color: Color::WHITE,
        letter_spacing: 0.,
    };
    let mut placed = 0;
    for town in towns {
        if placed >= 3 {
            break;
        }
        if !counties.iter().any(|c| inside(c, town.lon, town.lat)) {
            continue;
        }
        let (x, y) = fit.to(town.lon, town.lat);
        let tw = (measure(&town.name, &style) + 16. * u).min(width as f32 - 20. * u);
        for (dx, dy) in [(0., 12.), (0., -42.), (12., -16.), (-12., -16.)] {
            let b = Box2 {
                x: x - tw / 2. + dx * u,
                y: y + dy * u,
                w: tw,
                h: 30. * u,
            };
            if b.x < 0.
                || b.x + b.w > width as f32
                || b.y < 0.
                || b.y + b.h > map_h
                || occupied.iter().any(|&v| b.overlaps(v, 7. * u))
            {
                continue;
            }
            rect(&mut out, b, "#091727");
            words(
                &mut out,
                g,
                font,
                &town.name,
                b,
                18. * u,
                "#FFFFFF",
                Align::Center,
            );
            occupied.push(b);
            placed += 1;
            break;
        }
    }
    if !panel.available() {
        let b = Box2 {
            x: 0.,
            y: height as f32 - reserve_px,
            w: width as f32,
            h: reserve_px,
        };
        rect(&mut out, b, "#091727");
        words(
            &mut out,
            g,
            font,
            super::reference::MISSING,
            b,
            25. * u,
            "#FFFFFF",
            Align::Center,
        );
    }
    // All category fills are solid; the board is blended only once by the caller.
    Some(out)
}

fn arrow(pm: &mut Pixmap, from: (f32, f32), to: (f32, f32), u: f32) {
    let mut b = PathBuilder::new();
    b.move_to(from.0, from.1);
    b.line_to(to.0, to.1);
    if let Some(p) = b.finish() {
        pm.stroke_path(
            &p,
            &paint("#ABC9E5"),
            &Stroke {
                width: 3. * u,
                ..Default::default()
            },
            Transform::identity(),
            None,
        );
    }
    let a = (to.1 - from.1).atan2(to.0 - from.0);
    let mut b = PathBuilder::new();
    b.move_to(to.0, to.1);
    for d in [-0.55, 0.55] {
        b.line_to(
            to.0 - 13. * u * (a + d).cos(),
            to.1 - 13. * u * (a + d).sin(),
        );
    }
    b.close();
    if let Some(p) = b.finish() {
        pm.fill_path(
            &p,
            &paint("#ABC9E5"),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

#[cfg(test)]
mod geometry_cache_tests {
    use super::*;

    fn scope(county: Option<&str>, state: &str) -> Scope {
        Scope {
            states: vec![state.into()],
            county: county.map(str::to_owned),
            bbox: None,
        }
    }

    fn colored_scene(scope: Scope, category: usize) -> Scene {
        let mut county_values = BTreeMap::new();
        county_values.insert("18019".into(), category);
        Scene {
            reviewed: true,
            scope,
            palette: Palette::Category,
            legend_title: "Observed warning category".into(),
            legend: vec![
                Band {
                    label: "Lower category".into(),
                    color: "#147A38".into(),
                },
                Band {
                    label: "Higher category".into(),
                    color: "#C82D2D".into(),
                },
            ],
            panels: vec![Panel {
                counties: Some(county_values),
                zones: Some(Vec::new()),
                points: Some(Vec::new()),
                ..Default::default()
            }],
        }
    }

    fn render(scene: &Scene, width: u32, height: u32, use_cache: bool) -> Vec<u8> {
        let legend = bands(scene);
        map_impl(
            &mut GlyphCache::default(),
            "cache-test-font",
            scene,
            &scene.panels[0],
            &legend,
            width,
            height,
            use_cache,
        )
        .unwrap()
        .data()
        .to_vec()
    }

    #[test]
    fn cached_map_pixels_match_uncached_for_current_scene_and_geometry_changes() {
        // Boundary data is an application cache and is not checked into source control.
        // Skip this pixel-level regression on clean machines before the normal boundary
        // download has completed; all assertions run when the map data is present.
        let Some(all) = crate::map::boundaries::counties_named() else {
            return;
        };
        if !all.iter().any(|county| county.fips == "18019") {
            return;
        }

        let county = colored_scene(scope(Some("18019"), "18"), 0);
        let cached_county = render(&county, 420, 260, true);
        assert_eq!(cached_county, render(&county, 420, 260, false));

        // Reuse identical geometry while current weather/category fills change.
        let higher = colored_scene(scope(Some("18019"), "18"), 1);
        let cached_higher = render(&higher, 420, 260, true);
        assert_eq!(cached_higher, render(&higher, 420, 260, false));
        assert_ne!(cached_county, cached_higher);

        // Missing panels reserve the lower banner; state scope and dimensions change fit.
        let missing = Scene {
            reviewed: true,
            scope: scope(Some("18019"), "18"),
            panels: vec![Panel::default()],
            ..Default::default()
        };
        assert_eq!(
            render(&missing, 420, 260, true),
            render(&missing, 420, 260, false)
        );
        let state = colored_scene(scope(None, "18"), 1);
        assert_eq!(
            render(&state, 520, 340, true),
            render(&state, 520, 340, false)
        );
    }

    #[test]
    fn unknown_empty_scope_keeps_the_original_opaque_background() {
        let scene = Scene {
            reviewed: true,
            scope: Scope {
                states: Vec::new(),
                county: None,
                bbox: None,
            },
            panels: vec![Panel::default()],
            ..Default::default()
        };
        let pixels = render(&scene, 420, 260, true);
        assert_eq!(&pixels[..4], &[16, 36, 58, 255]);
    }
}

#[allow(clippy::too_many_arguments)]
/// A native phone layout: map above a two-column legend, followed by ranked places.
/// The map projection and collision handling are exactly the broadcast map renderer.
fn portrait_board(
    out: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    scene: &Scene,
    legend: &[Band],
) {
    let p = &scene.panels[0];
    let (sx, sy) = (out.width() as f32 / 972., out.height() as f32 / 1400.);
    let u = sx.min(sy);
    let scale = |b: Box2| Box2 {
        x: b.x * sx,
        y: b.y * sy,
        w: b.w * sx,
        h: b.h * sy,
    };
    macro_rules! text {
        ($value:expr, $b:expr, $size:expr, $color:expr, $align:expr) => {
            words(out, g, font, $value, scale($b), $size * u, $color, $align)
        };
    }
    let map_h = if p.summary.is_some() { 670. } else { 1000. };
    rect(
        out,
        scale(Box2 {
            x: 0.,
            y: 0.,
            w: 972.,
            h: map_h + 140.,
        }),
        "#132B44",
    );
    text!(
        &p.title,
        Box2 {
            x: 18.,
            y: 4.,
            w: 936.,
            h: 48.
        },
        34.,
        "#FFFFFF",
        Align::Left
    );
    let valid = if p.valid.trim().is_empty() {
        "Valid time not available yet"
    } else {
        &p.valid
    };
    let detail = if p.detail.trim().is_empty() {
        valid.to_string()
    } else {
        format!("{valid} • {}", p.detail)
    };
    text!(
        &detail,
        Box2 {
            x: 18.,
            y: 58.,
            w: 936.,
            h: 62.
        },
        25.,
        "#C2D4E6",
        Align::Left
    );
    let b = scale(Box2 {
        x: 8.,
        y: 130.,
        w: 956.,
        h: map_h,
    });
    if let Some(map) = map(
        g,
        font,
        scene,
        p,
        legend,
        b.w.round().max(1.) as u32,
        b.h.round().max(1.) as u32,
    ) {
        out.draw_pixmap(
            b.x.round() as i32,
            b.y.round() as i32,
            map.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    let legend_y = map_h + 158.;
    let title = if scene.legend_title.trim().is_empty() {
        "Legend not available yet"
    } else {
        &scene.legend_title
    };
    text!(
        title,
        Box2 {
            x: 10.,
            y: legend_y,
            w: 952.,
            h: 38.
        },
        27.,
        "#D5E4F2",
        Align::Left
    );
    for (i, band) in legend.iter().enumerate() {
        let (x, y) = (
            10. + (i % 2) as f32 * 486.,
            legend_y + 48. + (i / 2) as f32 * 48.,
        );
        rect(
            out,
            scale(Box2 {
                x,
                y: y + 6.,
                w: 28.,
                h: 28.,
            }),
            &band.color,
        );
        text!(
            &band.label,
            Box2 {
                x: x + 42.,
                y,
                w: 424.,
                h: 40.
            },
            25.,
            "#FFFFFF",
            Align::Left
        );
    }
    if let Some(rows) = &p.summary {
        text!(
            &p.summary_title,
            Box2 {
                x: 10.,
                y: 1090.,
                w: 952.,
                h: 36.
            },
            28.,
            "#C2D4E6",
            Align::Left
        );
        if rows.is_empty() {
            text!(
                super::reference::MISSING,
                Box2 {
                    x: 10.,
                    y: 1150.,
                    w: 952.,
                    h: 70.
                },
                28.,
                "#FFFFFF",
                Align::Left
            );
        }
        for (i, (place, value)) in rows.iter().take(7).enumerate() {
            let y = 1134. + i as f32 * 35.;
            rect(
                out,
                scale(Box2 {
                    x: 8.,
                    y,
                    w: 956.,
                    h: 31.,
                }),
                "#203B54",
            );
            text!(
                place,
                Box2 {
                    x: 18.,
                    y,
                    w: 700.,
                    h: 31.
                },
                25.,
                "#FFFFFF",
                Align::Left
            );
            text!(
                value,
                Box2 {
                    x: 728.,
                    y,
                    w: 220.,
                    h: 31.
                },
                27.,
                "#FFFFFF",
                Align::Right
            );
        }
    }
}

/// Square map layout: map and ranked places above a compact two-row legend.
/// Stacked comparisons, or a four-frame phone board with one shared legend.
fn portrait_multi(
    out: &mut Pixmap,
    g: &mut GlyphCache,
    font: &str,
    scene: &Scene,
    legend: &[Band],
    layout: Layout,
) {
    let (sx, sy) = (out.width() as f32 / 972., out.height() as f32 / 1400.);
    let u = sx.min(sy);
    let scale = |b: Box2| Box2 {
        x: b.x * sx,
        y: b.y * sy,
        w: b.w * sx,
        h: b.h * sy,
    };
    macro_rules! text {
        ($v:expr,$b:expr,$size:expr,$c:expr,$a:expr) => {
            words(out, g, font, $v, scale($b), $size * u, $c, $a)
        };
    }
    let timeline = layout == Layout::Timeline;
    let slots: Vec<_> = if timeline {
        (0..4)
            .map(|i| Box2 {
                x: (i % 2) as f32 * 506.,
                y: (i / 2) as f32 * 575.,
                w: 466.,
                h: 535.,
            })
            .collect()
    } else {
        (0..2)
            .map(|i| Box2 {
                x: 0.,
                y: i as f32 * 575.,
                w: 972.,
                h: 535.,
            })
            .collect()
    };
    for (i, (slot, p)) in slots.iter().zip(&scene.panels).enumerate() {
        rect(out, scale(*slot), "#132B44");
        let headline = !timeline && !p.headline.is_empty();
        let head = if headline { 170. } else { 110. };
        text!(
            &p.title,
            Box2 {
                x: slot.x + 14.,
                y: slot.y + 4.,
                w: slot.w - if timeline { 66. } else { 28. },
                h: 44.
            },
            if timeline { 27. } else { 34. },
            "#FFFFFF",
            Align::Left
        );
        if headline {
            text!(
                &p.headline,
                Box2 {
                    x: slot.x + 14.,
                    y: slot.y + 50.,
                    w: slot.w - 28.,
                    h: 60.
                },
                44.,
                "#FFFFFF",
                Align::Left
            );
        }
        let valid = if p.valid.trim().is_empty() {
            "Valid time not available yet"
        } else {
            &p.valid
        };
        let detail = if p.detail.trim().is_empty() {
            valid.to_string()
        } else {
            format!("{valid} • {}", p.detail)
        };
        text!(
            &detail,
            Box2 {
                x: slot.x + 14.,
                y: slot.y + if headline { 114. } else { 52. },
                w: slot.w - 28.,
                h: 48.
            },
            24.,
            "#C2D4E6",
            Align::Left
        );
        let b = scale(Box2 {
            x: slot.x + 8.,
            y: slot.y + head,
            w: slot.w - 16.,
            h: slot.h - head - 8.,
        });
        if let Some(map) = map(
            g,
            font,
            scene,
            p,
            legend,
            b.w.round().max(1.) as u32,
            b.h.round().max(1.) as u32,
        ) {
            out.draw_pixmap(
                b.x.round() as i32,
                b.y.round() as i32,
                map.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        if timeline {
            let badge = Box2 {
                x: slot.x + slot.w - 44.,
                y: slot.y + 8.,
                w: 32.,
                h: 32.,
            };
            rect(out, scale(badge), "#234966");
            text!(&(i + 1).to_string(), badge, 23., "#FFFFFF", Align::Center);
        }
    }
    if timeline {
        arrow(out, (476. * sx, 267. * sy), (496. * sx, 267. * sy), u);
        arrow(out, (739. * sx, 546. * sy), (233. * sx, 565. * sy), u);
        arrow(out, (476. * sx, 842. * sy), (496. * sx, 842. * sy), u);
    }
    let title = if scene.legend_title.trim().is_empty() {
        "Shared legend not available yet"
    } else {
        &scene.legend_title
    };
    text!(
        title,
        Box2 {
            x: 10.,
            y: 1140.,
            w: 952.,
            h: 42.
        },
        27.,
        "#D5E4F2",
        Align::Left
    );
    for (i, band) in legend.iter().enumerate() {
        let (x, y) = (10. + (i % 2) as f32 * 486., 1190. + (i / 2) as f32 * 48.);
        rect(
            out,
            scale(Box2 {
                x,
                y: y + 6.,
                w: 28.,
                h: 28.,
            }),
            &band.color,
        );
        text!(
            &band.label,
            Box2 {
                x: x + 42.,
                y,
                w: 424.,
                h: 40.
            },
            25.,
            "#FFFFFF",
            Align::Left
        );
    }
}

fn square_board(out: &mut Pixmap, g: &mut GlyphCache, font: &str, scene: &Scene, legend: &[Band]) {
    let p = &scene.panels[0];
    let (sx, sy) = (out.width() as f32 / 972., out.height() as f32 / 650.);
    let u = sx.min(sy);
    let scale = |b: Box2| Box2 {
        x: b.x * sx,
        y: b.y * sy,
        w: b.w * sx,
        h: b.h * sy,
    };
    macro_rules! text {
        ($value:expr,$b:expr,$size:expr,$color:expr,$align:expr) => {
            words(out, g, font, $value, scale($b), $size * u, $color, $align)
        };
    }
    let legend_at_side = p.summary.is_none() && scene.scope.bbox.is_none() && legend.len() <= 4;
    rect(
        out,
        scale(Box2 {
            x: 0.,
            y: 0.,
            w: 972.,
            h: if legend_at_side { 650. } else { 504. },
        }),
        "#132B44",
    );
    text!(
        &p.title,
        Box2 {
            x: 18.,
            y: 4.,
            w: 936.,
            h: 44.
        },
        30.,
        "#FFFFFF",
        Align::Left
    );
    let valid = if p.valid.trim().is_empty() {
        "Valid time not available yet"
    } else {
        &p.valid
    };
    let detail = if p.detail.trim().is_empty() {
        valid.to_string()
    } else {
        format!("{valid} • {}", p.detail)
    };
    text!(
        &detail,
        Box2 {
            x: 18.,
            y: 50.,
            w: 936.,
            h: 44.
        },
        21.,
        "#C2D4E6",
        Align::Left
    );
    let b = scale(Box2 {
        x: 8.,
        y: 104.,
        w: if legend_at_side {
            460.
        } else if p.summary.is_some() {
            600.
        } else {
            956.
        },
        h: if legend_at_side { 520. } else { 390. },
    });
    if let Some(map) = map(
        g,
        font,
        scene,
        p,
        legend,
        b.w.round().max(1.) as u32,
        b.h.round().max(1.) as u32,
    ) {
        out.draw_pixmap(
            b.x.round() as i32,
            b.y.round() as i32,
            map.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
    }
    if legend_at_side {
        let title = if scene.legend_title.trim().is_empty() {
            "Legend not available yet"
        } else {
            &scene.legend_title
        };
        text!(
            title,
            Box2 {
                x: 500.,
                y: 104.,
                w: 454.,
                h: 82.
            },
            27.,
            "#D5E4F2",
            Align::Left
        );
        for (i, band) in legend.iter().enumerate() {
            let y = 210. + i as f32 * 86.;
            rect(
                out,
                scale(Box2 {
                    x: 500.,
                    y: y + 16.,
                    w: 28.,
                    h: 28.,
                }),
                &band.color,
            );
            text!(
                &band.label,
                Box2 {
                    x: 542.,
                    y,
                    w: 412.,
                    h: 60.
                },
                30.,
                "#FFFFFF",
                Align::Left
            );
        }
        return;
    }
    if let Some(rows) = &p.summary {
        text!(
            &p.summary_title,
            Box2 {
                x: 630.,
                y: 104.,
                w: 324.,
                h: 40.
            },
            24.,
            "#C2D4E6",
            Align::Left
        );
        if rows.is_empty() {
            text!(
                super::reference::MISSING,
                Box2 {
                    x: 630.,
                    y: 175.,
                    w: 324.,
                    h: 80.
                },
                24.,
                "#FFFFFF",
                Align::Left
            );
        }
        for (i, (place, value)) in rows.iter().take(7).enumerate() {
            let y = 155. + i as f32 * 45.;
            rect(
                out,
                scale(Box2 {
                    x: 622.,
                    y,
                    w: 340.,
                    h: 38.,
                }),
                "#203B54",
            );
            text!(
                place,
                Box2 {
                    x: 630.,
                    y,
                    w: 215.,
                    h: 38.
                },
                21.,
                "#FFFFFF",
                Align::Left
            );
            text!(
                value,
                Box2 {
                    x: 852.,
                    y,
                    w: 100.,
                    h: 38.
                },
                25.,
                "#FFFFFF",
                Align::Right
            );
        }
    }
    let title = if scene.legend_title.trim().is_empty() {
        "Legend not available yet"
    } else {
        &scene.legend_title
    };
    text!(
        title,
        Box2 {
            x: 10.,
            y: 512.,
            w: 952.,
            h: 34.
        },
        23.,
        "#D5E4F2",
        Align::Left
    );
    for (i, band) in legend.iter().enumerate() {
        let (x, y) = (10. + (i % 4) as f32 * 243., 558. + (i / 4) as f32 * 38.);
        rect(
            out,
            scale(Box2 {
                x,
                y: y + 5.,
                w: 22.,
                h: 22.,
            }),
            &band.color,
        );
        text!(
            &band.label,
            Box2 {
                x: x + 32.,
                y,
                w: 201.,
                h: 32.
            },
            20.,
            "#FFFFFF",
            Align::Left
        );
    }
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
    if let Some(raw) = spec.strip_prefix("set:reportempty:") {
        if raw.parse::<usize>().ok() != Some(0) {
            return true;
        }
        let (dw, dh) = (
            (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
            (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
        );
        if let Some(mut out) = Pixmap::new(dw, dh) {
            out.fill(color("#132B44"));
            words(
                &mut out,
                g,
                font,
                super::reference::MISSING,
                Box2 {
                    x: 24.,
                    y: 0.,
                    w: dw as f32 - 48.,
                    h: dh as f32,
                },
                36. * dw as f32 / w.max(1.),
                "#FFFFFF",
                Align::Center,
            );
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
        }
        return true;
    }
    let Some(rest) = spec.strip_prefix("set:board:") else {
        return false;
    };
    let Some((kind, raw)) = rest.split_once(':') else {
        return true;
    };
    let layout = match kind {
        "compare" => Layout::Compare,
        "timeline" => Layout::Timeline,
        "zones" => Layout::Zones,
        "single" => Layout::Single,
        _ => return false,
    };
    let scene = parse(raw, layout).unwrap_or_else(|| {
        let mut unavailable = empty(layout);
        unavailable.scope.states.clear();
        unavailable
    });
    let _device_scale=BoardDeviceScale::new(local.sx.abs().max(0.0001));
    let legend = bands(&scene);
    let (dw, dh) = (
        (w * local.sx.abs()).round().clamp(1., 4096.) as u32,
        (h * local.sy.abs()).round().clamp(1., 4096.) as u32,
    );
    let Some(mut out) = Pixmap::new(dw, dh) else {
        return true;
    };
    if matches!(layout, Layout::Compare | Layout::Timeline) && h > w {
        portrait_multi(&mut out, g, font, &scene, &legend, layout);
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
        return true;
    }
    if layout == Layout::Single && h > w {
        portrait_board(&mut out, g, font, &scene, &legend);
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
        return true;
    }
    if layout == Layout::Single && h / w > 0.6 {
        square_board(&mut out, g, font, &scene, &legend);
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
        return true;
    }
    let (sx, sy) = (dw as f32 / 1780., dh as f32 / 715.);
    let native_square = h / w > 0.6;
    let u = sx.min(sy) * if native_square { 1.6 } else { 1. };
    let scale = |b: Box2| Box2 {
        x: b.x * sx,
        y: b.y * sy,
        w: b.w * sx,
        h: b.h * sy,
    };
    let timeline_strip =
        layout == Layout::Timeline && scene.scope.bbox.is_none() && scene.scope.county.is_none();
    let slots = match layout {
        Layout::Timeline if timeline_strip => (0..4)
            .map(|i| Box2 {
                x: i as f32 * 455.,
                y: 0.,
                w: 415.,
                h: 585.,
            })
            .collect(),
        Layout::Compare => vec![
            Box2 {
                x: 0.,
                y: 0.,
                w: 865.,
                h: 585.,
            },
            Box2 {
                x: 915.,
                y: 0.,
                w: 865.,
                h: 585.,
            },
        ],
        Layout::Timeline => vec![
            Box2 {
                x: 0.,
                y: 0.,
                w: 865.,
                h: 280.,
            },
            Box2 {
                x: 915.,
                y: 0.,
                w: 865.,
                h: 280.,
            },
            Box2 {
                x: 0.,
                y: 315.,
                w: 865.,
                h: 280.,
            },
            Box2 {
                x: 915.,
                y: 315.,
                w: 865.,
                h: 280.,
            },
        ],
        _ => vec![Box2 {
            x: 0.,
            y: 0.,
            w: 1780.,
            h: 585.,
        }],
    };
    for (i, (slot, p)) in slots.iter().zip(&scene.panels).enumerate() {
        let small = layout == Layout::Timeline;
        let headline = !small && !p.headline.is_empty();
        let head = if small && native_square && timeline_strip {
            110.
        } else if small {
            68.
        } else if headline {
            165.
        } else {
            105.
        };
        rect(&mut out, scale(*slot), "#132B44");
        words(
            &mut out,
            g,
            font,
            &p.title,
            scale(Box2 {
                x: slot.x + 18.,
                y: slot.y + 4.,
                w: slot.w - if small { 96. } else { 36. },
                h: if small { 34. } else { 48. },
            }),
            if small { 27. * u } else { 33. * u },
            "#FFFFFF",
            Align::Left,
        );
        if headline {
            words(
                &mut out,
                g,
                font,
                &p.headline,
                scale(Box2 {
                    x: slot.x + 18.,
                    y: slot.y + 49.,
                    w: slot.w - 36.,
                    h: 62.,
                }),
                47. * u,
                "#FFFFFF",
                Align::Left,
            );
        }
        let valid = if p.valid.trim().is_empty() {
            "Valid time not available yet"
        } else {
            &p.valid
        };
        let detail = if p.detail.trim().is_empty() {
            valid.to_string()
        } else {
            format!("{valid} • {}", p.detail)
        };
        words(
            &mut out,
            g,
            font,
            &detail,
            scale(Box2 {
                x: slot.x + 18.,
                y: slot.y
                    + if small {
                        36.
                    } else if headline {
                        112.
                    } else {
                        53.
                    },
                w: slot.w - 36.,
                h: if small && native_square && timeline_strip {
                    72.
                } else if small {
                    28.
                } else {
                    44.
                },
            }),
            21. * u,
            "#C2D4E6",
            Align::Left,
        );
        let b = scale(Box2 {
            x: slot.x + 8.,
            y: slot.y + head,
            w: if layout == Layout::Single && p.summary.is_some() {
                1100.
            } else {
                slot.w - 16.
            },
            h: slot.h - head - 8.,
        });
        if let Some(map) = map(
            g,
            font,
            &scene,
            p,
            &legend,
            b.w.round().max(1.) as u32,
            b.h.round().max(1.) as u32,
        ) {
            out.draw_pixmap(
                b.x.round() as i32,
                b.y.round() as i32,
                map.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        if layout == Layout::Single {
            if let Some(rows) = &p.summary {
                let x = slot.x + 1140.;
                let width = slot.w - 1160.;
                words(
                    &mut out,
                    g,
                    font,
                    &p.summary_title,
                    scale(Box2 {
                        x,
                        y: slot.y + head,
                        w: width,
                        h: 40.,
                    }),
                    28. * u,
                    "#C2D4E6",
                    Align::Left,
                );
                if rows.is_empty() {
                    words(
                        &mut out,
                        g,
                        font,
                        super::reference::MISSING,
                        scale(Box2 {
                            x,
                            y: slot.y + head + 64.,
                            w: width,
                            h: 80.,
                        }),
                        27. * u,
                        "#FFFFFF",
                        Align::Left,
                    );
                }
                for (i, (place, value)) in rows.iter().enumerate() {
                    let y = slot.y + head + 54. + i as f32 * 55.;
                    rect(
                        &mut out,
                        scale(Box2 {
                            x: x - 8.,
                            y,
                            w: width + 8.,
                            h: 47.,
                        }),
                        "#203B54",
                    );
                    words(
                        &mut out,
                        g,
                        font,
                        place,
                        scale(Box2 {
                            x,
                            y: y + 3.,
                            w: width - 150.,
                            h: 40.,
                        }),
                        26. * u,
                        "#FFFFFF",
                        Align::Left,
                    );
                    words(
                        &mut out,
                        g,
                        font,
                        value,
                        scale(Box2 {
                            x: x + width - 145.,
                            y: y + 3.,
                            w: 140.,
                            h: 40.,
                        }),
                        29. * u,
                        "#FFFFFF",
                        Align::Right,
                    );
                }
            }
        }
        if small {
            let number = Box2 {
                x: slot.x + slot.w - 58.,
                y: slot.y + 4.,
                w: 40.,
                h: 34.,
            };
            rect(&mut out, scale(number), "#234966");
            words(
                &mut out,
                g,
                font,
                &(i + 1).to_string(),
                scale(number),
                22. * u,
                "#FFFFFF",
                Align::Center,
            );
        }
    }
    if layout == Layout::Timeline {
        if timeline_strip {
            for i in 0..3 {
                let x = i as f32 * 455. + 425.;
                arrow(
                    &mut out,
                    (x * sx, 292. * sy),
                    ((x + 20.) * sx, 292. * sy),
                    u,
                );
            }
        } else {
            arrow(&mut out, (875. * sx, 140. * sy), (905. * sx, 140. * sy), u);
            arrow(&mut out, (1330. * sx, 291. * sy), (450. * sx, 304. * sy), u);
            arrow(&mut out, (875. * sx, 455. * sy), (905. * sx, 455. * sy), u);
        }
    }
    let title = if scene.legend_title.trim().is_empty() {
        "Shared legend not available yet"
    } else {
        &scene.legend_title
    };
    words(
        &mut out,
        g,
        font,
        title,
        scale(Box2 {
            x: 10.,
            y: 613.,
            w: 1760.,
            h: 32.,
        }),
        24. * u,
        "#D5E4F2",
        Align::Left,
    );
    let n = legend.len().max(1) as f32;
    let cell = 1760. / n;
    for (i, b) in legend.iter().enumerate() {
        let x = 10. + i as f32 * cell;
        rect(
            &mut out,
            scale(Box2 {
                x,
                y: 662.,
                w: 24.,
                h: 24.,
            }),
            &b.color,
        );
        words(
            &mut out,
            g,
            font,
            &b.label,
            scale(Box2 {
                x: x + 34.,
                y: 648.,
                w: cell - 46.,
                h: 54.,
            }),
            24. * u,
            "#FFFFFF",
            Align::Left,
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
