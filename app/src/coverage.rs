//! Coverage area (Settings > Coverage area): the station's supported counties, their centre
//! points and town names, and the clickable county map the Settings tab shows. Mirrors the
//! website's Admin > Social Graphics "Supported counties" (src/plugins/social-graphics/coverage.js).

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
};

use crate::{
    config::{CoverageArea, CoverageCounty, Settings},
    map::boundaries::County,
};

/// State abbreviation for a 2-digit FIPS ("18" -> "IN").
pub fn state_abbr(fips: &str) -> Option<&'static str> {
    crate::winter::STATES.iter().find(|s| s.0 == fips).map(|s| s.1)
}

/// 2-digit FIPS for a state abbreviation ("IN" -> "18").
pub fn state_fips(abbr: &str) -> Option<&'static str> {
    crate::winter::STATES.iter().find(|s| s.1.eq_ignore_ascii_case(abbr)).map(|s| s.0)
}

fn inside(ring: &[(f64, f64)], x: f64, y: f64) -> bool {
    crate::gfx::winter_map::inside_pub(ring, x, y)
}

fn contains(c: &County, lat: f64, lon: f64) -> bool {
    c.rings.iter().fold(false, |acc, r| acc ^ inside(r, lon, lat))
}

/// Area-weighted centre (lat, lon) of a county's largest ring, as the website computes it.
pub fn centre(rings: &[Vec<(f64, f64)>]) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64, f64)> = None;
    for ring in rings {
        let n = ring.len();
        if n < 3 {
            continue;
        }
        let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
        let mut j = n - 1;
        for i in 0..n {
            let (xi, yi) = ring[i];
            let (xj, yj) = ring[j];
            let f = xj * yi - xi * yj;
            a += f;
            cx += (xj + xi) * f;
            cy += (yj + yi) * f;
            j = i;
        }
        if a != 0.0 && best.is_none_or(|b| a.abs() > b.0) {
            best = Some((a.abs(), cy / (3.0 * a), cx / (3.0 * a)));
        }
    }
    best.map(|(_, lat, lon)| ((lat * 1e4).round() / 1e4, (lon * 1e4).round() / 1e4))
}

/// The county holding a point.
pub fn county_at(counties: &[County], lat: f64, lon: f64) -> Option<&County> {
    counties.iter().find(|c| contains(c, lat, lon))
}

/// The home county's FIPS: the chosen one when it is supported, else the supported county
/// holding the home point, else the first supported county; None with no supported counties.
pub fn home_fips(s: &Settings, counties: Option<&[County]>) -> Option<String> {
    let cov = &s.coverage;
    if cov.counties.is_empty() {
        return None;
    }
    if cov.counties.iter().any(|c| c.fips == cov.home_fips) {
        return Some(cov.home_fips.clone());
    }
    let at = counties.and_then(|all| county_at(all, s.home.lat, s.home.lon)).map(|c| c.fips.clone());
    if let Some(f) = at.filter(|f| cov.counties.iter().any(|c| c.fips == *f)) {
        return Some(f);
    }
    cov.counties.first().map(|c| c.fips.clone())
}

// --------------------------------------------------------------------- town names ---

fn towns() -> &'static Mutex<HashMap<String, String>> {
    static T: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    T.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The NWS relativeLocation city found for this county's centre (after a coverage fetch).
pub fn cached_town(fips: &str) -> Option<String> {
    towns().lock().ok()?.get(fips).cloned()
}

pub fn remember_town(fips: &str, city: &str) {
    if let Ok(mut t) = towns().lock() {
        t.insert(fips.to_string(), city.to_string());
    }
}

/// One supported county as a coverage place: (fips, label, lat, lon, home). The label is the
/// county's town name, else the NWS city found for its centre, else the county name.
pub struct CountyPlace {
    pub fips: String,
    pub label: String,
    /// True while the label is not the user's own town name (an NWS lookup may replace it).
    pub auto: bool,
    pub lat: f64,
    pub lon: f64,
    pub home: bool,
}

