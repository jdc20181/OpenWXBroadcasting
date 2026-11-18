//! Data sources: licensing registry, redistribution rules, lineage (source -> products),
//! quality scoring from live request stats, integrity checks and the compliance report.

use crate::data::WxSnapshot;

pub struct SourceInfo {
    pub name: &'static str,
    /// Host fragment matched against request stats.
    pub host: &'static str,
    /// Products fed by this source (lineage).
    pub feeds: &'static [&'static str],
    pub license: &'static str,
    /// What the station may do with it on air / online.
    pub redistribution: &'static str,
    /// Credit line to show when required (empty = none required).
    pub credit: &'static str,
}

pub const SOURCES: &[SourceInfo] = &[
    SourceInfo {
        name: "LibreWXR (through OpenWX Relay)",
        host: "api.librewxr.net",
        feeds: &["Radar loop", "Satellite", "Storm cells", "GOES GLM lightning"],
        license: "LibreWXR terms of use",
        redistribution: "Check the LibreWXR terms before redistributing imagery",
        credit: "LibreWXR",
    },
    SourceInfo {
        name: "National Weather Service",
        host: "api.weather.gov",
        feeds: &["Warnings, watches and advisories", "Forecast periods", "Observations"],
        license: "US Government work, public domain",
        redistribution: "Allowed. Do not imply NWS endorsement",
        credit: "National Weather Service",
    },
    SourceInfo {
        name: "NOAA NOMADS",
        host: "nomads.ncep.noaa.gov",
        feeds: &["GFS model grid (GRIB2)", "Futurecast", "Winter Studio snow and ice"],
        license: "US Government work, public domain",
        redistribution: "Allowed. Read through OpenWX Relay",
        credit: "NOAA GFS",
    },
    SourceInfo {
        name: "NOAA map services",
        host: "mapservices.weather.noaa.gov",
        feeds: &["WPC rainfall (QPF)", "Excessive rainfall outlook", "SPC mesoscale discussions", "River gauges"],
        license: "US Government work, public domain",
        redistribution: "Allowed",
        credit: "NOAA",
    },
    SourceInfo {
        name: "National Water Prediction Service",
        host: "api.water.noaa.gov",
        feeds: &["River hydrographs and flood stages"],
        license: "US Government work, public domain",
        redistribution: "Allowed",
        credit: "NOAA NWPS",
    },
    SourceInfo {
        name: "Iowa Environmental Mesonet",
        host: "mesonet.agron.iastate.edu",
        feeds: &["Single-site NEXRAD products", "GOES satellite", "MRMS rainfall", "SPC outlooks", "Storm reports"],
        license: "Free for public use (Iowa State University)",
        redistribution: "Allowed with credit; heavy automated use should be cached",
        credit: "Iowa Environmental Mesonet",
    },
    SourceInfo {
        name: "Open-Meteo",
        host: "open-meteo.com",
        feeds: &["Model grids other than GFS (HRRR, NBM, ECMWF, ICON, GEM)", "Air quality", "Climate history and records", "16-day outlook", "Model comparison", "Place search"],
        license: "CC BY 4.0 data; free API tier is non-commercial",
        redistribution: "Commercial broadcast needs an Open-Meteo subscription (API key). Credit required",
        credit: "Weather data by Open-Meteo.com",
    },
    SourceInfo {
        name: "NOAA Space Weather Prediction Center",
        host: "services.swpc.noaa.gov",
        feeds: &["Kp index and aurora outlook"],
        license: "US Government work, public domain",
        redistribution: "Allowed",
        credit: "NOAA SWPC",
    },
    SourceInfo {
        name: "FAA NAS status",
        host: "nasstatus.faa.gov",
        feeds: &["Airport delays"],
        license: "US Government work, public domain",
        redistribution: "Allowed",
        credit: "FAA",
    },
    SourceInfo {
        name: "Esri basemaps",
        host: "arcgisonline.com",
        feeds: &["Map background tiles"],
        license: "Esri terms of use",
        redistribution: "Broadcast use under Esri's terms; keep the attribution on screen",
        credit: "Esri, HERE, Garmin, USGS",
    },
    SourceInfo {
        name: "Zippopotam.us",
        host: "zippopotam.us",
        feeds: &["ZIP code search"],
        license: "Free API (GeoNames data, CC BY 4.0)",
        redistribution: "Not shown on air",
        credit: "",
    },
    SourceInfo {
        name: "GeoNames",
        host: "geonames.org",
        feeds: &["Town names and populations"],
        license: "CC BY 4.0",
        redistribution: "Allowed with credit",
        credit: "GeoNames",
    },
];

