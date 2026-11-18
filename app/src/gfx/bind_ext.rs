//! More graphics bindings: hour-by-hour and day-part forecasts from the model grid, sun / moon /
//! season, rainfall / snow / ice totals, storm reports, rivers, WPC rainfall and excessive-rain
//! risk at home, closings and warming / cooling centers, roads, nearby cities, weekend and
//! commute forecasts, storm / winter timing windows, and Spanish wording.
//!
//! Everything here is added by [`extend`], called at the end of `Bindings::build`.

use chrono::{DateTime, Datelike, Local, Timelike};

use super::bind::Bindings;
use crate::{
    config::{Settings, Units},
    data::WxSnapshot,
    grid::{PrecipKind, Var, WxGrid},
};

/// Every binding added here, with a description (merged into the binding picker).
pub const CATALOG_EXT: &[(&str, &str)] = &[
    ("climate.normal_hi", "Normal high today"),
    ("climate.normal_lo", "Normal low today"),
    ("climate.record_hi", "Record high today (year)"),
    ("climate.record_lo", "Record low today (year)"),
    ("climate.normals_source", "Where the normals come from (NOAA station or model reanalysis)"),
    ("climate.normals_station", "NOAA station behind the normals"),
    ("departure.hi", "Today's forecast high vs normal"),
    ("departure.lo", "Tonight's forecast low vs normal"),
    ("rain.mtd", "Rain so far this month"),
    ("rain.mtd_normal", "Normal rain to date this month"),
    ("rain.departure", "Rain this month vs normal"),
    ("frost.last", "Median last spring freeze"),
    ("frost.first", "Median first fall freeze"),
    ("season.growing", "Typical growing season length"),
    ("ten0.name", "10-day: day name (0-9)"),
    ("ten0.hi", "10-day: high (0-9)"),
    ("ten0.lo", "10-day: low (0-9)"),
    ("ten0.pop", "10-day: rain chance (0-9)"),
    ("ten0.cond", "10-day: conditions (0-9)"),
    ("models.tomorrow", "Tomorrow's high from GFS, ECMWF and ICON"),
    ("models.confidence", "Forecast confidence from model agreement"),
    ("models.confidence_color", "Forecast confidence color (for a text layer)"),
    ("aurora.kp", "Highest Kp index forecast, next 3 days"),
    ("aurora.chance", "Northern lights chance at home"),
    ("meteor.next", "Next meteor shower"),
    ("eclipse.next", "Next eclipse visible from the US"),
    ("sky.tonight", "Night sky viewing tonight"),
    ("airport.summary", "US airport delays"),
    ("airport0.line", "Airport delay (0-7)"),
    ("model.bias", "Model vs observed temperature now"),
    ("sky_image", "Background picture for the current sky (use as an Image layer's media)"),
    ("day0.sky_image", "Background picture for a forecast day (0-6)"),
    ("hour0.time", "Hour-by-hour: time (0-11)"),
    ("hour0.temp", "Hour-by-hour: temperature (0-11)"),
    ("hour0.cond", "Hour-by-hour: sky / precipitation (0-11)"),
    ("hour0.wind", "Hour-by-hour: wind (0-11)"),
    ("hour0.gust", "Hour-by-hour: gusts (0-11)"),
    ("hour0.rain", "Hour-by-hour: precipitation that hour (0-11)"),
    ("part.morning.temp", "Morning / afternoon / evening temperature"),
    ("part.morning.cond", "Morning / afternoon / evening conditions"),
    ("temp.hi24", "Model high, next 24 hours"),
    ("temp.lo24", "Model low, next 24 hours"),
    ("temp.change24", "Temperature change, now vs this time tomorrow"),
    ("gust.max24", "Strongest gust, next 24 hours"),
    ("pressure.trend", "Pressure trend (rising / falling / steady)"),
    ("rain.total48", "Model rain total, next 48 hours"),
    ("snow.total48", "Model snow total, next 48 hours"),
    ("ice.total48", "Model freezing rain / mix total, next 48 hours"),
    ("severe.window", "When thunderstorms are most likely (next 24 h)"),
    ("snd.mlcape", "Sounding at home: mixed-layer CAPE (also snd.sbcape, snd.mucape, snd.mlcin)"),
    ("snd.srh01", "Sounding: 0-1 km storm-relative helicity (also snd.srh03)"),
    ("snd.shear06", "Sounding: 0-6 km bulk shear (also snd.shear01)"),
    ("snd.stp", "Sounding: Significant Tornado Parameter"),
    ("snd.scp", "Sounding: Supercell Composite Parameter"),
    ("snd.lcl", "Sounding: cloud base (LCL)"),
    ("snd.lapse75", "Sounding: 700-500 mb lapse rate (also snd.lapse03)"),
    ("snd.freezing", "Sounding: freezing level (also snd.wbz wet-bulb zero)"),
    ("snd.pbl", "Sounding: boundary layer depth"),
    ("snd.inversion", "Sounding: lowest temperature inversion"),
    ("snd.inflow", "Sounding: effective inflow layer"),
    ("snd.convtemp", "Sounding: convective temperature"),
    ("snd.storm_motion", "Sounding: supercell (Bunkers right) storm motion"),
    ("ens.snow_range", "Ensemble snow range at home, next 72 h (also ens.rain_range)"),
    ("ens.best", "Best case (10th percentile) snow and rain (also ens.likely, ens.worst)"),
    ("ens.day1.confidence", "Forecast confidence for day 0-6 from ensemble spread (also ens.dayN.range)"),
    ("models.agreement", "How closely GFS, ECMWF and ICON agree"),
    ("verif.best", "Most accurate source for day-1 highs, last 30 days (also verif.best_mae)"),
    ("verif.station.mae", "Station forecast high-temperature error, last 30 days (also verif.station.bias, verif.gfs.mae …)"),
    ("verif.pairs", "Forecasts verified in the last 30 days"),
    ("power.total", "Customers without power, all utilities (also power.pct, power.peak, power.trend)"),
    ("power.top1", "County with the most outages (also .count, .pct, .etr, .restored; top1-8)"),
    ("power.utility1", "Utility 1 name (also .count, .pct; utility1-6)"),
    ("power.restored", "Share restored since the peak of this event"),
    ("power.status", "Where the outage numbers come from, or that no utilities are set up"),
    ("power.etr", "Latest estimated restoration time"),
    ("power.facilities", "Hospitals and water plants in hard-hit counties"),
    ("nowcast.arrival", "Chance a storm reaches home in the next hour (also nowcast.eta)"),
    ("nowcast.top", "Strongest storm's nowcast: hail, wind, heavy rain, weakening chance"),
    ("nowcast.hail", "Highest hail threat now (also nowcast.wind, nowcast.rain)"),
    ("nowcast.initiation", "Whether new storms may form nearby"),
    ("nowcast.error", "Average 30-minute nowcast position error, last 6 hours"),
    ("tropical.name", "Active tropical storm name (also tropical.category, .wind, .next, .arrival)"),
    ("aqi.home", "Air Quality Index at home now (also aqi.category, aqi.advice)"),
    ("fire.danger", "Fire danger at home (Fosberg index wording)"),
    ("fire.nearest", "Nearest wildfire: name, acres and containment"),
    ("redflag.text", "Red Flag Warning in effect, or none"),
    ("drought.home", "Drought category at home"),
    ("drought.valid", "Week the Drought Monitor map is valid for"),
    ("drought.outlook_month", "NOAA CPC drought outlook at home: next month"),
    ("drought.outlook_season", "NOAA CPC drought outlook at home: the season ahead"),
    ("clim.yesterday", "Yesterday: high, low, rain, snow"),
    ("clim.month_temp", "Month to date: temperature vs normal"),
    ("clim.month_precip", "Month to date: precipitation vs normal"),
    ("clim.ytd_precip", "Year-to-date precipitation vs normal"),
    ("clim.season_snow", "Season snowfall to date vs last year and the 10-year average"),
    ("clim.this_day", "On this date: the most notable year"),
    ("clim.source", "Credit line for climate cards: the NOAA station behind the history (ACIS)"),
    ("viewer1.photo", "Approved viewer photo 1-4 (Image layer media), with viewerN.credit and viewerN.caption"),
    ("question.text", "Viewer question on air (also question.from)"),
    ("poll.question", "Live poll question (chart series poll.results, poll.total)"),
    ("winter.squall_time", "When snow squalls could reach home"),
    ("winter.road_icing", "Road icing potential at home"),
    ("winter.ratio", "Snow-to-liquid ratio at home"),
    ("travel1.name", "Travel route 1-4: \"Evansville to Louisville\""),
    ("travel1.time", "Travel route 1-4: estimated drive time now"),
    ("travel1.normal", "Travel route 1-4: usual drive time"),
    ("travel1.delay", "Travel route 1-4: extra time (\"+15 min\" or \"On time\")"),
    ("travel1.note", "Travel route 1-4: why it is slower (weather, incidents)"),
    ("camera1.name", "Camera 1-4 name (Settings > Weather & maps > Cameras)"),
    ("camera1.image", "Camera 1-4 image (use as an Image layer's media)"),
    ("camera.storm.name", "Camera nearest the strongest storm"),
    ("camera.storm.image", "Image of the camera nearest the strongest storm"),
    ("cmp1.name", "Storm comparison 1-5: storm and nearest town"),
    ("cmp1.radar", "Storm comparison 1-5: radar (dBZ, hail)"),
    ("cmp1.lightning", "Storm comparison 1-5: lightning flashes in 10 min"),
    ("cmp1.reports", "Storm comparison 1-5: storm reports nearby"),
    ("cmp1.warning", "Storm comparison 1-5: warning in effect"),
    ("cmp1.agreement", "Storm comparison 1-5: \"Severe by 2 of 4 sources\" (radar, lightning, reports, warning)"),
    ("traffic1.text", "Traffic event 1-6: route and headline"),
    ("track1.id", "Storm track 1-5 (strongest first): id like S4"),
    ("track1.trend", "Storm track 1-5: Strengthening / Steady / Weakening"),
    ("track1.lightning", "Storm track 1-5: flashes per minute, with a lightning jump note"),
    ("mcs.summary", "Squall line / storm complex summary, or \"None\""),
    ("traffic.count", "Number of traffic incidents and closures"),
    ("spc.home", "SPC risk at home today (spc.day2.home, spc.day3.home for later days)"),
    ("spc.home.level", "SPC risk level at home, 0-5 (for risk meters)"),
    ("spc.home.short", "SPC risk at home: TSTM, MRGL, SLGT, ENH, MDT or HIGH"),
    ("spc.home.number", "SPC risk at home as \"Level 3 of 5\""),
    ("spc.home.tornado", "Tornado chance at home (also .wind, .hail; .level 0-5, .words, .sig)"),
    ("spc.home.tornado.level", "Tornado threat at home on the 1-5 scale (also wind, hail, flood)"),
    ("spc.home.hazards", "Hazard chances at home in one line"),
    ("spc.day1.weekday", "Outlook day name: Today, Tomorrow, Friday (day1-3; also .date)"),
    ("threat1.town", "Local threat summary 1-5: town name (largest towns in the coverage area)"),
    ("threat1.level", "Local threat 1-5: SPC risk level at the town"),
    ("threat1.hazards", "Local threat 1-5: tornado / hail / wind chances at the town"),
    ("threat1.timing", "Local threat 1-5: when storms are most likely there"),
    ("threat1.warning", "Local threat 1-5: warnings in effect there"),
    ("threat1.summary", "Local threat 1-5: one-line summary"),
    ("winter.timeline", "Winter precipitation start / end (next 48 h)"),
    ("sun.rise", "Sunrise today"),
    ("sun.set", "Sunset today"),
    ("sun.daylight", "Daylight today"),
    ("sun.change", "Daylight change since yesterday"),
    ("day0.sunrise", "Sunrise for forecast day (0-6)"),
    ("day0.sunset", "Sunset for forecast day (0-6)"),
    ("moon.phase", "Moon phase"),
    ("moon.illum", "Moon illumination"),
    ("moon.next_full", "Next full moon"),
    ("season.next", "Next season start"),
    ("season.days", "Days until the next season"),
    ("reports.count", "Storm reports, last 24 h"),
    ("reports.summary", "Storm reports by type"),
    ("reports.counties", "Counties with the most reports"),
    ("report0.line", "Storm report line, newest first (0-7)"),
    ("river.summary", "Rivers in flood near home"),
    ("river0.name", "River gauge name, worst first (0-5)"),
    ("river0.stage", "River gauge stage (0-5)"),
    ("river0.flood", "River gauge flood stage (0-5)"),
    ("river0.status", "River gauge category (0-5)"),
    ("river0.forecast", "River gauge forecast (0-5)"),
    ("qpf.day1", "WPC rain forecast at home, Day 1"),
    ("qpf.day3", "WPC rain forecast at home, 3 days"),
    ("qpf.day7", "WPC rain forecast at home, 7 days"),
    ("ero.day1", "Excessive rainfall risk at home, Day 1 (also day2, day3)"),
    ("closings.count", "Number of closings / delays"),
    ("closings.ticker", "All closings as one crawl line"),
    ("closing0.name", "Closing name (0-19)"),
    ("closing0.status", "Closing status (0-19)"),
    ("center0.name", "Warming / cooling center (0-9)"),
    ("center0.details", "Center address and hours (0-9)"),
    ("roads.summary", "INDOT closures and incidents"),
    ("road0.line", "Road event (0-5)"),
    ("city0.name", "Nearby city (0-5)"),
    ("city0.temp", "Nearby city temperature (0-5)"),
    ("city0.cond", "Nearby city conditions (0-5)"),
    ("weekend.sat", "Saturday forecast line"),
    ("weekend.sun", "Sunday forecast line"),
    ("commute.am", "Morning commute"),
    ("commute.pm", "Evening commute"),
    ("lightning.rate", "Strikes per 5 minutes near home, trend"),
    ("day0.name_es", "Day name in Spanish (0-6)"),
    ("day0.condition_es", "Forecast condition in Spanish (0-6)"),
    ("condition_es", "Current sky in Spanish"),
];