/// Supported counties as places, home county first. Counties whose outline is not available
/// (boundary file still downloading) are left out.
pub fn county_places(s: &Settings) -> Vec<CountyPlace> {
    let Some(all) = crate::map::boundaries::counties_named() else { return Vec::new() };
    let home = home_fips(s, Some(&all));
    let mut out: Vec<CountyPlace> = s
        .coverage
        .counties
        .iter()
        .filter_map(|c| {
            let shape = all.iter().find(|x| x.fips == c.fips)?;
            let (lat, lon) = centre(&shape.rings)?;
            let town = c.town.trim();
            let (label, auto) = if !town.is_empty() {
                (town.to_string(), false)
            } else {
                (cached_town(&c.fips).unwrap_or_else(|| c.name.clone()), true)
            };
            Some(CountyPlace { fips: c.fips.clone(), label, auto, lat, lon, home: home.as_deref() == Some(c.fips.as_str()) })
        })
        .collect();
    out.sort_by_key(|p| !p.home);
    out
}

/// "Southern Indiana" when the region is named, else None.
pub fn region_name(s: &Settings) -> Option<&str> {
    let n = s.coverage.name.trim();
    (!n.is_empty()).then_some(n)
}

// ------------------------------------------------------------------- editing ---

/// Add or remove a county (clicking it on the map or in the search list).
pub fn toggle_county(cov: &mut CoverageArea, c: &County) {
    if let Some(i) = cov.counties.iter().position(|x| x.fips == c.fips) {
        cov.counties.remove(i);
        if cov.home_fips == c.fips {
            cov.home_fips.clear();
        }
    } else {
        cov.counties.push(CoverageCounty {
            fips: c.fips.clone(),
            name: c.name.clone(),
            state: state_abbr(&c.state).unwrap_or_default().to_string(),
            town: String::new(),
        });
    }
}

/// Show or hide a state on the county map (at most 8, as on the website).
pub fn toggle_state(cov: &mut CoverageArea, abbr: &str) {
    if let Some(i) = cov.states.iter().position(|s| s == abbr) {
        cov.states.remove(i);
    } else if cov.states.len() < 8 {
        cov.states.push(abbr.to_string());
    }
}

/// State FIPS codes on the map: the chosen states, or the home state plus the states of the
/// supported counties when none are chosen.
pub fn map_states(s: &Settings, counties: &[County]) -> Vec<String> {
    let mut out: Vec<String> = s.coverage.states.iter().filter_map(|a| state_fips(a)).map(str::to_string).collect();
    if out.is_empty() {
        if let Some(c) = county_at(counties, s.home.lat, s.home.lon) {
            out.push(c.state.clone());
        }
        for c in &s.coverage.counties {
            let st = c.fips.get(..2).unwrap_or_default().to_string();
            if !st.is_empty() && !out.contains(&st) {
                out.push(st);
            }
        }
    }
    out
}

/// Counties whose name (or "name, ST") contains the search text, not yet supported.
pub fn search<'a>(cov: &CoverageArea, counties: &'a [County], states: &[String], query: &str) -> Vec<&'a County> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let mut hits: Vec<&County> = counties
        .iter()
        .filter(|c| states.contains(&c.state) || q.contains(','))
        .filter(|c| !cov.counties.iter().any(|x| x.fips == c.fips))
        .filter(|c| {
            let full = format!("{}, {}", c.name, state_abbr(&c.state).unwrap_or_default()).to_lowercase();
            full.contains(&q)
        })
        .collect();
    hits.sort_by(|a, b| a.name.cmp(&b.name));
    hits.truncate(12);
    hits
}

// ---------------------------------------------------------------------- map ---

/// Projection of the visible states into a w × h picture (equirectangular, cos-lat scaled).
#[derive(Clone, Copy, Debug)]
pub struct MapFrame {
    west: f64,
    north: f64,
    k: f64,
    s: f64,
    ox: f64,
    oy: f64,
}

