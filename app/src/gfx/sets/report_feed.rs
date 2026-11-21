//! Graphics-only rolling NWS LSR coverage via the approved IEM relay route.
//! Current in-memory data only: never persist live weather or substitute 24h for longer periods.
use crate::{
    config::{GeoView, Settings},
    data::StormReport,
};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const HOURS: i64 = 168;
const MAX_AGE: i64 = 20 * 60;
// Full-detail WPC QPF contours reach 15 MB on an ordinary day; leave room for big rain events.
const MAX_BYTES: usize = 64 * 1024 * 1024;

pub(super) struct Snapshot {
    pub reports: Vec<StormReport>,
    pub end: i64,
    home: [u64; 2],
    checked: i64,
}
impl Snapshot {
    fn usable(&self, home: GeoView, now: i64, hours: u16) -> bool {
        self.home == home_key(home)
            && (25..=168).contains(&hours)
            && (0..=MAX_AGE).contains(&(now - self.checked))
            && (0..=MAX_AGE + 60).contains(&(now - self.end))
    }
}
#[derive(Default)]
struct Store {
    generation: u64,
    current: Option<Arc<Snapshot>>,
}
impl Store {
    fn begin(&mut self) -> u64 {
        self.generation += 1;
        self.generation
    }
    fn finish(&mut self, ticket: u64, result: Option<Snapshot>) {
        if self.generation == ticket {
            self.current = result.map(Arc::new);
        }
    }
}
fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(Store::default()))
}
fn home_key(home: GeoView) -> [u64; 2] {
    [home.lon.to_bits(), home.lat.to_bits()]
}
pub(super) fn current(home: GeoView, now: i64, hours: u16) -> Option<Arc<Snapshot>> {
    let guard = store().lock().ok()?;
    guard
        .current
        .as_ref()
        .filter(|s| s.usable(home, now, hours))
        .cloned()
}