fn deg(v: f32, units: Units) -> String {
    match units {
        Units::Imperial => format!("{v:.0}°"),
        Units::Metric => format!("{:.0}°", (v - 32.0) * 5.0 / 9.0),
    }
}

fn mph(v: f32, units: Units) -> String {
    match units {
        Units::Imperial => format!("{v:.0} mph"),
        Units::Metric => format!("{:.0} km/h", v * 1.609_344),
    }
}

fn inches(v: f32, units: Units) -> String {
    match units {
        Units::Imperial if v < 0.01 => "0\"".into(),
        Units::Imperial if v < 1.0 => format!("{v:.2}\"").replacen("0.", ".", 1),
        Units::Imperial => format!("{v:.1}\""),
        Units::Metric => format!("{:.0} mm", v * 25.4),
    }
}

fn clock(t: DateTime<Local>, h24: bool) -> String {
    if h24 {
        t.format("%H:%M").to_string()
    } else {
        t.format("%-I:%M %p").to_string()
    }
}

fn hour_label(t: DateTime<Local>, h24: bool) -> String {
    if h24 {
        t.format("%H:00").to_string()
    } else {
        t.format("%-I %p").to_string()
    }
}

/// Plain sky / precipitation words for a model hour.
fn condition(grid: &WxGrid, tf: f32, lat: f64, lon: f64, night: bool) -> String {
    let code = grid.sample(Var::Code, tf, lat, lon).unwrap_or(0.0).round() as i32;
    if let Some((rate, kind)) = grid.precip(tf, lat, lon) {
        if rate >= 0.01 {
            let heavy = rate >= 0.25;
            let base = match kind {
                PrecipKind::Snow => if heavy { "Heavy snow" } else { "Snow" },
                PrecipKind::Mix => "Wintry mix",
                PrecipKind::Rain if (95..=99).contains(&code) => "Thunderstorms",
                PrecipKind::Rain => if heavy { "Heavy rain" } else if rate < 0.04 { "Light rain" } else { "Rain" },
            };
            return base.to_string();
        }
    }
    let cloud = grid.sample(Var::Cloud, tf, lat, lon).unwrap_or(0.0);
    match (cloud, night) {
        (c, false) if c < 20.0 => "Sunny".into(),
        (c, true) if c < 20.0 => "Clear".into(),
        (c, false) if c < 50.0 => "Mostly sunny".into(),
        (c, true) if c < 50.0 => "Mostly clear".into(),
        (c, _) if c < 80.0 => "Partly cloudy".into(),
        _ => "Cloudy".into(),
    }
}

fn is_night(t: DateTime<Local>, lat: f64, lon: f64) -> bool {
    match crate::astro::sun_times(t.date_naive(), lat, lon) {
        Some((rise, set)) => t < rise || t > set,
        None => !(6..19).contains(&t.hour()),
    }
}

/// Fractional grid index for a local clock time today (or tomorrow if already past).
fn tf_for_hour(grid: &WxGrid, hour: u32) -> Option<f32> {
    let now = Local::now();
    let mut target = now.date_naive().and_hms_opt(hour, 0, 0)?.and_local_timezone(Local).single()?;
    if target < now {
        target += chrono::Duration::days(1);
    }
    let offset = (target - now).num_minutes() as f32 / 60.0;
    (offset <= 47.0).then(|| grid.time_index(offset))
}

/// SPC risk words for categorical labels.
fn risk_words(label: &str) -> &'static str {
    match label.trim().to_uppercase().as_str() {
        "TSTM" | "GENERAL THUNDERSTORMS" => "General storms",
        "MRGL" | "MARGINAL" | "1" => "Marginal risk",
        "SLGT" | "SLIGHT" | "2" => "Slight risk",
        "ENH" | "ENHANCED" | "3" => "Enhanced risk",
        "MDT" | "MODERATE" | "4" => "Moderate risk",
        "HIGH" | "5" => "High risk",
        _ => "",
    }
}

/// SPC categorical level 0-5 from an outlook label (TSTM = 0).
pub fn spc_cat_level(label: &str) -> u8 {
    let t = label.trim().to_uppercase();
    if t.contains("HIGH") || t == "5" {
        5
    } else if t.contains("MDT") || t.contains("MODERATE") || t == "4" {
        4
    } else if t.contains("ENH") || t == "3" {
        3
    } else if t.contains("SLGT") || t.contains("SLIGHT") || t == "2" {
        2
    } else if t.contains("MRGL") || t.contains("MARGINAL") || t == "1" {
        1
    } else {
        0
    }
}

/// Probability (percent) from an SPC probability label ("0.15", "15%", "15").
pub fn spc_pct(label: &str) -> Option<f32> {
    let v: f32 = label.trim().trim_end_matches('%').trim().parse().ok()?;
    Some(if v > 0.0 && v <= 1.0 { v * 100.0 } else { v })
}

/// SPC's probability-to-category conversion for one hazard (Day 1-2 tables; "severe" is the
/// combined Day 3+ probability). `sig` = inside a significant-severe (hatched) area.
pub fn spc_hazard_level(hazard: &str, pct: f32, sig: bool) -> u8 {
    let p = pct.round() as i32;
    match hazard {
        "tornado" => match p {
            p if p >= 45 => 5,
            p if p >= 30 => if sig { 5 } else { 4 },
            p if p >= 15 => if sig { 4 } else { 3 },
            p if p >= 10 => 3,
            p if p >= 5 => 2,
            p if p >= 2 => 1,
            _ => 0,
        },
        "wind" => match p {
            p if p >= 75 => 5,
            p if p >= 60 => if sig { 5 } else { 4 },
            p if p >= 45 => if sig { 4 } else { 3 },
            p if p >= 30 => 3,
            p if p >= 15 => 2,
            p if p >= 5 => 1,
            _ => 0,
        },
        "hail" => match p {
            p if p >= 60 => 4,
            p if p >= 45 => if sig { 4 } else { 3 },
            p if p >= 30 => 3,
            p if p >= 15 => 2,
            p if p >= 5 => 1,
            _ => 0,
        },
        _ => match p {
            p if p >= 60 => 5,
            p if p >= 45 => 4,
            p if p >= 30 => 3,
            p if p >= 15 => 2,
            p if p >= 5 => 1,
            _ => 0,
        },
    }
}

/// WPC excessive rainfall level (Marginal 1, Slight 2, Moderate 4, High 5 on the severe scale).
pub fn ero_level(label: &str) -> u8 {
    let t = label.to_uppercase();
    if t.contains("HIGH") {
        5
    } else if t.contains("MODERATE") || t.contains("MDT") {
        4
    } else if t.contains("SLIGHT") || t.contains("SLGT") {
        2
    } else if t.contains("MARGINAL") || t.contains("MRGL") {
        1
    } else {
        0
    }
}

const LEVEL_WORDS: [&str; 6] = ["No severe risk", "Marginal risk", "Slight risk", "Enhanced risk", "Moderate risk", "High risk"];
const LEVEL_SHORT: [&str; 6] = ["NONE", "MRGL", "SLGT", "ENH", "MDT", "HIGH"];

/// SPC outlook at the home point for each day: overall level, each hazard's chance and its
/// level on the same 1-5 scale, and the flooding-rain level (spc.home.* = Day 1).
fn spc_home_bindings(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let today = Local::now().date_naive();
    for day in 1..=3u8 {
        let weekday = today + chrono::Duration::days(i64::from(day) - 1);
        b.set(&format!("spc.day{day}.weekday"), &if day == 1 { "Today".to_string() } else if day == 2 { "Tomorrow".to_string() } else { weekday.format("%A").to_string() });
        b.set(&format!("spc.day{day}.date"), &weekday.format("%A, %B %-d").to_string());
    }
    // No outlook loaded yet is not the same as "no risk": leave the values missing.
    if data.spc.is_empty() {
        return;
    }
    let (lat, lon) = (settings.home.lat, settings.home.lon);
    for day in 1..=3u8 {
        let prefix = format!("spc.day{day}.home");
        let spc = data.spc.iter().find(|d| d.day == day);
        let inside = |area: &crate::data::SpcArea| area.rings.iter().any(|r| crate::data::point_in_ring(r, lon, lat));
        let cat = spc.and_then(|d| d.categorical.iter().filter(|a| inside(a)).max_by_key(|a| a.rank));
        let level = cat.map(|a| spc_cat_level(&a.label)).unwrap_or(0);
        let tstm = cat.is_some() && level == 0;
        let words = if tstm { "General thunderstorms" } else { LEVEL_WORDS[level as usize] };
        b.set(&prefix, words);
        b.set(&format!("{prefix}.level"), &level.to_string());
        b.set(&format!("{prefix}.short"), if tstm { "TSTM" } else { LEVEL_SHORT[level as usize] });
        b.set(&format!("{prefix}.number"), &if level > 0 { format!("Level {level} of 5") } else if tstm { "Thunderstorms possible".to_string() } else { "No severe risk".to_string() });
        let weekday = today + chrono::Duration::days(i64::from(day) - 1);
        b.set(&format!("spc.day{day}.weekday"), &if day == 1 { "Today".to_string() } else if day == 2 { "Tomorrow".to_string() } else { weekday.format("%A").to_string() });
        b.set(&format!("spc.day{day}.date"), &weekday.format("%A, %B %-d").to_string());

        let mut hazard_lines = Vec::new();
        for key in ["tornado", "wind", "hail", "severe"] {
            let hz = spc.and_then(|d| d.probabilities.iter().find(|h| h.key == key));
            let pct = hz
                .and_then(|h| h.areas.iter().filter(|a| inside(a) && !a.label.to_uppercase().contains("SIG")).filter_map(|a| spc_pct(&a.label)).reduce(f32::max))
                .unwrap_or(0.0);
            let sig = hz.map(|h| h.areas.iter().any(|a| inside(a) && a.label.to_uppercase().contains("SIG"))).unwrap_or(false);
            let lvl = spc_hazard_level(key, pct, sig);
            b.set(&format!("{prefix}.{key}"), &if pct > 0.0 { format!("{pct:.0}%") } else { "Low".to_string() });
            b.set(&format!("{prefix}.{key}.level"), &lvl.to_string());
            b.set(&format!("{prefix}.{key}.words"), LEVEL_WORDS[lvl as usize]);
            b.set(&format!("{prefix}.{key}.sig"), if sig { "Significant" } else { "" });
            if pct > 0.0 && hz.is_some() {
                hazard_lines.push(format!("{} {pct:.0}%{}", capitalize(key), if sig { " (significant)" } else { "" }));
            }
        }
        // Day 3 has only the combined chance: use it for every hazard bar.
        if day >= 3 {
            let lvl = b.get(&format!("{prefix}.severe.level")).unwrap_or("0").to_string();
            for key in ["tornado", "wind", "hail"] {
                b.set(&format!("{prefix}.{key}.level"), &lvl);
            }
        }
        b.set(&format!("{prefix}.hazards"), &if hazard_lines.is_empty() { "Low chances".to_string() } else { hazard_lines.join(", ") });
        let flood = data
            .ero
            .get(usize::from(day) - 1)
            .and_then(|areas| areas.iter().filter(|a| inside(a)).map(|a| ero_level(&a.label)).max())
            .unwrap_or(0);
        b.set(&format!("{prefix}.flood.level"), &flood.to_string());
        b.set(&format!("{prefix}.flood.words"), match flood { 1 => "Marginal", 2 => "Slight", 4 => "Moderate", 5 => "High", _ => "Low" });
    }
    // spc.home.* = Day 1.
    let day1: Vec<(String, String)> = b.values().iter().filter(|(k, _)| k.starts_with("spc.day1.home")).map(|(k, v)| (k.replacen("spc.day1.home", "spc.home", 1), v.clone())).collect();
    for (k, v) in day1 {
        b.set(&k, &v);
    }
}