impl MapFrame {
    pub fn fit(counties: &[&County], w: f64, h: f64) -> Option<Self> {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(lon, lat) in counties.iter().flat_map(|c| c.rings.iter().flatten()) {
            b = (b.0.min(lon), b.1.min(lat), b.2.max(lon), b.3.max(lat));
        }
        if b.0 >= b.2 || b.1 >= b.3 {
            return None;
        }
        let k = ((b.1 + b.3) / 2.0).to_radians().cos();
        let (gw, gh) = ((b.2 - b.0) * k, b.3 - b.1);
        let s = ((w * 0.94) / gw).min((h * 0.94) / gh);
        Some(Self { west: b.0, north: b.3, k, s, ox: (w - gw * s) / 2.0, oy: (h - gh * s) / 2.0 })
    }

    pub fn to_px(&self, lon: f64, lat: f64) -> (f32, f32) {
        ((self.ox + (lon - self.west) * self.k * self.s) as f32, (self.oy + (self.north - lat) * self.s) as f32)
    }

    pub fn to_lonlat(&self, x: f64, y: f64) -> (f64, f64) {
        (self.west + (x - self.ox) / (self.s * self.k), self.north - (y - self.oy) / self.s)
    }
}

/// The county under picture pixel (x, y) on a map of these states.
pub fn pick<'a>(counties: &'a [County], states: &[String], w: f64, h: f64, x: f64, y: f64) -> Option<&'a County> {
    let shown: Vec<&County> = counties.iter().filter(|c| states.contains(&c.state)).collect();
    let frame = MapFrame::fit(&shown, w, h)?;
    let (lon, lat) = frame.to_lonlat(x, y);
    shown.into_iter().find(|c| contains(c, lat, lon))
}

