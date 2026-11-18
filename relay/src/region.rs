//! Coverage-region lookups shared by the desktop setup wizard and the `openwx` CLI: U.S. states,
//! the counties in a state (NWS county zones), a town's coordinates and the county at a point.

use anyhow::{anyhow, bail, Result};

use crate::{
    fetch::{FetchOptions, LIVE},
    AppState,
};

/// (FIPS, abbreviation, name)
pub const STATES: &[(&str, &str, &str)] = &[
    ("01", "AL", "Alabama"), ("02", "AK", "Alaska"), ("04", "AZ", "Arizona"), ("05", "AR", "Arkansas"), ("06", "CA", "California"),
    ("08", "CO", "Colorado"), ("09", "CT", "Connecticut"), ("10", "DE", "Delaware"), ("11", "DC", "District of Columbia"), ("12", "FL", "Florida"),
    ("13", "GA", "Georgia"), ("15", "HI", "Hawaii"), ("16", "ID", "Idaho"), ("17", "IL", "Illinois"), ("18", "IN", "Indiana"),
    ("19", "IA", "Iowa"), ("20", "KS", "Kansas"), ("21", "KY", "Kentucky"), ("22", "LA", "Louisiana"), ("23", "ME", "Maine"),
    ("24", "MD", "Maryland"), ("25", "MA", "Massachusetts"), ("26", "MI", "Michigan"), ("27", "MN", "Minnesota"), ("28", "MS", "Mississippi"),
    ("29", "MO", "Missouri"), ("30", "MT", "Montana"), ("31", "NE", "Nebraska"), ("32", "NV", "Nevada"), ("33", "NH", "New Hampshire"),
    ("34", "NJ", "New Jersey"), ("35", "NM", "New Mexico"), ("36", "NY", "New York"), ("37", "NC", "North Carolina"), ("38", "ND", "North Dakota"),
    ("39", "OH", "Ohio"), ("40", "OK", "Oklahoma"), ("41", "OR", "Oregon"), ("42", "PA", "Pennsylvania"), ("44", "RI", "Rhode Island"),
    ("45", "SC", "South Carolina"), ("46", "SD", "South Dakota"), ("47", "TN", "Tennessee"), ("48", "TX", "Texas"), ("49", "UT", "Utah"),
    ("50", "VT", "Vermont"), ("51", "VA", "Virginia"), ("53", "WA", "Washington"), ("54", "WV", "West Virginia"), ("55", "WI", "Wisconsin"),
    ("56", "WY", "Wyoming"), ("72", "PR", "Puerto Rico"),
];

pub fn state_abbr(input: &str) -> Option<&'static str> {
    let t = input.trim();
    STATES.iter().find(|s| s.1.eq_ignore_ascii_case(t) || s.2.eq_ignore_ascii_case(t)).map(|s| s.1)
}

pub fn state_fips(abbr: &str) -> Option<&'static str> {
    STATES.iter().find(|s| s.1.eq_ignore_ascii_case(abbr)).map(|s| s.0)
}

/// A county from the NWS county zone list.
#[derive(Clone, Debug, PartialEq)]
pub struct County {
    /// NWS zone id, "INC037".
    pub zone: String,
    /// 5-digit FIPS, "18037".
    pub fips: String,
    /// "Dubois County" (parishes and boroughs keep their own word).
    pub name: String,
    pub state: String,
}

pub fn county_name(raw: &str, state: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if state == "LA" && !lower.contains("parish") {
        format!("{raw} Parish")
    } else if lower.contains("county") || lower.contains("parish") || lower.contains("borough") || lower.contains("city") || lower.contains("census area") || state == "AK" {
        raw.to_owned()
    } else {
        format!("{raw} County")
    }
}

/// Every county in a state, from the NWS (`/zones?type=county&area=XX`).
pub async fn counties(relay: &AppState, state: &str) -> Result<Vec<County>> {
    let fips = state_fips(state).ok_or_else(|| anyhow!("unknown state {state}"))?;
    let mut target = url::Url::parse("https://api.weather.gov/zones")?;
    target.query_pairs_mut().append_pair("type", "county").append_pair("area", state).append_pair("include_geometry", "false");
    let data = relay.fetch.get(target, LIVE, FetchOptions::default()).await.map_err(|e| anyhow!(e.message))?.json().map_err(|e| anyhow!(e.message))?;
    let mut out: Vec<County> = data["features"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let p = &f["properties"];
            let zone = p["id"].as_str()?.to_owned();
            let digits = zone.get(3..6)?.to_owned();
            Some(County { fips: format!("{fips}{digits}"), name: county_name(p["name"].as_str()?, state), state: state.to_owned(), zone })
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out.dedup_by(|a, b| a.fips == b.fips);
    if out.is_empty() {
        bail!("the NWS returned no counties for {state}");
    }
    Ok(out)
}

/// Match typed county names ("Dubois, pike county") or list numbers (1-based) against a list.
pub fn pick_counties(all: &[County], typed: &str) -> Result<Vec<County>> {
    let mut picked = Vec::new();
    for part in typed.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let found = if let Ok(n) = part.parse::<usize>() {
            all.get(n.wrapping_sub(1)).cloned()
        } else {
            let want = part.to_ascii_lowercase().replace(" county", "").replace(" parish", "");
            all.iter().find(|c| c.name.to_ascii_lowercase().replace(" county", "").replace(" parish", "") == want).cloned()
        };
        match found {
            Some(c) if !picked.contains(&c) => picked.push(c),
            Some(_) => {}
            None => bail!("no county called \"{part}\""),
        }
    }
    if picked.is_empty() {
        bail!("choose at least one county");
    }
    Ok(picked)
}

/// A town's coordinates (Zippopotam.us place search by state and name).
pub async fn find_city(relay: &AppState, state: &str, city: &str) -> Result<Option<(String, f64, f64)>> {
    let target = url::Url::parse(&format!("https://api.zippopotam.us/us/{}/{}", state.to_ascii_lowercase(), city.trim().to_ascii_lowercase()))?;
    let data = match relay.fetch.get(target, LIVE, FetchOptions::default()).await {
        Ok(entry) => entry.json().map_err(|e| anyhow!(e.message))?,
        Err(e) if e.upstream == Some(404) => return Ok(None),
        Err(e) => return Err(anyhow!(e.message)),
    };
    // Several ZIP codes share a town: average their points.
    let places: Vec<(String, f64, f64)> = data["places"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["place name"].as_str()?.to_owned(), p["latitude"].as_str()?.parse().ok()?, p["longitude"].as_str()?.parse().ok()?)))
        .collect();
    if places.is_empty() {
        return Ok(None);
    }
    let n = places.len() as f64;
    let lat = places.iter().map(|p| p.1).sum::<f64>() / n;
    let lon = places.iter().map(|p| p.2).sum::<f64>() / n;
    Ok(Some((places[0].0.clone(), (lat * 1e4).round() / 1e4, (lon * 1e4).round() / 1e4)))
}

/// The county containing a point, via the NWS points API (`county` link -> zone id).
pub async fn county_at(relay: &AppState, lat: f64, lon: f64) -> Result<Option<String>> {
    let meta = crate::weather::points(relay, lat, lon).await.map_err(|e| anyhow!(e.message))?;
    Ok(meta["properties"]["county"].as_str().and_then(|u| u.rsplit('/').next()).map(str::to_owned))
}

