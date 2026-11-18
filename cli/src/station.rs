//! The station's settings (the desktop app's `settings.json`) and region lookups.
//!
//! Settings are edited as JSON so fields this tool does not know about are kept untouched. The
//! app reads the file at launch: restart it (or reopen Settings) after changes made here.

use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use openwx_relay::AppState;
pub use openwx_relay::region::*;

pub fn settings_path() -> PathBuf {
    if let Some(path) = std::env::var_os("OPENWX_SETTINGS") {
        return PathBuf::from(path);
    }
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("OpenWXBroadcasting").join("settings.json")
}

pub fn load() -> Value {
    std::fs::read_to_string(settings_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).filter(Value::is_object).unwrap_or_else(|| json!({}))
}

pub fn save(settings: &Value) -> Result<PathBuf> {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    if path.exists() {
        let _ = std::fs::copy(&path, path.with_extension("json.bak"));
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(settings)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Everything the region step decides.
#[derive(Clone, Debug)]
pub struct Region {
    pub states: Vec<String>,
    pub counties: Vec<County>,
    pub home_fips: String,
    pub city: String,
    pub state_of_city: String,
    pub lat: f64,
    pub lon: f64,
    pub name: String,
}

/// Resolve a region from plain answers (used by the setup wizard and the MCP tool).
pub async fn resolve_region(relay: &AppState, state: &str, county_names: &str, city: &str, name: &str) -> Result<Region> {
    let st = state_abbr(state).ok_or_else(|| anyhow!("\"{state}\" is not a U.S. state or territory"))?;
    let all = counties(relay, st).await?;
    let picked = pick_counties(&all, county_names)?;
    let (city_name, lat, lon) = find_city(relay, st, city).await?.with_context(|| format!("could not find the town \"{city}\" in {st}"))?;
    let zone = county_at(relay, lat, lon).await.ok().flatten();
    let home = zone
        .as_deref()
        .and_then(|z| picked.iter().find(|c| c.zone == z))
        .or(picked.first())
        .map(|c| c.fips.clone())
        .unwrap_or_default();
    Ok(Region {
        states: vec![st.to_owned()],
        counties: picked,
        home_fips: home,
        city: city_name,
        state_of_city: st.to_owned(),
        lat,
        lon,
        name: name.trim().to_owned(),
    })
}

/// Write a region into the app settings (coverage area, home, alert area, bookmarks).
pub fn apply_region(settings: &mut Value, region: &Region) {
    let s = settings.as_object_mut().expect("settings object");
    let home_county = region.counties.iter().find(|c| c.fips == region.home_fips);
    let counties: Vec<Value> = region
        .counties
        .iter()
        .map(|c| json!({ "fips": c.fips, "name": c.name, "state": c.state, "town": if c.fips == region.home_fips { region.city.clone() } else { String::new() } }))
        .collect();
    s.insert("coverage".into(), json!({ "name": region.name, "states": region.states, "home_fips": region.home_fips, "counties": counties }));
    s.insert("home".into(), json!({ "lat": region.lat, "lon": region.lon, "zoom": 8.2 }));
    s.insert("home_label".into(), json!(format!("{}, {}", region.city, region.state_of_city)));
    s.insert("alert_area".into(), json!(region.states.join(",")));
    let mut bookmarks = vec![json!({ "name": region.city, "view": { "lat": region.lat, "lon": region.lon, "zoom": 9.5 } })];
    if let Some(c) = home_county {
        bookmarks.push(json!({ "name": c.name, "view": { "lat": region.lat, "lon": region.lon, "zoom": 9.0 } }));
    }
    if !region.name.is_empty() {
        bookmarks.push(json!({ "name": region.name, "view": { "lat": region.lat, "lon": region.lon, "zoom": 7.2 } }));
    }
    bookmarks.push(json!({ "name": "CONUS", "view": { "lat": 38.5, "lon": -96.5, "zoom": 3.4 } }));
    s.insert("bookmarks".into(), json!(bookmarks));
    s.insert("setup_complete".into(), json!(true));
}

/// A short summary for people and AI assistants.
pub fn summary(settings: &Value) -> Value {
    let cov = &settings["coverage"];
    json!({
        "settingsFile": settings_path().display().to_string(),
        "station": settings["station_name"],
        "shortName": settings["station_short"],
        "tagline": settings["tagline"],
        "contactSet": settings["contact"].as_str().is_some_and(|c| !c.trim().is_empty()),
        "home": settings["home"],
        "homeLabel": settings["home_label"],
        "region": cov["name"],
        "states": cov["states"],
        "counties": cov["counties"].as_array().map(|a| a.iter().map(|c| c["name"].clone()).collect::<Vec<_>>()),
        "backend": settings["backend_mode"],
        "setupComplete": settings["setup_complete"],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<County> {
        ["Dubois", "Pike", "Spencer"].iter().enumerate().map(|(i, n)| County { zone: format!("INC{:03}", i), fips: format!("18{:03}", i), name: county_name(n, "IN"), state: "IN".into() }).collect()
    }

    #[test]
    fn states_by_name_or_code() {
        assert_eq!(state_abbr("indiana"), Some("IN"));
        assert_eq!(state_abbr(" ky "), Some("KY"));
        assert_eq!(state_fips("IN"), Some("18"));
        assert_eq!(state_abbr("Atlantis"), None);
    }

    #[test]
    fn county_picking() {
        let all = sample();
        let picked = pick_counties(&all, "dubois county, 2, Pike").unwrap();
        assert_eq!(picked.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Dubois County", "Pike County"]);
        assert!(pick_counties(&all, "Nowhere").is_err());
        assert!(pick_counties(&all, " , ").is_err());
        assert_eq!(county_name("Orleans", "LA"), "Orleans Parish");
    }

    #[test]
    fn region_is_written_into_settings() {
        let region = Region { states: vec!["IN".into()], counties: sample(), home_fips: "18000".into(), city: "Jasper".into(), state_of_city: "IN".into(), lat: 38.39, lon: -86.93, name: "Southern Indiana".into() };
        let mut s = json!({ "station_name": "Test", "unknown_field": 5 });
        apply_region(&mut s, &region);
        assert_eq!(s["home_label"], "Jasper, IN");
        assert_eq!(s["coverage"]["counties"][0]["town"], "Jasper");
        assert_eq!(s["coverage"]["counties"][1]["town"], "");
        assert_eq!(s["unknown_field"], 5, "unknown fields are kept");
        assert_eq!(s["setup_complete"], true);
        assert_eq!(summary(&s)["counties"][2], "Spencer County");
    }
}