/// Draw the county map: supported counties filled, the home county brightest with a white
/// outline. Returns an RGBA (premultiplied, tiny-skia) picture.
pub fn render_map(cov: &CoverageArea, home: Option<&str>, counties: &[County], states: &[String], w: u32, h: u32) -> Option<tiny_skia::Pixmap> {
    use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};
    let mut pm = Pixmap::new(w, h)?;
    pm.fill(Color::from_rgba8(10, 16, 28, 255));
    let shown: Vec<&County> = counties.iter().filter(|c| states.contains(&c.state)).collect();
    let frame = MapFrame::fit(&shown, w as f64, h as f64)?;
    let paint = |r: u8, g: u8, b: u8, a: u8| {
        let mut p = Paint::default();
        p.set_color(Color::from_rgba8(r, g, b, a));
        p.anti_alias = true;
        p
    };
    let path = |c: &County| {
        let mut pb = PathBuilder::new();
        for ring in &c.rings {
            for (i, &(lon, lat)) in ring.iter().enumerate() {
                let (x, y) = frame.to_px(lon, lat);
                if i == 0 {
                    pb.move_to(x, y);
                } else {
                    pb.line_to(x, y);
                }
            }
            pb.close();
        }
        pb.finish()
    };
    let thin = Stroke { width: 1.0, ..Stroke::default() };
    let thick = Stroke { width: 3.0, ..Stroke::default() };
    let mut home_path = None;
    for c in &shown {
        let Some(p) = path(c) else { continue };
        let supported = cov.counties.iter().any(|x| x.fips == c.fips);
        let is_home = home == Some(c.fips.as_str());
        let fill = if is_home {
            paint(56, 189, 248, 230)
        } else if supported {
            paint(34, 211, 238, 120)
        } else {
            paint(30, 41, 59, 255)
        };
        pm.fill_path(&p, &fill, FillRule::EvenOdd, Transform::identity(), None);
        pm.stroke_path(&p, &paint(100, 116, 139, 255), &thin, Transform::identity(), None);
        if is_home {
            home_path = Some(p);
        }
    }
    if let Some(p) = home_path {
        pm.stroke_path(&p, &paint(255, 255, 255, 255), &thick, Transform::identity(), None);
    }
    Some(pm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centre_of_a_square_is_its_middle() {
        let ring = vec![(-87.0, 38.0), (-86.0, 38.0), (-86.0, 39.0), (-87.0, 39.0), (-87.0, 38.0)];
        assert_eq!(centre(&[ring]), Some((38.5, -86.5)));
    }

    #[test]
    fn largest_ring_wins() {
        let small = vec![(0.0, 0.0), (0.1, 0.0), (0.1, 0.1), (0.0, 0.1)];
        let big = vec![(10.0, 10.0), (12.0, 10.0), (12.0, 12.0), (10.0, 12.0)];
        assert_eq!(centre(&[small, big]), Some((11.0, 11.0)));
    }

    #[test]
    fn home_fips_prefers_the_chosen_supported_county() {
        let mut s = Settings::default();
        assert_eq!(home_fips(&s, None), None);
        s.coverage.counties = vec![
            CoverageCounty { fips: "18037".into(), name: "Dubois County".into(), state: "IN".into(), town: String::new() },
            CoverageCounty { fips: "18125".into(), name: "Pike County".into(), state: "IN".into(), town: "Petersburg".into() },
        ];
        assert_eq!(home_fips(&s, None).as_deref(), Some("18037"));
        s.coverage.home_fips = "18125".into();
        assert_eq!(home_fips(&s, None).as_deref(), Some("18125"));
        s.coverage.home_fips = "99999".into();
        assert_eq!(home_fips(&s, None).as_deref(), Some("18037"));
    }

    #[test]
    fn toggling_states_caps_at_eight() {
        let mut cov = CoverageArea::default();
        for st in ["IN", "KY", "IL", "OH", "MI", "TN", "MO", "WI", "IA"] {
            toggle_state(&mut cov, st);
        }
        assert_eq!(cov.states.len(), 8);
        toggle_state(&mut cov, "IN");
        assert!(!cov.states.contains(&"IN".to_string()));
    }

    /// The Settings tab's county map with sample counties: clicking maps back to the county,
    /// and with `DCWX_OUT` set the picture is saved for review.
    #[test]
    fn settings_map_picks_and_renders_supported_counties() {
        let Some(all) = crate::map::boundaries::counties_named() else { return };
        let mut cov = CoverageArea { states: vec!["IN".into(), "KY".into()], home_fips: "18037".into(), ..Default::default() };
        for fips in ["18037", "18125", "18123", "21059"] {
            toggle_county(&mut cov, all.iter().find(|c| c.fips == fips).unwrap());
        }
        assert_eq!(cov.counties[3].state, "KY");
        let mut s = Settings::default();
        s.coverage = cov.clone();
        let states = map_states(&s, &all);
        assert_eq!(states, ["18", "21"]);
        let shown: Vec<&County> = all.iter().filter(|c| states.contains(&c.state)).collect();
        let frame = MapFrame::fit(&shown, 960.0, 540.0).unwrap();
        let dubois = all.iter().find(|c| c.fips == "18037").unwrap();
        let (lat, lon) = centre(&dubois.rings).unwrap();
        let (x, y) = frame.to_px(lon, lat);
        assert_eq!(pick(&all, &states, 960.0, 540.0, x as f64, y as f64).map(|c| c.fips.as_str()), Some("18037"));
        assert_eq!(search(&cov, &all, &states, "warrick, in").first().map(|c| c.fips.as_str()), Some("18173"));
        let pm = render_map(&cov, Some("18037"), &all, &states, 960, 540).unwrap();
        if let Ok(dir) = std::env::var("DCWX_OUT") {
            pm.save_png(std::path::Path::new(&dir).join("settings-coverage-map.png")).unwrap();
        }
    }

    #[test]
    fn map_frame_round_trips() {
        let f = MapFrame { west: -88.0, north: 41.0, k: 0.77, s: 100.0, ox: 10.0, oy: 5.0 };
        let (x, y) = f.to_px(-86.5, 39.25);
        let (lon, lat) = f.to_lonlat(x as f64, y as f64);
        assert!((lon + 86.5).abs() < 1e-3 && (lat - 39.25).abs() < 1e-3);
    }
}