fn bounds(home: GeoView) -> Option<[f64; 4]> {
    if !home.lon.is_finite()
        || !home.lat.is_finite()
        || !(-180.0..=180.0).contains(&home.lon)
        || !(-90.0..=90.0).contains(&home.lat)
    {
        return None;
    }
    let mut settings = Settings::default();
    settings.home = home;
    let scope = super::comparison::scope(&settings, super::board::Layout::Compare);
    let state = scope.states.first()?;
    let counties = crate::map::boundaries::counties_named()?;
    let mut b = [home.lon - 5., home.lat - 3., home.lon + 5., home.lat + 3.];
    let mut vertices = 0;
    for county in counties.iter().filter(|c| &c.state == state) {
        for &(lon, lat) in county.rings.iter().flatten() {
            if !lon.is_finite() || !lat.is_finite() {
                return None;
            }
            b[0] = b[0].min(lon);
            b[1] = b[1].min(lat);
            b[2] = b[2].max(lon);
            b[3] = b[3].max(lat);
            vertices += 1;
        }
    }
    if vertices == 0
        || b[0] < -180.
        || b[2] > 180.
        || b[1] < -90.
        || b[3] > 90.
        || b[2] - b[0] > 70.
        || b[3] - b[1] > 35.
    {
        return None;
    }
    b[0] = (b[0] * 100.).floor() / 100.;
    b[1] = (b[1] * 100.).floor() / 100.;
    b[2] = (b[2] * 100.).ceil() / 100.;
    b[3] = (b[3] * 100.).ceil() / 100.;
    Some(b)
}
fn decode(
    value: &serde_json::Value,
    home: GeoView,
    b: [f64; 4],
    end: i64,
    checked: i64,
) -> Option<Snapshot> {
    if value.get("type")?.as_str()? != "FeatureCollection" {
        return None;
    }
    let features = value.get("features")?.as_array()?;
    if features.len() > 50_000 {
        return None;
    }
    // Fail closed on malformed coordinate/time fields instead of claiming a complete query.
    let reports = crate::data::parse_lsr(value);
    if reports.len() != features.len()
        || reports
            .iter()
            .any(|r| r.time <= 0 || !r.lon.is_finite() || !r.lat.is_finite())
    {
        return None;
    }
    let reports = reports
        .into_iter()
        .filter(|r| {
            (end - HOURS * 3600..=end).contains(&r.time)
                && (b[0]..=b[2]).contains(&r.lon)
                && (b[1]..=b[3]).contains(&r.lat)
        })
        .collect();
    Some(Snapshot {
        reports,
        end,
        checked,
        home: home_key(home),
    })
}
async fn fetch(home: GeoView) -> Option<Snapshot> {
    let b = bounds(home)?;
    let now = chrono::Utc::now();
    let end = now.timestamp().div_euclid(60) * 60;
    let stamp = chrono::DateTime::from_timestamp(end, 0)?.format("%Y-%m-%dT%H:%MZ");
    let url = format!("https://mesonet.agron.iastate.edu/geojson/lsr.geojson?hours={HOURS}&ets={stamp}&west={:.2}&south={:.2}&east={:.2}&north={:.2}",b[0],b[1],b[2],b[3]);
    let value = get_json(&url).await?;
    decode(&value, home, b, end, chrono::Utc::now().timestamp())
}
/// Bounded JSON read for graphics-owned approved relay routes; no persistence.
pub(super) async fn get_json(url: &str) -> Option<serde_json::Value> {
    let bytes = get_bytes(url, Duration::from_secs(40)).await?;
    serde_json::from_slice(&bytes).ok()
}
pub(super) async fn get_bytes(url: &str, timeout: Duration) -> Option<Vec<u8>> {
    let mut response = crate::net::get_logged(url, timeout)
        .await
        .ok()?
        .error_for_status()
        .ok()?;
    if response
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        return None;
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len().saturating_add(chunk.len()) > MAX_BYTES {
            return None;
        }
        bytes.extend_from_slice(&chunk);
    }
    Some(bytes)
}
pub(super) async fn refresh(home: GeoView) {
    let ticket = match store().lock() {
        Ok(mut s) => s.begin(),
        Err(_) => return,
    };
    let result = fetch(home).await;
    if let Ok(mut s) = store().lock() {
        s.finish(ticket, result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        Snapshot {
            reports: Vec::new(),
            end: 100_000,
            checked: 100_000,
            home: home_key(Settings::default().home),
        }
    }
    #[test]
    fn longer_periods_require_fresh_matching_home_coverage() {
        let s = snapshot();
        let home = Settings::default().home;
        assert!(s.usable(home, 100_300, 48));
        assert!(s.usable(home, 100_300, 168));
        assert!(!s.usable(home, 101_201, 48));
        assert!(!s.usable(home, 99_999, 48));
        assert!(!s.usable(home, 100_300, 720));
        assert!(!s.usable(home, 100_300, 24));
        let mut moved = home;
        moved.lon += 0.01;
        assert!(!s.usable(moved, 100_300, 48));
    }
    #[test]
    fn old_responses_cannot_replace_newer_queries_and_failure_clears_data() {
        let mut s = Store::default();
        let old = s.begin();
        let new = s.begin();
        s.finish(new, Some(snapshot()));
        s.finish(old, None);
        assert!(s.current.is_some());
        s.finish(new, None);
        assert!(s.current.is_none());
    }
    #[test]
    fn geojson_decode_preserves_report_times_and_filters_query_bounds() {
        let home = Settings::default().home;
        let feature = |lon: f64, time: &str| {
            serde_json::json!({"type":"Feature", "properties":{
            "lat":38.4,"lon":lon,"valid":time,"type":"S","typetext":"SNOW",
            "magnitude":2.5,"city":"SAMPLE","county":"Dubois","st":"IN"}})
        };
        let end = chrono::DateTime::parse_from_rfc3339("2026-11-13T18:00:00Z")
            .unwrap()
            .timestamp();
        let value = serde_json::json!({"type":"FeatureCollection","features":[
            feature(-86.9,"2026-11-12T12:00:00Z"),
            feature(-100.,"2026-11-12T12:00:00Z"),
            feature(-86.9,"2026-11-13T18:01:00Z"),
            feature(-86.9,"2026-11-01T12:00:00Z")]});
        let s = decode(&value, home, [-90., 35., -80., 45.], end, end + 30).unwrap();
        assert_eq!(s.reports.len(), 1);
        assert_eq!(s.reports[0].time, end - 30 * 3600);
        assert_eq!(s.reports[0].magnitude, "2.5");
        assert_eq!(s.reports[0].county, "Dubois, IN");
    }
    #[test]
    fn malformed_queries_fail_closed_and_empty_feature_collection_is_valid() {
        let home = Settings::default().home;
        let b = [-90., 35., -80., 45.];
        assert!(decode(
            &serde_json::json!({"type":"FeatureCollection","features":[]}),
            home,
            b,
            100_000,
            100_000
        )
        .is_some());
        assert!(decode(
            &serde_json::json!({"features":[]}),
            home,
            b,
            100_000,
            100_000
        )
        .is_none());
        assert!(decode(
            &serde_json::json!({"type":"FeatureCollection","features":[{"properties":{}}]}),
            home,
            b,
            100_000,
            100_000
        )
        .is_none());
        let mut invalid = home;
        invalid.lat = f64::NAN;
        assert!(bounds(invalid).is_none());
    }
}