/// Threat summaries for the five largest towns in the coverage area: SPC level and hazard chances
/// at the town, storm timing from the model there, and warnings in effect.
fn threat_summaries(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let home = settings.home;
    let miles = settings.coverage_miles.max(25.0);
    let dlat = miles / 69.0;
    let dlon = miles / (69.0 * home.lat.to_radians().cos().max(0.2));
    let mut towns = crate::places::in_box(home.lat - dlat, home.lat + dlat, home.lon - dlon, home.lon + dlon);
    towns.sort_by(|a, b| b.population.cmp(&a.population));
    let day1 = data.spc.iter().find(|d| d.day == 1);
    let h24 = settings.clock_24h;
    for (n, town) in towns.iter().take(5).enumerate() {
        let k = n + 1;
        let inside = |area: &crate::data::SpcArea| area.rings.iter().any(|r| crate::data::point_in_ring(r, town.lon, town.lat));
        let level = day1
            .and_then(|d| d.categorical.iter().filter(|a| inside(a)).max_by_key(|a| a.rank))
            .map(|a| risk_words(&a.label).to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "No severe risk".into());
        let mut hazards = Vec::new();
        if let Some(d) = day1 {
            for h in &d.probabilities {
                if let Some(a) = h.areas.iter().filter(|a| inside(a) && !a.label.to_uppercase().contains("SIG")).max_by_key(|a| a.rank) {
                    // SPC labels are fractions ("0.05" = 5%); spc_pct reads either form.
                    if let Some(pct) = spc_pct(&a.label).filter(|p| *p > 0.0) {
                        let sig = h.areas.iter().any(|a| inside(a) && a.label.to_uppercase().contains("SIG"));
                        hazards.push(format!("{} {pct:.0}%{}", capitalize(&h.label), if sig { " (significant)" } else { "" }));
                    }
                }
            }
        }
        let mut timing = String::new();
        if let Some(grid) = home_grid(data, settings) {
            let hours: Vec<i64> = (0..24)
                .filter_map(|h| {
                    let tf = grid.time_index(h as f32);
                    (condition(grid, tf, town.lat, town.lon, false) == "Thunderstorms").then(|| grid.time_at(tf))
                })
                .collect();
            if let (Some(a), Some(z)) = (hours.first(), hours.last()) {
                let a = DateTime::from_timestamp(*a, 0).map(|t| t.with_timezone(&Local));
                let z = DateTime::from_timestamp(*z + 3600, 0).map(|t| t.with_timezone(&Local));
                if let (Some(a), Some(z)) = (a, z) {
                    timing = format!("{} to {}", hour_label(a, h24), hour_label(z, h24));
                }
            }
        }
        let warnings: Vec<String> = data
            .local_alerts
            .iter()
            .chain(data.map_alerts.iter())
            .filter(|a| a.is_warning() && a.contains(town.lat, town.lon))
            .map(|a| a.event.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let warning = warnings.join(", ");
        let summary = if !warning.is_empty() {
            format!("{warning} in effect now")
        } else if level == "No severe risk" && timing.is_empty() {
            "No severe weather expected".to_string()
        } else {
            let mut s = level.clone();
            if !hazards.is_empty() {
                s.push_str(&format!(": {}", hazards.join(", ")));
            }
            if !timing.is_empty() {
                s.push_str(&format!(". Storms {timing}"));
            }
            s
        };
        b.set(&format!("threat{k}.town"), &town.name);
        b.set(&format!("threat{k}.level"), &level);
        b.set(&format!("threat{k}.hazards"), &if hazards.is_empty() { "Low chances".to_string() } else { hazards.join(", ") });
        b.set(&format!("threat{k}.timing"), &if timing.is_empty() { "No storms expected".to_string() } else { timing });
        b.set(&format!("threat{k}.warning"), &if warning.is_empty() { "None".to_string() } else { warning });
        b.set(&format!("threat{k}.summary"), &summary);
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Travel times, cameras, storm comparison and traffic events.
fn travel_bindings(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    for (i, route) in crate::travel::routes(settings).iter().take(4).enumerate() {
        let k = i + 1;
        let e = crate::travel::estimate(route, data);
        let extra = e.now_min - e.normal_min;
        b.set(&format!("travel{k}.name"), &e.name);
        b.set(&format!("travel{k}.time"), &crate::travel::minutes_text(e.now_min));
        b.set(&format!("travel{k}.normal"), &crate::travel::minutes_text(e.normal_min));
        b.set(&format!("travel{k}.delay"), &if extra >= 3.0 { format!("+{}", crate::travel::minutes_text(extra)) } else { "On time".to_string() });
        b.set(&format!("travel{k}.note"), &e.note);
    }
    for (i, cam) in settings.webcams.iter().take(4).enumerate() {
        let st = crate::travel::cam_status(&cam.name);
        b.set(&format!("camera{}.name", i + 1), &cam.name);
        b.set(&format!("camera{}.image", i + 1), &crate::travel::cam_file(&cam.name));
        b.set(&format!("camera{}.credit", i + 1), &if cam.credit.is_empty() { cam.name.clone() } else { format!("Camera: {}", cam.credit) });
        b.set(&format!("camera{}.condition", i + 1), &st.condition);
    }
    // Automatic camera selection: the camera nearest the most serious warning in coverage.
    let mut warnings: Vec<&crate::data::Alert> = data.map_alerts.iter().filter(|a| a.is_warning()).collect();
    warnings.sort_by_key(|a| a.priority());
    if let Some((lat, lon)) = warnings.first().and_then(|a| a.centroid()) {
        if let Some((cam, mi)) = crate::travel::nearest_cam(&settings.webcams, lat, lon) {
            b.set("camera.warning.name", &format!("{} ({mi:.0} mi from the {})", cam.name, warnings[0].event.to_lowercase()));
            b.set("camera.warning.image", &crate::travel::cam_file(&cam.name));
        }
    }
    // Camera frame matching the newest radar frame (camera / radar timeline sync).
    if let (Some(cam), Some(t)) = (settings.webcams.first(), data.radar.as_ref().and_then(|r| r.latest_past()).map(|f| f.time)) {
        if let Some(name) = crate::travel::archive_near(&cam.name, t) {
            b.set("camera.radar_sync.image", &name);
        }
    }
    if let Some(cell) = data.cells.iter().max_by(|a, b| a.max_dbz.total_cmp(&b.max_dbz)) {
        if let Some((cam, mi)) = crate::travel::nearest_cam(&settings.webcams, cell.lat, cell.lon) {
            b.set("camera.storm.name", &format!("{} ({mi:.0} mi from the storm)", cam.name));
            b.set("camera.storm.image", &crate::travel::cam_file(&cam.name));
        }
    }
    for (i, c) in crate::travel::storm_comparison(data, settings).iter().enumerate() {
        let k = i + 1;
        b.set(&format!("cmp{k}.name"), &c.name);
        b.set(&format!("cmp{k}.radar"), &c.radar);
        b.set(&format!("cmp{k}.lightning"), &c.lightning);
        b.set(&format!("cmp{k}.reports"), &c.reports);
        b.set(&format!("cmp{k}.warning"), &c.warning);
        b.set(&format!("cmp{k}.agreement"), &match c.agreement {
            0 => "No source shows it severe".to_string(),
            n => format!("Severe by {n} of 4 sources"),
        });
    }
    let mut tracks: Vec<&crate::cells::Track> = data.analysis.tracks.iter().filter(|t| !t.ended).collect();
    tracks.sort_by(|a, b| b.points.last().map(|p| p.dbz).unwrap_or(0.0).total_cmp(&a.points.last().map(|p| p.dbz).unwrap_or(0.0)));
    for (i, t) in tracks.iter().take(5).enumerate() {
        let k = i + 1;
        let fl = t.points.last().map(|p| p.flashes as f32 / 2.0).unwrap_or(0.0);
        b.set(&format!("track{k}.id"), &t.id);
        b.set(&format!("track{k}.trend"), &t.trend_label);
        b.set(&format!("track{k}.lightning"), &format!("{fl:.0} flashes/min{}", if t.jump { " (lightning jump)" } else { "" }));
    }
    b.set("mcs.summary", &match data.analysis.mcs.first() {
        Some(m) => format!("{} about {:.0} miles long", if m.squall_line { "Squall line" } else { "Storm complex" }, m.length_mi),
        None => "None".to_string(),
    });
    let traffic: Vec<&crate::data::RoadEvent> = data.roads.iter().filter(|e| matches!(e.kind.as_str(), "closure" | "incident" | "delay" | "traffic-condition" | "obstruction")).collect();
    b.set("traffic.count", &traffic.len().to_string());
    for (i, e) in traffic.iter().take(6).enumerate() {
        b.set(&format!("traffic{}.text", i + 1), &format!("{}{}", if e.route.is_empty() { String::new() } else { format!("{}: ", e.route) }, if e.headline.is_empty() { e.label.clone() } else { e.headline.clone() }));
    }
}

/// Winter timing from the ingredient fields at home.
fn winter_bindings(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    use crate::map::scene::DataField;
    let Some(grid) = home_grid(data, settings) else { return };
    let home = settings.home;
    let h24 = settings.clock_24h;
    // Squall arrival: first hour the parameter reaches 1 within ~20 miles of home.
    let d = 20.0 / 69.0;
    let mut squall = None;
    'hours: for hour in 0..24 {
        let tf = grid.time_index(hour as f32);
        for (dy, dx) in [(0.0, 0.0), (d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)] {
            if grid.value(DataField::SnowSquall, tf, home.lat + dy, home.lon + dx).map(|v| v >= 1.0).unwrap_or(false) {
                squall = DateTime::from_timestamp(grid.time_at(tf), 0).map(|t| t.with_timezone(&Local));
                break 'hours;
            }
        }
    }
    b.set("winter.squall_time", &match squall {
        Some(t) => format!("Snow squalls possible around {}", hour_label(t, h24)),
        None => "No snow squalls expected".into(),
    });
    let now = grid.time_index(0.0);
    let road = grid.value(DataField::RoadHazard, now, home.lat, home.lon).unwrap_or(0.0);
    let ground = grid.value(DataField::GroundTemp, now, home.lat, home.lon);
    // Warm ground keeps roads wet even under freezing air.
    let road = match ground { Some(g) if g > 36.0 => (road - 1.0).max(0.0), _ => road };
    b.set("winter.road_icing", match road.round() as i32 { 3 => "Snow-covered roads likely", 2 => "Icy roads likely", 1 => "Slick spots possible", _ => "Roads mainly wet or dry" });
    if let Some(r) = grid.value(DataField::SnowRatio, now, home.lat, home.lon) {
        b.set("winter.ratio", &format!("{r:.0}:1"));
    }
}

/// How closely GFS, ECMWF and ICON agree on the daily highs.
fn model_agreement(b: &mut Bindings) {
    let get = |k: &str| b.series(k).map(|s| s.iter().map(|p| p.1).collect::<Vec<f32>>()).unwrap_or_default();
    let (g, e, i) = (get("models.gfs"), get("models.ecmwf"), get("models.icon"));
    let days = g.len().min(e.len()).min(i.len());
    if days == 0 {
        return;
    }
    let spreads: Vec<f32> = (0..days).map(|d| [g[d], e[d], i[d]].iter().fold(f32::MIN, |a, x| a.max(*x)) - [g[d], e[d], i[d]].iter().fold(f32::MAX, |a, x| a.min(*x))).collect();
    let avg = spreads.iter().sum::<f32>() / days as f32;
    b.set("models.agreement", &if avg <= 3.0 { format!("Models agree closely (within {avg:.0}°)") } else if avg <= 7.0 { format!("Models mostly agree (within {avg:.0}°)") } else { format!("Models disagree by about {avg:.0}°") });
    b.set_series("models.spread", spreads.iter().enumerate().map(|(d, s)| (format!("Day {}", d + 1), *s)).collect());
}

/// Tropical, fire, air quality, drought and climate history.
fn hazard_bindings(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    use crate::map::scene::DataField;
    let home = settings.home;
    let h = &data.hazards;
    let t = &h.tropical;
    if let Some(p) = t.points.first() {
        b.set("tropical.name", &t.storm);
        b.set("tropical.category", &p.category);
        b.set("tropical.wind", &format!("{:.0} mph", p.wind_mph));
        b.set("tropical.next", &t.points.get(1).map(|n| format!("{} by {}", n.category, n.label)).unwrap_or_default());
    } else {
        b.set("tropical.name", "No active storms");
    }
    let inside = |s: &crate::hazards::Shape| s.rings.iter().any(|r| crate::data::point_in_ring(r, home.lon, home.lat));
    b.set("tropical.arrival", &t.arrival.iter().find(|s| inside(s)).map(|s| format!("Tropical-storm winds could arrive {}", s.label)).unwrap_or_else(|| "Tropical-storm winds not expected".into()));
    // Drought: even-odd over the rings, so a town in a non-drought pocket reads "No drought".
    b.set("drought.home", crate::hazards::drought_label(crate::hazards::drought_at(&h.drought, home.lat, home.lon)));
    b.set("drought.valid", &format!("Valid {}", crate::hazards::drought_valid_date(chrono::Local::now().date_naive()).format("%A, %B %-d")));
    b.set("drought.outlook_month", &format!("{} ({})", crate::hazards::drought_trend_at(&h.drought_month, home.lat, home.lon), if h.drought_month_target.is_empty() { "next month" } else { h.drought_month_target.as_str() }));
    b.set("drought.outlook_season", &format!("{} ({})", crate::hazards::drought_trend_at(&h.drought_season, home.lat, home.lon), if h.drought_season_target.is_empty() { "this season" } else { h.drought_season_target.as_str() }));
    let nearest = h.wildfires.iter().filter_map(|s| s.rings.first().and_then(|r| r.first()).map(|(lo, la)| (s, crate::map::geo::haversine_km(home.lat, home.lon, *la, *lo) / 1.609))).min_by(|a, b| a.1.total_cmp(&b.1));
    b.set("fire.nearest", &nearest.map(|(s, mi)| format!("{} ({mi:.0} mi away)", s.label)).unwrap_or_else(|| "No active wildfires nearby".into()));
    let redflag = data.local_alerts.iter().chain(data.map_alerts.iter()).find(|a| a.event.eq_ignore_ascii_case("Red Flag Warning") && a.contains(home.lat, home.lon));
    b.set("redflag.text", &redflag.map(|a| format!("Red Flag Warning {}", a.expires_label(false).unwrap_or_default())).unwrap_or_else(|| "No Red Flag Warning".into()));
    if let Some(grid) = home_grid(data, settings) {
        let tf = grid.time_index(0.0);
        if let Some(f) = grid.value(DataField::FireDanger, tf, home.lat, home.lon) {
            b.set("fire.danger", match f { f if f >= 75.0 => "Extreme fire danger", f if f >= 50.0 => "High fire danger", f if f >= 30.0 => "Moderate fire danger", _ => "Low fire danger" });
        }
        if let Some(a) = grid.value(DataField::AirQuality, tf, home.lat, home.lon) {
            let (cat, advice) = match a {
                a if a <= 50.0 => ("Good", "Enjoy the outdoors"),
                a if a <= 100.0 => ("Moderate", "Unusually sensitive people: take it easier outside"),
                a if a <= 150.0 => ("Unhealthy for sensitive groups", "Children, older adults and people with asthma: limit time outside"),
                a if a <= 200.0 => ("Unhealthy", "Everyone: limit time outside, keep windows closed"),
                a if a <= 300.0 => ("Very unhealthy", "Stay indoors, run filters"),
                _ => ("Hazardous", "Stay indoors"),
            };
            b.set("aqi.home", &format!("{a:.0}"));
            b.set("aqi.category", cat);
            b.set("aqi.advice", advice);
        }
    }
    let c = &h.history;
    // Credit line for the climate cards: the NOAA station(s) the numbers come from.
    let station = |name: &str| crate::climate::station_label(name);
    let clim_source = match (c.station.as_str(), c.snow_station.as_str()) {
        ("", _) => "Climate data: NOAA climate stations (ACIS)".to_string(),
        (t, s) if s.is_empty() || s == t => format!("Climate data: NOAA, {} station (ACIS)", station(t)),
        (t, s) => format!("Climate data: NOAA, {} · snowfall {} (ACIS)", station(t), station(s)),
    };
    b.set("clim.source", &clim_source);
    if let Some((hi, lo, pr, sn)) = c.yesterday {
        b.set("clim.yesterday", &format!("High {hi:.0}°, low {lo:.0}°, rain {pr:.2}\"{}", if sn > 0.05 { format!(", snow {sn:.1}\"") } else { String::new() }));
    }
    if let Some((ah, al, nh, nl, p, np)) = c.month {
        let dep = (ah + al) / 2.0 - (nh + nl) / 2.0;
        b.set("clim.month_temp", &format!("{:+.1}° {} normal so far this month", dep, if dep >= 0.0 { "above" } else { "below" }));
        b.set("clim.month_precip", &format!("{p:.2}\" so far, normal {np:.2}\""));
    }
    if let Some((_, acc, nacc)) = c.ytd.last() {
        b.set("clim.ytd_precip", &format!("{acc:.2}\" this year, {:+.2}\" vs normal", acc - nacc));
        b.set_series("clim.ytd", c.ytd.iter().step_by(7).map(|(d, a, _)| (format!("{d}"), *a)).collect());
        b.set_series("clim.ytd_normal", c.ytd.iter().step_by(7).map(|(d, _, n)| (format!("{d}"), *n)).collect());
    }
    if let Some((_, now)) = c.snow_seasons.first() {
        let past: Vec<f32> = c.snow_seasons.iter().skip(1).map(|s| s.1).collect();
        let avg = past.iter().sum::<f32>() / past.len().max(1) as f32;
        let last = past.first().copied().unwrap_or(0.0);
        b.set("clim.season_snow", &format!("{now:.1}\" so far; last season {last:.1}\" by now; 10-year average {avg:.1}\""));
        b.set_series("clim.snow_seasons", c.snow_seasons.iter().rev().map(|(y, s)| (format!("{}-{:02}", y, (y + 1) % 100), *s)).collect());
    }
    if let Some((y, hi, lo, pr)) = c.this_day.first() {
        b.set("clim.this_day", &format!("On this date in {y}: high {hi:.0}°, low {lo:.0}°, {pr:.2}\" of rain"));
    }
}

/// The loaded historical event: name, counts, timeline, forecast vs observed.
fn history_bindings(b: &mut Bindings, data: &WxSnapshot) {
    let Some(ev) = data.history_event.as_ref() else {
        b.set("hist.name", "Event Timeline");
        b.set("hist.date", "No event loaded");
        b.set("hist.counts", "open one from Event history");
        return;
    };
    b.set("hist.name", if ev.case.name.trim().is_empty() { "Event Timeline" } else { &ev.case.name });
    b.set("hist.date", &DateTime::from_timestamp(ev.case.start, 0).map(|d| d.with_timezone(&Local).format("%B %-d, %Y").to_string()).unwrap_or_default());
    b.set("hist.counts", &format!("{} warnings, {} storm reports", ev.warnings.len(), ev.reports.len()));
    for (i, (t, text)) in ev.timeline.iter().take(8).enumerate() {
        let when = DateTime::from_timestamp(*t, 0).map(|d| d.with_timezone(&Local).format("%-I:%M %p").to_string()).unwrap_or_default();
        b.set(&format!("hist.t{}", i + 1), &format!("{when}  {text}"));
    }
    let label = |t: i64| DateTime::from_timestamp(t, 0).map(|d| d.with_timezone(&Local).format("%-I %p").to_string()).unwrap_or_default();
    b.set_series("hist.obs_temp", ev.obs.iter().map(|o| (label(o.0), o.1)).collect());
    // The MOS forecast is 3-hourly: line it up with the hourly observations (linear between
    // forecast times) so both chart lines share the same hours.
    let fcst_at = |t: i64| -> f32 {
        let f = &ev.forecast;
        match f.iter().position(|p| p.0 >= t) {
            Some(0) => if f[0].0 == t { f[0].1 } else { f32::NAN },
            Some(i) => {
                let (a, b) = (f[i - 1], f[i]);
                a.1 + (b.1 - a.1) * (t - a.0) as f32 / (b.0 - a.0).max(1) as f32
            }
            None => f32::NAN,
        }
    };
    let fcst: Vec<(String, f32)> = if ev.obs.is_empty() {
        ev.forecast.iter().map(|o| (label(o.0), o.1)).collect()
    } else {
        ev.obs.iter().map(|o| (label(o.0), fcst_at(o.0))).collect()
    };
    b.set_series("hist.fcst_temp", fcst);
    let fell: f32 = ev.obs.iter().map(|o| o.2).filter(|x| x.is_finite()).sum();
    let forecast_rain: Vec<f32> = ev.forecast.iter().map(|o| o.2).filter(|x| x.is_finite()).collect();
    let at = if ev.obs_station.is_empty() { String::new() } else { format!(" at {}", ev.obs_station) };
    b.set("hist.rain_vs_forecast", &if forecast_rain.is_empty() {
        // MOS has no rain amounts: say what fell, without inventing a forecast amount.
        format!("Rain: {fell:.2}\" fell{at}")
    } else {
        format!("Rain: {fell:.2}\" fell{at}, {:.2}\" forecast", forecast_rain.iter().sum::<f32>())
    });
}

/// Approved viewer photos, the question on air, and the live poll.
fn viewer_bindings(b: &mut Bindings, data: &WxSnapshot) {
    let photos: Vec<&crate::portal::Submission> = data.viewer.iter().filter(|s| s.status == "approved" && s.kind == "photo" && !s.media.is_empty()).collect();
    for (i, s) in photos.iter().take(4).enumerate() {
        let k = i + 1;
        b.set(&format!("viewer{k}.photo"), &s.media);
        b.set(&format!("viewer{k}.credit"), &format!("Photo: {}{}", if s.name.is_empty() { "a viewer" } else { &s.name }, if s.place.is_empty() { String::new() } else { format!(", {}", s.place) }));
        b.set(&format!("viewer{k}.caption"), &s.text);
    }
    b.set("question.text", "Have a weather question? Send it to us at our website.");
    b.set("question.from", "Viewer questions");
    if let Some(q) = data.viewer.iter().find(|s| s.kind == "question" && s.status == "onair") {
        b.set("question.text", &q.text);
        b.set("question.from", &format!("{}{}", if q.name.is_empty() { "A viewer" } else { &q.name }, if q.place.is_empty() { String::new() } else { format!(" in {}", q.place) }));
    }
    let p = crate::portal::poll();
    // Plain titles while nothing is set, so the cards never read "--".
    b.set("poll.question", "Viewer Poll");
    b.set("poll.total", "No poll running yet");
    if !p.question.is_empty() {
        let total = p.votes.iter().sum::<u32>().max(1) as f32;
        b.set("poll.question", &p.question);
        b.set("poll.total", &format!("{} votes", p.votes.iter().sum::<u32>()));
        b.set_series("poll.results", p.options.iter().zip(&p.votes).map(|(o, v)| (o.clone(), *v as f32 / total * 100.0)).collect());
    }
}

/// Flood impact timeline at home and storm flood reports against rainfall.
fn flood_bindings(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    use crate::map::scene::DataField;
    let home = settings.home;
    let h24 = settings.clock_24h;
    let mut events: Vec<(i64, String)> = Vec::new();
    for a in data.local_alerts.iter().chain(data.map_alerts.iter()).filter(|a| a.event.to_lowercase().contains("flood") && a.contains(home.lat, home.lon)) {
        if let Some(t) = a.sent.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) {
            events.push((t.timestamp(), format!("{} issued", a.event)));
        }
        if let Some(t) = a.expires.as_deref().and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok()) {
            events.push((t.timestamp(), format!("{} ends", a.event)));
        }
    }
    if let Some(grid) = home_grid(data, settings) {
        let mut heavy = None;
        for h in 0..48 {
            let tf = grid.time_index(h as f32);
            if grid.value(DataField::Futurecast, tf, home.lat, home.lon).unwrap_or(0.0) >= 0.25 && heavy.is_none() {
                heavy = Some(grid.time_at(tf));
            }
        }
        if let Some(t) = heavy {
            events.push((t, "Heaviest rain begins".into()));
        }
    }
    for g in data.rivers.iter().filter(|g| matches!(g.forecast_status.as_str(), "minor" | "moderate" | "major")).take(3) {
        events.push((chrono::Utc::now().timestamp() + 86_400, format!("{} forecast to reach {} flood", g.location, g.forecast_status)));
    }
    events.sort_by_key(|e| e.0);
    for (i, (t, text)) in events.iter().take(6).enumerate() {
        let when = DateTime::from_timestamp(*t, 0).map(|d| hour_label(d.with_timezone(&Local), h24)).unwrap_or_default();
        b.set(&format!("flood.t{}", i + 1), &format!("{when}  {text}"));
    }
    // Flood reports with the model rain total there (last 24 h).
    if let Some(grid) = home_grid(data, settings) {
        let since = chrono::Utc::now().timestamp() - 86_400;
        let now = grid.time_index(0.0);
        let mut rows: Vec<(String, f32)> = data
            .reports
            .iter()
            .filter(|r| r.time >= since && matches!(r.kind.as_str(), "F" | "E" | "R"))
            .map(|r| {
                let rain = grid.sample(crate::grid::Var::RainAcc, now, r.lat, r.lon).unwrap_or(0.0);
                (format!("{}: {}", r.city, r.text), rain)
            })
            .collect();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (i, (place, rain)) in rows.iter().take(5).enumerate() {
            b.set(&format!("floodrep{}", i + 1), &format!("{place} • model rain {rain:.2}\""));
        }
        b.set("floodrep.count", &format!("{} flood reports today", rows.len()));
    }
}

