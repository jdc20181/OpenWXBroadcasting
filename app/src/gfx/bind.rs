//! Live data bindings for graphics: `{{temp}}`, `{{day0.hi}}`, `{{alert0.headline}}`, …
//! Values come from the Dubois County Weather feeds and refresh while a graphic is on air.

use std::collections::BTreeMap;

use chrono::{DateTime, Local};

use crate::{
    config::{Settings, Units},
    data::WxSnapshot,
    map::geo::compass,
};

#[derive(Clone, Debug, Default)]
pub struct Bindings {
    values: BTreeMap<String, String>,
    /// Numeric series for chart layers: key -> (label, value) points.
    series: BTreeMap<String, Vec<(String, f32)>>,
}

/// Built-in transparent wordmark, resolved only by Image layers.
pub fn is_brand_image(media: &str) -> bool {
    matches!(media.trim(), "{brand}" | "{{brand}}")
}

/// Every binding with a description, for the Graphics Studio's binding picker.
pub const CATALOG: &[(&str, &str)] = &[
    ("station", "Station name"),
    ("station_short", "Short station name"),
    ("tagline", "Station tagline"),
    ("sponsor", "Sponsor name"),
    ("sponsor_line", "Sponsor line, e.g. Brought to you by …"),
    ("sponsor_logo", "Sponsor logo (use as an Image layer's media)"),
    ("location", "Home location label"),
    ("time", "Current time"),
    ("date", "Current date"),
    ("weekday", "Day of week"),
    ("daypart", "day / night right now"),
    ("temp", "Temperature"),
    ("feels", "Feels-like temperature"),
    ("dewpoint", "Dew point"),
    ("humidity", "Relative humidity (%)"),
    ("wind", "Wind (direction + speed)"),
    ("wind_speed", "Wind speed"),
    ("gust", "Wind gust"),
    ("pressure", "Barometric pressure"),
    ("visibility", "Visibility"),
    ("condition", "Current sky/weather"),
    ("obs_station", "Observation station"),
    ("obs_time", "Observation time"),
    ("day0.name", "Forecast day name (0-6)"),
    ("day0.short", "Forecast day abbreviation (0-6)"),
    ("day0.hi", "Forecast high (0-6)"),
    ("day0.lo", "Forecast low (0-6)"),
    ("next.hi", "Next forecast high (tomorrow's at night)"),
    ("next.lo", "Next forecast low"),
    ("day0.condition", "Forecast condition (0-6)"),
    ("day0.precip", "Forecast precip chance % (0-6)"),
    ("day0.daypart", "day / night for the period (0-6)"),
    ("period0.name", "NWS period name (0-13)"),
    ("period0.temp", "NWS period temperature (0-13)"),
    ("period0.text", "NWS detailed forecast (0-13)"),
    ("period0.short", "NWS short forecast (0-13)"),
    ("alert.count", "Number of active local alerts"),
    ("alert0.event", "Alert event (0-4)"),
    ("alert0.headline", "Alert headline (0-4)"),
    ("alert0.area", "Alert areas (0-4)"),
    ("alert0.expires", "Alert expiration (0-4)"),
    ("alert0.instruction", "Alert call to action (0-4)"),
    ("alerts.ticker", "All local alerts as one crawl line"),
    ("radar.time", "Radar frame time"),
    ("radar.palette", "Radar palette code"),
    ("spc.day1", "SPC Day 1 highest risk"),
    ("spc.day2", "SPC Day 2 highest risk"),
    ("spc.day3", "SPC Day 3 highest risk"),
    ("spc.day1.tornado", "SPC Day 1 highest tornado chance (also hail, wind; day2)"),
    ("lightning.near", "Lightning near home (distance, last strike)"),
    ("lightning.all_clear", "Lightning all-clear countdown"),
    ("cell0.summary", "Nearest storm cell summary (0-4)"),
];

pub(crate) fn temp(value: Option<f64>, units: Units) -> String {
    match value {
        Some(f) => match units {
            Units::Imperial => format!("{:.0}", f),
            Units::Metric => format!("{:.0}", (f - 32.0) * 5.0 / 9.0),
        },
        None => "--".into(),
    }
}