/// Products the station has turned off for redistribution (web exports, control-server pages).
pub fn blocked_for_web(settings: &crate::config::Settings, source: &str) -> bool {
    settings.no_redistribute.iter().any(|s| s == source)
}

/// Which registry sources a graphic draws on, from its bindings and layers (best effort).
pub fn doc_sources(doc: &crate::gfx::model::GraphicDoc) -> Vec<&'static str> {
    let json = serde_json::to_string(doc).unwrap_or_default().to_lowercase();
    const KEYS: &[(&str, &[&str])] = &[
        ("Open-Meteo", &["hi24", "lo24", "change24", "total48", "severe.window", "winter.timeline", "hourly.", "climate.", "normal", "record", "outlook16", "model", "futurecast", "gust.max24", "pressure.trend", "\"series\""]),
        ("National Weather Service", &["alert.", "warning", "forecast.", "day1", "day2", "day3", "day4", "day5", "day6", "day7"]),
        ("DCWeather API", &["current.", "temp.now", "radar", "lightning", "road", "\"map\""]),
        ("NOAA Space Weather Prediction Center", &["kp.", "aurora"]),
        ("FAA NAS status", &["airport", "delay"]),
        ("NOAA map services", &["river", "qpf", "ero"]),
    ];
    KEYS.iter().filter(|(_, keys)| keys.iter().any(|k| json.contains(k))).map(|(name, _)| *name).collect()
}

/// The first blocked source a graphic uses, if any (web exports and control-server pages skip it).
pub fn blocked_source(settings: &crate::config::Settings, doc: &crate::gfx::model::GraphicDoc) -> Option<&'static str> {
    if settings.no_redistribute.is_empty() {
        return None;
    }
    doc_sources(doc).into_iter().find(|s| blocked_for_web(settings, s))
}

pub fn info_for_host(host: &str) -> Option<&'static SourceInfo> {
    SOURCES.iter().find(|s| host.contains(s.host))
}

/// Quality 0-100 per registry source, merged from all hosts that match it (None = not used yet).
pub fn quality(source: &SourceInfo) -> Option<(u32, crate::broadcast_ops::SourceStats)> {
    let mut merged = crate::broadcast_ops::SourceStats::default();
    for (host, s) in crate::broadcast_ops::source_stats() {
        if host.contains(source.host) {
            merged.requests += s.requests;
            merged.failures += s.failures;
            merged.total_ms += s.total_ms;
            merged.last_ok = merged.last_ok.max(s.last_ok);
            if !s.last_error.is_empty() {
                merged.last_error = s.last_error.clone();
            }
        }
    }
    (merged.requests > 0).then(|| (merged.quality(), merged))
}