/// The Map Studio model grid, only when it covers home (it follows the map's view, so after
/// panning elsewhere it would describe the wrong place).
fn home_grid<'a>(data: &'a WxSnapshot, settings: &Settings) -> Option<&'a WxGrid> {
    data.grid.as_deref().filter(|g| g.sample(Var::Temp, g.time_index(0.0), settings.home.lat, settings.home.lon).is_some())
}

/// Hour-by-hour, day parts, workweek, 10-day and outdoor ratings from the NWS (hourly feed and
/// 7-day) so these graphics have data even when no forecast map is open. The model grid and
/// the 16-day feed overwrite these when they cover home.
fn home_fallbacks(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    let units = settings.units;
    let h24 = settings.clock_24h;
    let now = Local::now();
    let upcoming: Vec<&crate::homecast::Hour> = data.home_hourly.iter().filter(|h| h.t + 3600 > now.timestamp()).collect();
    let local = |t: i64| DateTime::from_timestamp(t, 0).map(|t| t.with_timezone(&Local)).unwrap_or(now);
    for (i, h) in upcoming.iter().take(12).enumerate() {
        let t = local(h.t);
        b.set(&format!("hour{i}.time"), &if i == 0 { "Now".to_string() } else { hour_label(t, h24) });
        b.set(&format!("hour{i}.temp"), &deg(h.temp, units));
        b.set(&format!("hour{i}.cond"), &h.short);
        b.set(&format!("hour{i}.daypart"), if h.is_day { "day" } else { "night" });
        b.set(&format!("hour{i}.wind"), &mph(h.wind_mph, units));
        b.set(&format!("hour{i}.rain"), &format!("{:.0}%", h.pop));
    }
    if !upcoming.is_empty() {
        let conv_t = |v: f32| if units == Units::Metric { (v - 32.0) * 5.0 / 9.0 } else { v };
        let conv_s = |v: f32| if units == Units::Metric { v * 1.609_344 } else { v };
        let take: Vec<&&crate::homecast::Hour> = upcoming.iter().take(25).collect();
        b.set_series("hourly.temp", take.iter().map(|h| (hour_label(local(h.t), h24), conv_t(h.temp))).collect());
        b.set_series("hourly.wind", take.iter().map(|h| (hour_label(local(h.t), h24), conv_s(h.wind_mph))).collect());
        b.set_series("hourly.pop", take.iter().map(|h| (hour_label(local(h.t), h24), h.pop)).collect());
        let next24: Vec<f32> = take.iter().map(|h| h.temp).collect();
        if let (Some(hi), Some(lo)) = (next24.iter().copied().reduce(f32::max), next24.iter().copied().reduce(f32::min)) {
            b.set("temp.hi24", &deg(hi, units));
            b.set("temp.lo24", &deg(lo, units));
        }
        let storms: Vec<i64> = take.iter().filter(|h| h.short.to_lowercase().contains("thunder") && h.pop >= 20.0).map(|h| h.t).collect();
        let window = match (storms.first(), storms.last()) {
            (Some(a), Some(z)) => {
                let (a, z) = (local(*a), local(*z + 3600));
                let day = |t: DateTime<Local>| if t.date_naive() == now.date_naive() { String::new() } else { format!("{} ", t.format("%a")) };
                format!("Storms most likely {}{} to {}{}", day(a), hour_label(a, h24), day(z), hour_label(z, h24))
            }
            _ => "No thunderstorms expected in the next 24 hours".to_string(),
        };
        b.set("severe.window", &window);
    }
    // Morning / afternoon / evening: the next 8 AM, 2 PM and 8 PM.
    for (name, hour) in [("morning", 8u32), ("afternoon", 14), ("evening", 20)] {
        let pick = data.home_hourly.iter().find(|h| {
            let t = local(h.t);
            t.hour() == hour && h.t + 3600 > now.timestamp()
        });
        if let Some(h) = pick {
            b.set(&format!("part.{name}.temp"), &deg(h.temp, units));
            b.set(&format!("part.{name}.cond"), &h.short);
        }
    }
    // Daily values from the NWS 7-day: 10-day slots 0-6, workweek and outdoor ratings.
    if let Some(ov) = &data.overview {
        let today = now.date_naive();
        let mut daily: Vec<(chrono::NaiveDate, crate::data::DaySummary)> = Vec::new();
        for (i, d) in ov.days().into_iter().enumerate() {
            let date = today + chrono::Duration::days(i as i64);
            daily.push((date, d));
        }
        for (i, (date, d)) in daily.iter().enumerate().take(10) {
            let hi = d.high.map(|v| deg(v as f32, units)).unwrap_or_else(|| "--".into());
            let lo = d.low.map(|v| deg(v as f32, units)).unwrap_or_else(|| "--".into());
            let pop = d.precip.unwrap_or(0.0) as f32;
            b.set(&format!("ten{i}.name"), &if i == 0 { "Today".to_string() } else { date.format("%a").to_string() });
            b.set(&format!("ten{i}.date"), &date.format("%-m/%-d").to_string());
            b.set(&format!("ten{i}.hi"), &hi);
            b.set(&format!("ten{i}.lo"), &lo);
            b.set(&format!("ten{i}.pop"), &format!("{pop:.0}%"));
            b.set(&format!("ten{i}.cond"), &d.short_forecast);
            let high = d.high.unwrap_or(65.0) as f32;
            let thunder = d.short_forecast.to_lowercase().contains("thunder");
            let rating = if pop >= 60.0 || high >= 95.0 || high <= 25.0 || (thunder && pop >= 40.0) {
                "Poor"
            } else if pop >= 30.0 || high >= 88.0 || high <= 40.0 {
                "Fair"
            } else {
                "Good"
            };
            b.set(&format!("outdoor{i}"), rating);
        }
        for (i, (date, d)) in daily.iter().filter(|(date, _)| date.weekday().number_from_monday() <= 5).take(5).enumerate() {
            b.set(&format!("week{i}.name"), &date.format("%a").to_string());
            b.set(&format!("week{i}.hi"), &d.high.map(|v| deg(v as f32, units)).unwrap_or_else(|| "--".into()));
            b.set(&format!("week{i}.lo"), &d.low.map(|v| deg(v as f32, units)).unwrap_or_else(|| "--".into()));
            b.set(&format!("week{i}.pop"), &format!("{:.0}%", d.precip.unwrap_or(0.0)));
            b.set(&format!("week{i}.cond"), &d.short_forecast);
        }
    }
}