fn speed(mph: Option<f64>, units: Units) -> String {
    match mph {
        Some(v) => match units {
            Units::Imperial => format!("{:.0} mph", v),
            Units::Metric => format!("{:.0} km/h", v * 1.609_344),
        },
        None => "Calm".into(),
    }
}

fn short_time(iso: &str, clock_24h: bool) -> String {
    DateTime::parse_from_rfc3339(iso)
        .map(|t| {
            let local = t.with_timezone(&Local);
            if clock_24h {
                local.format("%H:%M").to_string()
            } else {
                local.format("%-I:%M %p").to_string()
            }
        })
        .unwrap_or_else(|_| iso.to_string())
}

impl Bindings {
    pub fn build(data: &WxSnapshot, settings: &Settings, radar_time: Option<i64>, palette: &str) -> Self {
        let mut b = Bindings::default();
        let units = settings.units;
        let deg = "°";
        let now = Local::now();

        b.set("station", &settings.station_name);
        b.set("station_short", &settings.station_short);
        b.set("tagline", &settings.tagline);
        b.set("sponsor", &settings.sponsor_name);
        b.set(
            "sponsor_line",
            &if settings.sponsor_tagline.trim().is_empty() && !settings.sponsor_name.trim().is_empty() {
                format!("Brought to you by {}", settings.sponsor_name)
            } else {
                settings.sponsor_tagline.clone()
            },
        );
        b.set("sponsor_logo", settings.sponsor_logo_media.as_deref().unwrap_or_default());
        // The station logo replaces the station name on graphics (Settings > Output) when set.
        b.set("station_logo", if settings.logo_on_graphics { settings.brand.logo_media.as_deref().unwrap_or_default() } else { "" });
        b.set("location", &settings.home_label);
        // Coverage area (Regional Outlook): covday{j}.* headings, cov{i}.* rows, d{j} cells.
        b.set("cov.area", &crate::social_graphics::coverage_area(settings));
        {
            let r = data.coverage.as_ref();
            let rows = r.map(|r| r.cities.len()).unwrap_or(0);
            b.set("cov.count", &rows.to_string());
            for j in 0..7 {
                let col = r.and_then(|r| r.columns.get(j));
                b.set(&format!("covday{j}.short"), col.map(|c| c.0.as_str()).unwrap_or(""));
                b.set(&format!("covday{j}.date"), col.map(|c| c.1.as_str()).unwrap_or(""));
            }
            for i in 0..8 {
                let city = r.and_then(|r| r.cities.get(i));
                b.set(&format!("cov{i}.name"), city.map(|c| c.0.as_str()).unwrap_or(""));
                for j in 0..7 {
                    let d = city.and_then(|c| c.1.get(j));
                    let t = |v: Option<i32>| v.map(|v| format!("{}{deg}", temp(Some(v as f64), units))).unwrap_or_default();
                    b.set(&format!("cov{i}.d{j}.hi"), &t(d.and_then(|d| d.high)));
                    b.set(&format!("cov{i}.d{j}.lo"), &t(d.and_then(|d| d.low)));
                    // "No icon" keeps unused rows / days blank.
                    b.set(&format!("cov{i}.d{j}.condition"), d.map(|d| d.description.as_str()).filter(|s| !s.is_empty()).unwrap_or("No icon"));
                    b.set(&format!("cov{i}.d{j}.precip"), &d.and_then(|d| d.rain).map(|p| format!("{p}%")).unwrap_or_default());
                }
            }
        }
        b.set(
            "time",
            &if settings.clock_24h {
                now.format("%H:%M").to_string()
            } else {
                now.format("%-I:%M %p").to_string()
            },
        );
        b.set("date", &now.format("%B %-d, %Y").to_string());
        b.set("weekday", &now.format("%A").to_string());
        {
            use chrono::Timelike;
            let hour = now.hour();
            b.set("daypart", if (6..19).contains(&hour) { "day" } else { "night" });
        }

        if let Some(overview) = &data.overview {
            let c = &overview.current;
            b.set("temp", &format!("{}{deg}", temp(c.temperature_f, units)));
            b.set("feels", &format!("{}{deg}", temp(c.feels_like_f(), units)));
            b.set("dewpoint", &format!("{}{deg}", temp(c.dewpoint_f, units)));
            b.set(
                "humidity",
                &c.relative_humidity
                    .map(|h| format!("{h:.0}%"))
                    .unwrap_or_else(|| "--".into()),
            );
            let dir = c
                .wind_direction_cardinal
                .clone()
                .or_else(|| c.wind_direction.map(|d| compass(d).to_string()))
                .unwrap_or_default();
            let wind_speed = speed(c.wind_speed_mph, units);
            b.set(
                "wind",
                &if c.wind_speed_mph.unwrap_or(0.0) < 1.0 {
                    "Calm".to_string()
                } else {
                    format!("{dir} {wind_speed}").trim().to_string()
                },
            );
            b.set("wind_speed", &wind_speed);
            b.set(
                "gust",
                &c.wind_gust_mph
                    .map(|g| speed(Some(g), units))
                    .unwrap_or_else(|| "--".into()),
            );
            b.set(
                "pressure",
                &c.barometric_pressure_in_hg
                    .map(|p| match units {
                        Units::Imperial => format!("{p:.2} in"),
                        Units::Metric => format!("{:.0} mb", p * 33.8639),
                    })
                    .unwrap_or_else(|| "--".into()),
            );
            b.set(
                "visibility",
                &c.visibility_miles
                    .map(|v| match units {
                        Units::Imperial => format!("{v:.0} mi"),
                        Units::Metric => format!("{:.0} km", v * 1.609_344),
                    })
                    .unwrap_or_else(|| "--".into()),
            );
            b.set(
                "condition",
                c.text_description.as_deref().unwrap_or("--"),
            );
            b.set(
                "obs_station",
                c.station_label
                    .as_deref()
                    .or(c.station.as_deref())
                    .unwrap_or(""),
            );
            b.set(
                "obs_time",
                &c.timestamp
                    .as_deref()
                    .map(|t| short_time(t, settings.clock_24h))
                    .unwrap_or_default(),
            );

            // The next forecast high and low, whichever period carries them (at night the
            // next high is tomorrow's). Record and climate cards use these.
            let days = overview.days();
            if let Some(h) = days.iter().find_map(|d| d.high) {
                b.set("next.hi", &format!("{}{deg}", temp(Some(h), units)));
            }
            if let Some(l) = days.iter().find_map(|d| d.low) {
                b.set("next.lo", &format!("{}{deg}", temp(Some(l), units)));
            }
            for (i, day) in overview.days().iter().take(7).enumerate() {
                b.set(&format!("day{i}.name"), &day.name);
                b.set(&format!("day{i}.short"), &day.short_name);
                // At night the first card is "Tonight" with no high: show the low in the big
                // slot with the word "Low" under it, never "--°" (CWXB-82).
                let (hi, lo) = match (day.high, day.low) {
                    (Some(_), Some(_)) => (format!("{}{deg}", temp(day.high, units)), format!("{}{deg}", temp(day.low, units))),
                    (None, Some(_)) => (format!("{}{deg}", temp(day.low, units)), "Low".to_string()),
                    (Some(_), None) => (format!("{}{deg}", temp(day.high, units)), "High".to_string()),
                    (None, None) => (String::new(), String::new()),
                };
                b.set(&format!("day{i}.hi"), &hi);
                b.set(&format!("day{i}.lo"), &lo);
                b.set(&format!("day{i}.condition"), &day.short_forecast);
                b.set(
                    &format!("day{i}.precip"),
                    &day.precip
                        .map(|p| format!("{p:.0}%"))
                        .unwrap_or_else(|| "0%".into()),
                );
                b.set(
                    &format!("day{i}.daypart"),
                    if day.is_daytime { "day" } else { "night" },
                );
            }
            for (i, p) in overview.forecast.iter().take(14).enumerate() {
                b.set(&format!("period{i}.name"), &p.name);
                b.set(&format!("period{i}.temp"), &format!("{}{deg}", temp(p.temperature, units)));
                b.set(&format!("period{i}.text"), &p.detailed_forecast);
                b.set(&format!("period{i}.short"), &p.short_forecast);
                b.set(
                    &format!("period{i}.daypart"),
                    if p.is_daytime { "day" } else { "night" },
                );
            }
        }

        b.set("alert.count", &data.local_alerts.len().to_string());
        for (i, a) in data.local_alerts.iter().take(5).enumerate() {
            b.set(&format!("alert{i}.event"), &a.event);
            b.set(&format!("alert{i}.headline"), &a.headline);
            b.set(&format!("alert{i}.area"), &a.area_desc);
            b.set(
                &format!("alert{i}.expires"),
                &a.expires
                    .as_deref()
                    .map(|t| format!("Until {}", short_time(t, settings.clock_24h)))
                    .unwrap_or_default(),
            );
            b.set(&format!("alert{i}.instruction"), &a.instruction.replace('\n', " "));
        }
        b.set(
            "alerts.ticker",
            &if data.local_alerts.is_empty() {
                format!("No active weather alerts for {}", settings.home_label)
            } else {
                data.local_alerts
                    .iter()
                    .map(|a| {
                        if a.headline.is_empty() {
                            a.event.to_uppercase()
                        } else {
                            a.headline.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("   •   ")
            },
        );

        b.set(
            "radar.time",
            &radar_time
                .and_then(|t| DateTime::from_timestamp(t, 0))
                .map(|t| {
                    let local = t.with_timezone(&Local);
                    if settings.clock_24h {
                        local.format("%H:%M").to_string()
                    } else {
                        local.format("%-I:%M %p").to_string()
                    }
                })
                .unwrap_or_else(|| "--".into()),
        );
        b.set("radar.palette", palette);

        for day in &data.spc {
            let top = day
                .categorical
                .iter()
                .max_by_key(|a| a.rank)
                .map(|a| a.label.clone())
                .unwrap_or_else(|| "No severe risk".into());
            b.set(&format!("spc.day{}", day.day), &top);
            for hazard in &day.probabilities {
                let top = hazard
                    .areas
                    .iter()
                    .max_by_key(|a| a.rank)
                    .map(|a| a.label.clone())
                    .unwrap_or_else(|| "None".into());
                b.set(&format!("spc.day{}.{}", day.day, hazard.key), &top);
            }
        }

        match crate::data::lightning_near(&data.lightning, settings.home.lat, settings.home.lon) {
            Some(l) => {
                b.set("lightning.near", &l.summary());
                b.set("lightning.all_clear", &l.all_clear_text());
            }
            None => {
                b.set(
                    "lightning.near",
                    &format!("No lightning within {:.0} miles", crate::data::LIGHTNING_RADIUS_MI),
                );
                b.set("lightning.all_clear", "All clear");
            }
        }

        for (i, cell) in data.cells.iter().take(5).enumerate() {
            // Skip empty parts: a storm over home has no bearing, a new cell no direction.
            let motion = cell
                .motion
                .as_ref()
                .map(|m| match m.toward.trim() {
                    "" => format!(", moving at {:.0} mph", m.speed_mph),
                    toward => format!(", moving {toward} at {:.0} mph", m.speed_mph),
                })
                .unwrap_or_default();
            let place = if cell.distance_mi < 1.0 || cell.direction.trim().is_empty() {
                if cell.distance_mi < 1.0 { "overhead".to_string() } else { format!("{:.0} mi away", cell.distance_mi) }
            } else {
                format!("{:.0} mi {}", cell.distance_mi, cell.direction.trim())
            };
            b.set(
                &format!("cell{i}.summary"),
                &format!("{} {place}{motion}", cell.on_air_label(settings.calm_storm_labels)),
            );
        }

        super::bind_ext::extend(&mut b, data, settings);
        b
    }

    pub fn set_series(&mut self, key: &str, points: Vec<(String, f32)>) {
        self.series.insert(key.to_string(), points);
    }

    pub fn series(&self, key: &str) -> Option<&[(String, f32)]> {
        self.series.get(key).map(Vec::as_slice)
    }

    /// Every series whose key starts with `prefix` (ensemble members), in key order.
    pub fn series_with_prefix(&self, prefix: &str) -> Vec<Vec<f32>> {
        self.series.range(prefix.to_string()..).take_while(|(k, _)| k.starts_with(prefix)).map(|(_, v)| v.iter().map(|p| p.1).collect()).collect()
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_string(), value.to_string());
    }

    /// Same keys with every value transformed (preview modes: missing data, long text).
    pub fn map_values(&self, f: impl Fn(&str) -> String) -> Bindings {
        Bindings {
            values: self.values.iter().map(|(k, v)| (k.clone(), f(v))).collect(),
            series: if f("x").is_empty() { Default::default() } else { self.series.clone() },
        }
    }

    /// `{{keys}}` in `text` that have no value (misspelt or unknown bindings).
    pub fn unknown_keys(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            let after = &rest[start + 2..];
            let Some(end) = after.find("}}") else { break };
            let key = after[..end].trim();
            if self.get(key).is_none() && self.series(key).is_none() && dynamic(key).is_none() {
                out.push(key.to_string());
            }
            rest = &after[end + 2..];
        }
        out
    }

    /// Every text binding (public data feed).
    pub fn values(&self) -> &BTreeMap<String, String> {
        &self.values
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Replace every `{{key}}` in `text`. Unknown keys render as "--".
    pub fn apply(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(start) = rest.find("{{") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            if let Some(end) = after.find("}}") {
                let key = after[..end].trim();
                match self.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(dynamic(key).as_deref().unwrap_or("--")),
                }
                rest = &after[end + 2..];
            } else {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
        out.push_str(rest);
        out
    }
}

/// Bindings computed from the key itself, at render time:
/// `{{until:2026-12-25}}` → "Today" / "1 day" / "42 days" / "Passed";
/// `{{until:2026-11-14 18:30}}` (local time) → "25 min" / "3 h 20 min" / "4 days" / "Now".
pub fn dynamic(key: &str) -> Option<String> {
    let target = key.strip_prefix("until:")?.trim();
    countdown(target, chrono::Local::now().naive_local())
}

fn countdown(target: &str, now: chrono::NaiveDateTime) -> Option<String> {
    use chrono::{NaiveDate, NaiveDateTime};
    let plural = |n: i64, unit: &str| if n == 1 { format!("1 {unit}") } else { format!("{n} {unit}s") };
    if let Ok(date) = NaiveDate::parse_from_str(target, "%Y-%m-%d") {
        let days = (date - now.date()).num_days();
        return Some(match days {
            ..=-1 => "Passed".to_string(),
            0 => "Today".to_string(),
            n => plural(n, "day"),
        });
    }
    let at = NaiveDateTime::parse_from_str(target, "%Y-%m-%d %H:%M").ok()?;
    let mins = (at - now).num_minutes();
    Some(match mins {
        ..=0 => "Now".to_string(),
        1..=59 => format!("{mins} min"),
        60..=2879 => format!("{} h {} min", mins / 60, mins % 60),
        _ => plural(mins / 1440, "day"),
    })
}

#[cfg(test)]
mod countdown_tests {
    use super::countdown;

    #[test]
    fn until_dates_and_times() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 11, 14).unwrap().and_hms_opt(9, 0, 0).unwrap();
        assert_eq!(countdown("2026-11-14", now).as_deref(), Some("Today"));
        assert_eq!(countdown("2026-11-15", now).as_deref(), Some("1 day"));
        assert_eq!(countdown("2026-12-25", now).as_deref(), Some("41 days"));
        assert_eq!(countdown("2026-11-13", now).as_deref(), Some("Passed"));
        assert_eq!(countdown("2026-11-14 09:25", now).as_deref(), Some("25 min"));
        assert_eq!(countdown("2026-11-14 12:20", now).as_deref(), Some("3 h 20 min"));
        assert_eq!(countdown("2026-11-18 09:00", now).as_deref(), Some("4 days"));
        assert_eq!(countdown("2026-11-14 08:00", now).as_deref(), Some("Now"));
        assert_eq!(countdown("next week", now), None);
    }
}
