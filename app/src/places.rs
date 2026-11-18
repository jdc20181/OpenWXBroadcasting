//! National place list (towns and cities with population) for storm-track ETAs, the towns and
//! population inside warnings, and city value plots.
//!
//! Source: GeoNames `cities1000` (every place with 1,000+ people), US rows only. Downloaded once
//! (~11 MB zip) and cached as a small TSV; until it is ready, the built-in Ohio Valley list is used.

use std::{
    io::Read as _,
    sync::{Arc, OnceLock, RwLock},
};

use anyhow::{anyhow, Result};

const URL: &str = "https://download.geonames.org/export/dump/cities1000.zip";

#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    /// Two-letter state code.
    pub state: String,
    pub lat: f64,
    pub lon: f64,
    pub population: u32,
}

fn store() -> &'static RwLock<Option<Arc<Vec<Place>>>> {
    static STORE: OnceLock<RwLock<Option<Arc<Vec<Place>>>>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(None))
}

fn builtin() -> Arc<Vec<Place>> {
    static BUILTIN: OnceLock<Arc<Vec<Place>>> = OnceLock::new();
    BUILTIN
        .get_or_init(|| {
            Arc::new(
                crate::map::scene::PLACES
                    .iter()
                    .map(|(name, lat, lon)| Place {
                        name: name.to_string(),
                        state: String::new(),
                        lat: *lat,
                        lon: *lon,
                        population: 0,
                    })
                    .collect(),
            )
        })
        .clone()
}

/// All places, largest first (national list once loaded, else the built-in list).
pub fn all() -> Arc<Vec<Place>> {
    store().read().ok().and_then(|s| s.clone()).unwrap_or_else(builtin)
}

/// Places inside a lat/lon box, largest first.
pub fn in_box(south: f64, north: f64, west: f64, east: f64) -> Vec<Place> {
    all()
        .iter()
        .filter(|p| p.lat >= south && p.lat <= north && p.lon >= west && p.lon <= east)
        .cloned()
        .collect()
}

/// Towns matching a search ("evans", "jasper, in"), best first: exact name, then prefix, then
/// contains; larger towns first within each. At most `limit`.
pub fn search(query: &str, limit: usize) -> Vec<Place> {
    let q = query.trim().to_lowercase();
    if q.len() < 2 {
        return Vec::new();
    }
    let (name_q, state_q) = match q.split_once(',') {
        Some((n, s)) => (n.trim().to_string(), Some(s.trim().to_string())),
        None => (q.clone(), None),
    };
    let mut hits: Vec<(u8, Place)> = all()
        .iter()
        .filter(|p| state_q.as_deref().map(|s| s.is_empty() || p.state.to_lowercase() == s).unwrap_or(true))
        .filter_map(|p| {
            let name = p.name.to_lowercase();
            let rank = if name == name_q {
                0
            } else if name.starts_with(&name_q) {
                1
            } else if name.contains(&name_q) {
                2
            } else {
                return None;
            };
            Some((rank, p.clone()))
        })
        .collect();
    // `all()` is already largest-first, so a stable sort by rank keeps population order.
    hits.sort_by_key(|(rank, _)| *rank);
    hits.into_iter().take(limit).map(|(_, p)| p).collect()
}

/// US state abbreviations and names (for geocoder results).
pub const STATE_NAMES: &[(&str, &str)] = &[
    ("AL", "Alabama"), ("AK", "Alaska"), ("AZ", "Arizona"), ("AR", "Arkansas"), ("CA", "California"),
    ("CO", "Colorado"), ("CT", "Connecticut"), ("DE", "Delaware"), ("FL", "Florida"), ("GA", "Georgia"),
    ("HI", "Hawaii"), ("ID", "Idaho"), ("IL", "Illinois"), ("IN", "Indiana"), ("IA", "Iowa"),
    ("KS", "Kansas"), ("KY", "Kentucky"), ("LA", "Louisiana"), ("ME", "Maine"), ("MD", "Maryland"),
    ("MA", "Massachusetts"), ("MI", "Michigan"), ("MN", "Minnesota"), ("MS", "Mississippi"), ("MO", "Missouri"),
    ("MT", "Montana"), ("NE", "Nebraska"), ("NV", "Nevada"), ("NH", "New Hampshire"), ("NJ", "New Jersey"),
    ("NM", "New Mexico"), ("NY", "New York"), ("NC", "North Carolina"), ("ND", "North Dakota"), ("OH", "Ohio"),
    ("OK", "Oklahoma"), ("OR", "Oregon"), ("PA", "Pennsylvania"), ("RI", "Rhode Island"), ("SC", "South Carolina"),
    ("SD", "South Dakota"), ("TN", "Tennessee"), ("TX", "Texas"), ("UT", "Utah"), ("VT", "Vermont"),
    ("VA", "Virginia"), ("WA", "Washington"), ("WV", "West Virginia"), ("WI", "Wisconsin"), ("WY", "Wyoming"),
    ("DC", "District of Columbia"), ("PR", "Puerto Rico"),
];

/// "38.39, -86.93" -> (lat, lon).
pub fn parse_lat_lon(text: &str) -> Option<(f64, f64)> {
    let (a, b) = text.split_once(',')?;
    let lat: f64 = a.trim().parse().ok()?;
    let lon: f64 = b.trim().parse().ok()?;
    ((-90.0..=90.0).contains(&lat) && (-180.0..=180.0).contains(&lon)).then_some((lat, lon))
}

fn parse_tsv(text: &str) -> Vec<Place> {
    text.lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            Some(Place {
                name: f.first()?.to_string(),
                state: f.get(1)?.to_string(),
                lat: f.get(2)?.parse().ok()?,
                lon: f.get(3)?.parse().ok()?,
                population: f.get(4)?.parse().unwrap_or(0),
            })
        })
        .collect()
}

pub async fn ensure_loaded() -> Result<()> {
    let cache = crate::config::tile_cache_dir().join("..").join("places").join("us-places.tsv");
    let places = match tokio::fs::read_to_string(&cache).await {
        Ok(text) if text.len() > 1000 => parse_tsv(&text),
        _ => {
            let bytes = crate::net::get_logged(URL, std::time::Duration::from_secs(180)).await?.bytes().await?;
            let text = tokio::task::spawn_blocking(move || -> Result<String> {
                let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
                let mut file = archive.by_name("cities1000.txt").map_err(|e| anyhow!("cities1000.txt: {e}"))?;
                let mut raw = String::new();
                file.read_to_string(&mut raw)?;
                // geonameid, name, asciiname, alternates, lat, lon, class, code, country, cc2,
                // admin1, admin2, admin3, admin4, population, …
                let mut out = String::new();
                for line in raw.lines() {
                    let f: Vec<&str> = line.split('\t').collect();
                    if f.len() > 14 && f[8] == "US" {
                        out.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", f[1], f[10], f[4], f[5], f[14]));
                    }
                }
                Ok(out)
            })
            .await??;
            if let Some(dir) = cache.parent() {
                let _ = tokio::fs::create_dir_all(dir).await;
            }
            let _ = tokio::fs::write(&cache, &text).await;
            parse_tsv(&text)
        }
    };
    if places.is_empty() {
        return Err(anyhow!("place list was empty"));
    }
    let mut places = places;
    places.sort_by(|a, b| b.population.cmp(&a.population));
    if let Ok(mut slot) = store().write() {
        *slot = Some(Arc::new(places));
    }
    Ok(())
}