/// Weather Alert Day card wording (`alertday1.*`, `alertday2.*`), from the SPC risk at home
/// in calm, plain words: when, how widespread, and the two main hazards.
fn alert_day_bindings(b: &mut Bindings) {
    for day in 1..=2u8 {
        let p = format!("spc.day{day}.home");
        let num = |b: &Bindings, k: &str| b.get(&format!("{p}.{k}")).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
        let level = b.get(&format!("{p}.level")).and_then(|v| v.parse::<u8>().ok()).unwrap_or(0);
        let when = Local::now().date_naive() + chrono::Duration::days(i64::from(day) - 1);
        b.set(&format!("alertday{day}.when"), &when.format("%A").to_string().to_uppercase());
        b.set(&format!("alertday{day}.date"), &when.format("%A, %B %-d").to_string());
        let coverage = match level {
            0 => "Storms possible",
            1 => "Isolated strong storms possible",
            2 => "Scattered strong storms possible",
            3 => "Numerous strong storms possible",
            _ => "Widespread severe storms likely",
        };
        b.set(&format!("alertday{day}.line1"), coverage);
        let tornado = num(b, "tornado.level");
        let mut hazards: Vec<(u8, &str)> = vec![
            (num(b, "wind.level"), "Damaging wind possible"),
            (num(b, "hail.level"), "Large hail possible"),
            (tornado, if tornado >= 3 { "A few tornadoes possible" } else { "A brief tornado possible" }),
            (num(b, "flood.level").saturating_sub(1), "Flooding rain possible"),
        ];
        hazards.retain(|(l, _)| *l > 0);
        hazards.sort_by(|a, b| b.0.cmp(&a.0));
        for i in 0..2 {
            b.set(&format!("alertday{day}.line{}", i + 2), hazards.get(i).map(|h| h.1).unwrap_or(""));
        }
    }
}