/// Sanity checks on the data we are about to put on air. Each entry is a plain-language problem.
pub fn integrity(data: &WxSnapshot) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(ov) = &data.overview {
        let c = &ov.current;
        if let Some(t) = c.temperature_f {
            if !(-80.0..=135.0).contains(&t) {
                out.push(format!("Current temperature {t:.0}°F is outside the believable range"));
            }
            if let Some(d) = c.dewpoint_f {
                if d > t + 1.0 {
                    out.push(format!("Dew point {d:.0}°F is above the temperature {t:.0}°F"));
                }
            }
        }
        if let Some(h) = c.relative_humidity {
            if !(0.0..=100.5).contains(&h) {
                out.push(format!("Humidity {h:.0}% is impossible"));
            }
        }
        if let Some(p) = c.barometric_pressure_in_hg {
            if !(27.0..=31.5).contains(&p) {
                out.push(format!("Pressure {p:.2} inHg is outside the believable range"));
            }
        }
        if let Some(w) = c.wind_speed_mph {
            if !(0.0..=200.0).contains(&w) {
                out.push(format!("Wind speed {w:.0} mph is not believable"));
            }
        }
        for p in &ov.forecast {
            if let Some(t) = p.temperature {
                if !(-80.0..=135.0).contains(&t) {
                    out.push(format!("Forecast {} temperature {t:.0}° is not believable", p.name));
                }
            }
        }
    }
    if let Some(r) = &data.radar {
        let times: Vec<i64> = r.radar.past.iter().map(|f| f.time).collect();
        if times.windows(2).any(|w| w[1] <= w[0]) {
            out.push("Radar frames are out of order or repeated".to_string());
        }
        let now = chrono::Utc::now().timestamp();
        if times.iter().any(|t| *t > now + 600) {
            out.push("A radar frame is time-stamped in the future (check the PC clock)".to_string());
        }
    }
    let now = chrono::Utc::now();
    for a in data.local_alerts.iter().chain(data.map_alerts.iter()) {
        let sent = a.sent.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
        let exp = a.expires.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok());
        if let (Some(s), Some(e)) = (sent, exp) {
            if e < s {
                out.push(format!("{} expires before it was issued", a.event));
            }
        }
        if exp.map(|e| e < now - chrono::Duration::hours(1)).unwrap_or(false) {
            out.push(format!("{} expired over an hour ago but is still listed", a.event));
        }
        for ring in &a.polygons {
            if ring.iter().any(|(lon, lat)| !lat.is_finite() || !lon.is_finite() || lat.abs() > 90.0 || lon.abs() > 180.0) {
                out.push(format!("{} has a broken polygon", a.event));
                break;
            }
        }
    }
    if let Some(g) = &data.grid {
        let bad = g.invalid_fraction();
        if bad > 0.05 {
            out.push(format!("Model grid: {:.0}% of values are missing", bad * 100.0));
        }
    }
    if data.lightning.iter().any(|s| !s.lat.is_finite() || !s.lon.is_finite()) {
        out.push("Lightning feed has strikes without a valid location".to_string());
    }
    out.dedup();
    out
}

/// Markdown compliance report: sources used this session, licenses, credits, redistribution,
/// request counts and retention. Written to Documents\OpenWX\Exports.
pub fn compliance_report(settings: &crate::config::Settings) -> anyhow::Result<std::path::PathBuf> {
    let mut md = String::new();
    md.push_str(&format!("# Data compliance report\n\nStation: {}\nGenerated: {}\n\n", settings.station_name, chrono::Local::now().format("%Y-%m-%d %H:%M")));
    md.push_str("| Source | Feeds | License | Redistribution | Credit | Requests (session) | Failures | Quality | Web exports |\n|---|---|---|---|---|---|---|---|---|\n");
    for s in SOURCES {
        let q = quality(s);
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            s.name,
            s.feeds.join(", "),
            s.license,
            s.redistribution,
            if s.credit.is_empty() { "-" } else { s.credit },
            q.as_ref().map(|(_, st)| st.requests.to_string()).unwrap_or_else(|| "0".into()),
            q.as_ref().map(|(_, st)| st.failures.to_string()).unwrap_or_else(|| "0".into()),
            q.as_ref().map(|(v, _)| format!("{v}/100")).unwrap_or_else(|| "not used".into()),
            if blocked_for_web(settings, s.name) { "blocked" } else { "allowed" },
        ));
    }
    md.push_str(&format!(
        "\nRetention: logs, warning archive, incidents and audit kept for {}.\n\nDaily request logs: Developer Logs folder, data-access-YYYY-MM-DD.log.\n",
        if settings.retention_days == 0 { "ever".to_string() } else { format!("{} days", settings.retention_days) }
    ));
    let dir = crate::config::LibraryPaths::get().exports;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("Data compliance {}.md", chrono::Local::now().format("%Y-%m-%d")));
    std::fs::write(&path, md)?;
    Ok(path)
}