pub fn extend(b: &mut Bindings, data: &WxSnapshot, settings: &Settings) {
    home_fallbacks(b, data, settings);
    crate::gfx::holidays::bind(b, data, settings);
    crate::gfx::riskmap::remember(data, settings);
    // Catalog sets (gfx::sets) add their own keys.
    crate::gfx::sets::bind(b, data, settings);
    crate::gfx::history_set::bind(b, data, settings);
    flood_bindings(b, data, settings);
    viewer_bindings(b, data);
    history_bindings(b, data);
    hazard_bindings(b, data, settings);
    crate::sounding::bind(b);
    crate::ensemble::bind(b);
    crate::verify::bind(b);
    crate::outages::bind(b, &data.outages);
    crate::nowcast::bind(b, &data.nowcast);
    winter_bindings(b, data, settings);
    spc_home_bindings(b, data, settings);
    alert_day_bindings(b);
    {
        // Storm timing map legend: the three one-hour bands from now, as clock times.
        let now = Local::now();
        let fmt = |t: chrono::DateTime<Local>| if settings.clock_24h { t.format("%H:%M").to_string() } else { t.format("%-I:%M %p").to_string() };
        for i in 0..3i64 {
            let from = now + chrono::Duration::hours(i);
            let to = now + chrono::Duration::hours(i + 1);
            let text = if i == 0 { format!("Now to {}", fmt(to)) } else { format!("{} to {}", fmt(from), fmt(to)) };
            b.set(&format!("timing.band{}", i + 1), &text);
        }
    }
    threat_summaries(b, data, settings);
    travel_bindings(b, data, settings);
    model_agreement(b);
    let units = settings.units;
    let h24 = settings.clock_24h;
    let home = settings.home;
    let now = Local::now();

    // ---- Model grid at home -------------------------------------------------------------
    if let Some(grid) = home_grid(data, settings) {
        let at = |var: Var, offset: f32| grid.sample(var, grid.time_index(offset), home.lat, home.lon);
        for i in 0..12 {
            let tf = grid.time_index(i as f32);
            let t = DateTime::from_timestamp(grid.time_at(tf), 0).map(|t| t.with_timezone(&Local)).unwrap_or(now);
            let night = is_night(t, home.lat, home.lon);
            b.set(&format!("hour{i}.time"), &if i == 0 { "Now".to_string() } else { hour_label(t, h24) });
            if let Some(v) = grid.sample(Var::Temp, tf, home.lat, home.lon) {
                b.set(&format!("hour{i}.temp"), &deg(v, units));
            }
            b.set(&format!("hour{i}.cond"), &condition(grid, tf, home.lat, home.lon, night));
            b.set(&format!("hour{i}.daypart"), if night { "night" } else { "day" });
            if let Some(v) = grid.sample(Var::Wind, tf, home.lat, home.lon) {
                b.set(&format!("hour{i}.wind"), &mph(v, units));
            }
            if let Some(v) = grid.sample(Var::Gust, tf, home.lat, home.lon) {
                b.set(&format!("hour{i}.gust"), &mph(v, units));
            }
            if let Some(v) = grid.sample(Var::Precip, tf, home.lat, home.lon) {
                b.set(&format!("hour{i}.rain"), &inches(v, units));
            }
        }

        for (name, hour) in [("morning", 8u32), ("afternoon", 14), ("evening", 20)] {
            if let Some(tf) = tf_for_hour(grid, hour) {
                if let Some(v) = grid.sample(Var::Temp, tf, home.lat, home.lon) {
                    b.set(&format!("part.{name}.temp"), &deg(v, units));
                }
                b.set(&format!("part.{name}.cond"), &condition(grid, tf, home.lat, home.lon, name == "evening"));
            }
        }

        let next24: Vec<f32> = (0..=24).filter_map(|h| at(Var::Temp, h as f32)).collect();
        if let (Some(hi), Some(lo)) = (
            next24.iter().copied().reduce(f32::max),
            next24.iter().copied().reduce(f32::min),
        ) {
            b.set("temp.hi24", &deg(hi, units));
            b.set("temp.lo24", &deg(lo, units));
        }
        if let (Some(a), Some(z)) = (at(Var::Temp, 0.0), at(Var::Temp, 24.0)) {
            let d = z - a;
            let d = if units == Units::Metric { d * 5.0 / 9.0 } else { d };
            b.set("temp.change24", &if d.abs() < 1.0 { "About the same".to_string() } else { format!("{:+.0}°", d) });
        }
        if let Some(g) = (0..=24).filter_map(|h| at(Var::Gust, h as f32)).reduce(f32::max) {
            b.set("gust.max24", &mph(g, units));
        }
        if let (Some(p0), Some(p3)) = (at(Var::Pressure, 0.0), at(Var::Pressure, 3.0)) {
            let d = p3 - p0;
            b.set("pressure.trend", if d > 1.0 { "Rising" } else if d < -1.0 { "Falling" } else { "Steady" });
        }

        // Totals over 48 h: rain / snow from the accumulations; ice from hours typed as mix.
        let start = grid.time_index(0.0);
        let end = grid.time_index(47.0);
        let total = |var: Var| -> f32 {
            match (grid.sample(var, end, home.lat, home.lon), grid.sample(var, start, home.lat, home.lon)) {
                (Some(a), Some(z)) => (a - z).max(0.0),
                _ => 0.0,
            }
        };
        b.set("rain.total48", &inches(total(Var::RainAcc), units));
        let snow = total(Var::SnowAcc);
        b.set("snow.total48", &match units {
            Units::Imperial => format!("{snow:.1}\""),
            Units::Metric => format!("{:.0} cm", snow * 2.54),
        });
        let mut ice = 0.0f32;
        let mut wintry: Vec<i64> = Vec::new();
        let mut storms: Vec<i64> = Vec::new();
        for h in 0..48 {
            let tf = grid.time_index(h as f32);
            if let Some((rate, kind)) = grid.precip(tf, home.lat, home.lon) {
                if rate >= 0.01 {
                    if kind == PrecipKind::Mix {
                        ice += rate;
                    }
                    if kind != PrecipKind::Rain {
                        wintry.push(grid.time_at(tf));
                    }
                    let code = grid.sample(Var::Code, tf, home.lat, home.lon).unwrap_or(0.0).round() as i32;
                    if h < 24 && (95..=99).contains(&code) {
                        storms.push(grid.time_at(tf));
                    }
                }
            }
        }
        b.set("ice.total48", &inches(ice, units));
        let span = |times: &[i64]| -> Option<String> {
            let (a, z) = (times.first()?, times.last()?);
            let a = DateTime::from_timestamp(*a, 0)?.with_timezone(&Local);
            let z = DateTime::from_timestamp(*z + 3600, 0)?.with_timezone(&Local);
            let day = |t: DateTime<Local>| if t.date_naive() == now.date_naive() { String::new() } else { format!("{} ", t.format("%a")) };
            Some(format!("{}{} to {}{}", day(a), hour_label(a, h24), day(z), hour_label(z, h24)))
        };
        b.set("severe.window", &span(&storms).map(|s| format!("Storms most likely {s}")).unwrap_or_else(|| "No thunderstorms expected in the next 24 hours".into()));
        b.set("winter.timeline", &span(&wintry).map(|s| format!("Wintry weather {s}")).unwrap_or_else(|| "No wintry weather in the next 48 hours".into()));

        // Nearby cities: largest towns within ~70 miles, model "now" values.
        let r = 1.0;
        let mut cities = crate::places::in_box(home.lat - r, home.lat + r, home.lon - r * 1.3, home.lon + r * 1.3);
        cities.truncate(6);
        let tf = grid.time_index(0.0);
        let night = is_night(now, home.lat, home.lon);
        for (i, c) in cities.iter().enumerate() {
            b.set(&format!("city{i}.name"), &c.name);
            if let Some(v) = grid.sample(Var::Temp, tf, c.lat, c.lon) {
                b.set(&format!("city{i}.temp"), &deg(v, units));
            }
            b.set(&format!("city{i}.cond"), &condition(grid, tf, c.lat, c.lon, night));
            let end = grid.time_index(47.0);
            if let (Some(a), Some(z)) = (grid.sample(Var::RainAcc, end, c.lat, c.lon), grid.sample(Var::RainAcc, tf, c.lat, c.lon)) {
                b.set(&format!("city{i}.rain48"), &inches((a - z).max(0.0), units));
            }
        }

        // Commutes: 7 AM and 5 PM.
        for (key, hour) in [("commute.am", 7u32), ("commute.pm", 17)] {
            if let Some(tf) = tf_for_hour(grid, hour) {
                let t = grid.sample(Var::Temp, tf, home.lat, home.lon).unwrap_or(50.0);
                let gust = grid.sample(Var::Gust, tf, home.lat, home.lon).unwrap_or(0.0);
                let cond = condition(grid, tf, home.lat, home.lon, hour < 8);
                let roads = match grid.precip(tf, home.lat, home.lon) {
                    Some((r, PrecipKind::Snow)) if r >= 0.01 => "snow-covered roads possible",
                    Some((r, PrecipKind::Mix)) if r >= 0.01 => "icy spots likely",
                    Some((r, _)) if r >= 0.01 && t <= 34.0 => "watch for slick spots",
                    Some((r, _)) if r >= 0.01 => "wet roads",
                    _ if t <= 32.0 => "dry, but watch for frost on bridges",
                    _ => "dry roads",
                };
                let wind = if gust >= 35.0 { format!(", gusts to {}", mph(gust, units)) } else { String::new() };
                b.set(key, &format!("{cond}, {}, {roads}{wind}", deg(t, units)));
            }
        }
    }

    // ---- Chart series -------------------------------------------------------------------
    if let Some(grid) = home_grid(data, settings) {
        let conv_t = |v: f32| if units == Units::Metric { (v - 32.0) * 5.0 / 9.0 } else { v };
        let conv_s = |v: f32| if units == Units::Metric { v * 1.609_344 } else { v };
        let mut hourly = |key: &str, var: Var, f: &dyn Fn(f32) -> f32| {
            let pts: Vec<(String, f32)> = (0..=24)
                .filter_map(|i| {
                    let tf = grid.time_index(i as f32);
                    let t = DateTime::from_timestamp(grid.time_at(tf), 0)?.with_timezone(&Local);
                    Some((hour_label(t, h24), f(grid.sample(var, tf, home.lat, home.lon)?)))
                })
                .collect();
            b.set_series(key, pts);
        };
        hourly("hourly.temp", Var::Temp, &conv_t);
        hourly("hourly.feels", Var::Feels, &conv_t);
        hourly("hourly.wind", Var::Wind, &conv_s);
        hourly("hourly.gust", Var::Gust, &conv_s);
        hourly("hourly.rain", Var::Precip, &|v| if units == Units::Metric { v * 25.4 } else { v });
        hourly("hourly.cloud", Var::Cloud, &|v| v);
        hourly("hourly.pressure", Var::Pressure, &|v| v);
    }
    if let Some(ov) = &data.overview {
        let days = ov.days();
        let pick = |f: &dyn Fn(&crate::data::DaySummary) -> Option<f64>| -> Vec<(String, f32)> {
            days.iter().take(7).filter_map(|d| Some((d.short_name.clone(), f(d)? as f32))).collect()
        };
        let t = |v: f64| if units == Units::Metric { (v - 32.0) * 5.0 / 9.0 } else { v };
        b.set_series("day.hi", pick(&|d| d.high.map(t)));
        b.set_series("day.lo", pick(&|d| d.low.map(t)));
        b.set_series("day.precip", pick(&|d| Some(d.precip.unwrap_or(0.0))));
    }
    {
        let today = now.date_naive();
        let next30: Vec<(String, f32)> = (0..30)
            .filter_map(|i| {
                let d = today.checked_add_days(chrono::Days::new(i))?;
                let m = crate::astro::daylight_minutes(d, home.lat, home.lon)?;
                Some((d.format("%b %-d").to_string(), m as f32 / 60.0))
            })
            .collect();
        b.set_series("daylight.next30", next30);
        let year: Vec<(String, f32)> = (1..=12)
            .filter_map(|m| {
                let d = chrono::NaiveDate::from_ymd_opt(today.year(), m, 15)?;
                let mins = crate::astro::daylight_minutes(d, home.lat, home.lon)?;
                Some((d.format("%b").to_string(), mins as f32 / 60.0))
            })
            .collect();
        b.set_series("daylight.year", year);
    }
    {
        let rate = crate::map::extras::flash_rate(&data.lightning, home.lat, home.lon, 25.0);
        let pts = rate.iter().enumerate().map(|(i, n)| (format!("-{}m", (11 - i) * 5), *n as f32)).collect();
        b.set_series("lightning.rate", pts);
        let (kinds, _) = crate::data::report_counts(&data.reports, 24, "");
        b.set_series("reports.kinds", kinds.iter().map(|(k, n)| (k.to_string(), *n as f32)).collect());
    }
    if let Some(grid) = home_grid(data, settings) {
        let r = 1.0;
        let mut cities = crate::places::in_box(home.lat - r, home.lat + r, home.lon - r * 1.3, home.lon + r * 1.3);
        cities.truncate(6);
        let tf = grid.time_index(0.0);
        let pts = cities
            .iter()
            .filter_map(|c| {
                let v = grid.sample(Var::Temp, tf, c.lat, c.lon)?;
                Some((c.name.clone(), if units == Units::Metric { (v - 32.0) * 5.0 / 9.0 } else { v }))
            })
            .collect();
        b.set_series("cities.temp", pts);
    }
    if let Some(h) = &data.hydrograph {
        let label = |t: i64| DateTime::from_timestamp(t, 0).map(|t| t.with_timezone(&Local).format("%a %-I%p").to_string()).unwrap_or_default();
        let pts: Vec<(String, f32)> = h.observed.iter().chain(h.forecast.iter()).step_by(4).map(|p| (label(p.time), p.stage as f32)).collect();
        let flood = data.rivers.iter().find(|g| g.lid == h.lid).and_then(|g| g.flood);
        if let Some(fs) = flood {
            b.set_series("river.flood", pts.iter().map(|(l, _)| (l.clone(), fs as f32)).collect());
        }
        b.set_series("river.stage", pts);
    }
    if let Some((_, hist)) = data.cell_history.iter().max_by(|a, b| {
        let peak = |v: &Vec<(i64, f32)>| v.iter().map(|p| p.1).fold(0.0f32, f32::max);
        peak(a.1).total_cmp(&peak(b.1))
    }) {
        let pts = hist
            .iter()
            .map(|(t, dbz)| (DateTime::from_timestamp(*t, 0).map(|t| t.with_timezone(&Local).format("%-I:%M").to_string()).unwrap_or_default(), *dbz))
            .collect();
        b.set_series("cell.dbz", pts);
    }

    // ---- Sun, moon, seasons -------------------------------------------------------------
    let today = now.date_naive();
    if let Some((rise, set)) = crate::astro::sun_times(today, home.lat, home.lon) {
        b.set("sun.rise", &clock(rise, h24));
        b.set("sun.set", &clock(set, h24));
        let len = (set - rise).num_minutes();
        b.set("sun.daylight", &crate::astro::duration_text(len));
        if let Some(prev) = today.pred_opt().and_then(|d| crate::astro::daylight_minutes(d, home.lat, home.lon)) {
            let d = len - prev;
            b.set("sun.change", &match d {
                0 => "Same as yesterday".to_string(),
                d if d > 0 => format!("{d} min more than yesterday"),
                d => format!("{} min less than yesterday", -d),
            });
        }
    }
    for i in 0..7 {
        if let Some((rise, set)) = today
            .checked_add_days(chrono::Days::new(i))
            .and_then(|d| crate::astro::sun_times(d, home.lat, home.lon))
        {
            b.set(&format!("day{i}.sunrise"), &clock(rise, h24));
            b.set(&format!("day{i}.sunset"), &clock(set, h24));
        }
    }
    let (age, illum) = crate::astro::moon(chrono::Utc::now());
    b.set("moon.phase", crate::astro::moon_phase_name(age));
    b.set("moon.illum", &format!("{:.0}%", illum * 100.0));
    let (full, new) = crate::astro::next_full_and_new(chrono::Utc::now());
    b.set("moon.next_full", &full.format("%B %-d").to_string());
    b.set("moon.next_new", &new.format("%B %-d").to_string());
    let (season, date) = crate::astro::next_season(today);
    b.set("season.next", &format!("{season} {}", date.format("%B %-d")));
    b.set("season.days", &format!("{} days", (date - today).num_days()));

    // ---- Storm reports ------------------------------------------------------------------
    let (kinds, counties) = crate::data::report_counts(&data.reports, 24, "");
    let total: usize = kinds.iter().map(|(_, n)| n).sum();
    b.set("reports.count", &total.to_string());
    let kind_word = |k: &str| match k {
        "T" => "tornado",
        "H" => "hail",
        "W" => "wind damage",
        "F" => "flooding",
        "S" => "snow / ice",
        "L" => "lightning",
        _ => "other",
    };
    b.set(
        "reports.summary",
        &if total == 0 {
            "No storm reports in the last 24 hours".into()
        } else {
            kinds.iter().map(|(k, n)| format!("{n} {}", kind_word(k))).collect::<Vec<_>>().join("  •  ")
        },
    );
    b.set(
        "reports.counties",
        &counties.iter().take(5).map(|(c, n)| format!("{c}: {n}")).collect::<Vec<_>>().join("  •  "),
    );
    let mut newest: Vec<&crate::data::StormReport> = data.reports.iter().collect();
    newest.sort_by(|a, b| b.time.cmp(&a.time));
    for (i, r) in newest.iter().take(8).enumerate() {
        let t = DateTime::from_timestamp(r.time, 0).map(|t| clock(t.with_timezone(&Local), h24)).unwrap_or_default();
        let mag = if r.magnitude.is_empty() || r.magnitude == "0" || r.magnitude == "null" {
            String::new()
        } else if r.kind == "H" {
            format!(" {}\"", r.magnitude)
        } else if matches!(r.kind.as_str(), "G" | "D" | "M" | "N") {
            format!(" {} mph", r.magnitude)
        } else {
            String::new()
        };
        b.set(&format!("report{i}.line"), &format!("{t}  {}{mag}  {}", r.text, r.city));
    }

    // ---- Rivers -------------------------------------------------------------------------
    let flooding: Vec<&crate::feeds::RiverGauge> =
        data.rivers.iter().filter(|g| crate::feeds::RiverGauge::severity(g.worst_status()) >= 2).collect();
    b.set(
        "river.summary",
        &if flooding.is_empty() {
            "No rivers in flood near you".into()
        } else {
            flooding.iter().take(4).map(|g| format!("{} at {}", g.waterbody, g.location)).collect::<Vec<_>>().join("  •  ")
        },
    );
    for (i, g) in data.rivers.iter().take(6).enumerate() {
        b.set(&format!("river{i}.name"), &format!("{} at {}", g.waterbody, g.location));
        b.set(&format!("river{i}.stage"), &g.observed.map(|v| format!("{v:.1} ft")).unwrap_or_else(|| "--".into()));
        b.set(&format!("river{i}.flood"), &g.flood.map(|v| format!("{v:.0} ft")).unwrap_or_else(|| "--".into()));
        b.set(&format!("river{i}.status"), crate::feeds::RiverGauge::status_words(g.worst_status()));
        b.set(&format!("river{i}.forecast"), &g.forecast.map(|v| format!("{v:.1} ft")).unwrap_or_else(|| "--".into()));
    }

    // ---- WPC rain / excessive rain at home ----------------------------------------------
    for (i, key) in ["qpf.day1", "qpf.day3", "qpf.day7"].iter().enumerate() {
        let v = crate::feeds::area_at(&data.qpf[i], home.lat, home.lon).map(|a| a.label.clone()).unwrap_or_else(|| "Under .01\"".into());
        b.set(key, &v);
    }
    for d in 0..3 {
        // On-air words ("Marginal risk"), never the WPC code ("MRGL").
        let v = crate::feeds::area_at(&data.ero[d], home.lat, home.lon)
            .map(|a| match risk_words(&a.label) {
                "" => a.label.clone(),
                w => w.to_string(),
            })
            .unwrap_or_else(|| "None".into());
        b.set(&format!("ero.day{}", d + 1), &v);
    }

    // ---- Community board ----------------------------------------------------------------
    let c = &data.community;
    b.set("closings.count", &c.closings.len().to_string());
    b.set(
        "closings.ticker",
        &if c.closings.is_empty() {
            "No closings or delays reported".into()
        } else {
            c.closings.iter().map(|x| format!("{}: {}", x.name.to_uppercase(), x.status)).collect::<Vec<_>>().join("   •   ")
        },
    );
    for (i, x) in c.closings.iter().take(20).enumerate() {
        b.set(&format!("closing{i}.name"), &x.name);
        b.set(&format!("closing{i}.status"), &x.status);
    }
    for (i, x) in c.centers.iter().take(10).enumerate() {
        let kind = if x.kind == "cooling" { "Cooling center" } else { "Warming center" };
        if x.kind == "shelter" {
            continue;
        }
        b.set(&format!("center{i}.name"), &x.name);
        b.set(&format!("center{i}.kind"), kind);
        b.set(&format!("center{i}.details"), &[x.address.as_str(), x.hours.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  •  "));
    }

    for (key, _) in crate::community::CATEGORIES {
        let items: Vec<&crate::community::Closing> = c
            .closings
            .iter()
            .filter(|x| x.category == *key || (x.category.is_empty() && *key == "schools"))
            .collect();
        b.set(&format!("closings.{key}.count"), &items.len().to_string());
        b.set(
            &format!("closings.{key}.ticker"),
            &if items.is_empty() { "None reported".into() } else { items.iter().map(|x| format!("{}: {}", x.name.to_uppercase(), x.status)).collect::<Vec<_>>().join("   •   ") },
        );
        b.set(
            &format!("closings.{key}.list"),
            &if items.is_empty() { "None reported".into() } else { items.iter().map(|x| format!("{} - {}", x.name, x.status)).collect::<Vec<_>>().join("\n") },
        );
    }
    let warming_cooling: Vec<&crate::map::extras::Center> = c.centers.iter().filter(|x| x.kind != "shelter").collect();
    let shelters: Vec<&crate::map::extras::Center> = c.centers.iter().filter(|x| x.kind == "shelter").collect();
    b.set("centers.count", &warming_cooling.len().to_string());
    b.set("shelters.count", &shelters.len().to_string());
    for (i, x) in shelters.iter().take(10).enumerate() {
        b.set(&format!("shelter{i}.name"), &x.name);
        b.set(&format!("shelter{i}.details"), &[x.address.as_str(), x.hours.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  •  "));
    }

    // ---- Warnings today -----------------------------------------------------------------
    b.set("history.count", &data.warning_history.len().to_string());
    for (i, (t, event, area)) in data.warning_history.iter().rev().take(10).enumerate() {
        let when = DateTime::from_timestamp(*t, 0).map(|d| clock(d.with_timezone(&Local), h24)).unwrap_or_default();
        b.set(&format!("history{i}.line"), &format!("{when}  {event}  {area}"));
    }

    // ---- Workweek, outdoor ratings, confidence by day (16-day feed) ----------------------
    {
        let days = &data.climate.daily16;
        let workdays: Vec<&crate::climate::Day16> = days.iter().filter(|d| d.date.weekday().number_from_monday() <= 5).take(5).collect();
        for (i, d) in workdays.iter().enumerate() {
            b.set(&format!("week{i}.name"), &d.date.format("%a").to_string());
            b.set(&format!("week{i}.hi"), &deg(d.hi, units));
            b.set(&format!("week{i}.lo"), &deg(d.lo, units));
            b.set(&format!("week{i}.pop"), &format!("{:.0}%", d.pop));
            b.set(&format!("week{i}.cond"), wmo_words(d.code));
        }
        for (i, d) in days.iter().take(10).enumerate() {
            let rating = if d.pop >= 60.0 || d.hi >= 95.0 || d.hi <= 25.0 || (95..=99).contains(&d.code) {
                "Poor"
            } else if d.pop >= 30.0 || d.hi >= 88.0 || d.hi <= 40.0 {
                "Fair"
            } else {
                "Good"
            };
            b.set(&format!("outdoor{i}"), rating);
            let base = match i {
                0..=1 => "High",
                2..=3 => "Medium-high",
                4..=6 => "Medium",
                _ => "Low",
            };
            // Model disagreement lowers confidence for the days the models cover.
            let spread = {
                let his: Vec<f32> = data.climate.models.iter().filter_map(|m| m.days.iter().find(|x| x.0 == d.date).map(|x| x.1)).collect();
                if his.len() >= 2 { his.iter().copied().fold(f32::MIN, f32::max) - his.iter().copied().fold(f32::MAX, f32::min) } else { 0.0 }
            };
            b.set(&format!("confidence{i}"), if spread >= 8.0 { "Low" } else if spread >= 5.0 && base == "High" { "Medium-high" } else { base });
        }
    }

    // ---- Roads --------------------------------------------------------------------------
    let hazards: Vec<&crate::data::RoadEvent> = data.roads.iter().filter(|r| r.is_hazard()).collect();
    b.set(
        "roads.summary",
        &if hazards.is_empty() { "No road closures or incidents reported".into() } else { format!("{} closures and incidents", hazards.len()) },
    );
    for (i, r) in hazards.iter().take(6).enumerate() {
        b.set(&format!("road{i}.line"), &format!("{}: {}", r.route, r.headline));
    }

    // ---- Weekend (from the NWS 7-day) ---------------------------------------------------
    if let Some(ov) = &data.overview {
        for day in ov.days() {
            let key = match day.name.as_str() {
                n if n.starts_with("Saturday") => "weekend.sat",
                n if n.starts_with("Sunday") => "weekend.sun",
                _ => continue,
            };
            let hi = day.high.map(|h| deg(h as f32, units)).unwrap_or_else(|| "--".into());
            let lo = day.low.map(|l| deg(l as f32, units)).unwrap_or_else(|| "--".into());
            let rain = day.precip.map(|p| format!(", rain {p:.0}%")).unwrap_or_default();
            b.set(key, &format!("{}: {}, high {hi}, low {lo}{rain}", day.name, day.short_forecast));
        }
        // Spanish wording.
        for (i, day) in ov.days().iter().take(7).enumerate() {
            b.set(&format!("day{i}.name_es"), &day_es(&day.name));
            b.set(&format!("day{i}.condition_es"), &condition_es(&day.short_forecast));
        }
        if let Some(t) = &ov.current.text_description {
            b.set("condition_es", &condition_es(t));
        }
    }

    // ---- Climate, outlooks, sky, airports -----------------------------------------------
    let clim = &data.climate;
    let doy = crate::climate::DayNormals::index(now.date_naive());
    if let Some(n) = &clim.normals {
        b.set("climate.normals_source", &n.source_label());
        if !n.station.is_empty() {
            b.set("climate.normals_station", &crate::climate::station_label(&n.station));
        }
        if let (Some(h), Some(l)) = (n.hi.get(doy), n.lo.get(doy)) {
            b.set("climate.normal_hi", &deg(*h, units));
            b.set("climate.normal_lo", &deg(*l, units));
            if let Some(ov) = &data.overview {
                if let Some(d0) = ov.days().first() {
                    let dep = |f: Option<f64>, normal: f32| -> String {
                        match f {
                            Some(v) => {
                                let d = v as f32 - normal;
                                let d = if units == Units::Metric { d * 5.0 / 9.0 } else { d };
                                if d.abs() < 1.0 { "Near normal".into() } else { format!("{:+.0}° vs normal", d) }
                            }
                            None => "--".into(),
                        }
                    };
                    b.set("departure.hi", &dep(d0.high, *h));
                    b.set("departure.lo", &dep(d0.low, *l));
                }
            }
        }
        if let Some((v, y)) = n.rec_hi.get(doy) {
            b.set("climate.record_hi", &format!("{} ({y})", deg(*v, units)));
        }
        if let Some((v, y)) = n.rec_lo.get(doy) {
            b.set("climate.record_lo", &format!("{} ({y})", deg(*v, units)));
        }
        let fmt_doy = |d: u32| chrono::NaiveDate::from_yo_opt(now.year(), d + 1).map(|d| d.format("%B %-d").to_string()).unwrap_or_default();
        if let Some(d) = n.last_freeze {
            b.set("frost.last", &fmt_doy(d));
        }
        if let Some(d) = n.first_freeze {
            b.set("frost.first", &fmt_doy(d));
        }
        if let Some(g) = n.growing_days {
            b.set("season.growing", &format!("{g} days"));
        }
        if let (Some(mtd), Some(month_normal)) = (clim.mtd_precip, n.month_precip.get(now.month0() as usize)) {
            let days_in_month = chrono::NaiveDate::from_ymd_opt(now.year(), now.month() % 12 + 1, 1)
                .map(|d| d.pred_opt().map(|p| p.day()).unwrap_or(30))
                .unwrap_or(31) as f32;
            let to_date = month_normal * (now.day() as f32 - 1.0).max(0.0) / days_in_month;
            b.set("rain.mtd", &inches(mtd, units));
            b.set("rain.mtd_normal", &inches(to_date, units));
            let d = mtd - to_date;
            b.set("rain.departure", &if d.abs() < 0.05 { "Near normal".to_string() } else if d > 0.0 { format!("{} above normal", inches(d, units)) } else { format!("{} below normal", inches(-d, units)) });
        }
    }
    for (i, d) in clim.daily16.iter().take(10).enumerate() {
        b.set(&format!("ten{i}.name"), &if i == 0 { "Today".to_string() } else { d.date.format("%a").to_string() });
        b.set(&format!("ten{i}.date"), &d.date.format("%-m/%-d").to_string());
        b.set(&format!("ten{i}.hi"), &deg(d.hi, units));
        b.set(&format!("ten{i}.lo"), &deg(d.lo, units));
        b.set(&format!("ten{i}.pop"), &format!("{:.0}%", d.pop));
        b.set(&format!("ten{i}.cond"), wmo_words(d.code));
    }
    if !clim.daily16.is_empty() {
        b.set_series("ten.hi", clim.daily16.iter().take(10).map(|d| (d.date.format("%a").to_string(), d.hi)).collect());
        b.set_series("ten.lo", clim.daily16.iter().take(10).map(|d| (d.date.format("%a").to_string(), d.lo)).collect());
        b.set_series("ten.pop", clim.daily16.iter().take(10).map(|d| (d.date.format("%a").to_string(), d.pop)).collect());
    }
    let tomorrow = now.date_naive().succ_opt();
    let picks: Vec<(&str, f32)> = clim.models.iter().filter_map(|m| m.days.iter().find(|d| Some(d.0) == tomorrow).map(|d| (m.name, d.1))).collect();
    if !picks.is_empty() {
        b.set("models.tomorrow", &picks.iter().map(|(n, h)| format!("{n} {}", deg(*h, units))).collect::<Vec<_>>().join("  •  "));
        let spread = picks.iter().map(|p| p.1).fold(f32::MIN, f32::max) - picks.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let (level, color) = if picks.len() < 2 {
            ("Awaiting model comparison", "$muted")
        } else if spread <= 3.0 {
            ("High confidence", "#65E5B1")
        } else if spread <= 7.0 {
            ("Medium confidence", "#FFD778")
        } else {
            ("Low confidence", "#FFA297")
        };
        b.set("models.confidence", level);
        b.set("models.confidence_color", color);
        for m in &clim.models {
            b.set_series(&format!("models.{}", m.name.to_lowercase()), m.days.iter().map(|d| (d.0.format("%a").to_string(), d.1)).collect());
        }
    }
    if let Some(kp) = clim.kp_max {
        b.set("aurora.kp", &format!("Kp {kp:.0}"));
        // Rough visibility latitude: Kp 5 ~ 50°N, each Kp step ~ 2.5° further south.
        let visible_to = 66.5 - kp as f64 * 3.3;
        let chance = if home.lat >= visible_to + 3.0 { "Likely if skies are clear" } else if home.lat >= visible_to { "Possible low on the northern horizon" } else { "Unlikely this far south" };
        b.set("aurora.chance", chance);
    }
    {
        let today = now.date_naive();
        let next = crate::climate::METEOR_SHOWERS
            .iter()
            .filter_map(|(m, d, name, rate)| {
                let this = chrono::NaiveDate::from_ymd_opt(today.year(), *m, *d)?;
                let date = if this >= today { this } else { chrono::NaiveDate::from_ymd_opt(today.year() + 1, *m, *d)? };
                Some((date, *name, *rate))
            })
            .min_by_key(|(d, _, _)| *d);
        if let Some((date, name, rate)) = next {
            let (age, illum) = crate::astro::moon(date.and_hms_opt(4, 0, 0).map(|d| d.and_utc()).unwrap_or_else(chrono::Utc::now));
            let moon = if illum > 0.6 { "a bright moon will wash out fainter meteors" } else { "a dark sky helps" };
            let _ = age;
            b.set("meteor.next", &format!("{name}: peaks {} • up to {rate} per hour • {moon}", date.format("%B %-d")));
        }
        let next_eclipse = crate::climate::US_ECLIPSES
            .iter()
            .filter_map(|(y, m, d, kind, wh)| Some((chrono::NaiveDate::from_ymd_opt(*y, *m, *d)?, *kind, *wh)))
            .find(|(d, _, _)| *d >= today);
        if let Some((d, kind, wh)) = next_eclipse {
            b.set("eclipse.next", &format!("{kind}: {} • {wh}", d.format("%B %-d, %Y")));
        }
    }
    if let Some(grid) = home_grid(data, settings) {
        if let Some(tf) = tf_for_hour(grid, 22) {
            let cloud = grid.sample(Var::Cloud, tf, home.lat, home.lon).unwrap_or(50.0);
            let (_, illum) = crate::astro::moon(chrono::Utc::now());
            let sky = match (cloud, illum) {
                (c, i) if c < 20.0 && i < 0.3 => "Excellent: clear and dark",
                (c, _) if c < 20.0 => "Clear, but the moon brightens the sky",
                (c, _) if c < 50.0 => "Fair: some clouds",
                _ => "Poor: mostly cloudy",
            };
            b.set("sky.tonight", sky);
        }
        if let (Some(obs), Some(model)) = (
            data.overview.as_ref().and_then(|o| o.current.temperature_f),
            grid.sample(Var::Temp, grid.time_index(0.0), home.lat, home.lon),
        ) {
            let d = obs as f32 - model;
            b.set("model.bias", &if d.abs() < 1.0 { "Model matches observations".into() } else { format!("Model is {:.0}° too {}", d.abs(), if d > 0.0 { "cold" } else { "warm" }) });
        }
    }
    b.set(
        "airport.summary",
        &if clim.airports.is_empty() { "No major US airport delays reported".into() } else { format!("{} airports reporting delays", clim.airports.len()) },
    );
    for (i, a) in clim.airports.iter().take(8).enumerate() {
        b.set(&format!("airport{i}.line"), &format!("{} • {} • {}{}", a.code, a.kind, a.reason, if a.detail.is_empty() { String::new() } else { format!(" ({})", a.detail) }));
    }

    // ---- Forecaster edits (published) ---------------------------------------------------
    for d in &data.forecast_edits.days {
        let i = d.index;
        if let Some(h) = d.hi {
            b.set(&format!("day{i}.hi"), &deg(h, units));
        }
        if let Some(l) = d.lo {
            b.set(&format!("day{i}.lo"), &deg(l, units));
        }
        if let Some(c) = &d.condition {
            b.set(&format!("day{i}.condition"), c);
            b.set(&format!("day{i}.condition_es"), &condition_es(c));
        }
        if let Some(p) = d.pop {
            b.set(&format!("day{i}.precip"), &format!("{p:.0}%"));
        }
    }
    for (r, region) in data.forecast_edits.regions.iter().enumerate() {
        b.set(&format!("region{r}.name"), &region.name);
        b.set(&format!("region{r}.note"), &region.note);
        if let Some(grid) = home_grid(data, settings) {
            // Region high / low from the model inside the region, plus the forecaster's delta.
            let (clat, clon) = region.shape.center();
            let temps: Vec<f32> = (0..=24).filter_map(|h| grid.sample(Var::Temp, grid.time_index(h as f32), clat, clon)).collect();
            if let (Some(hi), Some(lo)) = (temps.iter().copied().reduce(f32::max), temps.iter().copied().reduce(f32::min)) {
                b.set(&format!("region{r}.hi"), &deg(hi + region.hi_delta, units));
                b.set(&format!("region{r}.lo"), &deg(lo + region.lo_delta, units));
            }
            let night = is_night(now, clat, clon);
            b.set(&format!("region{r}.cond"), &condition(grid, grid.time_index(0.0), clat, clon, night));
            let end = grid.time_index(47.0);
            if let (Some(a), Some(z)) = (grid.sample(Var::SnowAcc, end, clat, clon), grid.sample(Var::SnowAcc, grid.time_index(0.0), clat, clon)) {
                b.set(&format!("region{r}.snow48"), &format!("{:.1}\"", (a - z).max(0.0)));
            }
        }
    }

    // ---- Condition backgrounds ----------------------------------------------------------
    if let Some(ov) = &data.overview {
        let night = is_night(now, home.lat, home.lon);
        let cond = ov.current.text_description.clone().unwrap_or_default();
        b.set("sky_image", crate::gfx::backgrounds::for_condition(&cond, night));
        for (i, d) in ov.days().iter().take(7).enumerate() {
            b.set(&format!("day{i}.sky_image"), crate::gfx::backgrounds::for_condition(&d.short_forecast, !d.is_daytime));
        }
    }

    // ---- Lightning trend near home ------------------------------------------------------
    let rate = crate::map::extras::flash_rate(&data.lightning, home.lat, home.lon, 25.0);
    let last = rate.last().copied().unwrap_or(0);
    let prev = rate.get(rate.len().saturating_sub(4)..rate.len() - 1).map(|s| s.iter().sum::<u32>() as f32 / 3.0).unwrap_or(0.0);
    b.set(
        "lightning.rate",
        &format!(
            "{last} strikes in the last 5 min ({})",
            if last as f32 > prev * 1.3 + 1.0 { "increasing" } else if (last as f32) < prev * 0.7 { "decreasing" } else { "steady" }
        ),
    );
    let _ = now.weekday();
}

fn day_es(name: &str) -> String {
    let map = [
        ("Monday", "Lunes"),
        ("Tuesday", "Martes"),
        ("Wednesday", "Miércoles"),
        ("Thursday", "Jueves"),
        ("Friday", "Viernes"),
        ("Saturday", "Sábado"),
        ("Sunday", "Domingo"),
        ("Today", "Hoy"),
        ("Tonight", "Esta noche"),
        ("This Afternoon", "Esta tarde"),
        ("Night", "Noche"),
    ];
    let mut out = name.to_string();
    for (en, es) in map {
        out = out.replace(en, es);
    }
    out
}

/// Common NWS short-forecast phrases in Spanish; unknown words are kept.
pub fn condition_es(text: &str) -> String {
    let map = [
        ("Chance Showers And Thunderstorms", "Posibles chubascos y tormentas"),
        ("Showers And Thunderstorms", "Chubascos y tormentas"),
        ("Thunderstorms", "Tormentas"),
        ("Slight Chance", "Ligera probabilidad de"),
        ("Chance", "Posibles"),
        ("Likely", "probables"),
        ("Mostly Sunny", "Mayormente soleado"),
        ("Partly Sunny", "Parcialmente soleado"),
        ("Mostly Clear", "Mayormente despejado"),
        ("Partly Cloudy", "Parcialmente nublado"),
        ("Mostly Cloudy", "Mayormente nublado"),
        ("Sunny", "Soleado"),
        ("Clear", "Despejado"),
        ("Cloudy", "Nublado"),
        ("Light Rain", "Lluvia ligera"),
        ("Heavy Rain", "Lluvia fuerte"),
        ("Rain And Snow", "Lluvia y nieve"),
        ("Freezing Rain", "Lluvia helada"),
        ("Showers", "Chubascos"),
        ("Rain", "Lluvia"),
        ("Snow Showers", "Chubascos de nieve"),
        ("Snow", "Nieve"),
        ("Sleet", "Aguanieve"),
        ("Fog", "Niebla"),
        ("Patchy", "Parcial"),
        ("Areas Of", "Áreas de"),
        ("Windy", "Ventoso"),
        ("Breezy", "Con brisa"),
        ("Haze", "Bruma"),
        ("Smoke", "Humo"),
        ("Drizzle", "Llovizna"),
        ("Fair", "Despejado"),
        ("Overcast", "Cubierto"),
        ("then", "luego"),
        ("And", "y"),
    ];
    let mut out = text.to_string();
    for (en, es) in map {
        out = out.replace(en, es);
    }
    out
}

/// Plain words for an Open-Meteo / WMO weather code.
pub fn wmo_words(code: i32) -> &'static str {
    match code {
        0 => "Sunny",
        1 => "Mostly sunny",
        2 => "Partly cloudy",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51 | 53 | 55 => "Drizzle",
        56 | 57 | 66 | 67 => "Freezing rain",
        61 => "Light rain",
        63 => "Rain",
        65 => "Heavy rain",
        71 => "Light snow",
        73 => "Snow",
        75 => "Heavy snow",
        77 => "Snow grains",
        80 | 81 => "Showers",
        82 => "Heavy showers",
        85 | 86 => "Snow showers",
        95 => "Thunderstorms",
        96 | 99 => "Storms with hail",
        _ => "Mixed",
    }
}
